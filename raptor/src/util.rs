use crate::config::Config;
use axum::http::HeaderMap;

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn random_token() -> String {
    use rand::RngCore;
    let mut b = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}

/// Base URL for _links: config `url` wins, else derived from Host header.
pub fn base_url(cfg: &Config, headers: &HeaderMap) -> String {
    if let Some(u) = &cfg.url {
        return u.trim_end_matches('/').to_string();
    }
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    format!("{proto}://{host}")
}
