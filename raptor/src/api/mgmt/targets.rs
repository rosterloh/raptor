//! Target CRUD (`/rest/v1/targets`), attributes, type assignment, and
//! auto-confirm toggles. Action/deployment sub-resources live in `actions`,
//! metadata sub-resources in `metadata`.

use super::mappers::{TargetRest, target_rest};
use crate::api::paging::{ListParams, Paged, apply_sort, page};
use crate::entity::{
    distribution_set, distribution_set_type, target, target_attribute, target_metadata, target_tag,
    target_tag_assignment, target_type,
};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::{base_url, now_ms, random_token};
use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use raptor_api_types::{DsRef, TagRef, TargetCreate, TargetUpdate};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait,
    QueryFilter,
};
use std::collections::{BTreeMap, HashSet};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/rest/v1/targets", post(create).get(list))
        .route(
            "/rest/v1/targets/{cid}",
            get(get_one).put(update).delete(delete),
        )
        .route("/rest/v1/targets/{cid}/attributes", get(attributes))
        .route(
            "/rest/v1/targets/{cid}/targettype",
            post(assign_type).delete(unassign_type),
        )
        .route(
            "/rest/v1/targets/{cid}/autoConfirm",
            get(auto_confirm_status),
        )
        .route(
            "/rest/v1/targets/{cid}/autoConfirm/activate",
            post(activate_auto_confirm),
        )
        .route(
            "/rest/v1/targets/{cid}/autoConfirm/deactivate",
            post(deactivate_auto_confirm),
        )
}

pub fn fiql_map(f: &str) -> Option<target::Column> {
    match f {
        "id" | "controllerId" => Some(target::Column::ControllerId),
        "name" => Some(target::Column::Name),
        "description" => Some(target::Column::Description),
        "updateStatus" => Some(target::Column::UpdateStatus),
        "lastControllerRequestAt" => Some(target::Column::LastPollAt),
        "address" => Some(target::Column::Address),
        _ => None,
    }
}

/// Compiles a FIQL query over targets. Shared by the target list, saved target
/// filters and rollouts so `tag==beta` means the same thing everywhere.
pub fn condition(q: &str) -> Result<sea_orm::Condition, AppError> {
    let expr = crate::fiql::parse(q).map_err(AppError::BadRequest)?;
    crate::fiql::to_condition_ext(&expr, &fiql_map, &super::tags::target_tag_field)
}

pub async fn find_by_cid(db: &DatabaseConnection, cid: &str) -> Result<target::Model, AppError> {
    target::Entity::find()
        .filter(target::Column::ControllerId.eq(cid))
        .one(db)
        .await?
        .ok_or(AppError::NotFound("target"))
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Vec<TargetCreate>>,
) -> Result<(StatusCode, Json<Vec<TargetRest>>), AppError> {
    // Phase 1: Validate all items first
    let mut seen = HashSet::new();
    for c in &body {
        if !seen.insert(&c.controller_id) {
            return Err(AppError::Conflict(format!(
                "duplicate controllerId {} in request",
                c.controller_id
            )));
        }
        match find_by_cid(&st.db, &c.controller_id).await {
            Ok(_) => {
                return Err(AppError::Conflict(format!(
                    "target {} already exists",
                    c.controller_id
                )));
            }
            Err(AppError::NotFound(_)) => {}
            Err(e) => return Err(e),
        }
        if let Some(tt) = c.target_type {
            target_type::Entity::find_by_id(tt)
                .one(&st.db)
                .await?
                .ok_or(AppError::NotFound("target type"))?;
        }
    }

    // Phase 2: Insert all items
    let base = base_url(&st.cfg, &headers);
    let interval = st.cfg.ddi.polling_duration();
    let mut out = Vec::with_capacity(body.len());
    for c in body {
        let now = now_ms();
        let t = target::ActiveModel {
            name: Set(c.name.unwrap_or_else(|| c.controller_id.clone())),
            controller_id: Set(c.controller_id),
            description: Set(c.description),
            security_token: Set(c.security_token.unwrap_or_else(random_token)),
            update_status: Set("unknown".into()),
            type_id: Set(c.target_type),
            auto_confirm: Set(st.cfg.ddi.auto_confirm_default),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&st.db)
        .await?;
        out.push(target_rest(&t, interval, &base));
    }
    Ok((StatusCode::CREATED, Json(out)))
}

