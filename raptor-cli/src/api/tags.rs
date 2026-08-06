//! Target tags. `raptorctl target tag add/rm` takes a tag *name*, but the
//! assignment endpoint wants an id — `find_id` resolves it via a list call,
//! same as the web console would.

use crate::client::Client;
use anyhow::Result;
use raptor_api_types::TagRest;

pub async fn list(c: &Client) -> Result<Vec<TagRest>> {
    let paged: raptor_api_types::PagedList<TagRest> =
        c.get("/rest/v1/targettags?limit=1000").await?;
    Ok(paged.content)
}

pub async fn find_id(c: &Client, name: &str) -> Result<i64> {
    list(c)
        .await?
        .into_iter()
        .find(|t| t.name == name)
        .map(|t| t.id)
        .ok_or_else(|| anyhow::anyhow!("no target tag named '{name}'"))
}

pub async fn assign(c: &Client, tag_id: i64, cid: &str) -> Result<()> {
    c.post_empty(&format!("/rest/v1/targettags/{tag_id}/assigned/{cid}"), &())
        .await
}

pub async fn unassign(c: &Client, tag_id: i64, cid: &str) -> Result<()> {
    c.delete(&format!("/rest/v1/targettags/{tag_id}/assigned/{cid}"))
        .await
}
