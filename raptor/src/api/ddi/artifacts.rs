use crate::entity::artifact;
use crate::error::AppError;
use crate::state::AppState;
use crate::util::base_url;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::{Extension, Json};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

pub async fn list(
    State(st): State<AppState>,
    Extension(_auth): Extension<crate::auth::ddi::AuthKind>,
    headers: HeaderMap,
    Path((_tenant, cid, module_id)): Path<(String, String, i64)>,
) -> Result<Json<Vec<Value>>, AppError> {
    let ddi = super::ddi_base(&base_url(&st.cfg, &headers), &cid);
    let rows = artifact::Entity::find().filter(artifact::Column::ModuleId.eq(module_id)).all(&st.db).await?;
    Ok(Json(rows.iter().map(|ar| {
        let dl = format!("{ddi}/softwaremodules/{module_id}/artifacts/{}", ar.filename);
        json!({
            "filename": ar.filename,
            "hashes": {"sha1": ar.sha1, "md5": ar.md5, "sha256": ar.sha256},
            "size": ar.size,
            "_links": {"download-http": {"href": dl}, "md5sum-http": {"href": format!("{dl}.MD5SUM")}}
        })
    }).collect()))
}

/// Parse "bytes=a-b" / "bytes=a-" into (start, inclusive_end).
fn parse_range(h: &str, total: i64) -> Option<(i64, i64)> {
    let spec = h.strip_prefix("bytes=")?;
    let (start, end) = spec.split_once('-')?;
    let start: i64 = start.parse().ok()?;
    let end: i64 = if end.is_empty() { total - 1 } else { end.parse().ok()? };
    (start <= end && start < total).then_some((start, end.min(total - 1)))
}

pub async fn download(
    State(st): State<AppState>,
    Extension(_auth): Extension<crate::auth::ddi::AuthKind>,
    headers: HeaderMap,
    Path((_tenant, _cid, module_id, filename)): Path<(String, String, i64, String)>,
) -> Result<Response, AppError> {
    // .MD5SUM companion file
    if let Some(real) = filename.strip_suffix(".MD5SUM") {
        let a = find(&st, module_id, real).await?;
        return Ok(Response::builder()
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from(format!("{}  {}\n", a.md5, a.filename))).unwrap());
    }

    let a = find(&st, module_id, &filename).await?;
    let path = st.store.path_for(&a.sha256);

    if let Some(range) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        let Some((start, end)) = parse_range(range, a.size) else {
            return Ok(Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{}", a.size))
                .body(Body::empty()).unwrap());
        };
        let mut file = tokio::fs::File::open(&path).await?;
        file.seek(std::io::SeekFrom::Start(start as u64)).await?;
        let len = end - start + 1;
        let stream = tokio_util::io::ReaderStream::new(file.take(len as u64));
        return Ok(Response::builder()
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header(header::CONTENT_LENGTH, len)
            .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{}", a.size))
            .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", a.filename))
            .body(Body::from_stream(stream)).unwrap());
    }

    let file = tokio::fs::File::open(&path).await?;
    Ok(Response::builder()
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, a.size)
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", a.filename))
        .body(Body::from_stream(tokio_util::io::ReaderStream::new(file))).unwrap())
}

async fn find(st: &AppState, module_id: i64, filename: &str) -> Result<artifact::Model, AppError> {
    artifact::Entity::find()
        .filter(artifact::Column::ModuleId.eq(module_id))
        .filter(artifact::Column::Filename.eq(filename))
        .one(&st.db).await?
        .ok_or(AppError::NotFound("artifact"))
}
