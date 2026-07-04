use super::dto::{sm_rest, SmRest};
use crate::api::paging::{apply_sort, page, ListParams, Paged};
use crate::entity::{software_module, software_module_type};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::{base_url, now_ms};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmCreate {
    pub name: String,
    pub version: String,
    #[serde(rename = "type")]
    pub module_type: String,
    pub vendor: Option<String>,
    pub description: Option<String>,
}

fn fiql_map(f: &str) -> Option<software_module::Column> {
    match f {
        "id" => Some(software_module::Column::Id),
        "name" => Some(software_module::Column::Name),
        "version" => Some(software_module::Column::Version),
        "vendor" => Some(software_module::Column::Vendor),
        "description" => Some(software_module::Column::Description),
        _ => None,
    }
}

/// type_id -> key lookup, used by every handler that renders SmRest.
pub async fn type_keys(db: &sea_orm::DatabaseConnection) -> Result<HashMap<i64, String>, AppError> {
    Ok(software_module_type::Entity::find().all(db).await?
        .into_iter().map(|t| (t.id, t.key)).collect())
}

pub async fn create(
    State(st): State<AppState>, headers: HeaderMap, Json(body): Json<Vec<SmCreate>>,
) -> Result<(StatusCode, Json<Vec<SmRest>>), AppError> {
    let base = base_url(&st.cfg, &headers);
    let mut out = Vec::with_capacity(body.len());
    for c in body {
        let ty = software_module_type::Entity::find()
            .filter(software_module_type::Column::Key.eq(&c.module_type))
            .one(&st.db).await?
            .ok_or_else(|| AppError::BadRequest(format!("unknown software module type: {}", c.module_type)))?;
        let dup = software_module::Entity::find()
            .filter(software_module::Column::Name.eq(&c.name))
            .filter(software_module::Column::Version.eq(&c.version))
            .filter(software_module::Column::TypeId.eq(ty.id))
            .one(&st.db).await?;
        if dup.is_some() {
            return Err(AppError::Conflict(format!("software module {}:{} already exists", c.name, c.version)));
        }
        let now = now_ms();
        let m = software_module::ActiveModel {
            type_id: Set(ty.id), name: Set(c.name), version: Set(c.version),
            vendor: Set(c.vendor), description: Set(c.description),
            created_at: Set(now), updated_at: Set(now),
            ..Default::default()
        }.insert(&st.db).await?;
        out.push(sm_rest(&m, &ty.key, &base));
    }
    Ok((StatusCode::CREATED, Json(out)))
}

pub async fn list(
    State(st): State<AppState>, headers: HeaderMap, Query(p): Query<ListParams>,
) -> Result<Json<Paged<SmRest>>, AppError> {
    let base = base_url(&st.cfg, &headers);
    let mut sel = software_module::Entity::find();
    if let Some(q) = &p.q {
        let expr = crate::fiql::parse(q).map_err(AppError::BadRequest)?;
        sel = sel.filter(crate::fiql::to_condition(&expr, &fiql_map)?);
    }
    sel = apply_sort(sel, &p.sort, &fiql_map)?;
    let (rows, total) = page(&st.db, sel, &p).await?;
    let keys = type_keys(&st.db).await?;
    let content = rows.iter().map(|m| sm_rest(m, keys.get(&m.type_id).map(String::as_str).unwrap_or("?"), &base)).collect();
    Ok(Json(Paged::new(content, total)))
}

pub async fn get_one(
    State(st): State<AppState>, headers: HeaderMap, Path(id): Path<i64>,
) -> Result<Json<SmRest>, AppError> {
    let m = software_module::Entity::find_by_id(id).one(&st.db).await?
        .ok_or(AppError::NotFound("software module"))?;
    let keys = type_keys(&st.db).await?;
    Ok(Json(sm_rest(&m, keys.get(&m.type_id).map(String::as_str).unwrap_or("?"), &base_url(&st.cfg, &headers))))
}

#[derive(Deserialize)]
pub struct SmUpdate { pub vendor: Option<String>, pub description: Option<String> }

pub async fn update(
    State(st): State<AppState>, headers: HeaderMap, Path(id): Path<i64>, Json(u): Json<SmUpdate>,
) -> Result<Json<SmRest>, AppError> {
    let m = software_module::Entity::find_by_id(id).one(&st.db).await?
        .ok_or(AppError::NotFound("software module"))?;
    let mut am: software_module::ActiveModel = m.into();
    if let Some(v) = u.vendor { am.vendor = Set(Some(v)); }
    if let Some(d) = u.description { am.description = Set(Some(d)); }
    am.updated_at = Set(now_ms());
    let m = am.update(&st.db).await?;
    let keys = type_keys(&st.db).await?;
    Ok(Json(sm_rest(&m, keys.get(&m.type_id).map(String::as_str).unwrap_or("?"), &base_url(&st.cfg, &headers))))
}

pub async fn delete(State(st): State<AppState>, Path(id): Path<i64>) -> Result<StatusCode, AppError> {
    let m = software_module::Entity::find_by_id(id).one(&st.db).await?
        .ok_or(AppError::NotFound("software module"))?;
    // Task 9 extends this to clean up artifact rows + blobs.
    software_module::Entity::delete_by_id(m.id).exec(&st.db).await?;
    Ok(StatusCode::OK)
}
