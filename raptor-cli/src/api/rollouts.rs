use crate::client::Client;
use anyhow::Result;
use raptor_api_types::{PagedList, RolloutRest};

/// Backs both the TUI's rollout widget and `raptorctl rollout list`.
pub async fn list(c: &Client) -> Result<Vec<RolloutRest>> {
    let paged: PagedList<RolloutRest> = c.get("/rest/v1/rollouts?limit=50").await?;
    Ok(paged.content)
}

pub async fn get(c: &Client, id: i64) -> Result<RolloutRest> {
    c.get(&format!("/rest/v1/rollouts/{id}")).await
}

/// Approve or deny a rollout awaiting approval. Both answer 204 with no body,
/// so callers re-read the rollout to show the outcome.
pub async fn decide(c: &Client, id: i64, approve: bool, remark: Option<&str>) -> Result<()> {
    let verb = if approve { "approve" } else { "deny" };
    let query = match remark {
        Some(r) => format!("?remark={}", super::urlencode(r)),
        None => String::new(),
    };
    c.post_no_body(&format!("/rest/v1/rollouts/{id}/{verb}{query}"))
        .await
}
