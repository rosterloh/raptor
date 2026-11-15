//! In-memory `SessionStore` for the mgmt UI's cookie session: random tokens
//! with sliding idle expiry. By design there is no persistence — a restart
//! logs every session out.

use axum::http::HeaderMap;
use std::collections::HashMap;
use std::sync::Mutex;

use super::principal::Principal;

pub const COOKIE: &str = "raptor_session";

/// Sliding idle expiry. In-memory by design: restart logs everyone out.
const IDLE_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Default)]
pub struct SessionStore(Mutex<HashMap<String, (Principal, i64)>>);

impl SessionStore {
    pub fn create(&self, principal: Principal) -> String {
        use rand::Rng;
        let mut b = [0u8; 32];
        rand::rng().fill_bytes(&mut b);
        let token = hex::encode(b);
        let now = crate::util::now_ms();
        let mut m = self.0.lock().expect("lock poisoned by an earlier panic");
        m.retain(|_, (_, exp)| *exp > now);
        m.insert(token.clone(), (principal, now + IDLE_MS));
        token
    }

    /// The session's principal, sliding its expiry; `None` if unknown or expired.
    pub fn validate(&self, token: &str) -> Option<Principal> {
        let now = crate::util::now_ms();
        let mut m = self.0.lock().expect("lock poisoned by an earlier panic");
        match m.get_mut(token) {
            Some((p, exp)) if *exp > now => {
                *exp = now + IDLE_MS;
                Some(p.clone())
            }
            Some(_) => {
                m.remove(token);
                None
            }
            None => None,
        }
    }

    pub fn remove(&self, token: &str) {
        self.0
            .lock()
            .expect("lock poisoned by an earlier panic")
            .remove(token);
    }

    /// Ends every session of `username`, after a delete, disable or role change.
    pub fn remove_user(&self, username: &str) {
        self.0
            .lock()
            .expect("lock poisoned by an earlier panic")
            .retain(|_, (p, _)| p.username != username);
    }
}

pub fn session_cookie(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    raw.split(';')
        .map(str::trim)
        .find_map(|kv| kv.strip_prefix("raptor_session=").map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::auth::principal::{Role, Source};

    fn who(name: &str) -> Principal {
        Principal {
            username: name.into(),
            role: Role::Admin,
            source: Source::Local,
        }
    }

    #[test]
    fn create_then_validate() {
        let s = SessionStore::default();
        let tok = s.create(who("a"));
        assert_eq!(tok.len(), 64); // 32 bytes hex
        assert_eq!(s.validate(&tok), Some(who("a")));
        assert_eq!(s.validate("nope"), None);
    }

    #[test]
    fn remove_invalidates() {
        let s = SessionStore::default();
        let tok = s.create(who("a"));
        s.remove(&tok);
        assert_eq!(s.validate(&tok), None);
    }

    #[test]
    fn remove_user_ends_only_their_sessions() {
        let s = SessionStore::default();
        let (a1, a2, b) = (s.create(who("a")), s.create(who("a")), s.create(who("b")));
        s.remove_user("a");
        assert_eq!(s.validate(&a1), None);
        assert_eq!(s.validate(&a2), None);
        assert!(s.validate(&b).is_some());
    }

    #[test]
    fn cookie_header_parsed() {
        let mut h = axum::http::HeaderMap::new();
        h.insert(
            axum::http::header::COOKIE,
            "other=1; raptor_session=abc123; x=2".parse().unwrap(),
        );
        assert_eq!(session_cookie(&h).as_deref(), Some("abc123"));
        let empty = axum::http::HeaderMap::new();
        assert_eq!(session_cookie(&empty), None);
    }
}
