//! Software-module-type CRUD (`/rest/v1/softwaremoduletypes`) — the type
//! catalogue that software modules reference (e.g. "os", "app").

use crate::api::paging::{ListParams, Paged, page};
use crate::entity::{ds_type_module, software_module, software_module_type};
use crate::error::AppError;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use raptor_api_types::{SmTypeCreate, SmTypeUpdate};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};
use serde_json::{Value, json};

pub(super) fn sm_type_json(t: &software_module_type::Model) -> Value {
    json!({
        "id": t.id,
        "key": t.key,
        "name": t.name,
        "description": t.description,
        "maxAssignments": t.max_assignments,
        "deleted": false,
        "_links": {"self": {"href": format!("/rest/v1/softwaremoduletypes/{}", t.id)}},
    })
}

pub async fn sm_types(
    State(st): State<AppState>,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<Value>>, AppError> {
    let (rows, total) = page(&st.db, software_module_type::Entity::find(), &p).await?;
    Ok(Json(Paged::new(
        rows.iter().map(sm_type_json).collect(),
        total,
    )))
}

pub async fn sm_type(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    let t = software_module_type::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("software module type"))?;
    Ok(Json(sm_type_json(&t)))
}

pub async fn sm_type_create(
    State(st): State<AppState>,
    Json(body): Json<Vec<SmTypeCreate>>,
) -> Result<(StatusCode, Json<Vec<Value>>), AppError> {
    let mut out = Vec::with_capacity(body.len());
    for c in body {
        if software_module_type::Entity::find()
            .filter(software_module_type::Column::Key.eq(&c.key))
            .one(&st.db)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict(format!(
                "software module type {} already exists",
                c.key
            )));
        }
        let t = software_module_type::ActiveModel {
            key: Set(c.key),
            name: Set(c.name),
            description: Set(c.description),
            max_assignments: Set(c.max_assignments.unwrap_or(1)),
            ..Default::default()
        }
        .insert(&st.db)
        .await?;
        out.push(sm_type_json(&t));
    }
    Ok((StatusCode::CREATED, Json(out)))
}

pub async fn sm_type_update(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(u): Json<SmTypeUpdate>,
) -> Result<Json<Value>, AppError> {
    let t = software_module_type::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("software module type"))?;
    let mut am: software_module_type::ActiveModel = t.into();
    if let Some(d) = u.description {
        am.description = Set(Some(d));
    }
    let t = am.update(&st.db).await?;
    Ok(Json(sm_type_json(&t)))
}

pub async fn sm_type_delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let t = software_module_type::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("software module type"))?;
    let modules = software_module::Entity::find()
        .filter(software_module::Column::TypeId.eq(t.id))
        .count(&st.db)
        .await?;
    let in_ds_types = ds_type_module::Entity::find()
        .filter(ds_type_module::Column::ModuleTypeId.eq(t.id))
        .count(&st.db)
        .await?;
    if modules > 0 || in_ds_types > 0 {
        return Err(AppError::Conflict("software module type is in use".into()));
    }
    software_module_type::Entity::delete_by_id(t.id)
        .exec(&st.db)
        .await?;
    Ok(StatusCode::OK)
}
