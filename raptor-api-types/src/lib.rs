//! Shared Management API DTOs. Compiles to wasm32: serde/serde_json only.

use serde::{Deserialize, Serialize};
use serde_json::Value;

fn default_true() -> bool {
    true
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    pub exception_class: String,
    pub error_code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PollStatus {
    pub last_request_at: i64,
    pub next_expected_request_at: i64,
    pub overdue: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetRest {
    pub controller_id: String,
    pub name: String,
    pub description: Option<String>,
    pub update_status: String,
    pub security_token: String,
    pub created_at: i64,
    pub last_modified_at: i64,
    pub address: Option<String>,
    pub ip_address: Option<String>,
    pub last_controller_request_at: Option<i64>,
    pub poll_status: Option<PollStatus>,
    #[serde(rename = "_links", default)]
    pub links: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SmRest {
    pub id: i64,
    pub name: String,
    pub version: String,
    #[serde(rename = "type")]
    pub module_type: String,
    pub vendor: Option<String>,
    pub description: Option<String>,
    pub created_at: i64,
    pub last_modified_at: i64,
    #[serde(rename = "_links", default)]
    pub links: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DsRest {
    pub id: i64,
    pub name: String,
    pub version: String,
    #[serde(rename = "type")]
    pub ds_type: String,
    pub description: Option<String>,
    pub required_migration_step: bool,
    pub complete: bool,
    pub deleted: bool,
    /// False once the set has been invalidated (hawkBit `valid`).
    #[serde(default = "default_true")]
    pub valid: bool,
    pub created_at: i64,
    pub last_modified_at: i64,
    pub modules: Vec<SmRest>,
    #[serde(rename = "_links", default)]
    pub links: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtifactHashes {
    pub sha1: String,
    pub md5: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRest {
    pub id: i64,
    pub provided_filename: String,
    pub size: i64,
    pub hashes: ArtifactHashes,
    #[serde(rename = "_links", default)]
    pub links: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionRest {
    pub id: i64,
    #[serde(rename = "type")]
    pub action_type: String,
    pub status: String,
    pub detail_status: String,
    pub force_type: String,
    pub created_at: i64,
    pub last_modified_at: i64,
    /// raptor extension (additive, not in hawkBit): target controllerId.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target: Option<String>,
    #[serde(rename = "_links", default)]
    pub links: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionRef {
    pub id: i64,
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

/// One entry of an action's status history
/// (`GET /rest/v1/targets/{cid}/actions/{aid}/status`), matching hawkBit's
/// `MgmtActionStatus` shape: the status `type`, its `messages`, and when it was
/// reported.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionStatusRest {
    pub id: i64,
    #[serde(rename = "type")]
    pub status_type: String,
    pub messages: Vec<String>,
    pub reported_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssignResult {
    pub assigned: u64,
    pub already_assigned: u64,
    pub total: u64,
    #[serde(default)]
    pub assigned_actions: Vec<ActionRef>,
}

// ---- request bodies ----

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetCreate {
    pub controller_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub security_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetUpdate {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub security_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SmCreate {
    pub name: String,
    pub version: String,
    #[serde(rename = "type")]
    pub module_type: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmUpdate {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleRef {
    pub id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DsCreate {
    pub name: String,
    pub version: String,
    #[serde(rename = "type")]
    pub ds_type: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(default)]
    pub required_migration_step: bool,
    #[serde(default)]
    pub modules: Vec<ModuleRef>,
}

/// Body of `PUT /rest/v1/distributionsets/{id}`. All fields optional; omitted
/// fields are left unchanged (hawkBit `MgmtDistributionSetRequestBodyPut`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DsUpdate {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub required_migration_step: Option<bool>,
}

/// Body of `POST /rest/v1/distributionsets/{id}/invalidate`
/// (hawkBit `MgmtInvalidateDistributionSetRequestBody`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DsInvalidate {
    /// How to treat in-flight actions on the set: `force`, `soft`, or `none`
    /// (default). hawkBit spells this `actionCancelationType`.
    #[serde(default)]
    pub action_cancelation_type: Option<String>,
    /// Also stop rollouts that deploy this set.
    #[serde(default)]
    pub cancel_rollouts: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DsAssignment {
    pub id: i64,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none", default)]
    pub assign_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RolloutCondition {
    pub condition: String,
    pub expression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RolloutCreate {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    pub distribution_set_id: i64,
    pub target_filter_query: String,
    pub amount_groups: i64,
    pub success_condition: RolloutCondition,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error_condition: Option<RolloutCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RolloutRest {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub distribution_set_id: i64,
    pub target_filter_query: String,
    pub status: String,
    pub total_targets: i64,
    pub created_at: i64,
    pub last_modified_at: i64,
    #[serde(rename = "_links", default)]
    pub links: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RolloutGroupRest {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub total_targets: i64,
    pub success_condition: RolloutCondition,
    pub error_condition: RolloutCondition,
    #[serde(rename = "_links", default)]
    pub links: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetFilterCreate {
    pub name: String,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetFilterUpdate {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetFilterRest {
    pub id: i64,
    pub name: String,
    pub query: String,
    pub auto_assign_distribution_set: Option<i64>,
    pub auto_assign_action_type: Option<String>,
    pub created_at: i64,
    pub last_modified_at: i64,
    #[serde(rename = "_links", default)]
    pub links: Value,
}

/// Body of `POST /rest/v1/targetfilters/{id}/autoAssignDS`: the distribution set
/// id plus an optional action type ("forced" default, or "soft").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutoAssignRequest {
    pub id: i64,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none", default)]
    pub action_type: Option<String>,
}

/// A single key/value metadata entry (hawkBit `MgmtMetadata`). `targetVisible`
/// is only present for software-module metadata
/// (`MgmtSoftwareModuleMetadata`); target and distribution-set metadata omit it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRest {
    pub key: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target_visible: Option<bool>,
}

/// One element of the `POST .../metadata` request array. `targetVisible` only
/// applies to software-module metadata and is ignored elsewhere.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MetadataCreate {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub target_visible: bool,
}

/// Body of `PUT .../metadata/{key}` (hawkBit `MgmtMetadataBodyPut`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MetadataUpdate {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target_visible: Option<bool>,
}

/// Auto-confirm state for a target (hawkBit `GET /rest/v1/targets/{cid}/autoConfirm`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutoConfirmState {
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub activated_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Round-trip through serde and compare Values: proves both key names
    // (Deserialize) and output shape (Serialize) match the server's JSON.
    fn round_trip<T: serde::Serialize + serde::de::DeserializeOwned>(v: serde_json::Value) {
        let t: T = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(&t).unwrap(), v);
    }

    #[test]
    fn target_shape() {
        round_trip::<TargetRest>(json!({
            "controllerId": "d1", "name": "device one", "description": null,
            "updateStatus": "in_sync", "securityToken": "abc123",
            "createdAt": 1, "lastModifiedAt": 2, "address": null, "ipAddress": null,
            "lastControllerRequestAt": 3,
            "pollStatus": {"lastRequestAt": 3, "nextExpectedRequestAt": 4, "overdue": false},
            "_links": {"self": {"href": "http://x/rest/v1/targets/d1"}}
        }));
    }

    #[test]
    fn software_module_shape() {
        round_trip::<SmRest>(json!({
            "id": 1, "name": "fw", "version": "1.0", "type": "os",
            "vendor": null, "description": null, "createdAt": 1, "lastModifiedAt": 2,
            "_links": {"self": {"href": "http://x/rest/v1/softwaremodules/1"}}
        }));
    }

    #[test]
    fn distribution_set_shape() {
        round_trip::<DsRest>(json!({
            "id": 1, "name": "stable", "version": "1.0", "type": "os",
            "description": null, "requiredMigrationStep": false, "complete": true,
            "deleted": false, "valid": true, "createdAt": 1, "lastModifiedAt": 2,
            "modules": [{
                "id": 1, "name": "fw", "version": "1.0", "type": "os",
                "vendor": null, "description": null, "createdAt": 1, "lastModifiedAt": 2,
                "_links": {"self": {"href": "http://x/rest/v1/softwaremodules/1"}}
            }],
            "_links": {"self": {"href": "http://x/rest/v1/distributionsets/1"}}
        }));
    }

    #[test]
    fn artifact_shape() {
        round_trip::<ArtifactRest>(json!({
            "id": 1, "providedFilename": "fw.bin", "size": 11,
            "hashes": {"sha1": "a", "md5": "b", "sha256": "c"},
            "_links": {"self": {"href": "http://x"}, "download": {"href": "http://x/download"}}
        }));
    }

    #[test]
    fn action_shape() {
        round_trip::<ActionRest>(json!({
            "id": 1, "type": "update", "status": "pending", "detailStatus": "running",
            "forceType": "forced", "createdAt": 1, "lastModifiedAt": 2,
            "target": "d1",
            "_links": {"self": {"href": "http://x/rest/v1/actions/1"}}
        }));
    }

    #[test]
    fn action_target_field_omitted_when_none() {
        let a = ActionRest {
            id: 1,
            action_type: "update".into(),
            status: "pending".into(),
            detail_status: "running".into(),
            force_type: "forced".into(),
            created_at: 1,
            last_modified_at: 2,
            target: None,
            links: serde_json::Value::Null,
        };
        let v = serde_json::to_value(&a).unwrap();
        assert!(v.get("target").is_none());
    }

    #[test]
    fn action_status_shape() {
        round_trip::<ActionStatusRest>(json!({
            "id": 7, "type": "canceled",
            "messages": ["force canceled by operator"],
            "reportedAt": 1699999999000i64
        }));
    }

    #[test]
    fn rollout_shape() {
        round_trip::<RolloutRest>(json!({
            "id": 1, "name": "r1", "description": null, "distributionSetId": 5,
            "targetFilterQuery": "name==*", "status": "running", "totalTargets": 10,
            "createdAt": 1, "lastModifiedAt": 2,
            "_links": {"self": {"href": "http://x/rest/v1/rollouts/1"}}
        }));
    }

    #[test]
    fn rollout_group_shape() {
        round_trip::<RolloutGroupRest>(json!({
            "id": 6, "name": "group-1", "status": "running", "totalTargets": 5,
            "successCondition": {"condition": "THRESHOLD", "expression": "50"},
            "errorCondition": {"condition": "THRESHOLD", "expression": "50"},
            "_links": {"self": {"href": "http://x/rest/v1/rollouts/1/deploygroups/6"}}
        }));
    }

    #[test]
    fn target_filter_shape() {
        round_trip::<TargetFilterRest>(json!({
            "id": 3, "name": "beta", "query": "controllerId==dev-*",
            "autoAssignDistributionSet": 7, "autoAssignActionType": "forced",
            "createdAt": 1, "lastModifiedAt": 2,
            "_links": {"self": {"href": "http://x/rest/v1/targetfilters/3"}}
        }));
    }

    #[test]
    fn target_filter_no_auto_assign() {
        round_trip::<TargetFilterRest>(json!({
            "id": 3, "name": "beta", "query": "name==*",
            "autoAssignDistributionSet": null, "autoAssignActionType": null,
            "createdAt": 1, "lastModifiedAt": 2, "_links": null
        }));
    }

    #[test]
    fn auto_assign_request_shape() {
        let a = AutoAssignRequest {
            id: 7,
            action_type: Some("soft".into()),
        };
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            json!({"id": 7, "type": "soft"})
        );
        // type omitted when None
        let a = AutoAssignRequest {
            id: 7,
            action_type: None,
        };
        assert_eq!(serde_json::to_value(&a).unwrap(), json!({"id": 7}));
    }

    #[test]
    fn auto_confirm_state_shape() {
        round_trip::<AutoConfirmState>(json!({"active": true, "activatedAt": 5}));
        // activatedAt omitted when None
        let s = AutoConfirmState {
            active: false,
            activated_at: None,
        };
        assert_eq!(serde_json::to_value(&s).unwrap(), json!({"active": false}));
    }

    #[test]
    fn metadata_shape() {
        // target / DS metadata: targetVisible omitted
        round_trip::<MetadataRest>(json!({"key": "region", "value": "eu"}));
        // software-module metadata: targetVisible present
        round_trip::<MetadataRest>(json!({
            "key": "region", "value": "eu", "targetVisible": true
        }));
    }

    #[test]
    fn metadata_create_defaults_target_visible() {
        let c: MetadataCreate = serde_json::from_value(json!({"key": "k", "value": "v"})).unwrap();
        assert!(!c.target_visible);
    }

    #[test]
    fn metadata_update_shape() {
        round_trip::<MetadataUpdate>(json!({"value": "v"}));
        round_trip::<MetadataUpdate>(json!({"value": "v", "targetVisible": false}));
    }

    #[test]
    fn paged_envelope() {
        let p = PagedList::new(vec![1, 2, 3], 10);
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v, json!({"content": [1, 2, 3], "total": 10, "size": 3}));
        let back: PagedList<i64> = serde_json::from_value(v).unwrap();
        assert_eq!(back.content, vec![1, 2, 3]);
    }

    #[test]
    fn error_body_shape() {
        round_trip::<ErrorBody>(json!({
            "exceptionClass": "x.Y", "errorCode": "hawkbit.server.error.z", "message": "boom"
        }));
    }

    #[test]
    fn assignment_request_shape() {
        let a = DsAssignment {
            id: 5,
            assign_type: Some("forced".into()),
        };
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            json!({"id": 5, "type": "forced"})
        );
    }

    #[test]
    fn assign_result_shape() {
        round_trip::<AssignResult>(json!({
            "assigned": 1, "alreadyAssigned": 0, "total": 1, "assignedActions": [{"id": 7}]
        }));
    }
}
