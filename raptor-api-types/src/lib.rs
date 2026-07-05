//! Shared Management API DTOs. Compiles to wasm32: serde/serde_json only.

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DsAssignment {
    pub id: i64,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none", default)]
    pub assign_type: Option<String>,
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
            "deleted": false, "createdAt": 1, "lastModifiedAt": 2,
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
