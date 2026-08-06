use crate::client::Client;
use anyhow::Result;
use raptor_api_types::SystemStatistics;

pub async fn statistics(c: &Client) -> Result<SystemStatistics> {
    c.get("/rest/v1/system/statistics").await
}
