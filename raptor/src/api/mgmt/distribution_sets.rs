use super::dto::{ds_rest, DsRest, SmRest};
use super::software_modules::type_keys;
use crate::api::paging::{apply_sort, page, ListParams, Paged};
use crate::entity::{
    action, distribution_set, distribution_set_type, ds_module, ds_type_module, rollout,
    software_module, target, target_filter,
};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::{base_url, now_ms};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use raptor_api_types::{DsCreate, DsInvalidate, DsUpdate, ModuleRef};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};
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

/// A distribution set is `complete` when every mandatory software-module type of
/// its distribution-set type is represented by at least one assigned module.
/// A type with no mandatory module types is trivially complete (hawkBit).
pub async fn compute_complete(
    db: &sea_orm::DatabaseConnection,
    ds_type_id: i64,
    ds_id: i64,
) -> Result<bool, AppError> {
    let mandatory: Vec<i64> = ds_type_module::Entity::find()
        .filter(ds_type_module::Column::DsTypeId.eq(ds_type_id))
        .filter(ds_type_module::Column::Mandatory.eq(true))
        .all(db)
        .await?
        .into_iter()
        .map(|r| r.module_type_id)
        .collect();
    if mandatory.is_empty() {
        return Ok(true);
    }
    let module_ids: Vec<i64> = ds_module::Entity::find()
        .filter(ds_module::Column::DsId.eq(ds_id))
        .all(db)
        .await?
        .into_iter()
        .map(|l| l.module_id)
        .collect();
    let present: HashSet<i64> = if module_ids.is_empty() {
        HashSet::new()
    } else {
        software_module::Entity::find()
            .filter(software_module::Column::Id.is_in(module_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|m| m.type_id)
            .collect()
    };
    Ok(mandatory.iter().all(|m| present.contains(m)))
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
            super::dto::sm_rest(
                m,
                keys.get(&m.type_id).map(String::as_str).unwrap_or("?"),
                base,
            )
        })
        .collect())
}

async fn ds_with_modules(
    st: &AppState,
    ds: &distribution_set::Model,
    base: &str,
) -> Result<DsRest, AppError> {
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
) -> Result<(StatusCode, Json<Vec<DsRest>>), AppError> {
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
            complete: Set(false),
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
        // Completeness derives from the type's mandatory module types.
        let complete = compute_complete(&st.db, ty.id, ds.id).await?;
        let mut am: distribution_set::ActiveModel = ds.into();
        am.complete = Set(complete);
        let ds = am.update(&st.db).await?;
        out.push(ds_with_modules(&st, &ds, &base).await?);
    }
    Ok((StatusCode::CREATED, Json(out)))
}

pub async fn list(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<DsRest>>, AppError> {
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
) -> Result<Json<DsRest>, AppError> {
    let ds = distribution_set::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set"))?;
    Ok(Json(
        ds_with_modules(&st, &ds, &base_url(&st.cfg, &headers)).await?,
    ))
}

pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<DsUpdate>,
) -> Result<Json<DsRest>, AppError> {
    let ds = distribution_set::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set"))?;

    let new_name = body.name.clone().unwrap_or_else(|| ds.name.clone());
    let new_version = body.version.clone().unwrap_or_else(|| ds.version.clone());
    // Keep (name, version) unique if either changed.
    if new_name != ds.name || new_version != ds.version {
        let clash = distribution_set::Entity::find()
            .filter(distribution_set::Column::Name.eq(&new_name))
            .filter(distribution_set::Column::Version.eq(&new_version))
            .one(&st.db)
            .await?
            .is_some_and(|other| other.id != ds.id);
        if clash {
            return Err(AppError::Conflict(format!(
                "distribution set {new_name}:{new_version} already exists"
            )));
        }
    }

    let mut am: distribution_set::ActiveModel = ds.into();
    if body.name.is_some() {
        am.name = Set(new_name);
    }
    if body.version.is_some() {
        am.version = Set(new_version);
    }
    if let Some(d) = body.description {
        am.description = Set(Some(d));
    }
    if let Some(r) = body.required_migration_step {
        am.required_migration_step = Set(r);
    }
    am.updated_at = Set(now_ms());
    let ds = am.update(&st.db).await?;
    Ok(Json(
        ds_with_modules(&st, &ds, &base_url(&st.cfg, &headers)).await?,
    ))
}

