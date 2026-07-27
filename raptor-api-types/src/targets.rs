//! Target resource: the `MgmtTarget` DTO, its poll status and auto-confirm
//! state, create/update request bodies, and target-type CRUD bodies.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Controller polling cadence info embedded in [`TargetRest`] (hawkBit
/// `MgmtPollStatus`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PollStatus {
    pub last_request_at: i64,
    pub next_expected_request_at: i64,
    pub overdue: bool,
}

/// A managed device (hawkBit `MgmtTarget`), keyed by `controllerId`.
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
    /// Assigned target type id, if any (hawkBit `targetType`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target_type: Option<i64>,
    /// Whether the DDI base poll is currently asking this device to (re-)send
    /// its attributes (hawkBit `requestAttributes`).
    #[serde(default)]
    pub request_attributes: bool,
    #[serde(rename = "_links", default)]
    pub links: Value,
}

/// Body of `POST /rest/v1/targets` (hawkBit `MgmtTargetRequestBody`).
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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target_type: Option<i64>,
}

/// Body of `PUT /rest/v1/targets/{id}`. Omitted fields are left unchanged.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetUpdate {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub security_token: Option<String>,
    /// Set to `true` to make the next DDI base poll re-advertise `configData`,
    /// asking the device to re-send its attributes (hawkBit
    /// `MgmtTargetRequestBody.requestAttributes`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub request_attributes: Option<bool>,
}

/// Body of `POST /rest/v1/targettypes` (hawkBit
/// `MgmtTargetTypeRequestBodyPost`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetTypeCreate {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub colour: Option<String>,
    #[serde(default)]
    pub compatibledistributionsettypes: Vec<crate::common::TypeRef>,
}

/// Body of `PUT /rest/v1/targettypes/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct TargetTypeUpdate {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub colour: Option<String>,
}

/// Auto-confirm state for a target (hawkBit `GET /rest/v1/targets/{cid}/autoConfirm`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutoConfirmState {
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub activated_at: Option<i64>,
}
