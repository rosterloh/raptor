use super::ListArgs;
use crate::client::Client;
use anyhow::Result;
use raptor_api_types::{DsCreate, DsInvalidate, DsRest, PagedList};

pub async fn list(c: &Client, args: &ListArgs) -> Result<PagedList<DsRest>> {
    c.get(&format!("/rest/v1/distributionsets{}", args.query_string()))
        .await
}

pub async fn get(c: &Client, id: i64) -> Result<DsRest> {
    c.get(&format!("/rest/v1/distributionsets/{id}")).await
}

pub async fn create(c: &Client, body: &DsCreate) -> Result<DsRest> {
    c.post("/rest/v1/distributionsets", &vec![body.clone()])
        .await
        .map(|mut v: Vec<DsRest>| v.remove(0))
}

/// Returns 200 with no body, so nothing to decode.
pub async fn invalidate(c: &Client, id: i64, body: &DsInvalidate) -> Result<()> {
    c.post_empty(&format!("/rest/v1/distributionsets/{id}/invalidate"), body)
        .await
}
