use crate::api::paging::{page, ListParams, Paged};
use crate::entity::{
    distribution_set, distribution_set_type, ds_type_module, software_module, software_module_type,
    target, target_type, target_type_ds_type,
};
use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use raptor_api_types::{
    DsTypeCreate, DsTypeUpdate, SmTypeCreate, SmTypeUpdate, TargetTypeCreate, TargetTypeUpdate,
    TypeRef,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};
use serde_json::{json, Value};

// --------------------------------------------------------------------------
// JSON rendering (hawkBit shapes)
// --------------------------------------------------------------------------

fn sm_type_json(t: &software_module_type::Model) -> Value {
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

fn ds_type_json(t: &distribution_set_type::Model) -> Value {
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

// --------------------------------------------------------------------------
// Software module types
// --------------------------------------------------------------------------

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

// --------------------------------------------------------------------------
// Distribution set types
// --------------------------------------------------------------------------

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

// --------------------------------------------------------------------------
// Target types
// --------------------------------------------------------------------------

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
    if let Some(name) = &u.name {
        if target_type::Entity::find()
            .filter(target_type::Column::Name.eq(name))
            .one(&st.db)
            .await?
            .is_some_and(|other| other.id != t.id)
        {
            return Err(AppError::Conflict(format!(
                "target type {name} already exists"
            )));
        }
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