/// The distribution sets and tags a batch of targets refers to, resolved in a
/// fixed three queries rather than one per row.
///
/// `target` stores `assigned_ds_id` / `installed_ds_id` as plain columns, so the
/// sets are one `IN` lookup over the distinct ids on the page, their type keys a
/// second, and the tag assignments a third. That is what makes it safe to put
/// these on a list endpoint at all — the alternative is the caller issuing a
/// request per row, which is what this exists to remove.
async fn batch_refs(
    db: &DatabaseConnection,
    models: &[target::Model],
) -> Result<(BTreeMap<i64, DsRef>, BTreeMap<i64, Vec<TagRef>>), AppError> {
    let ds_ids: Vec<i64> = models
        .iter()
        .flat_map(|t| [t.assigned_ds_id, t.installed_ds_id])
        .flatten()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let mut sets = BTreeMap::new();
    if !ds_ids.is_empty() {
        let ds_rows = distribution_set::Entity::find()
            .filter(distribution_set::Column::Id.is_in(ds_ids))
            .all(db)
            .await?;
        let type_ids: Vec<i64> = ds_rows
            .iter()
            .map(|d| d.type_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let type_keys: BTreeMap<i64, String> = distribution_set_type::Entity::find()
            .filter(distribution_set_type::Column::Id.is_in(type_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|t| (t.id, t.key))
            .collect();
        for d in ds_rows {
            sets.insert(
                d.id,
                DsRef {
                    id: d.id,
                    name: d.name,
                    version: d.version,
                    // A set whose type row vanished is a broken invariant, not a
                    // reason to fail the whole page.
                    ds_type: type_keys.get(&d.type_id).cloned().unwrap_or_default(),
                },
            );
        }
    }

    let target_ids: Vec<i64> = models.iter().map(|t| t.id).collect();
    let mut tags: BTreeMap<i64, Vec<TagRef>> = BTreeMap::new();
    if !target_ids.is_empty() {
        let links = target_tag_assignment::Entity::find()
            .filter(target_tag_assignment::Column::TargetId.is_in(target_ids))
            .all(db)
            .await?;
        let tag_ids: Vec<i64> = links
            .iter()
            .map(|l| l.tag_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if !tag_ids.is_empty() {
            let by_id: BTreeMap<i64, TagRef> = target_tag::Entity::find()
                .filter(target_tag::Column::Id.is_in(tag_ids))
                .all(db)
                .await?
                .into_iter()
                .map(|t| {
                    (
                        t.id,
                        TagRef {
                            id: t.id,
                            name: t.name,
                            colour: t.colour,
                        },
                    )
                })
                .collect();
            for l in links {
                if let Some(tag) = by_id.get(&l.tag_id) {
                    tags.entry(l.target_id).or_default().push(tag.clone());
                }
            }
        }
    }
    Ok((sets, tags))
}

/// Fills the additive reference fields on an already-mapped target.
fn attach_refs(
    out: &mut TargetRest,
    model: &target::Model,
    sets: &BTreeMap<i64, DsRef>,
    tags: &BTreeMap<i64, Vec<TagRef>>,
) {
    out.installed_ds = model.installed_ds_id.and_then(|id| sets.get(&id).cloned());
    out.assigned_ds = model.assigned_ds_id.and_then(|id| sets.get(&id).cloned());
    out.tags = tags.get(&model.id).cloned().unwrap_or_default();
}

pub async fn list(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<TargetRest>>, AppError> {
    let base = base_url(&st.cfg, &headers);
    let interval = st.cfg.ddi.polling_duration();
    let mut sel = target::Entity::find();
    if let Some(q) = &p.q {
        sel = sel.filter(condition(q)?);
    }
    sel = apply_sort(sel, &p.sort, &fiql_map)?;
    let (rows, total) = page(&st.db, sel, &p).await?;
    let (sets, tags) = batch_refs(&st.db, &rows).await?;
    let content = rows
        .iter()
        .map(|t| {
            let mut out = target_rest(t, interval, &base);
            attach_refs(&mut out, t, &sets, &tags);
            out
        })
        .collect();
    Ok(Json(Paged::new(content, total)))
}

pub async fn get_one(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(cid): Path<String>,
) -> Result<Json<TargetRest>, AppError> {
    let t = find_by_cid(&st.db, &cid).await?;
    // Same shape as a row of the list, so a consumer does not have to special-case
    // the single-target payload.
    let (sets, tags) = batch_refs(&st.db, std::slice::from_ref(&t)).await?;
    let mut out = target_rest(
        &t,
        st.cfg.ddi.polling_duration(),
        &base_url(&st.cfg, &headers),
    );
    attach_refs(&mut out, &t, &sets, &tags);
    Ok(Json(out))
}

pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(cid): Path<String>,
    Json(u): Json<TargetUpdate>,
) -> Result<Json<TargetRest>, AppError> {
    let t = find_by_cid(&st.db, &cid).await?;
    let mut am: target::ActiveModel = t.into();
    if let Some(v) = u.name {
        am.name = Set(v);
    }
    if let Some(v) = u.description {
        am.description = Set(Some(v));
    }
    if let Some(v) = u.security_token {
        am.security_token = Set(v);
    }
    if let Some(v) = u.request_attributes {
        am.request_attributes = Set(v);
    }
    am.updated_at = Set(now_ms());
    let t = am.update(&st.db).await?;
    Ok(Json(target_rest(
        &t,
        st.cfg.ddi.polling_duration(),
        &base_url(&st.cfg, &headers),
    )))
}

pub async fn delete(
    State(st): State<AppState>,
    Path(cid): Path<String>,
) -> Result<StatusCode, AppError> {
    let t = find_by_cid(&st.db, &cid).await?;
    target_attribute::Entity::delete_many()
        .filter(target_attribute::Column::TargetId.eq(t.id))
        .exec(&st.db)
        .await?;
    target_metadata::Entity::delete_many()
        .filter(target_metadata::Column::TargetId.eq(t.id))
        .exec(&st.db)
        .await?;
    t.delete(&st.db).await?;
    Ok(StatusCode::OK)
}

pub async fn auto_confirm_status(
    State(st): State<AppState>,
    Path(cid): Path<String>,
) -> Result<Json<raptor_api_types::AutoConfirmState>, AppError> {
    let t = find_by_cid(&st.db, &cid).await?;
    Ok(Json(raptor_api_types::AutoConfirmState {
        active: t.auto_confirm,
        activated_at: None,
    }))
}

async fn set_auto_confirm(st: &AppState, cid: &str, on: bool) -> Result<StatusCode, AppError> {
    let t = find_by_cid(&st.db, cid).await?;
    let target_id = t.id;
    let mut am: target::ActiveModel = t.into();
    am.auto_confirm = Set(on);
    am.updated_at = Set(now_ms());
    am.update(&st.db).await?;
    if on {
        crate::domain::deployment::confirm_waiting_actions(st, target_id).await?;
    }
    Ok(StatusCode::OK)
}

pub async fn activate_auto_confirm(
    State(st): State<AppState>,
    Path(cid): Path<String>,
) -> Result<StatusCode, AppError> {
    set_auto_confirm(&st, &cid, true).await
}

pub async fn deactivate_auto_confirm(
    State(st): State<AppState>,
    Path(cid): Path<String>,
) -> Result<StatusCode, AppError> {
    set_auto_confirm(&st, &cid, false).await
}

/// `POST /rest/v1/targets/{cid}/targettype` — assign (or change) the target's
/// type. Body is `{ "id": <targetTypeId> }`.
pub async fn assign_type(
    State(st): State<AppState>,
    Path(cid): Path<String>,
    Json(r): Json<raptor_api_types::TypeRef>,
) -> Result<StatusCode, AppError> {
    let t = find_by_cid(&st.db, &cid).await?;
    target_type::Entity::find_by_id(r.id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("target type"))?;
    let mut am: target::ActiveModel = t.into();
    am.type_id = Set(Some(r.id));
    am.updated_at = Set(now_ms());
    am.update(&st.db).await?;
    Ok(StatusCode::OK)
}

/// `DELETE /rest/v1/targets/{cid}/targettype` — unassign the target's type.
pub async fn unassign_type(
    State(st): State<AppState>,
    Path(cid): Path<String>,
) -> Result<StatusCode, AppError> {
    let t = find_by_cid(&st.db, &cid).await?;
    let mut am: target::ActiveModel = t.into();
    am.type_id = Set(None);
    am.updated_at = Set(now_ms());
    am.update(&st.db).await?;
    Ok(StatusCode::OK)
}

pub async fn attributes(
    State(st): State<AppState>,
    Path(cid): Path<String>,
) -> Result<Json<BTreeMap<String, String>>, AppError> {
    let t = find_by_cid(&st.db, &cid).await?;
    let rows = target_attribute::Entity::find()
        .filter(target_attribute::Column::TargetId.eq(t.id))
        .all(&st.db)
        .await?;
    Ok(Json(rows.into_iter().map(|r| (r.key, r.value)).collect()))
}
