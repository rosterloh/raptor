use axum::http::HeaderMap;
use std::collections::HashMap;
use std::sync::Mutex;

pub const COOKIE: &str = "raptor_session";

/// Sliding idle expiry. In-memory by design: restart logs everyone out.
const IDLE_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Default)]
pub struct SessionStore(Mutex<HashMap<String, i64>>);

impl SessionStore {
    pub fn create(&self) -> String {
        use rand::RngCore;
        let mut b = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut b);
        let token = hex::encode(b);
        let now = crate::util::now_ms();
        let mut m = self.0.lock().unwrap();
        m.retain(|_, exp| *exp > now);
        m.insert(token.clone(), now + IDLE_MS);
        token
    }

    pub fn validate(&self, token: &str) -> bool {
        let now = crate::util::now_ms();
        let mut m = self.0.lock().unwrap();
        match m.get_mut(token) {
            Some(exp) if *exp > now => {
                *exp = now + IDLE_MS;
                true
            }
            Some(_) => {
                m.remove(token);
                false
            }
            None => false,
        }
    }

    pub fn remove(&self, token: &str) {
        self.0.lock().unwrap().remove(token);
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

    #[test]
    fn create_then_validate() {
        let s = SessionStore::default();
        let tok = s.create();
        assert_eq!(tok.len(), 64); // 32 bytes hex
        assert!(s.validate(&tok));
        assert!(!s.validate("nope"));
    }

    #[test]
    fn remove_invalidates() {
        let s = SessionStore::default();
        let tok = s.create();
        s.remove(&tok);
        assert!(!s.validate(&tok));
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
