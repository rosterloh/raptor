//! Target-type CRUD (`/rest/v1/targettypes`), including the
//! compatible-distribution-set-type membership sub-resource.

use super::distribution_set_types::ds_type_json;
use crate::api::paging::{ListParams, Paged, page};
use crate::entity::{distribution_set_type, target, target_type, target_type_ds_type};
use crate::error::AppError;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use raptor_api_types::{TargetTypeCreate, TargetTypeUpdate, TypeRef};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};
use serde_json::{Value, json};

fn target_type_json(t: &target_type::Model) -> Value {
    let base = format!("/rest/v1/targettypes/{}", t.id);
    json!({
        "id": t.id,
        "name": t.name,
        "description": t.description,
        "colour": t.colour,
        "deleted": false,
        "_links": {
            "self": {"href": base},
            "compatibledistributionsettypes": {"href": format!("{base}/compatibledistributionsettypes")},
        },
    })
}

async fn check_ds_types(st: &AppState, refs: &[TypeRef]) -> Result<(), AppError> {
    for r in refs {
        distribution_set_type::Entity::find_by_id(r.id)
            .one(&st.db)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("unknown distribution set type: {}", r.id))
            })?;
    }
    Ok(())
}

pub async fn tt_list(
    State(st): State<AppState>,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<Value>>, AppError> {
    let (rows, total) = page(&st.db, target_type::Entity::find(), &p).await?;
    Ok(Json(Paged::new(
        rows.iter().map(target_type_json).collect(),
        total,
    )))
}

pub async fn tt_one(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    let t = target_type::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("target type"))?;
    Ok(Json(target_type_json(&t)))
}

pub async fn tt_create(
    State(st): State<AppState>,
    Json(body): Json<Vec<TargetTypeCreate>>,
) -> Result<(StatusCode, Json<Vec<Value>>), AppError> {
    for c in &body {
        if target_type::Entity::find()
            .filter(target_type::Column::Name.eq(&c.name))
            .one(&st.db)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict(format!(
                "target type {} already exists",
                c.name
            )));
        }
        check_ds_types(&st, &c.compatibledistributionsettypes).await?;
    }
    let mut out = Vec::with_capacity(body.len());
    for c in body {
        let t = target_type::ActiveModel {
            name: Set(c.name),
            description: Set(c.description),
            colour: Set(c.colour),
            ..Default::default()
        }
        .insert(&st.db)
        .await?;
        for r in &c.compatibledistributionsettypes {
            target_type_ds_type::ActiveModel {
                target_type_id: Set(t.id),
                ds_type_id: Set(r.id),
            }
            .insert(&st.db)
            .await?;
        }
        out.push(target_type_json(&t));
    }
    Ok((StatusCode::CREATED, Json(out)))
}

pub async fn tt_update(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(u): Json<TargetTypeUpdate>,
) -> Result<Json<Value>, AppError> {
    let t = target_type::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("target type"))?;
    // A rename must not collide with another type.
    if let Some(name) = &u.name
        && target_type::Entity::find()
            .filter(target_type::Column::Name.eq(name))
            .one(&st.db)
            .await?
            .is_some_and(|other| other.id != t.id)
    {
        return Err(AppError::Conflict(format!(
            "target type {name} already exists"
        )));
    }
    let mut am: target_type::ActiveModel = t.into();
    if let Some(n) = u.name {
        am.name = Set(n);
    }
    if let Some(d) = u.description {
        am.description = Set(Some(d));
    }
    if let Some(c) = u.colour {
        am.colour = Set(Some(c));
    }
    let t = am.update(&st.db).await?;
    Ok(Json(target_type_json(&t)))
}

pub async fn tt_delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let t = target_type::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("target type"))?;
    if target::Entity::find()
        .filter(target::Column::TypeId.eq(t.id))
        .count(&st.db)
        .await?
        > 0
    {
        return Err(AppError::Conflict("target type is in use".into()));
    }
    target_type_ds_type::Entity::delete_many()
        .filter(target_type_ds_type::Column::TargetTypeId.eq(t.id))
        .exec(&st.db)
        .await?;
    target_type::Entity::delete_by_id(t.id).exec(&st.db).await?;
    Ok(StatusCode::OK)
}

pub async fn tt_compat_list(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Paged<Value>>, AppError> {
    target_type::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("target type"))?;
    let links = target_type_ds_type::Entity::find()
        .filter(target_type_ds_type::Column::TargetTypeId.eq(id))
        .all(&st.db)
        .await?;
    let ids: Vec<i64> = links.iter().map(|l| l.ds_type_id).collect();
    let types = distribution_set_type::Entity::find()
        .filter(distribution_set_type::Column::Id.is_in(ids))
        .all(&st.db)
        .await?;
    let total = types.len() as u64;
    Ok(Json(Paged::new(
        types.iter().map(ds_type_json).collect(),
        total,
    )))
}

pub async fn tt_add_compat(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(r): Json<TypeRef>,
) -> Result<StatusCode, AppError> {
    target_type::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("target type"))?;
    distribution_set_type::Entity::find_by_id(r.id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set type"))?;
    if target_type_ds_type::Entity::find_by_id((id, r.id))
        .one(&st.db)
        .await?
        .is_none()
    {
        target_type_ds_type::ActiveModel {
            target_type_id: Set(id),
            ds_type_id: Set(r.id),
        }
        .insert(&st.db)
        .await?;
    }
    Ok(StatusCode::OK)
}

pub async fn tt_remove_compat(
    State(st): State<AppState>,
    Path((id, dsid)): Path<(i64, i64)>,
) -> Result<StatusCode, AppError> {
    target_type::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("target type"))?;
    let row = target_type_ds_type::Entity::find_by_id((id, dsid))
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set type"))?;
    target_type_ds_type::Entity::delete_by_id((row.target_type_id, row.ds_type_id))
        .exec(&st.db)
        .await?;
    Ok(StatusCode::OK)
}
