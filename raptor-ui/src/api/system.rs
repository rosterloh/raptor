//! System endpoints: fleet statistics and read-only tenant configuration.

use super::{ApiResult, get_json};
use raptor_api_types::{SystemStatistics, TenantConfigValue};
use std::collections::BTreeMap;

/// Server-computed fleet counters — the dashboard tiles read these instead of
/// tallying a page of targets/actions client-side.
pub async fn system_statistics() -> ApiResult<SystemStatistics> {
    get_json("/rest/v1/system/statistics").await
}

/// The same counters scoped to a FIQL query — one request per saved target
/// filter, instead of one request per status per filter. The catalogue counters
/// in the response stay fleet-wide; only the target ones narrow.
pub async fn system_statistics_for(q: &str) -> ApiResult<SystemStatistics> {
    get_json(&format!(
        "/rest/v1/system/statistics?q={}",
        crate::logic::urlencode(q)
    ))
    .await
}

/// Tenant configuration as the DDI clients see it. Read-only: raptor's config
/// is file-driven, so the server answers PUT/DELETE with 403.
pub async fn system_configs() -> ApiResult<BTreeMap<String, TenantConfigValue>> {
    get_json("/rest/v1/system/configs").await
}
