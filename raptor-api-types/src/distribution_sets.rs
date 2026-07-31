//! Distribution set resource: the `MgmtDistributionSet` DTO, create/update/
//! invalidate/assignment request bodies, and distribution-set-type CRUD
//! bodies.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::common::TypeRef;
use crate::software_modules::{ModuleRef, SmRest};

fn default_true() -> bool {
    true
}

/// A distribution set (hawkBit `MgmtDistributionSet`): a named, versioned
/// bundle of software modules that gets assigned/rolled out to targets.
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

/// Compact reference to a distribution set, for embedding in a payload where the
/// full [`DsRest`] would be wasteful — it carries every module, which is a lot of
/// bytes repeated once per row of a target list.
///
/// Enough to answer "which set, which version, could this device even take it":
/// the type is what constrains whether a given device class can install it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DsRef {
    pub id: i64,
    pub name: String,
    pub version: String,
    #[serde(rename = "type")]
    pub ds_type: String,
}

/// Body of `POST /rest/v1/distributionsets` (hawkBit
/// `MgmtDistributionSetRequestBodyPost`).
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

/// Element of `POST /rest/v1/targets/{cid}/assignedDS` (and similar
/// assignment endpoints): the distribution set id plus an optional action
/// type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DsAssignment {
    pub id: i64,
    /// `forced` (default), `soft`, `timeforced` or `downloadonly`.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none", default)]
    pub assign_type: Option<String>,
    /// For `timeforced`: epoch millis after which the action becomes forced.
    /// hawkBit spells this all-lowercase on the request body while the action
    /// response uses `forceTime` — mirrored deliberately, don't "fix" it.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub forcetime: Option<i64>,
}

/// Body of `POST /rest/v1/distributionsettypes` (hawkBit
/// `MgmtDistributionSetTypeRequestBodyPost`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DsTypeCreate {
    pub key: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(default)]
    pub mandatorymodules: Vec<TypeRef>,
    #[serde(default)]
    pub optionalmodules: Vec<TypeRef>,
}

/// Body of `PUT /rest/v1/distributionsettypes/{id}` — only description mutable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DsTypeUpdate {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
}
