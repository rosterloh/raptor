//! Key/value metadata CRUD for targets, software modules and distribution sets
//! (hawkBit `.../metadata` sub-resources). Software-module entries additionally
//! carry a `targetVisible` flag which surfaces them to devices in DDI
//! `deploymentBase` chunks (see `api::ddi::deployment`).

use super::targets::find_by_cid;
use crate::api::paging::{page, ListParams, Paged};
use crate::entity::{distribution_set, ds_metadata, sm_metadata, software_module, target_metadata};
use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use raptor_api_types::{MetadataCreate, MetadataRest, MetadataUpdate};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, ModelTrait, QueryFilter,
    QueryOrder,
};
use std::collections::HashSet;

fn target_meta_rest(m: &target_metadata::Model) -> MetadataRest {
    MetadataRest {
        key: m.key.clone(),
        value: m.value.clone(),
        target_visible: None,
    }
}

fn ds_meta_rest(m: &ds_metadata::Model) -> MetadataRest {
    MetadataRest {
        key: m.key.clone(),
        value: m.value.clone(),
        target_visible: None,
    }
}

fn sm_meta_rest(m: &sm_metadata::Model) -> MetadataRest {
    MetadataRest {
        key: m.key.clone(),
        value: m.value.clone(),
        target_visible: Some(m.target_visible),
    }
}

/// Reject keys duplicated inside a single POST array before any write.
fn check_request_dups(body: &[MetadataCreate]) -> Result<(), AppError> {
    let mut seen = HashSet::new();
    for c in body {
        if !seen.insert(c.key.as_str()) {
            return Err(AppError::Conflict(format!(
                "duplicate metadata key {} in request",
                c.key
            )));
        }
    }
    Ok(())
}

// ---- targets ----

pub async fn target_list(
    State(st): State<AppState>,
    Path(cid): Path<String>,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<MetadataRest>>, AppError> {
    let t = find_by_cid(&st.db, &cid).await?;
    let sel = target_metadata::Entity::find()
        .filter(target_metadata::Column::TargetId.eq(t.id))
        .order_by_asc(target_metadata::Column::Key);
    let (rows, total) = page(&st.db, sel, &p).await?;
    Ok(Json(Paged::new(
        rows.iter().map(target_meta_rest).collect(),
        total,
    )))
}

pub async fn target_create(
    State(st): State<AppState>,
    Path(cid): Path<String>,
    Json(body): Json<Vec<MetadataCreate>>,
) -> Result<(StatusCode, Json<Vec<MetadataRest>>), AppError> {
    let t = find_by_cid(&st.db, &cid).await?;
    check_request_dups(&body)?;
    for c in &body {
        let dup = target_metadata::Entity::find()
            .filter(target_metadata::Column::TargetId.eq(t.id))
            .filter(target_metadata::Column::Key.eq(&c.key))
            .one(&st.db)
            .await?;
        if dup.is_some() {
            return Err(AppError::Conflict(format!(
                "metadata {} already exists",
                c.key
            )));
        }
    }
    let mut out = Vec::with_capacity(body.len());
    for c in body {
        let m = target_metadata::ActiveModel {
            target_id: Set(t.id),
            key: Set(c.key),
            value: Set(c.value),
            ..Default::default()
        }
        .insert(&st.db)
        .await?;
        out.push(target_meta_rest(&m));
    }
    Ok((StatusCode::CREATED, Json(out)))
}

async fn target_find(
    st: &AppState,
    cid: &str,
    key: &str,
) -> Result<target_metadata::Model, AppError> {
    let t = find_by_cid(&st.db, cid).await?;
    target_metadata::Entity::find()
        .filter(target_metadata::Column::TargetId.eq(t.id))
        .filter(target_metadata::Column::Key.eq(key))
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("metadata"))
}

pub async fn target_get(
    State(st): State<AppState>,
    Path((cid, key)): Path<(String, String)>,
) -> Result<Json<MetadataRest>, AppError> {
    Ok(Json(target_meta_rest(&target_find(&st, &cid, &key).await?)))
}

pub async fn target_update(
    State(st): State<AppState>,
    Path((cid, key)): Path<(String, String)>,
    Json(u): Json<MetadataUpdate>,
) -> Result<Json<MetadataRest>, AppError> {
    let m = target_find(&st, &cid, &key).await?;
    let mut am: target_metadata::ActiveModel = m.into();
    am.value = Set(u.value);
    let m = am.update(&st.db).await?;
    Ok(Json(target_meta_rest(&m)))
}

