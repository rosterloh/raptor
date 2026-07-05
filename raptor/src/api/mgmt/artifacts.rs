use crate::entity::artifact;
use crate::error::AppError;
use crate::state::AppState;
use crate::util::base_url;
use axum::body::Body;
use axum::extract::{Multipart, Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::Json;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};
use serde_json::{json, Value};

pub fn artifact_json(a: &artifact::Model, module_id: i64, base: &str) -> Value {
    let self_href = format!(
        "{base}/rest/v1/softwaremodules/{module_id}/artifacts/{}",
        a.id
    );
    json!({
        "id": a.id,
        "providedFilename": a.filename,
        "size": a.size,
        "hashes": {"sha1": a.sha1, "md5": a.md5, "sha256": a.sha256},
        "_links": {"self": {"href": self_href}, "download": {"href": format!("{self_href}/download")}}
    })
}

async fn find_owned(
    st: &AppState,
    module_id: i64,
    artifact_id: i64,
) -> Result<artifact::Model, AppError> {
    artifact::Entity::find_by_id(artifact_id)
        .one(&st.db)
        .await?
        .filter(|a| a.module_id == module_id)
        .ok_or(AppError::NotFound("artifact"))
}

/// Remove a blob from disk if no artifact rows reference its sha256 anymore.
async fn gc_blob(st: &AppState, sha256: &str) -> Result<(), AppError> {
    let refs = artifact::Entity::find()
        .filter(artifact::Column::Sha256.eq(sha256))
        .count(&st.db)
        .await?;
    if refs == 0 {
        st.store.remove(sha256)?;
    }
    Ok(())
}

pub async fn delete_module_artifacts(st: &AppState, module_id: i64) -> Result<(), AppError> {
    let rows = artifact::Entity::find()
        .filter(artifact::Column::ModuleId.eq(module_id))
        .all(&st.db)
        .await?;
    for a in rows {
        artifact::Entity::delete_by_id(a.id).exec(&st.db).await?;
        gc_blob(st, &a.sha256).await?;
    }
    Ok(())
}

pub async fn upload(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(module_id): Path<i64>,
    mut mp: Multipart,
) -> Result<(StatusCode, Json<Value>), AppError> {
    crate::entity::software_module::Entity::find_by_id(module_id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("software module"))?;
    while let Some(field) = mp
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let filename = field.file_name().unwrap_or("artifact").to_string();
        let dup = artifact::Entity::find()
            .filter(artifact::Column::ModuleId.eq(module_id))
            .filter(artifact::Column::Filename.eq(&filename))
            .one(&st.db)
            .await?;
        if dup.is_some() {
            return Err(AppError::Conflict(format!(
                "artifact {filename} already exists on module {module_id}"
            )));
        }
        let meta = st
            .store
            .store_bytes_stream(Box::pin(futures::stream::unfold(
                field,
                |mut f| async move { f.chunk().await.transpose().map(|c| (c, f)) },
            )))
            .await?;
        let a = artifact::ActiveModel {
            module_id: Set(module_id),
            filename: Set(filename),
            size: Set(meta.size),
            sha1: Set(meta.sha1),
            md5: Set(meta.md5),
            sha256: Set(meta.sha256),
            ..Default::default()
        }
        .insert(&st.db)
        .await?;
        return Ok((
            StatusCode::CREATED,
            Json(artifact_json(&a, module_id, &base_url(&st.cfg, &headers))),
        ));
    }
    Err(AppError::BadRequest(
        "multipart field 'file' missing".into(),
    ))
}

pub async fn list(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(module_id): Path<i64>,
) -> Result<Json<Vec<Value>>, AppError> {
    let base = base_url(&st.cfg, &headers);
    let rows = artifact::Entity::find()
        .filter(artifact::Column::ModuleId.eq(module_id))
        .all(&st.db)
        .await?;
    Ok(Json(
        rows.iter()
            .map(|a| artifact_json(a, module_id, &base))
            .collect(),
    ))
}

pub async fn get_one(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path((module_id, artifact_id)): Path<(i64, i64)>,
) -> Result<Json<Value>, AppError> {
    let a = find_owned(&st, module_id, artifact_id).await?;
    Ok(Json(artifact_json(
        &a,
        module_id,
        &base_url(&st.cfg, &headers),
    )))
}

pub async fn download(
    State(st): State<AppState>,
    Path((module_id, artifact_id)): Path<(i64, i64)>,
) -> Result<Response, AppError> {
    let a = find_owned(&st, module_id, artifact_id).await?;
    let file = tokio::fs::File::open(st.store.path_for(&a.sha256)).await?;
    let stream = tokio_util::io::ReaderStream::new(file);
    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, a.size)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", a.filename),
        )
        .body(Body::from_stream(stream))
        .unwrap())
}

pub async fn delete(
    State(st): State<AppState>,
    Path((module_id, artifact_id)): Path<(i64, i64)>,
) -> Result<StatusCode, AppError> {
    let a = find_owned(&st, module_id, artifact_id).await?;
    artifact::Entity::delete_by_id(a.id).exec(&st.db).await?;
    gc_blob(&st, &a.sha256).await?;
    Ok(StatusCode::OK)
}
