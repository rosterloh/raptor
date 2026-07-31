//! Distribution-set-type CRUD (`/rest/v1/distributionsettypes`), including the
//! mandatory/optional software-module-type membership sub-resources.

use super::software_module_types::sm_type_json;
use crate::api::paging::{ListParams, Paged, page};
use crate::entity::{
    distribution_set, distribution_set_type, ds_type_module, software_module_type,
    target_type_ds_type,
};
use crate::error::AppError;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use raptor_api_types::{DsTypeCreate, DsTypeUpdate, TypeRef};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};
use serde_json::{Value, json};

pub(super) fn ds_type_json(t: &distribution_set_type::Model) -> Value {
    let base = format!("/rest/v1/distributionsettypes/{}", t.id);
    json!({
        "id": t.id,
        "key": t.key,
        "name": t.name,
        "description": t.description,
        "deleted": false,
        "_links": {
            "self": {"href": base},
            "mandatorymodules": {"href": format!("{base}/mandatorymoduletypes")},
            "optionalmodules": {"href": format!("{base}/optionalmoduletypes")},
        },
    })
}

pub async fn ds_types(
    State(st): State<AppState>,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<Value>>, AppError> {
    let (rows, total) = page(&st.db, distribution_set_type::Entity::find(), &p).await?;
    Ok(Json(Paged::new(
        rows.iter().map(ds_type_json).collect(),
        total,
    )))
}

pub async fn ds_type(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, AppError> {
    let t = distribution_set_type::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set type"))?;
    Ok(Json(ds_type_json(&t)))
}

/// Verify every referenced module type exists, returning 400 otherwise.
async fn check_module_types(st: &AppState, refs: &[TypeRef]) -> Result<(), AppError> {
    for r in refs {
        software_module_type::Entity::find_by_id(r.id)
            .one(&st.db)
            .await?
            .ok_or_else(|| {
                AppError::BadRequest(format!("unknown software module type: {}", r.id))
            })?;
    }
    Ok(())
}

pub async fn ds_type_create(
    State(st): State<AppState>,
    Json(body): Json<Vec<DsTypeCreate>>,
) -> Result<(StatusCode, Json<Vec<Value>>), AppError> {
    // Validate first so a bad item doesn't leave a partial write.
    for c in &body {
        if distribution_set_type::Entity::find()
            .filter(distribution_set_type::Column::Key.eq(&c.key))
            .one(&st.db)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict(format!(
                "distribution set type {} already exists",
                c.key
            )));
        }
        check_module_types(&st, &c.mandatorymodules).await?;
        check_module_types(&st, &c.optionalmodules).await?;
    }
    let mut out = Vec::with_capacity(body.len());
    for c in body {
        let t = distribution_set_type::ActiveModel {
            key: Set(c.key),
            name: Set(c.name),
            description: Set(c.description),
            ..Default::default()
        }
        .insert(&st.db)
        .await?;
        for (refs, mandatory) in [(&c.mandatorymodules, true), (&c.optionalmodules, false)] {
            for r in refs {
                ds_type_module::ActiveModel {
                    ds_type_id: Set(t.id),
                    module_type_id: Set(r.id),
                    mandatory: Set(mandatory),
                }
                .insert(&st.db)
                .await?;
            }
        }
        out.push(ds_type_json(&t));
    }
    Ok((StatusCode::CREATED, Json(out)))
}

pub async fn ds_type_update(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(u): Json<DsTypeUpdate>,
) -> Result<Json<Value>, AppError> {
    let t = distribution_set_type::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set type"))?;
    let mut am: distribution_set_type::ActiveModel = t.into();
    if let Some(d) = u.description {
        am.description = Set(Some(d));
    }
    let t = am.update(&st.db).await?;
    Ok(Json(ds_type_json(&t)))
}

