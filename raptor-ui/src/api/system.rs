//! System endpoints: fleet statistics and read-only tenant configuration.

use super::{ApiResult, get_json};
use raptor_api_types::{SystemStatistics, TenantConfigValue};
use std::collections::BTreeMap;

/// Server-computed fleet counters — the dashboard tiles read these instead of
/// tallying a page of targets/actions client-side.
pub async fn system_statistics() -> ApiResult<SystemStatistics> {
    get_json("/rest/v1/system/statistics").await
}

/// Tenant configuration as the DDI clients see it. Read-only: raptor's config
/// is file-driven, so the server answers PUT/DELETE with 403.
pub async fn system_configs() -> ApiResult<BTreeMap<String, TenantConfigValue>> {
    get_json("/rest/v1/system/configs").await
}
