//! `ArtifactStore`: content-addressed (sha256) blob storage for uploaded
//! artifacts, on local disk (`fs`) or in an S3-compatible bucket (`s3`).
//!
//! Uploads always stream to a local spool file under `<artifact_dir>/.tmp`
//! while sha1/md5/sha256 are computed (hawkBit-compatible metadata); only then
//! is the blob's key known, so it is committed to the backend afterwards —
//! renamed into place on disk, or PUT to the bucket. An existing blob with the
//! same hash is kept rather than overwritten (dedup).
//!
//! Callers only ever address a blob by its sha256 and get bytes back as a
//! stream (`open`); nothing outside this module sees a path or an object key.
//!
//! The backend is a closed enum rather than a trait object: there are exactly
//! two, both live in this module, and a `match` keeps every call site
//! readable without `async_trait` or boxing.

mod backfill;
mod fs;
mod s3;

use crate::config::{Config, StorageBackend};
use crate::error::AppError;
use futures::StreamExt;
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tokio::io::AsyncWriteExt;

pub use backfill::{BackfillReport, backfill_fs_to_s3};
pub use s3::S3Store;

/// A blob's bytes, as handed to an HTTP response body.
pub type ByteStream = Pin<Box<dyn futures::Stream<Item = std::io::Result<bytes::Bytes>> + Send>>;

#[derive(Clone)]
pub struct ArtifactStore {
    /// Directory upload spool files are written to (`<artifact_dir>/.tmp`).
    spool: PathBuf,
    backend: Backend,
}

#[derive(Clone)]
enum Backend {
    Fs(fs::FsStore),
    S3(s3::S3Store),
}

#[derive(Debug, Clone)]
pub struct StoredArtifact {
    pub size: i64,
    pub sha1: String,
    pub md5: String,
    pub sha256: String,
}

/// `<sha256[0..2]>/<sha256>`: the git-object-store layout shared by both
/// backends, so a blob's relative path on disk is its key in the bucket.
pub(crate) fn blob_rel_path(sha256: &str) -> String {
    format!("{}/{sha256}", &sha256[..2])
}

impl ArtifactStore {
    /// Local-disk store rooted at `root`.
    pub fn new(root: PathBuf) -> std::io::Result<Self> {
        let spool = spool_dir(&root)?;
        Ok(Self {
            spool,
            backend: Backend::Fs(fs::FsStore::new(root)),
        })
    }

    /// S3-backed store; `spool_root` (the `artifact_dir`) only holds uploads
    /// in flight.
    pub fn with_s3(spool_root: &Path, s3: S3Store) -> std::io::Result<Self> {
        Ok(Self {
            spool: spool_dir(spool_root)?,
            backend: Backend::S3(s3),
        })
    }

    /// Build the store `[storage]` asks for.
    pub fn from_config(cfg: &Config) -> std::io::Result<Self> {
        match cfg.storage.backend {
            StorageBackend::Fs => Self::new(cfg.artifact_dir.clone()),
            StorageBackend::S3 => {
                let s3cfg = cfg.storage.s3.as_ref().ok_or_else(|| {
                    std::io::Error::other("[storage] backend = \"s3\" needs a [storage.s3] section")
                })?;
                Self::with_s3(&cfg.artifact_dir, S3Store::new(s3cfg)?)
            }
        }
    }

    /// Short name for logs: `fs` or `s3`.
    pub fn backend_name(&self) -> &'static str {
        match &self.backend {
            Backend::Fs(_) => "fs",
            Backend::S3(_) => "s3",
        }
    }

    /// Fail fast at startup if the backend is unreachable or misconfigured
    /// (bad credentials, missing bucket), rather than on the first upload.
    pub async fn check(&self) -> std::io::Result<()> {
        match &self.backend {
            Backend::Fs(_) => Ok(()),
            Backend::S3(s) => s.check().await,
        }
    }

    pub async fn store_bytes_stream<S, E>(&self, stream: S) -> Result<StoredArtifact, AppError>
    where
        S: futures::Stream<Item = Result<bytes::Bytes, E>> + Unpin,
        E: std::fmt::Display,
    {
        let tmp = self.spool.join(crate::util::random_token());
        let meta = match spool_and_hash(&tmp, stream).await {
            Ok(meta) => meta,
            Err(e) => {
                let _ = tokio::fs::remove_file(&tmp).await;
                return Err(e);
            }
        };
        let committed = match &self.backend {
            Backend::Fs(f) => f.commit(&tmp, &meta.sha256).await,
            Backend::S3(s) => s.commit(&tmp, &meta).await,
        };
        // fs renamed it away on success; anything left is ours to clean up.
        let _ = tokio::fs::remove_file(&tmp).await;
        committed?;
        Ok(meta)
    }

    /// Stream a blob's bytes, or only `range` of them (half-open, already
    /// clamped to the blob's size by the caller).
    pub async fn open(
        &self,
        sha256: &str,
        range: Option<Range<u64>>,
    ) -> std::io::Result<ByteStream> {
        match &self.backend {
            Backend::Fs(f) => f.open(sha256, range).await,
            Backend::S3(s) => s.open(sha256, range).await,
        }
    }

    pub async fn exists(&self, sha256: &str) -> std::io::Result<bool> {
        match &self.backend {
            Backend::Fs(f) => f.exists(sha256).await,
            Backend::S3(s) => s.exists(sha256).await,
        }
    }

    /// Delete a blob; a blob that is already gone is not an error.
    pub async fn remove(&self, sha256: &str) -> std::io::Result<()> {
        match &self.backend {
            Backend::Fs(f) => f.remove(sha256).await,
            Backend::S3(s) => s.remove(sha256).await,
        }
    }
}

