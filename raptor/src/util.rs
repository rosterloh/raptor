//! Small stateless helpers shared across handlers: timestamps, random tokens,
//! and base-URL resolution for `_links`. No domain logic.

use crate::config::Config;
use axum::http::HeaderMap;

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn random_token() -> String {
    use rand::Rng;
    let mut b = [0u8; 16];
    rand::rng().fill_bytes(&mut b);
    hex::encode(b)
}

/// The device's source address, for `target.address`. Uses the configured
/// trusted proxy header when raptor sits behind a proxy, else the socket peer.
pub fn client_address(
    cfg: &Config,
    headers: &HeaderMap,
    peer: Option<std::net::SocketAddr>,
) -> Option<String> {
    if let Some(name) = &cfg.ddi.trusted_proxy_header {
        if let Some(ip) = headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(forwarded_ip)
        {
            return Some(ip);
        }
    }
    peer.map(|p| p.ip().to_string())
}

/// The *rightmost* entry of a forwarded-for header: the hop appended by the
/// proxy directly in front of raptor. Anything to its left was supplied by the
/// caller and so can be spoofed by the device itself. Handles both
/// `X-Forwarded-For` lists and RFC 7239 `Forwarded` (`for=...`) syntax, with or
/// without a port; anything that isn't a parseable IP is ignored.
fn forwarded_ip(header: &str) -> Option<String> {
    let last = header.rsplit(',').next()?.trim();
    // `Forwarded: for=1.2.3.4;proto=https` -> `1.2.3.4`
    let token = last
        .split(';')
        .find_map(|p| p.trim().strip_prefix("for="))
        .unwrap_or(last)
        .trim()
        .trim_matches('"');
    token
        .parse::<std::net::IpAddr>()
        .or_else(|_| token.parse::<std::net::SocketAddr>().map(|s| s.ip()))
        .ok()
        .map(|ip| ip.to_string())
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

#[cfg(test)]
mod tests {
    use super::forwarded_ip;

    #[test]
    fn forwarded_ip_takes_the_proxy_appended_hop() {
        // leftmost entries are device-supplied; the last one is the real peer
        assert_eq!(
            forwarded_ip("203.0.113.9, 10.0.0.5").as_deref(),
            Some("10.0.0.5")
        );
        assert_eq!(forwarded_ip("10.0.0.5").as_deref(), Some("10.0.0.5"));
        // RFC 7239 syntax, quoted values, ports, IPv6
        assert_eq!(
            forwarded_ip("for=192.0.2.60;proto=http").as_deref(),
            Some("192.0.2.60")
        );
        assert_eq!(
            forwarded_ip("for=\"[2001:db8::1]:4711\"").as_deref(),
            Some("2001:db8::1")
        );
        assert_eq!(forwarded_ip("10.0.0.5:9000").as_deref(), Some("10.0.0.5"));
        // junk is ignored rather than stored
        assert_eq!(forwarded_ip("unknown"), None);
        assert_eq!(forwarded_ip(""), None);
        assert_eq!(forwarded_ip("<script>"), None);
    }
}
