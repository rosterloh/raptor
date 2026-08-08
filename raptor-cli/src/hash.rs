//! Local sha256 of a file, computed by streaming rather than reading it whole
//! into memory — same reason artifact upload streams (`client.rs::upload_file`).

use anyhow::Result;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;

/// Returns `(sha256 hex, size in bytes)`.
pub async fn sha256_file(path: &std::path::Path) -> Result<(String, u64)> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size += n as u64;
    }
    Ok((hex::encode(hasher.finalize()), size))
}
