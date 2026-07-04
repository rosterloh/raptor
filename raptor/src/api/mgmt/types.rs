use crate::api::paging::{ListParams, Paged};
use crate::entity::{distribution_set_type, software_module_type};
use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;
use sea_orm::EntityTrait;
use serde_json::{json, Value};

fn type_json(id: i64, key: &str, name: &str, kind: &str) -> Value {
    json!({"id": id, "key": key, "name": name, "deleted": false,
           "_links": {"self": {"href": format!("/rest/v1/{kind}/{id}")}}})
}

pub async fn sm_types(State(st): State<AppState>, Query(p): Query<ListParams>) -> Result<Json<Paged<Value>>, AppError> {
    let (rows, total) = crate::api::paging::page(&st.db, software_module_type::Entity::find(), &p).await?;
    Ok(Json(Paged::new(rows.iter().map(|t| type_json(t.id, &t.key, &t.name, "softwaremoduletypes")).collect(), total)))
}

pub async fn sm_type(State(st): State<AppState>, Path(id): Path<i64>) -> Result<Json<Value>, AppError> {
    let t = software_module_type::Entity::find_by_id(id).one(&st.db).await?.ok_or(AppError::NotFound("software module type"))?;
    Ok(Json(type_json(t.id, &t.key, &t.name, "softwaremoduletypes")))
}

pub async fn ds_types(State(st): State<AppState>, Query(p): Query<ListParams>) -> Result<Json<Paged<Value>>, AppError> {
    let (rows, total) = crate::api::paging::page(&st.db, distribution_set_type::Entity::find(), &p).await?;
    Ok(Json(Paged::new(rows.iter().map(|t| type_json(t.id, &t.key, &t.name, "distributionsettypes")).collect(), total)))
}

pub async fn ds_type(State(st): State<AppState>, Path(id): Path<i64>) -> Result<Json<Value>, AppError> {
    let t = distribution_set_type::Entity::find_by_id(id).one(&st.db).await?.ok_or(AppError::NotFound("distribution set type"))?;
    Ok(Json(type_json(t.id, &t.key, &t.name, "distributionsettypes")))
}
