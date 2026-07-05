use crate::error::AppError;
use futures::StreamExt;
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

#[derive(Clone)]
pub struct ArtifactStore {
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StoredArtifact {
    pub size: i64,
    pub sha1: String,
    pub md5: String,
    pub sha256: String,
}

impl ArtifactStore {
    pub fn new(root: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(root.join(".tmp"))?;
        Ok(Self { root })
    }

    pub fn path_for(&self, sha256: &str) -> PathBuf {
        self.root.join(&sha256[..2]).join(sha256)
    }

    pub async fn store_bytes_stream<S, E>(&self, mut stream: S) -> Result<StoredArtifact, AppError>
    where
        S: futures::Stream<Item = Result<bytes::Bytes, E>> + Unpin,
        E: std::fmt::Display,
    {
        let tmp = self.root.join(".tmp").join(crate::util::random_token());
        let mut file = tokio::fs::File::create(&tmp).await?;
        let write_result = async {
            let (mut h1, mut h5, mut h256) = (Sha1::new(), Md5::new(), Sha256::new());
            let mut size: i64 = 0;
            while let Some(chunk) = stream.next().await {
                let chunk =
                    chunk.map_err(|e| AppError::BadRequest(format!("upload stream: {e}")))?;
                h1.update(&chunk);
                h5.update(&chunk);
                h256.update(&chunk);
                size += chunk.len() as i64;
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
            Ok((h1, h5, h256, size))
        }
        .await;
        let (h1, h5, h256, size) = match write_result {
            Ok(v) => v,
            Err(e) => {
                let _ = tokio::fs::remove_file(&tmp).await;
                return Err(e);
            }
        };
        drop(file);
        let meta = StoredArtifact {
            size,
            sha1: hex::encode(h1.finalize()),
            md5: hex::encode(h5.finalize()),
            sha256: hex::encode(h256.finalize()),
        };
        let final_path = self.path_for(&meta.sha256);
        if final_path.exists() {
            tokio::fs::remove_file(&tmp).await?; // dedup
        } else {
            tokio::fs::create_dir_all(final_path.parent().unwrap()).await?;
            tokio::fs::rename(&tmp, &final_path).await?;
        }
        Ok(meta)
    }

    pub fn remove(&self, sha256: &str) -> std::io::Result<()> {
        match std::fs::remove_file(self.path_for(sha256)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            r => r,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream_of(
        chunks: Vec<&'static [u8]>,
    ) -> impl futures::Stream<Item = Result<bytes::Bytes, std::convert::Infallible>> + Unpin {
        futures::stream::iter(chunks.into_iter().map(|c| Ok(bytes::Bytes::from_static(c))))
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
        let path = store.path_for(&meta.sha256);
        assert_eq!(std::fs::read(&path).unwrap(), b"hello world");
        assert!(path.starts_with(dir.path()));
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
        assert!(store.path_for(&a.sha256).exists());
        store.remove(&a.sha256).unwrap();
        assert!(!store.path_for(&a.sha256).exists());
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
