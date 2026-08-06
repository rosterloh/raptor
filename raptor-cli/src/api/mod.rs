//! One module per Management API resource, mirroring `raptor-ui/src/api/`.
//! Each function is a thin wrapper: build the path/query, call `Client`,
//! return the already-typed `raptor-api-types` DTO.

pub mod actions;
pub mod distribution_sets;
pub mod modules;
pub mod rollouts;
pub mod system;
pub mod tags;
pub mod targets;

/// Shared list-query flags, mirroring `raptor::api::paging::ListParams`.
#[derive(Debug, Clone, Default)]
pub struct ListArgs {
    pub q: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

impl ListArgs {
    /// Build a `?a=b&c=d` query string (empty string if nothing is set).
    pub fn query_string(&self) -> String {
        let mut parts = Vec::new();
        if let Some(q) = &self.q {
            parts.push(format!("q={}", urlencode(q)));
        }
        if let Some(s) = &self.sort {
            parts.push(format!("sort={}", urlencode(s)));
        }
        if let Some(l) = self.limit {
            parts.push(format!("limit={l}"));
        }
        if let Some(o) = self.offset {
            parts.push(format!("offset={o}"));
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("?{}", parts.join("&"))
        }
    }
}

/// Minimal percent-encoding for query values — no external dependency for
/// the handful of characters FIQL/sort strings actually use.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
