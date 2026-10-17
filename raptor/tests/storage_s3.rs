//! S3 backend against a real S3-compatible server. Skipped unless
//! `TEST_S3_ENDPOINT` is set (CI runs it against a RustFS service container);
//! the rest of the suite also runs against S3 when it is, via
//! `common::setup()`. This file covers what HTTP-level tests can't reach:
//! multipart uploads, ranges across part boundaries, and startup checks.

mod common;

use futures::StreamExt;
use raptor::storage::{ArtifactStore, ByteStream, S3Store};

fn s3_store() -> Option<(ArtifactStore, raptor::config::S3Config)> {
    let Some(cfg) = common::test_s3_config() else {
        eprintln!("TEST_S3_ENDPOINT not set; skipping S3 backend test");
        return None;
    };
    let dir = tempfile::tempdir().unwrap();
    let store = ArtifactStore::with_s3(dir.path(), S3Store::new(&cfg).unwrap()).unwrap();
    std::mem::forget(dir);
    Some((store, cfg))
}

async fn read_all(mut s: ByteStream) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(chunk) = s.next().await {
        out.extend_from_slice(&chunk.unwrap());
    }
    out
}

/// Deterministic, non-repeating-at-part-boundaries content.
fn blob(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

async fn put(store: &ArtifactStore, data: &[u8]) -> raptor::storage::StoredArtifact {
    let chunks: Vec<Result<bytes::Bytes, std::convert::Infallible>> = data
        .chunks(64 * 1024)
        .map(|c| Ok(bytes::Bytes::copy_from_slice(c)))
        .collect();
    store
        .store_bytes_stream(futures::stream::iter(chunks))
        .await
        .unwrap()
}

#[tokio::test]
async fn check_accepts_good_credentials_and_rejects_bad_ones() {
    let Some((store, mut cfg)) = s3_store() else {
        return;
    };
    store.check().await.unwrap();
    cfg.secret_key = "definitely-wrong".into();
    let bad = ArtifactStore::with_s3(
        tempfile::tempdir().unwrap().path(),
        S3Store::new(&cfg).unwrap(),
    )
    .unwrap();
    assert!(bad.check().await.is_err());
}

#[tokio::test]
async fn single_put_roundtrip_range_and_remove() {
    let Some((store, _)) = s3_store() else {
        return;
    };
    let data = blob(1000);
    let meta = put(&store, &data).await;
    assert_eq!(meta.size, 1000);
    assert!(store.exists(&meta.sha256).await.unwrap());
    assert_eq!(read_all(store.open(&meta.sha256, None).await.unwrap()).await, data);
    assert_eq!(
        read_all(store.open(&meta.sha256, Some(10..20)).await.unwrap()).await,
        &data[10..20]
    );
    // dedup: a second identical upload succeeds without re-uploading
    let again = put(&store, &data).await;
    assert_eq!(again.sha256, meta.sha256);

    store.remove(&meta.sha256).await.unwrap();
    assert!(!store.exists(&meta.sha256).await.unwrap());
    store.remove(&meta.sha256).await.unwrap(); // idempotent
    let err = store.open(&meta.sha256, None).await.err().unwrap();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[tokio::test]
async fn multipart_upload_roundtrip_and_cross_part_range() {
    let Some((store, cfg)) = s3_store() else {
        return;
    };
    // Two full parts plus a short tail.
    let part = cfg.part_size as usize;
    let data = blob(2 * part + 12345);
    let meta = put(&store, &data).await;
    assert_eq!(meta.size as usize, data.len());
    assert_eq!(read_all(store.open(&meta.sha256, None).await.unwrap()).await, data);

    let (a, b) = (part - 100, part + 100);
    assert_eq!(
        read_all(
            store
                .open(&meta.sha256, Some(a as u64..b as u64))
                .await
                .unwrap()
        )
        .await,
        &data[a..b]
    );
    store.remove(&meta.sha256).await.unwrap();
}

#[tokio::test]
async fn spool_is_cleaned_up_after_upload() {
    let Some(cfg) = common::test_s3_config() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let store = ArtifactStore::with_s3(dir.path(), S3Store::new(&cfg).unwrap()).unwrap();
    let meta = put(&store, b"spool me").await;
    assert_eq!(std::fs::read_dir(dir.path().join(".tmp")).unwrap().count(), 0);
    store.remove(&meta.sha256).await.unwrap();
}
