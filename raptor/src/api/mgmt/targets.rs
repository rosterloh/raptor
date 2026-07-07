use super::dto::{target_rest, TargetRest};
use crate::api::paging::{apply_sort, page, ListParams, Paged};
use crate::entity::{target, target_attribute};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::{base_url, now_ms, random_token};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use raptor_api_types::{TargetCreate, TargetUpdate};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait,
    QueryFilter,
};
use std::collections::{BTreeMap, HashSet};

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
                )))
            }
            Err(AppError::NotFound(_)) => {}
            Err(e) => return Err(e),
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

pub async fn list(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<TargetRest>>, AppError> {
    let base = base_url(&st.cfg, &headers);
    let interval = st.cfg.ddi.polling_duration();
    let mut sel = target::Entity::find();
    if let Some(q) = &p.q {
        let expr = crate::fiql::parse(q).map_err(AppError::BadRequest)?;
        sel = sel.filter(crate::fiql::to_condition(&expr, &fiql_map)?);
    }
    sel = apply_sort(sel, &p.sort, &fiql_map)?;
    let (rows, total) = page(&st.db, sel, &p).await?;
    Ok(Json(Paged::new(
        rows.iter()
            .map(|t| target_rest(t, interval, &base))
            .collect(),
        total,
    )))
}

pub async fn get_one(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(cid): Path<String>,
) -> Result<Json<TargetRest>, AppError> {
    let t = find_by_cid(&st.db, &cid).await?;
    Ok(Json(target_rest(
        &t,
        st.cfg.ddi.polling_duration(),
        &base_url(&st.cfg, &headers),
    )))
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
    t.delete(&st.db).await?;
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
