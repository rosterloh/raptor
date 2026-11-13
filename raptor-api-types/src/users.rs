//! Operator users (`/rest/v1/users`, raptor extension) and hawkBit's
//! `MgmtUserInfo` (`/rest/v1/userinfo`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An operator account. Never carries a password or hash.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserRest {
    pub id: i64,
    pub username: String,
    /// `admin` or `read_only`.
    pub role: String,
    /// `local` or `oidc`.
    pub source: String,
    pub enabled: bool,
    pub created_at: i64,
    pub last_modified_at: i64,
    #[serde(rename = "_links", default)]
    pub links: Value,
}

/// Body of `POST /rest/v1/users`: always a local user.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserCreate {
    pub username: String,
    pub password: String,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub enabled: Option<bool>,
}

/// Body of `PUT /rest/v1/users/{id}`; absent fields are left unchanged.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserUpdate {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub password: Option<String>,
}

/// hawkBit `MgmtUserInfo`, plus raptor's `role`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub tenant: String,
    pub username: String,
    pub permissions: Vec<String>,
    pub role: String,
}
