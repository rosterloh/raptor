//! S3-compatible backend: blobs at `<prefix><sha256[0..2]>/<sha256>` in one
//! bucket.
//!
//! Hand-rolled on purpose: `rusty-s3` is sans-IO — it only builds presigned
//! request URLs (SigV4 query-string auth) and the XML bodies — and every
//! request is sent with a plain `reqwest` client, so each S3 call here is a
//! visible HTTP request with a status check, nothing more. Only the handful of
//! operations raptor needs exist: HEAD/GET/PUT/DELETE object, multipart upload
//! for large blobs, and HEAD bucket for the startup check.
//!
//! Targets self-hosted stores (RustFS, MinIO, Garage, Ceph RGW, TrueNAS) as
//! much as AWS: path-style addressing by default, any signing region, and an
//! optional extra CA for a private or self-signed endpoint certificate.

use super::{ByteStream, StoredArtifact, blob_rel_path};
use crate::config::S3Config;
use futures::StreamExt;
use reqwest::{StatusCode, header};
use rusty_s3::actions::{CreateMultipartUpload, S3Action};
use rusty_s3::{Bucket, Credentials, UrlStyle};
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;

/// Lifetime of each presigned URL. A request is signed immediately before it
/// is sent, so this only has to cover clock skew between raptor and the
/// store; validity is checked when a request starts, not while a long
/// download streams.
const PRESIGN_TTL: Duration = Duration::from_secs(300);

/// S3's floor for every multipart part except the last.
const MIN_PART_SIZE: u64 = 5 * 1024 * 1024;

/// S3's cap on parts per multipart upload (part numbers are 1..=10000).
const MAX_PARTS: u64 = 10_000;

#[derive(Clone)]
pub struct S3Store(Arc<Inner>);

struct Inner {
    bucket: Bucket,
    creds: Credentials,
    prefix: String,
    part_size: u64,
    http: reqwest::Client,
}

impl S3Store {
    pub fn new(cfg: &S3Config) -> std::io::Result<Self> {
        let endpoint: url::Url = cfg
            .endpoint
            .parse()
            .map_err(|e| invalid(format!("[storage.s3] endpoint {:?}: {e}", cfg.endpoint)))?;
        let style = if cfg.path_style {
            UrlStyle::Path
        } else {
            UrlStyle::VirtualHost
        };
        let bucket = Bucket::new(endpoint, style, cfg.bucket.clone(), cfg.region.clone())
            .map_err(|e| invalid(format!("[storage.s3] endpoint/bucket: {e:?}")))?;
        if cfg.part_size < MIN_PART_SIZE {
            return Err(invalid(format!(
                "[storage.s3] part_size must be at least {MIN_PART_SIZE} bytes (S3's minimum)"
            )));
        }
        let mut http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            // Per read, not per request: a stalled transfer fails, a long
            // healthy one doesn't.
            .read_timeout(Duration::from_secs(60));
        if let Some(path) = &cfg.ca_cert {
            let pem = std::fs::read(path).map_err(|e| {
                std::io::Error::new(e.kind(), format!("[storage.s3] ca_cert {path:?}: {e}"))
            })?;
            let certs = reqwest::Certificate::from_pem_bundle(&pem)
                .map_err(|e| invalid(format!("[storage.s3] ca_cert {path:?}: {e}")))?;
            http = http.tls_certs_merge(certs);
        }
        let http = http
            .build()
            .map_err(|e| invalid(format!("[storage.s3] http client: {e}")))?;
        Ok(Self(Arc::new(Inner {
            bucket,
            creds: Credentials::new(cfg.access_key.clone(), cfg.secret_key.clone()),
            prefix: cfg.prefix.clone(),
            part_size: cfg.part_size,
            http,
        })))
    }

    fn key(&self, sha256: &str) -> String {
        format!("{}{}", self.0.prefix, blob_rel_path(sha256))
    }

