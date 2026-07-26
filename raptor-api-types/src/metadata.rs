//! Metadata resource: key/value metadata entries attached to targets,
//! distribution sets, and software modules (hawkBit `MgmtMetadata`).

use serde::{Deserialize, Serialize};

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
