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

/// Shape of the dynamic groups a dynamic rollout appends
/// (hawkBit `MgmtDynamicRolloutGroupTemplate`). Only accepted when the
/// rollout is created with `dynamic: true`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DynamicRolloutGroupTemplate {
    /// Appended to the generated `group-<n>` name, e.g. `-dynamic`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name_suffix: Option<String>,
    /// How many targets one dynamic group absorbs before the next is created.
    /// Defaults to the size of the last static group.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub target_count: Option<i64>,
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
    /// Action type for every action this rollout creates: `forced` (default),
    /// `soft`, `timeforced` or `downloadonly`.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none", default)]
    pub rollout_type: Option<String>,
    /// `timeforced` deadline in epoch millis. Lowercase on both request and
    /// response for rollouts, unlike the action resource's `forceTime`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub forcetime: Option<i64>,
    /// Keep absorbing targets that start matching `targetFilterQuery` after
    /// creation, into a trailing group that runs until the rollout is stopped.
    #[serde(default)]
    pub dynamic: bool,
    /// Shape of those trailing groups. Rejected unless `dynamic` is set.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub dynamic_group_template: Option<DynamicRolloutGroupTemplate>,
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
    /// The action type actions created by this rollout inherit.
    #[serde(rename = "type")]
    pub rollout_type: String,
    /// `timeforced` deadline in epoch millis. Lowercase on both request and
    /// response for rollouts, unlike the action resource's `forceTime`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub forcetime: Option<i64>,
    pub total_targets: i64,
    #[serde(default)]
    pub total_targets_per_status: RolloutTargetsPerStatus,
    pub created_at: i64,
    pub last_modified_at: i64,
    /// Operator who approved or denied the rollout, once the approval workflow
    /// has been through it. Serialized as `approveDecidedBy`, not
    /// `approvalDecidedBy`: hawkBit's own DTO field is spelled that way even
    /// though its domain model calls it `approvalDecidedBy`, and the wire
    /// format is the contract — see `MgmtRolloutResponseBody`.
    #[serde(
        rename = "approveDecidedBy",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub approve_decided_by: Option<String>,
    /// Free-form note left with the approve/deny decision.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub approval_remark: Option<String>,
    /// Whether this rollout keeps absorbing newly-matching targets. Always
    /// serialized, matching hawkBit's primitive `boolean` field.
    #[serde(default)]
    pub dynamic: bool,
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
    /// Whether this group absorbs targets that newly match the rollout's
    /// filter, rather than holding a fixed membership.
    #[serde(default)]
    pub dynamic: bool,
    pub total_targets: i64,
    #[serde(default)]
    pub total_targets_per_status: RolloutTargetsPerStatus,
    pub success_condition: RolloutCondition,
    pub error_condition: RolloutCondition,
    #[serde(rename = "_links", default)]
    pub links: Value,
}
