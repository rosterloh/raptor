use super::dto::{ds_rest, SmRest};
use super::software_modules::type_keys;
use crate::api::paging::{apply_sort, page, ListParams, Paged};
use crate::entity::{action, distribution_set, distribution_set_type, ds_module, software_module};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::{base_url, now_ms};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

fn fiql_map(f: &str) -> Option<distribution_set::Column> {
    match f {
        "id" => Some(distribution_set::Column::Id),
        "name" => Some(distribution_set::Column::Name),
        "version" => Some(distribution_set::Column::Version),
        "description" => Some(distribution_set::Column::Description),
        "complete" => Some(distribution_set::Column::Complete),
        _ => None,
    }
}

#[derive(Deserialize)]
pub struct ModuleRef {
    pub id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DsCreate {
    pub name: String,
    pub version: String,
    #[serde(rename = "type")]
    pub ds_type: String,
    pub description: Option<String>,
    #[serde(default)]
    pub required_migration_step: bool,
    #[serde(default)]
    pub modules: Vec<ModuleRef>,
}

pub async fn load_modules(st: &AppState, ds_id: i64, base: &str) -> Result<Vec<SmRest>, AppError> {
    let links = ds_module::Entity::find()
        .filter(ds_module::Column::DsId.eq(ds_id))
        .all(&st.db)
        .await?;
    let ids: Vec<i64> = links.iter().map(|l| l.module_id).collect();
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let keys = type_keys(&st.db).await?;
    let mods = software_module::Entity::find()
        .filter(software_module::Column::Id.is_in(ids))
        .all(&st.db)
        .await?;
    Ok(mods
        .iter()
        .map(|m| {
            super::dto::sm_rest(m, keys.get(&m.type_id).map(String::as_str).unwrap_or("?"), base)
        })
        .collect())
}

async fn ds_with_modules(st: &AppState, ds: &distribution_set::Model, base: &str) -> Result<Value, AppError> {
    let ty = distribution_set_type::Entity::find_by_id(ds.type_id)
        .one(&st.db)
        .await?
        .map(|t| t.key)
        .unwrap_or_else(|| "?".into());
    let modules = load_modules(st, ds.id, base).await?;
    Ok(ds_rest(ds, &ty, modules, base))
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Vec<DsCreate>>,
) -> Result<(StatusCode, Json<Vec<Value>>), AppError> {
    // Phase 1: Validate all items first (no writes)
    let mut seen = HashSet::new();
    for c in &body {
        let key = (c.name.as_str(), c.version.as_str());
        if !seen.insert(key) {
            return Err(AppError::Conflict(format!(
                "duplicate distribution set {}:{} in request",
                c.name, c.version
            )));
        }

        // Check type key exists
        let _ty = distribution_set_type::Entity::find()
            .filter(distribution_set_type::Column::Key.eq(&c.ds_type))
            .one(&st.db)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("unknown distribution set type: {}", c.ds_type))
            })?;

        // Check (name, version) uniqueness in DB
        match distribution_set::Entity::find()
            .filter(distribution_set::Column::Name.eq(&c.name))
            .filter(distribution_set::Column::Version.eq(&c.version))
            .one(&st.db)
            .await
        {
            Ok(Some(_)) => {
                return Err(AppError::Conflict(format!(
                    "distribution set {}:{} already exists",
                    c.name, c.version
                )))
            }
            Ok(None) => {}
            Err(e) => return Err(AppError::from(e)),
        }

        // Check all referenced modules exist
        for m in &c.modules {
            software_module::Entity::find_by_id(m.id)
                .one(&st.db)
                .await?
                .ok_or(AppError::NotFound("software module"))?;
        }
    }

    // Phase 2: Insert all items
    let base = base_url(&st.cfg, &headers);
    let mut out = Vec::with_capacity(body.len());
    for c in body {
        let ty = distribution_set_type::Entity::find()
            .filter(distribution_set_type::Column::Key.eq(&c.ds_type))
            .one(&st.db)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("unknown distribution set type: {}", c.ds_type))
            })?;
        let now = now_ms();
        let ds = distribution_set::ActiveModel {
            type_id: Set(ty.id),
            name: Set(c.name),
            version: Set(c.version),
            description: Set(c.description),
            required_migration_step: Set(c.required_migration_step),
            complete: Set(!c.modules.is_empty()),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&st.db)
        .await?;
        for m in &c.modules {
            ds_module::ActiveModel {
                ds_id: Set(ds.id),
                module_id: Set(m.id),
            }
            .insert(&st.db)
            .await?;
        }
        out.push(ds_with_modules(&st, &ds, &base).await?);
    }
    Ok((StatusCode::CREATED, Json(out)))
}

pub async fn list(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<Value>>, AppError> {
    let base = base_url(&st.cfg, &headers);
    let mut sel = distribution_set::Entity::find();
    if let Some(q) = &p.q {
        let expr = crate::fiql::parse(q).map_err(AppError::BadRequest)?;
        sel = sel.filter(crate::fiql::to_condition(&expr, &fiql_map)?);
    }
    sel = apply_sort(sel, &p.sort, &fiql_map)?;
    let (rows, total) = page(&st.db, sel, &p).await?;
    let mut content = Vec::with_capacity(rows.len());
    for ds in &rows {
        content.push(ds_with_modules(&st, ds, &base).await?);
    }
    Ok(Json(Paged::new(content, total)))
}

pub async fn get_one(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    let ds = distribution_set::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set"))?;
    Ok(Json(ds_with_modules(&st, &ds, &base_url(&st.cfg, &headers)).await?))
}

pub async fn delete(State(st): State<AppState>, Path(id): Path<i64>) -> Result<StatusCode, AppError> {
    let ds = distribution_set::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set"))?;
    let refs = action::Entity::find()
        .filter(action::Column::DsId.eq(ds.id))
        .count(&st.db)
        .await?;
    if refs > 0 {
        return Err(AppError::Conflict(
            "distribution set is referenced by actions".into(),
        ));
    }
    ds_module::Entity::delete_many()
        .filter(ds_module::Column::DsId.eq(ds.id))
        .exec(&st.db)
        .await?;
    distribution_set::Entity::delete_by_id(ds.id)
        .exec(&st.db)
        .await?;
    Ok(StatusCode::OK)
}

pub async fn assign_modules(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(mods): Json<Vec<ModuleRef>>,
) -> Result<StatusCode, AppError> {
    let ds = distribution_set::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set"))?;
    for m in &mods {
        software_module::Entity::find_by_id(m.id)
            .one(&st.db)
            .await?
            .ok_or(AppError::NotFound("software module"))?;
        let exists = ds_module::Entity::find()
            .filter(ds_module::Column::DsId.eq(ds.id))
            .filter(ds_module::Column::ModuleId.eq(m.id))
            .one(&st.db)
            .await?;
        if exists.is_none() {
            ds_module::ActiveModel {
                ds_id: Set(ds.id),
                module_id: Set(m.id),
            }
            .insert(&st.db)
            .await?;
        }
    }
    let mut am: distribution_set::ActiveModel = ds.into();
    am.complete = Set(true);
    am.updated_at = Set(now_ms());
    am.update(&st.db).await?;
    Ok(StatusCode::OK)
}

pub async fn assigned_modules(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(_p): Query<ListParams>,
) -> Result<Json<Paged<SmRest>>, AppError> {
    distribution_set::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set"))?;
    let mods = load_modules(&st, id, &base_url(&st.cfg, &headers)).await?;
    let total = mods.len() as u64;
    Ok(Json(Paged::new(mods, total)))
}
