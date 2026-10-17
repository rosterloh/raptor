//! Local-disk backend: blobs at `<artifact_dir>/<sha256[0..2]>/<sha256>`.

use super::{ByteStream, blob_rel_path};
use std::ops::Range;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// Read buffer for streaming a blob out.
const READ_CHUNK: usize = 64 * 1024;

#[derive(Clone)]
pub(super) struct FsStore {
    root: PathBuf,
}

impl FsStore {
    pub(super) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn path_for(&self, sha256: &str) -> PathBuf {
        self.root.join(blob_rel_path(sha256))
    }

    /// Move a fully written spool file into place. The spool lives under the
    /// same root, so this is a same-filesystem rename.
    pub(super) async fn commit(&self, tmp: &Path, sha256: &str) -> std::io::Result<()> {
        let final_path = self.path_for(sha256);
        if tokio::fs::try_exists(&final_path).await? {
            return Ok(()); // dedup: caller removes the spool file
        }
        tokio::fs::create_dir_all(final_path.parent().expect("blob path has a parent")).await?;
        tokio::fs::rename(tmp, &final_path).await
    }

    pub(super) async fn open(
        &self,
        sha256: &str,
        range: Option<Range<u64>>,
    ) -> std::io::Result<ByteStream> {
        let mut file = tokio::fs::File::open(self.path_for(sha256)).await?;
        Ok(match range {
            Some(r) => {
                file.seek(std::io::SeekFrom::Start(r.start)).await?;
                let take = file.take(r.end.saturating_sub(r.start));
                Box::pin(tokio_util::io::ReaderStream::with_capacity(
                    take, READ_CHUNK,
                ))
            }
            None => Box::pin(tokio_util::io::ReaderStream::with_capacity(
                file, READ_CHUNK,
            )),
        })
    }

    pub(super) async fn exists(&self, sha256: &str) -> std::io::Result<bool> {
        tokio::fs::try_exists(self.path_for(sha256)).await
    }

    pub(super) async fn remove(&self, sha256: &str) -> std::io::Result<()> {
        match tokio::fs::remove_file(self.path_for(sha256)).await {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            r => r,
        }
    }
}
