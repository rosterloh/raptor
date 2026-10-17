//! Software-module artifact (binary) CRUD and download
//! (`/rest/v1/softwaremodules/{id}/artifacts`). The upload/list route carries
//! its own body-size limit layer, sized from `Config::max_artifact_size`.

use crate::entity::artifact;
use crate::error::AppError;
use crate::state::AppState;
use crate::util::base_url;
use axum::Json;
use axum::Router;
use axum::body::Body;
use axum::extract::{Multipart, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};

pub fn routes(max_artifact_size: usize) -> Router<AppState> {
    Router::new()
        .route(
            "/rest/v1/softwaremodules/{id}/artifacts",
            axum::routing::post(upload)
                .get(list)
                .layer(axum::extract::DefaultBodyLimit::max(max_artifact_size)),
        )
        .route(
            "/rest/v1/softwaremodules/{id}/artifacts/{aid}",
            get(get_one).delete(delete),
        )
        .route(
            "/rest/v1/softwaremodules/{id}/artifacts/{aid}/download",
            get(download),
        )
}

pub fn artifact_rest(
    a: &artifact::Model,
    module_id: i64,
    base: &str,
) -> raptor_api_types::ArtifactRest {
    let self_href = format!(
        "{base}/rest/v1/softwaremodules/{module_id}/artifacts/{}",
        a.id
    );
    raptor_api_types::ArtifactRest {
        id: a.id,
        provided_filename: a.filename.clone(),
        size: a.size,
        hashes: raptor_api_types::ArtifactHashes {
            sha1: a.sha1.clone(),
            md5: a.md5.clone(),
            sha256: a.sha256.clone(),
        },
        links: serde_json::json!({
            "self": {"href": self_href},
            "download": {"href": format!("{self_href}/download")}
        }),
    }
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

pub async fn upload(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(module_id): Path<i64>,
    mut mp: Multipart,
) -> Result<(StatusCode, Json<raptor_api_types::ArtifactRest>), AppError> {
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
        let existing = artifact::Entity::find()
            .filter(artifact::Column::ModuleId.eq(module_id))
            .count(&st.db)
            .await?;
        crate::domain::quota::assert_quota(
            existing,
            1,
            st.cfg.quota.max_artifacts_per_software_module,
            "artifact",
            &format!("software module {module_id}"),
        )?;
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
        st.metrics.bytes_uploaded(a.size.max(0) as u64);
        return Ok((
            StatusCode::CREATED,
            Json(artifact_rest(&a, module_id, &base_url(&st.cfg, &headers))),
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
) -> Result<Json<Vec<raptor_api_types::ArtifactRest>>, AppError> {
    let base = base_url(&st.cfg, &headers);
    let rows = artifact::Entity::find()
        .filter(artifact::Column::ModuleId.eq(module_id))
        .all(&st.db)
        .await?;
    Ok(Json(
        rows.iter()
            .map(|a| artifact_rest(a, module_id, &base))
            .collect(),
    ))
}

pub async fn get_one(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path((module_id, artifact_id)): Path<(i64, i64)>,
) -> Result<Json<raptor_api_types::ArtifactRest>, AppError> {
    let a = find_owned(&st, module_id, artifact_id).await?;
    Ok(Json(artifact_rest(
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
    let stream = st.store.open(&a.sha256, None).await?;
    st.metrics.bytes_downloaded(a.size.max(0) as u64);
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
    crate::domain::artifact::gc_blob(&st, &a.sha256).await?;
    Ok(StatusCode::OK)
}
