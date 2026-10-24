//! DDI artifact listing and download, including HTTP byte-range support
//! (single and `multipart/byteranges`, see [`super::range`]) and
//! the `.MD5SUM` companion file. Actual blob storage lives in `storage`; this
//! module only resolves metadata and streams bytes.

use super::range::Resolved;
use crate::entity::{artifact, ds_module, target};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::base_url;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use axum::{Extension, Json};
use bytes::Bytes;
use futures::StreamExt;
use raptor_api_types::DownloadEvent;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::Value;

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
            // The response is already committed, so the device only sees a
            // short body; this is the one place the cause gets recorded.
            if let Err(e) = &chunk {
                tracing::warn!(controller_id = %cid, %filename, sent, total, error = %e, "artifact download aborted");
            }
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

    let parts = match headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        Some(h) => super::range::resolve(h, total),
        None => Resolved::Full,
    };
    match parts {
        Resolved::Full => {}
        Resolved::Unsatisfiable => {
            return Ok(Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{total}"))
                .body(Body::empty())
                .unwrap());
        }
        Resolved::Parts(parts) if parts.len() == 1 => {
            let r = parts[0].clone();
            let len = r.end - r.start;
            let content_range = format!("bytes {}-{}/{total}", r.start, r.end - 1);
            let stream = st
                .store
                .open(&a.sha256, Some(r.clone()))
                .await?
                .map(progress(r.start));
            st.metrics.bytes_downloaded(len);
            return Ok(Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::ACCEPT_RANGES, "bytes")
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .header(header::CONTENT_LENGTH, len)
                .header(header::CONTENT_RANGE, content_range)
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", a.filename),
                )
                .body(Body::from_stream(stream))
                .unwrap());
        }
        Resolved::Parts(parts) => {
            let boundary = crate::util::random_token();
            let len = multipart_len(&parts, total, &boundary);
            st.metrics
                .bytes_downloaded(parts.iter().map(|r| r.end - r.start).sum());
            let store = st.store.clone();
            let sha256 = a.sha256.clone();
            let tail = Bytes::from(format!("\r\n--{boundary}--\r\n"));
            let parts: Vec<_> = parts
                .into_iter()
                .enumerate()
                .map(|(i, r)| {
                    let head = Bytes::from(part_header(&r, total, &boundary, i == 0));
                    let progress = progress(r.start);
                    (r, head, progress)
                })
                .collect();
            let body = futures::stream::iter(parts)
                .then(move |(r, head, progress)| {
                    let (store, sha256) = (store.clone(), sha256.clone());
                    async move {
                        // Routed through `progress` so a failed open is logged too.
                        let data = match store.open(&sha256, Some(r)).await {
                            Ok(s) => s,
                            Err(e) => futures::stream::once(async { Err(e) }).boxed(),
                        }
                        .map(progress);
                        futures::stream::once(async { Ok(head) }).chain(data)
                    }
                })
                .flatten()
                .chain(futures::stream::once(async { Ok(tail) }));
            return Ok(Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::ACCEPT_RANGES, "bytes")
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/byteranges; boundary={boundary}"),
                )
                .header(header::CONTENT_LENGTH, len)
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", a.filename),
                )
                .body(Body::from_stream(body))
                .unwrap());
        }
    }

    let stream = st.store.open(&a.sha256, None).await?.map(progress(0));
    st.metrics.bytes_downloaded(a.size.max(0) as u64);
    Ok(Response::builder()
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, a.size)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", a.filename),
        )
        .body(Body::from_stream(stream))
        .unwrap())
}

/// Delimiter and headers that precede one part of a `multipart/byteranges`
/// body. The body starts at the first delimiter (no preamble), which is what
/// SWUpdate's multipart parser expects; later delimiters carry the CRLF that
/// ends the previous part's data.
fn part_header(r: &std::ops::Range<u64>, total: u64, boundary: &str, first: bool) -> String {
    let crlf = if first { "" } else { "\r\n" };
    format!(
        "{crlf}--{boundary}\r\nContent-Type: application/octet-stream\r\nContent-Range: bytes {}-{}/{total}\r\n\r\n",
        r.start,
        r.end - 1
    )
}

fn multipart_len(parts: &[std::ops::Range<u64>], total: u64, boundary: &str) -> u64 {
    let closing = format!("\r\n--{boundary}--\r\n").len() as u64;
    parts
        .iter()
        .enumerate()
        .map(|(i, r)| part_header(r, total, boundary, i == 0).len() as u64 + (r.end - r.start))
        .sum::<u64>()
        + closing
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
