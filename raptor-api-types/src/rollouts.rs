//! Rollout resource: the `MgmtRollout` DTO for staged, auto-progressing
//! deployments, its deployment groups, and the create request body.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A success/error threshold condition for a rollout or rollout group
/// (hawkBit `MgmtRolloutCondition`), e.g. `{"condition": "THRESHOLD",
/// "expression": "50"}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RolloutCondition {
    pub condition: String,
    pub expression: String,
}

/// Body of `POST /rest/v1/rollouts` (hawkBit `MgmtRolloutRequestBody`).
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

/// Targets of a rollout (or one of its groups) counted by deployment outcome,
/// hawkBit's `totalTargetsPerStatus`. `notstarted` are targets of a rollout that
/// has not been started, `scheduled` targets of a group awaiting its turn.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RolloutTargetsPerStatus {
    pub notstarted: i64,
    pub scheduled: i64,
    pub running: i64,
    pub error: i64,
    pub finished: i64,
    pub cancelled: i64,
}

impl std::ops::AddAssign for RolloutTargetsPerStatus {
    fn add_assign(&mut self, o: Self) {
        self.notstarted += o.notstarted;
        self.scheduled += o.scheduled;
        self.running += o.running;
        self.error += o.error;
        self.finished += o.finished;
        self.cancelled += o.cancelled;
    }
}

/// A rollout (hawkBit `MgmtRollout`): a staged deployment of a distribution
/// set to all targets matching a filter query.
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
    #[serde(default)]
    pub total_targets_per_status: RolloutTargetsPerStatus,
    pub created_at: i64,
    pub last_modified_at: i64,
    #[serde(rename = "_links", default)]
    pub links: Value,
}

/// One deployment group within a rollout (hawkBit `MgmtRolloutGroup`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RolloutGroupRest {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub total_targets: i64,
    #[serde(default)]
    pub total_targets_per_status: RolloutTargetsPerStatus,
    pub success_condition: RolloutCondition,
    pub error_condition: RolloutCondition,
    #[serde(rename = "_links", default)]
    pub links: Value,
}
