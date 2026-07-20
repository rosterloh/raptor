use crate::api::mgmt::actions::ds_rest_for;
use crate::api::paging::{apply_sort, page, ListParams, Paged};
use crate::entity::{distribution_set, target_filter};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::{base_url, now_ms};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use raptor_api_types::{
    AutoAssignRequest, TargetFilterCreate, TargetFilterRest, TargetFilterUpdate,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, ModelTrait, QueryFilter,
};

fn fiql_map(f: &str) -> Option<target_filter::Column> {
    match f {
        "id" => Some(target_filter::Column::Id),
        "name" => Some(target_filter::Column::Name),
        _ => None,
    }
}

pub fn filter_rest(f: &target_filter::Model, base: &str) -> TargetFilterRest {
    TargetFilterRest {
        id: f.id,
        name: f.name.clone(),
        query: f.query.clone(),
        auto_assign_distribution_set: f.auto_assign_ds_id,
        auto_assign_action_type: f.auto_assign_action_type.clone(),
        created_at: f.created_at,
        last_modified_at: f.updated_at,
        links: serde_json::json!({
            "self": {"href": format!("{base}/rest/v1/targetfilters/{}", f.id)},
            "autoAssignDS": {
                "href": format!("{base}/rest/v1/targetfilters/{}/autoAssignDS", f.id)
            }
        }),
    }
}

async fn find(st: &AppState, id: i64) -> Result<target_filter::Model, AppError> {
    target_filter::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("target filter"))
}

/// Parses and compiles the FIQL query against the target field map so bad
/// queries are rejected at write time with a hawkBit-style 400.
fn validate_query(q: &str) -> Result<(), AppError> {
    let expr = crate::fiql::parse(q).map_err(AppError::BadRequest)?;
    crate::fiql::to_condition(&expr, &crate::api::mgmt::targets::fiql_map)?;
    Ok(())
}

pub async fn create(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<TargetFilterCreate>,
) -> Result<(StatusCode, Json<TargetFilterRest>), AppError> {
    validate_query(&body.query)?;
    if target_filter::Entity::find()
        .filter(target_filter::Column::Name.eq(&body.name))
        .one(&st.db)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(format!(
            "target filter {} already exists",
            body.name
        )));
    }
    let now = now_ms();
    let f = target_filter::ActiveModel {
        name: Set(body.name),
        query: Set(body.query),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&st.db)
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(filter_rest(&f, &base_url(&st.cfg, &headers))),
    ))
}

pub async fn list(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<TargetFilterRest>>, AppError> {
    let base = base_url(&st.cfg, &headers);
    let mut sel = target_filter::Entity::find();
    if let Some(q) = &p.q {
        let expr = crate::fiql::parse(q).map_err(AppError::BadRequest)?;
        sel = sel.filter(crate::fiql::to_condition(&expr, &fiql_map)?);
    }
    sel = apply_sort(sel, &p.sort, &fiql_map)?;
    let (rows, total) = page(&st.db, sel, &p).await?;
    Ok(Json(Paged::new(
        rows.iter().map(|f| filter_rest(f, &base)).collect(),
        total,
    )))
}

pub async fn get_one(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<TargetFilterRest>, AppError> {
    let f = find(&st, id).await?;
    Ok(Json(filter_rest(&f, &base_url(&st.cfg, &headers))))
}

pub async fn update(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(u): Json<TargetFilterUpdate>,
) -> Result<Json<TargetFilterRest>, AppError> {
    let f = find(&st, id).await?;
    let mut am: target_filter::ActiveModel = f.into();
    if let Some(name) = u.name {
        am.name = Set(name);
    }
    if let Some(query) = u.query {
        validate_query(&query)?;
        am.query = Set(query);
    }
    am.updated_at = Set(now_ms());
    let f = am.update(&st.db).await?;
    Ok(Json(filter_rest(&f, &base_url(&st.cfg, &headers))))
}

pub async fn delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let f = find(&st, id).await?;
    f.delete(&st.db).await?;
    Ok(StatusCode::OK)
}

pub async fn get_auto_assign(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    let f = find(&st, id).await?;
    match ds_rest_for(&st, f.auto_assign_ds_id, &headers).await? {
        Some(v) => Ok(Json(v).into_response()),
        None => Ok(StatusCode::NO_CONTENT.into_response()),
    }
}

pub async fn set_auto_assign(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<AutoAssignRequest>,
) -> Result<Json<TargetFilterRest>, AppError> {
    let f = find(&st, id).await?;
    let ds = distribution_set::Entity::find_by_id(body.id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set"))?;
    if !ds.complete {
        return Err(AppError::BadRequest(
            "distribution set is incomplete".into(),
        ));
    }
    let action_type = match body.action_type.as_deref() {
        None | Some("forced") => "forced",
        Some("soft") => "soft",
        Some(other) => {
            return Err(AppError::BadRequest(format!(
                "unsupported action type: {other}"
            )))
        }
    };
    let mut am: target_filter::ActiveModel = f.into();
    am.auto_assign_ds_id = Set(Some(ds.id));
    am.auto_assign_action_type = Set(Some(action_type.into()));
    am.updated_at = Set(now_ms());
    let f = am.update(&st.db).await?;
    // Assign to every currently matching target immediately (hawkBit semantics).
    crate::domain::target_filter::run_auto_assign(&st, &f).await?;
    Ok(Json(filter_rest(&f, &base_url(&st.cfg, &headers))))
}

pub async fn delete_auto_assign(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<TargetFilterRest>, AppError> {
    let f = find(&st, id).await?;
    let mut am: target_filter::ActiveModel = f.into();
    am.auto_assign_ds_id = Set(None);
    am.auto_assign_action_type = Set(None);
    am.updated_at = Set(now_ms());
    let f = am.update(&st.db).await?;
    Ok(Json(filter_rest(&f, &base_url(&st.cfg, &headers))))
}
