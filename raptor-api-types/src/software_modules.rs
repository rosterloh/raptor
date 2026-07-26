//! Software module resource: the `MgmtSoftwareModule` DTO, its artifacts,
//! create/update request bodies, and software-module-type CRUD bodies.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A software module (hawkBit `MgmtSoftwareModule`): one build artifact
/// grouping (e.g. an OS image or app package) that can be assigned into a
/// [`crate::distribution_sets::DsRest`].
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

/// The three checksums hawkBit reports for every artifact (hawkBit
/// `MgmtArtifactHash`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtifactHashes {
    pub sha1: String,
    pub md5: String,
    pub sha256: String,
}

/// A binary attached to a software module (hawkBit `MgmtArtifact`).
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

/// Body of `POST /rest/v1/softwaremodules` (hawkBit
/// `MgmtSoftwareModuleRequestBodyPost`).
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

/// Body of `PUT /rest/v1/softwaremodules/{id}` — only vendor/description are
/// mutable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SmUpdate {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub vendor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
}

/// Bare `{ "id": N }` reference to a software module, used when assigning
/// modules into a distribution set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleRef {
    pub id: i64,
}

/// Body of `POST /rest/v1/softwaremoduletypes` (hawkBit
/// `MgmtSoftwareModuleTypeRequestBodyPost`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SmTypeCreate {
    pub key: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    /// Max modules of this type per distribution set (hawkBit default 1).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_assignments: Option<i32>,
}

/// Body of `PUT /rest/v1/softwaremoduletypes/{id}` — only description is mutable
/// (key/name are immutable in hawkBit).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SmTypeUpdate {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
}