pub async fn ds_type_delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let t = distribution_set_type::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set type"))?;
    let sets = distribution_set::Entity::find()
        .filter(distribution_set::Column::TypeId.eq(t.id))
        .count(&st.db)
        .await?;
    let in_target_types = target_type_ds_type::Entity::find()
        .filter(target_type_ds_type::Column::DsTypeId.eq(t.id))
        .count(&st.db)
        .await?;
    if sets > 0 || in_target_types > 0 {
        return Err(AppError::Conflict("distribution set type is in use".into()));
    }
    ds_type_module::Entity::delete_many()
        .filter(ds_type_module::Column::DsTypeId.eq(t.id))
        .exec(&st.db)
        .await?;
    distribution_set_type::Entity::delete_by_id(t.id)
        .exec(&st.db)
        .await?;
    Ok(StatusCode::OK)
}

async fn ds_type_module_list(
    st: &AppState,
    ds_type_id: i64,
    mandatory: bool,
) -> Result<Json<Paged<Value>>, AppError> {
    distribution_set_type::Entity::find_by_id(ds_type_id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set type"))?;
    let links = ds_type_module::Entity::find()
        .filter(ds_type_module::Column::DsTypeId.eq(ds_type_id))
        .filter(ds_type_module::Column::Mandatory.eq(mandatory))
        .all(&st.db)
        .await?;
    let ids: Vec<i64> = links.iter().map(|l| l.module_type_id).collect();
    let types = software_module_type::Entity::find()
        .filter(software_module_type::Column::Id.is_in(ids))
        .all(&st.db)
        .await?;
    let total = types.len() as u64;
    Ok(Json(Paged::new(
        types.iter().map(sm_type_json).collect(),
        total,
    )))
}

pub async fn ds_type_mandatory(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Paged<Value>>, AppError> {
    ds_type_module_list(&st, id, true).await
}

pub async fn ds_type_optional(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Paged<Value>>, AppError> {
    ds_type_module_list(&st, id, false).await
}

async fn ds_type_add_module(
    st: &AppState,
    ds_type_id: i64,
    module_type_id: i64,
    mandatory: bool,
) -> Result<StatusCode, AppError> {
    distribution_set_type::Entity::find_by_id(ds_type_id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set type"))?;
    software_module_type::Entity::find_by_id(module_type_id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("software module type"))?;
    // Adding to a type already used by sets would change their completeness
    // retroactively; hawkBit forbids editing an in-use type's composition.
    if distribution_set::Entity::find()
        .filter(distribution_set::Column::TypeId.eq(ds_type_id))
        .count(&st.db)
        .await?
        > 0
    {
        return Err(AppError::Conflict(
            "distribution set type is in use and cannot be modified".into(),
        ));
    }
    let existing = ds_type_module::Entity::find_by_id((ds_type_id, module_type_id))
        .one(&st.db)
        .await?;
    match existing {
        Some(row) if row.mandatory != mandatory => {
            let mut am: ds_type_module::ActiveModel = row.into();
            am.mandatory = Set(mandatory);
            am.update(&st.db).await?;
        }
        Some(_) => {}
        None => {
            ds_type_module::ActiveModel {
                ds_type_id: Set(ds_type_id),
                module_type_id: Set(module_type_id),
                mandatory: Set(mandatory),
            }
            .insert(&st.db)
            .await?;
        }
    }
    Ok(StatusCode::OK)
}

pub async fn ds_type_add_mandatory(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(r): Json<TypeRef>,
) -> Result<StatusCode, AppError> {
    ds_type_add_module(&st, id, r.id, true).await
}

pub async fn ds_type_add_optional(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(r): Json<TypeRef>,
) -> Result<StatusCode, AppError> {
    ds_type_add_module(&st, id, r.id, false).await
}

pub async fn ds_type_remove_module(
    State(st): State<AppState>,
    Path((id, mid)): Path<(i64, i64)>,
) -> Result<StatusCode, AppError> {
    distribution_set_type::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set type"))?;
    if distribution_set::Entity::find()
        .filter(distribution_set::Column::TypeId.eq(id))
        .count(&st.db)
        .await?
        > 0
    {
        return Err(AppError::Conflict(
            "distribution set type is in use and cannot be modified".into(),
        ));
    }
    let row = ds_type_module::Entity::find_by_id((id, mid))
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("software module type"))?;
    ds_type_module::Entity::delete_by_id((row.ds_type_id, row.module_type_id))
        .exec(&st.db)
        .await?;
    Ok(StatusCode::OK)
}
