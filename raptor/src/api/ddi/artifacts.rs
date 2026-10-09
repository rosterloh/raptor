//! DDI artifact listing and download, including HTTP byte-range support and
//! the `.MD5SUM` companion file. Actual blob storage lives in `storage`; this
//! module only resolves metadata and streams bytes.

use crate::entity::{artifact, ds_module, target};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::base_url;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use axum::{Extension, Json};
use futures::StreamExt;
use raptor_api_types::DownloadEvent;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

pub async fn list(
    State(st): State<AppState>,
    Extension(_auth): Extension<crate::auth::ddi::AuthKind>,
    headers: HeaderMap,
    Path((_tenant, cid, module_id)): Path<(String, String, i64)>,
) -> Result<Json<Vec<Value>>, AppError> {
    let base = base_url(&st.cfg, &headers);
    let ddi = super::ddi_base(&base, &st.cfg.tenant, &cid);
    let https = base.starts_with("https://");
    let http_ddi = super::ddi_http_base(&st.cfg, &cid);
    let rows = artifact::Entity::find()
        .filter(artifact::Column::ModuleId.eq(module_id))
        .all(&st.db)
        .await?;
    Ok(Json(
        rows.iter()
            .map(|ar| {
                super::deployment::ddi_artifact_json(
                    ar,
                    &ddi,
                    module_id,
                    https,
                    http_ddi.as_deref(),
                )
            })
            .collect(),
    ))
}

/// Parse "bytes=a-b" / "bytes=a-" into (start, inclusive_end).
fn parse_range(h: &str, total: i64) -> Option<(i64, i64)> {
    let spec = h.strip_prefix("bytes=")?;
    let (start, end) = spec.split_once('-')?;
    let start: i64 = start.parse().ok()?;
    let end: i64 = if end.is_empty() {
        total - 1
    } else {
        end.parse().ok()?
    };
    (start <= end && start < total).then_some((start, end.min(total - 1)))
}

pub async fn download(
    State(st): State<AppState>,
    Extension(_auth): Extension<crate::auth::ddi::AuthKind>,
    headers: HeaderMap,
    Path((_tenant, cid, module_id, filename)): Path<(String, String, i64, String)>,
) -> Result<Response, AppError> {
    // .MD5SUM companion file
    if let Some(real) = filename.strip_suffix(".MD5SUM") {
        let a = find(&st, module_id, real).await?;
        return Ok(Response::builder()
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from(format!("{}  {}\n", a.md5, a.filename)))
            .unwrap());
    }

    let a = find(&st, module_id, &filename).await?;
    let path = st.store.path_for(&a.sha256);
    let owner = download_owner(&st, &cid, module_id)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = ?e, "download progress lookup failed");
            None
        });
    let total = a.size.max(0) as u64;
    let progress = |start: u64| {
        let (st, cid, filename) = (st.clone(), cid.clone(), a.filename.clone());
        let mut sent = start;
        move |chunk: std::io::Result<bytes::Bytes>| {
            if let (Some(action_id), Ok(b)) = (owner, &chunk) {
                sent += b.len() as u64;
                st.events.record_download(DownloadEvent {
                    controller_id: cid.clone(),
                    action_id,
                    filename: filename.clone(),
                    sent,
                    total,
                });
            }
            chunk
        }
    };
    // An empty body may yield no chunks, so report completion up front.
    if let (Some(action_id), 0) = (owner, total) {
        st.events.record_download(DownloadEvent {
            controller_id: cid.clone(),
            action_id,
            filename: a.filename.clone(),
            sent: 0,
            total: 0,
        });
    }

    if let Some(range) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        let Some((start, end)) = parse_range(range, a.size) else {
            return Ok(Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{}", a.size))
                .body(Body::empty())
                .unwrap());
        };
        let mut file = tokio::fs::File::open(&path).await?;
        file.seek(std::io::SeekFrom::Start(start as u64)).await?;
        let len = end - start + 1;
        st.metrics.bytes_downloaded(len.max(0) as u64);
        let stream = tokio_util::io::ReaderStream::with_capacity(file.take(len as u64), 64 * 1024)
            .map(progress(start as u64));
        return Ok(Response::builder()
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header(header::CONTENT_LENGTH, len)
            .header(
                header::CONTENT_RANGE,
                format!("bytes {start}-{end}/{}", a.size),
            )
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", a.filename),
            )
            .body(Body::from_stream(stream))
            .unwrap());
    }

    let file = tokio::fs::File::open(&path).await?;
    st.metrics.bytes_downloaded(a.size.max(0) as u64);
    Ok(Response::builder()
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, a.size)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", a.filename),
        )
        .body(Body::from_stream(
            tokio_util::io::ReaderStream::with_capacity(file, 64 * 1024).map(progress(0)),
        ))
        .unwrap())
}

async fn find(st: &AppState, module_id: i64, filename: &str) -> Result<artifact::Model, AppError> {
    artifact::Entity::find()
        .filter(artifact::Column::ModuleId.eq(module_id))
        .filter(artifact::Column::Filename.eq(filename))
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("artifact"))
}

/// Active action id iff `cid` has an active action whose DS contains `module_id`.
async fn download_owner(st: &AppState, cid: &str, module_id: i64) -> Result<Option<i64>, AppError> {
    let Some(t) = target::Entity::find()
        .filter(target::Column::ControllerId.eq(cid))
        .one(&st.db)
        .await?
    else {
        return Ok(None);
    };
    let Some(a) = crate::domain::deployment::active_action(&st.db, t.id).await? else {
        return Ok(None);
    };
    let linked = ds_module::Entity::find()
        .filter(ds_module::Column::DsId.eq(a.ds_id))
        .filter(ds_module::Column::ModuleId.eq(module_id))
        .one(&st.db)
        .await?;
    Ok(linked.map(|_| a.id))
}
