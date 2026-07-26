//! Action resource: the `MgmtAction` DTO representing an in-flight or
//! completed deployment on a target, its status history, and assignment
//! results.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A deployment action against a target (hawkBit `MgmtAction`): created when
/// a distribution set is assigned to a target.
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

/// Bare `{ "id": N }` reference to an action, returned in [`AssignResult`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionRef {
    pub id: i64,
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

/// Response of a distribution-set-to-target(s) assignment call (hawkBit
/// `MgmtActionsAssignmentResult` / `MgmtTargetAssignmentResponseBody`
/// depending on endpoint).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssignResult {
    pub assigned: u64,
    pub already_assigned: u64,
    pub total: u64,
    #[serde(default)]
    pub assigned_actions: Vec<ActionRef>,
}
