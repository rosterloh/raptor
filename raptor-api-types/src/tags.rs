//! Tag resource: create/update bodies shared by target tags and
//! distribution-set tags (hawkBit `MgmtTag`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A target or distribution-set tag (hawkBit `MgmtTag`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TagRest {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    /// Display colour, hawkBit's British spelling.
    pub colour: Option<String>,
    pub created_at: i64,
    pub last_modified_at: i64,
    /// How many targets (or distribution sets) carry this tag (raptor
    /// extension, not in hawkBit's `MgmtTag`).
    pub assigned_count: i64,
    #[serde(rename = "_links", default)]
    pub links: Value,
}

/// Compact reference to a tag, for embedding in a list payload. [`TagRest`]'s
/// timestamps, description and `_links` are noise when repeated for every tag of
/// every row; a name and a colour are all a chip needs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TagRef {
    pub id: i64,
    pub name: String,
    /// Display colour, hawkBit's British spelling. Operator-supplied free text,
    /// so a consumer must validate it before putting it anywhere near CSS.
    pub colour: Option<String>,
}

/// One element of `POST /rest/v1/targettags` or `/rest/v1/distributionsettags`
/// (hawkBit `MgmtTagRequestBodyPut`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TagCreate {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    /// Display colour, hawkBit's British spelling.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub colour: Option<String>,
}

/// Body of `PUT /rest/v1/targettags/{id}` / `/rest/v1/distributionsettags/{id}`.
/// Omitted fields are left unchanged.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TagUpdate {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub colour: Option<String>,
}
