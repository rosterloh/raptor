//! The signed-in operator, from `/rest/v1/userinfo`. `Shell` provides it; the
//! hooks here decide which controls a page draws. The server enforces the same
//! rules, so a hidden control is a courtesy, not the protection.

use dioxus::prelude::*;
use raptor_api_types::UserInfo;

#[derive(Clone, Copy)]
pub struct Me(pub Signal<Option<UserInfo>>);

impl Me {
    pub fn provide() -> Self {
        use_context_provider(|| Me(Signal::new(None)))
    }
}

fn me() -> Option<UserInfo> {
    try_use_context::<Me>().and_then(|m| m.0.read().clone())
}

/// Whether the operator may change anything: any permission beyond `READ_*`.
pub fn use_can_write() -> bool {
    me().is_some_and(|u| u.permissions.iter().any(|p| !p.starts_with("READ_")))
}

/// Whether the operator may manage users.
pub fn use_is_admin() -> bool {
    me().is_some_and(|u| u.role == "admin")
}