    /// HEAD the bucket: proves the endpoint, TLS, credentials and bucket name
    /// in one request.
    pub(super) async fn check(&self) -> std::io::Result<()> {
        let url = self
            .0
            .bucket
            .head_bucket(Some(&self.0.creds))
            .sign(PRESIGN_TTL);
        let resp = self.send(self.0.http.head(url), "HEAD bucket").await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(fail("HEAD bucket", self.0.bucket.name(), resp).await)
        }
    }

    pub async fn exists(&self, sha256: &str) -> std::io::Result<bool> {
        let key = self.key(sha256);
        let url = self
            .0
            .bucket
            .head_object(Some(&self.0.creds), &key)
            .sign(PRESIGN_TTL);
        let resp = self.send(self.0.http.head(url), "HEAD object").await?;
        match resp.status() {
            s if s.is_success() => Ok(true),
            StatusCode::NOT_FOUND => Ok(false),
            _ => Err(fail("HEAD object", &key, resp).await),
        }
    }

    pub(super) async fn open(
        &self,
        sha256: &str,
        range: Option<Range<u64>>,
    ) -> std::io::Result<ByteStream> {
        let key = self.key(sha256);
        let url = self
            .0
            .bucket
            .get_object(Some(&self.0.creds), &key)
            .sign(PRESIGN_TTL);
        let mut req = self.0.http.get(url);
        if let Some(r) = &range {
            // HTTP ranges are inclusive at both ends.
            req = req.header(header::RANGE, format!("bytes={}-{}", r.start, r.end - 1));
        }
        let resp = self.send(req, "GET object").await?;
        let expected = if range.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        };
        match resp.status() {
            s if s == expected => {}
            StatusCode::NOT_FOUND => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("s3 object {key} not found"),
                ));
            }
            // A store that ignored the Range header and answered 200 would
            // have us send the wrong bytes under a 206; refuse instead.
            _ => return Err(fail("GET object", &key, resp).await),
        }
        Ok(Box::pin(
            resp.bytes_stream()
                .map(|r| r.map_err(std::io::Error::other)),
        ))
    }

    pub(super) async fn remove(&self, sha256: &str) -> std::io::Result<()> {
        let key = self.key(sha256);
        let url = self
            .0
            .bucket
            .delete_object(Some(&self.0.creds), &key)
            .sign(PRESIGN_TTL);
        let resp = self.send(self.0.http.delete(url), "DELETE object").await?;
        // S3 answers 204 for a missing key too; 404 is accepted for stores
        // that don't.
        if resp.status().is_success() || resp.status() == StatusCode::NOT_FOUND {
            Ok(())
        } else {
            Err(fail("DELETE object", &key, resp).await)
        }
    }

    /// Upload a fully written, already hashed spool file under its sha256 key,
    /// unless the bucket already holds that blob.
    pub(super) async fn commit(&self, tmp: &Path, meta: &StoredArtifact) -> std::io::Result<()> {
        if self.exists(&meta.sha256).await? {
            return Ok(()); // dedup
        }
        self.upload_file(tmp, &meta.sha256, meta.size.max(0) as u64)
            .await
    }

    /// Upload a local file under `sha256`'s key: one PUT when it fits in a
    /// part, a multipart upload otherwise. Used by `commit` and by the
    /// fs → s3 backfill.
    pub async fn upload_file(&self, path: &Path, sha256: &str, size: u64) -> std::io::Result<()> {
        let key = self.key(sha256);
        if size <= self.0.part_size {
            let body = tokio::fs::read(path).await?;
            let url = self
                .0
                .bucket
                .put_object(Some(&self.0.creds), &key)
                .sign(PRESIGN_TTL);
            let resp = self
                .send(self.0.http.put(url).body(body), "PUT object")
                .await?;
            return if resp.status().is_success() {
                Ok(())
            } else {
                Err(fail("PUT object", &key, resp).await)
            };
        }
        self.multipart_upload(path, &key, size).await
    }

    /// Upload sequentially in `part_size` pieces (grown if needed to stay
    /// within S3's 10,000-part limit). Memory use is one part buffer. On any
    /// failure the upload is aborted so the store can discard its parts.
    async fn multipart_upload(&self, path: &Path, key: &str, size: u64) -> std::io::Result<()> {
        let url = self
            .0
            .bucket
            .create_multipart_upload(Some(&self.0.creds), key)
            .sign(PRESIGN_TTL);
        let resp = self
            .send(self.0.http.post(url), "CreateMultipartUpload")
            .await?;
        if !resp.status().is_success() {
            return Err(fail("CreateMultipartUpload", key, resp).await);
        }
        let body = resp.text().await.map_err(std::io::Error::other)?;
        let upload_id = CreateMultipartUpload::parse_response(&body)
            .map_err(|e| invalid(format!("CreateMultipartUpload response: {e}")))?
            .upload_id()
            .to_owned();

        let result = self.upload_parts(path, key, size, &upload_id).await;
        if result.is_err() {
            let url = self
                .0
                .bucket
                .abort_multipart_upload(Some(&self.0.creds), key, &upload_id)
                .sign(PRESIGN_TTL);
            if let Err(e) = self.0.http.delete(url).send().await {
                tracing::warn!(error = %e, key, "aborting multipart upload failed");
            }
        }
        result
    }

    async fn upload_parts(
        &self,
        path: &Path,
        key: &str,
        size: u64,
        upload_id: &str,
    ) -> std::io::Result<()> {
        let part_size = self.0.part_size.max(size.div_ceil(MAX_PARTS));
        let mut file = tokio::fs::File::open(path).await?;
        let mut etags: Vec<String> = Vec::new();
        let mut sent: u64 = 0;
        while sent < size {
            let len = part_size.min(size - sent) as usize;
            let mut buf = vec![0u8; len];
            file.read_exact(&mut buf).await?;
            let part_number = u16::try_from(etags.len() + 1).map_err(std::io::Error::other)?;
            let url = self
                .0
                .bucket
                .upload_part(Some(&self.0.creds), key, part_number, upload_id)
                .sign(PRESIGN_TTL);
            let resp = self
                .send(self.0.http.put(url).body(buf), "UploadPart")
                .await?;
            if !resp.status().is_success() {
                return Err(fail("UploadPart", key, resp).await);
            }
            let etag = resp
                .headers()
                .get(header::ETAG)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| invalid(format!("UploadPart {key}: response has no ETag")))?;
            etags.push(etag.to_owned());
            sent += len as u64;
        }

        let complete = self.0.bucket.complete_multipart_upload(
            Some(&self.0.creds),
            key,
            upload_id,
            etags.iter().map(String::as_str),
        );
        let url = complete.sign(PRESIGN_TTL);
        let resp = self
            .send(
                self.0.http.post(url).body(complete.body()),
                "CompleteMultipartUpload",
            )
            .await?;
        if !resp.status().is_success() {
            return Err(fail("CompleteMultipartUpload", key, resp).await);
        }
        // S3 can report a failed completion as 200 with an <Error> body.
        let body = resp.text().await.map_err(std::io::Error::other)?;
        if body.contains("<Error>") {
            return Err(std::io::Error::other(format!(
                "s3 CompleteMultipartUpload {key}: {}",
                truncate(&body)
            )));
        }
        Ok(())
    }

    async fn send(
        &self,
        req: reqwest::RequestBuilder,
        op: &str,
    ) -> std::io::Result<reqwest::Response> {
        req.send()
            .await
            .map_err(|e| std::io::Error::other(format!("s3 {op}: {e}")))
    }
}

/// An S3 error response as an `io::Error`, with the start of its XML body
/// (which carries S3's error code and message).
async fn fail(op: &str, what: &str, resp: reqwest::Response) -> std::io::Error {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    std::io::Error::other(format!(
        "s3 {op} {what}: HTTP {status}: {}",
        truncate(&body)
    ))
}

fn truncate(s: &str) -> &str {
    match s.char_indices().nth(512) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

fn invalid(msg: String) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, msg)
}