fn spool_dir(root: &Path) -> std::io::Result<PathBuf> {
    let spool = root.join(".tmp");
    std::fs::create_dir_all(&spool)?;
    Ok(spool)
}

/// Write `stream` to `tmp`, hashing as it goes.
async fn spool_and_hash<S, E>(tmp: &Path, mut stream: S) -> Result<StoredArtifact, AppError>
where
    S: futures::Stream<Item = Result<bytes::Bytes, E>> + Unpin,
    E: std::fmt::Display,
{
    let mut file = tokio::fs::File::create(tmp).await?;
    let (mut h1, mut h5, mut h256) = (Sha1::new(), Md5::new(), Sha256::new());
    let mut size: i64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::BadRequest(format!("upload stream: {e}")))?;
        h1.update(&chunk);
        h5.update(&chunk);
        h256.update(&chunk);
        size += chunk.len() as i64;
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    Ok(StoredArtifact {
        size,
        sha1: hex::encode(h1.finalize()),
        md5: hex::encode(h5.finalize()),
        sha256: hex::encode(h256.finalize()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream_of(
        chunks: Vec<&'static [u8]>,
    ) -> impl futures::Stream<Item = Result<bytes::Bytes, std::convert::Infallible>> + Unpin {
        futures::stream::iter(chunks.into_iter().map(|c| Ok(bytes::Bytes::from_static(c))))
    }

    pub(super) async fn read_all(mut s: ByteStream) -> Vec<u8> {
        let mut out = Vec::new();
        while let Some(chunk) = s.next().await {
            out.extend_from_slice(&chunk.unwrap());
        }
        out
    }

    #[tokio::test]
    async fn stores_and_hashes_content() {
        let dir = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(dir.path().to_path_buf()).unwrap();
        // b"hello world" split across chunks
        let meta = store
            .store_bytes_stream(stream_of(vec![b"hello ", b"world"]))
            .await
            .unwrap();
        assert_eq!(meta.size, 11);
        assert_eq!(meta.md5, "5eb63bbbe01eeed093cb22bb8f5acdc3");
        assert_eq!(meta.sha1, "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed");
        assert_eq!(
            meta.sha256,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
        let path = dir.path().join(blob_rel_path(&meta.sha256));
        assert_eq!(std::fs::read(&path).unwrap(), b"hello world");
        assert_eq!(
            read_all(store.open(&meta.sha256, None).await.unwrap()).await,
            b"hello world"
        );
        assert_eq!(
            read_all(store.open(&meta.sha256, Some(3..8)).await.unwrap()).await,
            b"lo wo"
        );
    }

    #[tokio::test]
    async fn dedups_identical_content() {
        let dir = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(dir.path().to_path_buf()).unwrap();
        let a = store
            .store_bytes_stream(stream_of(vec![b"hello world"]))
            .await
            .unwrap();
        let b = store
            .store_bytes_stream(stream_of(vec![b"hello world"]))
            .await
            .unwrap();
        assert_eq!(a.sha256, b.sha256);
        assert!(store.exists(&a.sha256).await.unwrap());
        store.remove(&a.sha256).await.unwrap();
        assert!(!store.exists(&a.sha256).await.unwrap());
        // removing an absent blob is idempotent
        store.remove(&a.sha256).await.unwrap();
    }

    #[tokio::test]
    async fn cleans_up_temp_file_on_stream_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(dir.path().to_path_buf()).unwrap();
        let stream = futures::stream::iter(vec![
            Ok(bytes::Bytes::from_static(b"x")),
            Err(std::io::Error::other("boom")),
        ]);
        let result = store.store_bytes_stream(stream).await;
        assert!(result.is_err());
        let tmp_count = std::fs::read_dir(dir.path().join(".tmp")).unwrap().count();
        assert_eq!(tmp_count, 0);
    }
}