pub async fn target_delete(
    State(st): State<AppState>,
    Path((cid, key)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let m = target_find(&st, &cid, &key).await?;
    m.delete(&st.db).await?;
    Ok(StatusCode::OK)
}

// ---- distribution sets ----

async fn ds_require(st: &AppState, id: i64) -> Result<(), AppError> {
    distribution_set::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set"))?;
    Ok(())
}

pub async fn ds_list(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<MetadataRest>>, AppError> {
    ds_require(&st, id).await?;
    let sel = ds_metadata::Entity::find()
        .filter(ds_metadata::Column::DsId.eq(id))
        .order_by_asc(ds_metadata::Column::Key);
    let (rows, total) = page(&st.db, sel, &p).await?;
    Ok(Json(Paged::new(
        rows.iter().map(ds_meta_rest).collect(),
        total,
    )))
}

pub async fn ds_create(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Vec<MetadataCreate>>,
) -> Result<(StatusCode, Json<Vec<MetadataRest>>), AppError> {
    ds_require(&st, id).await?;
    check_request_dups(&body)?;
    for c in &body {
        let dup = ds_metadata::Entity::find()
            .filter(ds_metadata::Column::DsId.eq(id))
            .filter(ds_metadata::Column::Key.eq(&c.key))
            .one(&st.db)
            .await?;
        if dup.is_some() {
            return Err(AppError::Conflict(format!(
                "metadata {} already exists",
                c.key
            )));
        }
    }
    let mut out = Vec::with_capacity(body.len());
    for c in body {
        let m = ds_metadata::ActiveModel {
            ds_id: Set(id),
            key: Set(c.key),
            value: Set(c.value),
            ..Default::default()
        }
        .insert(&st.db)
        .await?;
        out.push(ds_meta_rest(&m));
    }
    Ok((StatusCode::CREATED, Json(out)))
}

async fn ds_find(st: &AppState, id: i64, key: &str) -> Result<ds_metadata::Model, AppError> {
    ds_require(st, id).await?;
    ds_metadata::Entity::find()
        .filter(ds_metadata::Column::DsId.eq(id))
        .filter(ds_metadata::Column::Key.eq(key))
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("metadata"))
}

pub async fn ds_get(
    State(st): State<AppState>,
    Path((id, key)): Path<(i64, String)>,
) -> Result<Json<MetadataRest>, AppError> {
    Ok(Json(ds_meta_rest(&ds_find(&st, id, &key).await?)))
}

pub async fn ds_update(
    State(st): State<AppState>,
    Path((id, key)): Path<(i64, String)>,
    Json(u): Json<MetadataUpdate>,
) -> Result<Json<MetadataRest>, AppError> {
    let m = ds_find(&st, id, &key).await?;
    let mut am: ds_metadata::ActiveModel = m.into();
    am.value = Set(u.value);
    let m = am.update(&st.db).await?;
    Ok(Json(ds_meta_rest(&m)))
}

pub async fn ds_delete(
    State(st): State<AppState>,
    Path((id, key)): Path<(i64, String)>,
) -> Result<StatusCode, AppError> {
    let m = ds_find(&st, id, &key).await?;
    m.delete(&st.db).await?;
    Ok(StatusCode::OK)
}

// ---- software modules ----

async fn sm_require(st: &AppState, id: i64) -> Result<(), AppError> {
    software_module::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("software module"))?;
    Ok(())
}

pub async fn sm_list(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<MetadataRest>>, AppError> {
    sm_require(&st, id).await?;
    let sel = sm_metadata::Entity::find()
        .filter(sm_metadata::Column::ModuleId.eq(id))
        .order_by_asc(sm_metadata::Column::Key);
    let (rows, total) = page(&st.db, sel, &p).await?;
    Ok(Json(Paged::new(
        rows.iter().map(sm_meta_rest).collect(),
        total,
    )))
}

pub async fn sm_create(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<Vec<MetadataCreate>>,
) -> Result<(StatusCode, Json<Vec<MetadataRest>>), AppError> {
    sm_require(&st, id).await?;
    check_request_dups(&body)?;
    for c in &body {
        let dup = sm_metadata::Entity::find()
            .filter(sm_metadata::Column::ModuleId.eq(id))
            .filter(sm_metadata::Column::Key.eq(&c.key))
            .one(&st.db)
            .await?;
        if dup.is_some() {
            return Err(AppError::Conflict(format!(
                "metadata {} already exists",
                c.key
            )));
        }
    }
    let mut out = Vec::with_capacity(body.len());
    for c in body {
        let m = sm_metadata::ActiveModel {
            module_id: Set(id),
            key: Set(c.key),
            value: Set(c.value),
            target_visible: Set(c.target_visible),
            ..Default::default()
        }
        .insert(&st.db)
        .await?;
        out.push(sm_meta_rest(&m));
    }
    Ok((StatusCode::CREATED, Json(out)))
}

async fn sm_find(st: &AppState, id: i64, key: &str) -> Result<sm_metadata::Model, AppError> {
    sm_require(st, id).await?;
    sm_metadata::Entity::find()
        .filter(sm_metadata::Column::ModuleId.eq(id))
        .filter(sm_metadata::Column::Key.eq(key))
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("metadata"))
}

pub async fn sm_get(
    State(st): State<AppState>,
    Path((id, key)): Path<(i64, String)>,
) -> Result<Json<MetadataRest>, AppError> {
    Ok(Json(sm_meta_rest(&sm_find(&st, id, &key).await?)))
}

pub async fn sm_update(
    State(st): State<AppState>,
    Path((id, key)): Path<(i64, String)>,
    Json(u): Json<MetadataUpdate>,
) -> Result<Json<MetadataRest>, AppError> {
    let m = sm_find(&st, id, &key).await?;
    let mut am: sm_metadata::ActiveModel = m.into();
    am.value = Set(u.value);
    if let Some(v) = u.target_visible {
        am.target_visible = Set(v);
    }
    let m = am.update(&st.db).await?;
    Ok(Json(sm_meta_rest(&m)))
}

pub async fn sm_delete(
    State(st): State<AppState>,
    Path((id, key)): Path<(i64, String)>,
) -> Result<StatusCode, AppError> {
    let m = sm_find(&st, id, &key).await?;
    m.delete(&st.db).await?;
    Ok(StatusCode::OK)
}
