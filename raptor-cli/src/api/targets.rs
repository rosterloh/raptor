use super::ListArgs;
use crate::client::Client;
use anyhow::Result;
use raptor_api_types::{PagedList, TargetCreate, TargetRest, TargetUpdate};
use std::collections::BTreeMap;

pub async fn list(c: &Client, args: &ListArgs) -> Result<PagedList<TargetRest>> {
    c.get(&format!("/rest/v1/targets{}", args.query_string()))
        .await
}

pub async fn get(c: &Client, cid: &str) -> Result<TargetRest> {
    c.get(&format!("/rest/v1/targets/{cid}")).await
}

pub async fn create(c: &Client, body: &TargetCreate) -> Result<TargetRest> {
    c.post("/rest/v1/targets", &vec![body.clone()])
        .await
        .map(|mut v: Vec<TargetRest>| v.remove(0))
}

pub async fn update(c: &Client, cid: &str, body: &TargetUpdate) -> Result<TargetRest> {
    c.put(&format!("/rest/v1/targets/{cid}"), body).await
}

pub async fn delete(c: &Client, cid: &str) -> Result<()> {
    c.delete(&format!("/rest/v1/targets/{cid}")).await
}

pub async fn attributes(c: &Client, cid: &str) -> Result<BTreeMap<String, String>> {
    c.get(&format!("/rest/v1/targets/{cid}/attributes")).await
}
