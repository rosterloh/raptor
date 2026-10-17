//! One-shot copy of an existing on-disk store into a bucket, for switching a
//! deployment from `backend = "fs"` to `"s3"`.
//!
//! The database only records each artifact's hashes, never where its bytes
//! live, so moving the blobs is the whole migration. Keys are content
//! addresses: a blob already in the bucket is skipped, which makes a re-run
//! after an interruption cheap and safe. Local files are never modified or
//! deleted.

use super::S3Store;
use std::path::Path;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BackfillReport {
    /// Blobs uploaded by this run.
    pub uploaded: u64,
    /// Blobs the bucket already held.
    pub skipped: u64,
    /// Bytes uploaded by this run.
    pub bytes: u64,
}

/// Upload every blob under `root` (laid out `<sha256[0..2]>/<sha256>`) that
/// `s3` doesn't hold yet. Anything else in `root` — the `.tmp` upload spool,
/// stray files — is ignored. Stops at the first error; re-run to resume.
pub async fn backfill_fs_to_s3(root: &Path, s3: &S3Store) -> std::io::Result<BackfillReport> {
    let mut report = BackfillReport::default();
    let mut shards = tokio::fs::read_dir(root).await?;
    while let Some(shard) = shards.next_entry().await? {
        let shard_name = shard.file_name();
        let Some(shard_name) = shard_name.to_str() else {
            continue;
        };
        if !shard.file_type().await?.is_dir() || !is_hex(shard_name, 2) {
            continue;
        }
        let mut blobs = tokio::fs::read_dir(shard.path()).await?;
        while let Some(blob) = blobs.next_entry().await? {
            let name = blob.file_name();
            let Some(sha256) = name.to_str() else {
                continue;
            };
            if !blob.file_type().await?.is_file()
                || !is_hex(sha256, 64)
                || !sha256.starts_with(shard_name)
            {
                continue;
            }
            if s3.exists(sha256).await? {
                report.skipped += 1;
                continue;
            }
            let size = blob.metadata().await?.len();
            s3.upload_file(&blob.path(), sha256, size).await?;
            tracing::info!(sha256, size, "uploaded blob");
            report.uploaded += 1;
            report.bytes += size;
        }
    }
    Ok(report)
}

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