/// `POST /rest/v1/distributionsets/{id}/invalidate` — mark a set invalid so it
/// can no longer be assigned or rolled out, detach any auto-assignments that
/// reference it, and optionally stop its rollouts and cancel its in-flight
/// actions (hawkBit `MgmtInvalidateDistributionSetRequestBody`).
pub async fn invalidate(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<DsInvalidate>,
) -> Result<StatusCode, AppError> {
    let ds = distribution_set::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set"))?;
    if ds.invalid {
        return Err(AppError::Conflict(
            "distribution set already invalidated".into(),
        ));
    }
    let mode = body.action_cancelation_type.as_deref().unwrap_or("none");
    if !matches!(mode, "none" | "soft" | "force") {
        return Err(AppError::BadRequest(format!(
            "invalid actionCancelationType: {mode}"
        )));
    }

    // 1. Mark invalid — blocks any further assignment (see domain::assign_ds).
    let mut am: distribution_set::ActiveModel = ds.into();
    am.invalid = Set(true);
    am.updated_at = Set(now_ms());
    am.update(&st.db).await?;

    // 2. Detach auto-assignments that point at this set.
    for f in target_filter::Entity::find()
        .filter(target_filter::Column::AutoAssignDsId.eq(id))
        .all(&st.db)
        .await?
    {
        let mut fm: target_filter::ActiveModel = f.into();
        fm.auto_assign_ds_id = Set(None);
        fm.auto_assign_action_type = Set(None);
        fm.updated_at = Set(now_ms());
        fm.update(&st.db).await?;
    }

    // 3. Optionally stop rollouts deploying this set. "stopped" is terminal —
    //    the evaluator only advances "running" rollouts.
    if body.cancel_rollouts {
        for r in rollout::Entity::find()
            .filter(rollout::Column::DsId.eq(id))
            .filter(rollout::Column::Status.is_not_in(["finished", "stopped"]))
            .all(&st.db)
            .await?
        {
            let mut rm: rollout::ActiveModel = r.into();
            rm.status = Set("stopped".into());
            rm.updated_at = Set(now_ms());
            rm.update(&st.db).await?;
        }
    }

    // 4. Cancel in-flight actions referencing this set.
    if mode != "none" {
        for a in action::Entity::find()
            .filter(action::Column::DsId.eq(id))
            .filter(action::Column::Active.eq(true))
            .all(&st.db)
            .await?
        {
            let (aid, target_id) = (a.id, a.target_id);
            let mut aam: action::ActiveModel = a.into();
            if mode == "force" {
                aam.status = Set("canceled".into());
                aam.active = Set(false);
                aam.updated_at = Set(now_ms());
                aam.update(&st.db).await?;
                crate::domain::deployment::add_action_status(
                    &st.db,
                    aid,
                    "canceled",
                    &["distribution set invalidated".into()],
                )
                .await?;
                // Reset the target so it isn't left "pending" forever.
                if let Some(t) = target::Entity::find_by_id(target_id).one(&st.db).await? {
                    let installed = t.installed_ds_id.is_some();
                    let mut tm: target::ActiveModel = t.into();
                    tm.update_status = Set(if installed { "in_sync" } else { "registered" }.into());
                    tm.updated_at = Set(now_ms());
                    tm.update(&st.db).await?;
                }
            } else {
                aam.status = Set("canceling".into());
                aam.updated_at = Set(now_ms());
                aam.update(&st.db).await?;
                crate::domain::deployment::add_action_status(
                    &st.db,
                    aid,
                    "canceling",
                    &["distribution set invalidated".into()],
                )
                .await?;
            }
        }
    }

    Ok(StatusCode::OK)
}

pub async fn delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
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
    let complete = compute_complete(&st.db, ds.type_id, ds.id).await?;
    let mut am: distribution_set::ActiveModel = ds.into();
    am.complete = Set(complete);
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
