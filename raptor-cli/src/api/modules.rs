use super::ListArgs;
use crate::client::Client;
use crate::hash::sha256_file;
use anyhow::{Result, bail};
use raptor_api_types::{ArtifactRest, PagedList, SmCreate, SmRest};
use std::path::Path;

pub async fn list(c: &Client, args: &ListArgs) -> Result<PagedList<SmRest>> {
    c.get(&format!("/rest/v1/softwaremodules{}", args.query_string()))
        .await
}

pub async fn create(c: &Client, body: &SmCreate) -> Result<SmRest> {
    c.post("/rest/v1/softwaremodules", &vec![body.clone()])
        .await
        .map(|mut v: Vec<SmRest>| v.remove(0))
}

pub async fn artifact_list(c: &Client, module_id: i64) -> Result<Vec<ArtifactRest>> {
    c.get(&format!("/rest/v1/softwaremodules/{module_id}/artifacts"))
        .await
}

/// Uploads `path` streamed (no full read into memory) and checks the
/// server-reported sha256 against a locally computed one before returning —
/// a mismatch means a corrupted upload reached the fleet's artifact store,
/// which for an OTA server is the one check that has to be there.
pub async fn artifact_upload(c: &Client, module_id: i64, path: &Path) -> Result<ArtifactRest> {
    let filename = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("not a file: {}", path.display()))?
        .to_string_lossy()
        .to_string();
    let (local_sha256, size) = sha256_file(path).await?;
    let file = tokio::fs::File::open(path).await?;
    let artifact: ArtifactRest = c
        .upload_file(
            &format!("/rest/v1/softwaremodules/{module_id}/artifacts"),
            filename,
            file,
            size,
        )
        .await?;
    if artifact.hashes.sha256 != local_sha256 {
        bail!(
            "sha256 mismatch after upload: local {local_sha256}, server {} — upload is corrupted",
            artifact.hashes.sha256
        );
    }
    Ok(artifact)
}

pub async fn artifact_delete(c: &Client, module_id: i64, artifact_id: i64) -> Result<()> {
    c.delete(&format!(
        "/rest/v1/softwaremodules/{module_id}/artifacts/{artifact_id}"
    ))
    .await
}
