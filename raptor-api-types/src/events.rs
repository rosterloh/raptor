//! Live event payloads for the `/rest/v1/events` server-sent-events stream
//! (raptor extension, not part of hawkBit). Each type is the `data:` body of
//! the SSE event whose name is the matching `EVENT_*` constant.

use serde::{Deserialize, Serialize};

/// SSE event name for [`TargetEvent`].
pub const EVENT_TARGET: &str = "target";
/// SSE event name for [`ActionEvent`].
pub const EVENT_ACTION: &str = "action";
/// SSE event name for [`RolloutEvent`].
pub const EVENT_ROLLOUT: &str = "rollout";
/// SSE event name for [`DownloadEvent`].
pub const EVENT_DOWNLOAD: &str = "download";
/// SSE event name for [`ProgressEvent`].
pub const EVENT_PROGRESS: &str = "progress";
/// SSE event name sent when a subscriber has missed events and must refetch.
pub const EVENT_RESYNC: &str = "resync";

/// A target was created, updated or polled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetEvent {
    pub controller_id: String,
}

/// An action changed state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionEvent {
    pub controller_id: String,
    pub action_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollout_id: Option<i64>,
}

/// A rollout changed state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloutEvent {
    pub rollout_id: i64,
}

/// Artifact download progress for one file of an action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadEvent {
    pub controller_id: String,
    pub action_id: i64,
    pub filename: String,
    pub sent: u64,
    pub total: u64,
}

/// Device-reported installation progress (`cnt` of `of` steps).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub controller_id: String,
    pub action_id: i64,
    pub cnt: u32,
    pub of: u32,
}
