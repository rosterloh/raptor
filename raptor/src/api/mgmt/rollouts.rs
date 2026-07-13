use crate::api::paging::{apply_sort, page, ListParams, Paged};
use crate::domain::rollout::{rollout_group_rest, rollout_rest};
use crate::entity::{rollout, rollout_group, rollout_target_group};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::base_url;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use raptor_api_types::{RolloutCreate, RolloutGroupRest, RolloutRest};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

fn fiql_map(f: &str) -> Option<rollout::Column> {
    match f {
        "id" => Some(rollout::Column::Id),
        "name" => Some(rollout::Column::Name),
        "status" => Some(rollout::Column::Status),
        _ => None,
    }
}

async fn find_rollout(st: &AppState, id: i64) -> Result<rollout::Model, AppError> {
    rollout::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("rollout"))
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RolloutCreate>,
) -> Result<(StatusCode, Json<RolloutRest>), AppError> {
    let r = crate::domain::rollout::create_rollout(&st, &body).await?;
    let base = base_url(&st.cfg, &headers);
    Ok((StatusCode::CREATED, Json(rollout_rest(&r, &base))))
}

pub async fn list(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<RolloutRest>>, AppError> {
    let base = base_url(&st.cfg, &headers);
    let mut sel = rollout::Entity::find();
    if let Some(q) = &p.q {
        let expr = crate::fiql::parse(q).map_err(AppError::BadRequest)?;
        sel = sel.filter(crate::fiql::to_condition(&expr, &fiql_map)?);
    }
    sel = apply_sort(sel, &p.sort, &fiql_map)?;
    let (rows, total) = page(&st.db, sel, &p).await?;
    Ok(Json(Paged::new(
        rows.iter().map(|r| rollout_rest(r, &base)).collect(),
        total,
    )))
}

pub async fn get_one(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<RolloutRest>, AppError> {
    let r = find_rollout(&st, id).await?;
    Ok(Json(rollout_rest(&r, &base_url(&st.cfg, &headers))))
}

pub async fn delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let r = find_rollout(&st, id).await?;
    crate::domain::rollout::delete_rollout(&st, r).await?;
    Ok(StatusCode::OK)
}

pub async fn start(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<RolloutRest>, AppError> {
    let r = find_rollout(&st, id).await?;
    let r = crate::domain::rollout::start_rollout(&st, r).await?;
    Ok(Json(rollout_rest(&r, &base_url(&st.cfg, &headers))))
}

pub async fn pause(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<RolloutRest>, AppError> {
    let r = find_rollout(&st, id).await?;
    let r = crate::domain::rollout::pause_rollout(&st, r).await?;
    Ok(Json(rollout_rest(&r, &base_url(&st.cfg, &headers))))
}

pub async fn resume(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<RolloutRest>, AppError> {
    let r = find_rollout(&st, id).await?;
    let r = crate::domain::rollout::resume_rollout(&st, r).await?;
    Ok(Json(rollout_rest(&r, &base_url(&st.cfg, &headers))))
}

pub async fn groups(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<RolloutGroupRest>>, AppError> {
    let r = find_rollout(&st, id).await?;
    let base = base_url(&st.cfg, &headers);
    let sel = rollout_group::Entity::find()
        .filter(rollout_group::Column::RolloutId.eq(r.id))
        .order_by_asc(rollout_group::Column::OrderIndex);
    let (rows, total) = page(&st.db, sel, &p).await?;
    Ok(Json(Paged::new(
        rows.iter()
            .map(|g| rollout_group_rest(g, r.id, &base))
            .collect(),
        total,
    )))
}

pub async fn group_one(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path((id, gid)): Path<(i64, i64)>,
) -> Result<Json<RolloutGroupRest>, AppError> {
    let r = find_rollout(&st, id).await?;
    let g = rollout_group::Entity::find_by_id(gid)
        .one(&st.db)
        .await?
        .filter(|g| g.rollout_id == r.id)
        .ok_or(AppError::NotFound("rollout group"))?;
    Ok(Json(rollout_group_rest(
        &g,
        r.id,
        &base_url(&st.cfg, &headers),
    )))
}

pub async fn group_targets(
    State(st): State<AppState>,
    Path((id, gid)): Path<(i64, i64)>,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<String>>, AppError> {
    let r = find_rollout(&st, id).await?;
    let g = rollout_group::Entity::find_by_id(gid)
        .one(&st.db)
        .await?
        .filter(|g| g.rollout_id == r.id)
        .ok_or(AppError::NotFound("rollout group"))?;
    let sel = rollout_target_group::Entity::find()
        .filter(rollout_target_group::Column::RolloutGroupId.eq(g.id));
    let (rows, total) = page(&st.db, sel, &p).await?;
    let target_ids: Vec<i64> = rows.iter().map(|m| m.target_id).collect();
    let cids: Vec<String> = crate::entity::target::Entity::find()
        .filter(crate::entity::target::Column::Id.is_in(target_ids))
        .all(&st.db)
        .await?
        .into_iter()
        .map(|t| t.controller_id)
        .collect();
    Ok(Json(Paged::new(cids, total)))
}
