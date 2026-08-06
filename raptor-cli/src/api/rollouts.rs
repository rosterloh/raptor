use crate::client::Client;
use anyhow::Result;
use raptor_api_types::{PagedList, RolloutRest};

/// Read-only in v1 — used only by the TUI's rollout widget.
pub async fn list(c: &Client) -> Result<Vec<RolloutRest>> {
    let paged: PagedList<RolloutRest> = c.get("/rest/v1/rollouts?limit=50").await?;
    Ok(paged.content)
}
