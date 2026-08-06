use super::ListArgs;
use crate::client::Client;
use anyhow::Result;
use raptor_api_types::{
    ActionRest, ActionStatusRest, ActionUpdate, AssignResult, DsAssignment, PagedList,
};

pub async fn list_all(c: &Client, args: &ListArgs) -> Result<PagedList<ActionRest>> {
    c.get(&format!("/rest/v1/actions{}", args.query_string()))
        .await
}

pub async fn list_for_target(
    c: &Client,
    cid: &str,
    args: &ListArgs,
) -> Result<PagedList<ActionRest>> {
    c.get(&format!(
        "/rest/v1/targets/{cid}/actions{}",
        args.query_string()
    ))
    .await
}

pub async fn status_history(c: &Client, cid: &str, aid: i64) -> Result<Vec<ActionStatusRest>> {
    let paged: PagedList<ActionStatusRest> = c
        .get(&format!(
            "/rest/v1/targets/{cid}/actions/{aid}/status?limit=1000"
        ))
        .await?;
    Ok(paged.content)
}

pub async fn assign(c: &Client, cid: &str, ds: &DsAssignment) -> Result<AssignResult> {
    c.post(&format!("/rest/v1/targets/{cid}/assignedDS"), ds)
        .await
}

pub async fn force(c: &Client, cid: &str, aid: i64) -> Result<ActionRest> {
    let update = ActionUpdate {
        force_type: Some("forced".into()),
    };
    c.put(&format!("/rest/v1/targets/{cid}/actions/{aid}"), &update)
        .await
}

pub async fn cancel(c: &Client, cid: &str, aid: i64, force: bool) -> Result<()> {
    let suffix = if force { "?force=true" } else { "" };
    c.delete(&format!("/rest/v1/targets/{cid}/actions/{aid}{suffix}"))
        .await
}
