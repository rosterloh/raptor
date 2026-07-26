//! Cross-resource DTOs: the paged-list envelope, error body, login request,
//! tenant configuration, and fleet-wide statistics. Nothing here is tied to a
//! single resource area.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// hawkBit paged-list envelope used by every list endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PagedList<T> {
    pub content: Vec<T>,
    pub total: u64,
    pub size: usize,
}

impl<T> PagedList<T> {
    pub fn new(content: Vec<T>, total: u64) -> Self {
        let size = content.len();
        Self {
            content,
            total,
            size,
        }
    }
}

/// hawkBit's standard error envelope, returned on any non-2xx response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    pub exception_class: String,
    pub error_code: String,
    pub message: String,
}

/// Body of the raptor login endpoint (not part of the hawkBit Management API).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// One tenant-configuration value (hawkBit `MgmtSystemTenantConfigurationValue`).
/// raptor is config-file driven, so every value is `global` and read-only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TenantConfigValue {
    pub value: Value,
    pub global: bool,
}

/// Fleet counters for `GET /rest/v1/system/statistics` (raptor operational
/// aid; feeds the web console dashboard). Not a fixed hawkBit schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatistics {
    pub total_targets: u64,
    pub total_distribution_sets: u64,
    pub total_software_modules: u64,
    pub total_actions: u64,
    pub total_rollouts: u64,
    /// Targets grouped by `updateStatus` (in_sync / pending / error / registered).
    pub targets_by_status: std::collections::BTreeMap<String, u64>,
    /// Currently active (in-flight) actions.
    pub active_actions: u64,
}

/// Bare `{ "id": N }` reference used by type composition/compatibility bodies
/// (shared between software-module, distribution-set, and target types).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct TypeRef {
    pub id: i64,
}
