use super::ListArgs;
use crate::client::Client;
use anyhow::Result;
use raptor_api_types::{ArtifactRest, PagedList, SmCreate, SmRest};

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

pub async fn artifact_upload(
    c: &Client,
    module_id: i64,
    filename: String,
    bytes: Vec<u8>,
) -> Result<ArtifactRest> {
    c.upload(
        &format!("/rest/v1/softwaremodules/{module_id}/artifacts"),
        filename,
        bytes,
    )
    .await
}

pub async fn artifact_delete(c: &Client, module_id: i64, artifact_id: i64) -> Result<()> {
    c.delete(&format!(
        "/rest/v1/softwaremodules/{module_id}/artifacts/{artifact_id}"
    ))
    .await
}
