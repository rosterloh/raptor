use crate::api::paging::{apply_sort, page, ListParams, Paged};
use crate::domain::deployment::{action_rest, assign_ds};
use crate::entity::{action, distribution_set, distribution_set_type, target};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::{base_url, now_ms};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder};
use serde::Deserialize;
use serde_json::{json, Value};

fn fiql_map(f: &str) -> Option<action::Column> {
    match f {
        "id" => Some(action::Column::Id),
        "active" => Some(action::Column::Active),
        "detailstatus" | "detailStatus" => Some(action::Column::Status),
        _ => None,
    }
}

#[derive(Deserialize)]
pub struct DsAssignment {
    pub id: i64,
    #[serde(rename = "type")]
    pub assign_type: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum OneOrMany {
    One(DsAssignment),
    Many(Vec<DsAssignment>),
}

pub async fn assign(
    State(st): State<AppState>, Path(cid): Path<String>, Json(body): Json<OneOrMany>,
) -> Result<Json<Value>, AppError> {
    let t = super::targets::find_by_cid(&st.db, &cid).await?;
    let items = match body { OneOrMany::One(a) => vec![a], OneOrMany::Many(v) => v };
    let (mut assigned, mut already, mut ids) = (0, 0, Vec::new());
    for item in items {
        let forced = item.assign_type.as_deref() != Some("soft");
        // refetch target each round: assign_ds mutates it
        let t = super::targets::find_by_cid(&st.db, &t.controller_id).await?;
        let r = assign_ds(&st, &t, item.id, forced).await?;
        match r.action_id {
            Some(id) => { assigned += 1; ids.push(json!({"id": id})); }
            None => already += 1,
        }
    }
    Ok(Json(json!({"assigned": assigned, "alreadyAssigned": already, "total": assigned + already, "assignedActions": ids})))
}

async fn ds_json_for(st: &AppState, ds_id: Option<i64>, headers: &HeaderMap) -> Result<Option<Value>, AppError> {
    let Some(id) = ds_id else { return Ok(None) };
    let Some(ds) = distribution_set::Entity::find_by_id(id).one(&st.db).await? else { return Ok(None) };
    let base = base_url(&st.cfg, headers);
    let ty = distribution_set_type::Entity::find_by_id(ds.type_id).one(&st.db).await?
        .map(|t| t.key).unwrap_or_else(|| "?".into());
    let modules = super::distribution_sets::load_modules(st, ds.id, &base).await?;
    Ok(Some(super::dto::ds_rest(&ds, &ty, modules, &base)))
}

pub async fn assigned_ds(
    State(st): State<AppState>, headers: HeaderMap, Path(cid): Path<String>,
) -> Result<axum::response::Response, AppError> {
    let t = super::targets::find_by_cid(&st.db, &cid).await?;
    match ds_json_for(&st, t.assigned_ds_id, &headers).await? {
        Some(v) => Ok(axum::response::IntoResponse::into_response(Json(v))),
        None => Ok(axum::response::IntoResponse::into_response(StatusCode::NO_CONTENT)),
    }
}

pub async fn installed_ds(
    State(st): State<AppState>, headers: HeaderMap, Path(cid): Path<String>,
) -> Result<axum::response::Response, AppError> {
    let t = super::targets::find_by_cid(&st.db, &cid).await?;
    match ds_json_for(&st, t.installed_ds_id, &headers).await? {
        Some(v) => Ok(axum::response::IntoResponse::into_response(Json(v))),
        None => Ok(axum::response::IntoResponse::into_response(StatusCode::NO_CONTENT)),
    }
}

pub async fn target_actions(
    State(st): State<AppState>, headers: HeaderMap, Path(cid): Path<String>, Query(p): Query<ListParams>,
) -> Result<Json<Paged<Value>>, AppError> {
    let t = super::targets::find_by_cid(&st.db, &cid).await?;
    let base = base_url(&st.cfg, &headers);
    let mut sel = action::Entity::find().filter(action::Column::TargetId.eq(t.id));
    if let Some(q) = &p.q {
        let expr = crate::fiql::parse(q).map_err(AppError::BadRequest)?;
        sel = sel.filter(crate::fiql::to_condition(&expr, &fiql_map)?);
    }
    sel = if p.sort.is_some() { apply_sort(sel, &p.sort, &fiql_map)? } else { sel.order_by(action::Column::Id, Order::Desc) };
    let (rows, total) = page(&st.db, sel, &p).await?;
    Ok(Json(Paged::new(rows.iter().map(|a| action_rest(a, &base)).collect(), total)))
}

pub async fn target_action(
    State(st): State<AppState>, headers: HeaderMap, Path((cid, aid)): Path<(String, i64)>,
) -> Result<Json<Value>, AppError> {
    let t = super::targets::find_by_cid(&st.db, &cid).await?;
    let a = action::Entity::find_by_id(aid).one(&st.db).await?
        .filter(|a| a.target_id == t.id).ok_or(AppError::NotFound("action"))?;
    Ok(Json(action_rest(&a, &base_url(&st.cfg, &headers))))
}

#[derive(Deserialize)]
pub struct CancelParams { #[serde(default)] pub force: bool }

pub async fn cancel_action(
    State(st): State<AppState>, Path((cid, aid)): Path<(String, i64)>, Query(cp): Query<CancelParams>,
) -> Result<StatusCode, AppError> {
    let t = super::targets::find_by_cid(&st.db, &cid).await?;
    let a = action::Entity::find_by_id(aid).one(&st.db).await?
        .filter(|a| a.target_id == t.id).ok_or(AppError::NotFound("action"))?;
    if !a.active {
        return Err(AppError::Gone);
    }
    let action_id = a.id;
    let mut am: action::ActiveModel = a.into();
    if cp.force {
        am.status = Set("canceled".into());
        am.active = Set(false);
        am.updated_at = Set(now_ms());
        am.update(&st.db).await?;
        crate::domain::deployment::add_action_status(&st.db, action_id, "canceled", &["force canceled by operator".into()]).await?;
        let mut tm: target::ActiveModel = t.clone().into();
        tm.update_status = Set(if t.installed_ds_id.is_some() { "in_sync".into() } else { "registered".into() });
        tm.updated_at = Set(now_ms());
        tm.update(&st.db).await?;
    } else {
        am.status = Set("canceling".into());
        am.updated_at = Set(now_ms());
        am.update(&st.db).await?;
        crate::domain::deployment::add_action_status(&st.db, action_id, "canceling", &["cancel requested by operator".into()]).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn all_actions(
    State(st): State<AppState>, headers: HeaderMap, Query(p): Query<ListParams>,
) -> Result<Json<Paged<Value>>, AppError> {
    let base = base_url(&st.cfg, &headers);
    let mut sel = action::Entity::find();
    if let Some(q) = &p.q {
        let expr = crate::fiql::parse(q).map_err(AppError::BadRequest)?;
        sel = sel.filter(crate::fiql::to_condition(&expr, &fiql_map)?);
    }
    sel = if p.sort.is_some() { apply_sort(sel, &p.sort, &fiql_map)? } else { sel.order_by(action::Column::Id, Order::Desc) };
    let (rows, total) = page(&st.db, sel, &p).await?;
    Ok(Json(Paged::new(rows.iter().map(|a| action_rest(a, &base)).collect(), total)))
}
