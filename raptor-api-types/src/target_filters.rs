//! Target filter resource: the `MgmtTargetFilterQuery` DTO for saved target
//! queries, its create/update bodies, and the auto-assign-DS request.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Body of `POST /rest/v1/targetfilters` (hawkBit
/// `MgmtTargetFilterQueryRequestBody`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetFilterCreate {
    pub name: String,
    pub query: String,
}

/// Body of `PUT /rest/v1/targetfilters/{id}`. Omitted fields are left
/// unchanged.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TargetFilterUpdate {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub query: Option<String>,
}

/// A saved target filter query (hawkBit `MgmtTargetFilterQuery`), optionally
/// wired to auto-assign a distribution set to any target that matches.
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
