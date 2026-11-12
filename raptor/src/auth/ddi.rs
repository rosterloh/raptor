//! DDI auth middleware: accepts a `GatewayToken` (config-wide secret),
//! `TargetToken` (per-target `security_token`), proxy-verified certificate
//! headers when `[ddi.header_auth]` is on, or nothing when `ddi.anonymous` is
//! set. The resolved `AuthKind` is stashed as a request
//! extension for handlers that need to distinguish gateway from target auth.

use crate::entity::target;
use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{ConnectInfo, RawPathParams, Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AuthKind {
    Anonymous,
    Gateway,
    Target,
    /// Proxy-forwarded certificate identity (`[ddi.header_auth]`); like
    /// `Target`, it proves exactly one device.
    Certificate,
}

pub async fn ddi_auth(
    State(state): State<AppState>,
    params: RawPathParams,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    // Checked once here rather than per handler: raptor answers to exactly
    // one tenant (`[tenant]`, default `DEFAULT`), matching hawkBit's own
    // 404-on-unknown-tenant behaviour rather than silently folding every
    // segment into one fleet. Case-insensitive: Zephyr's
    // CONFIG_HAWKBIT_TENANT defaults to lowercase "default".
    let tenant = params.iter().find(|(k, _)| *k == "tenant").map(|(_, v)| v);
    if !tenant.is_some_and(|t| t.eq_ignore_ascii_case(&state.cfg.tenant)) {
        return Err(AppError::NotFound("tenant"));
    }

    let peer = req
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|ConnectInfo(p)| p.ip());
    let kind = match authenticate(&state, &params, req.headers(), peer).await {
        Ok(kind) => kind,
        Err(e) => {
            state.metrics.auth_failure("ddi");
            // The usual reason a device silently never updates.
            tracing::warn!(path = req.uri().path(), "ddi authentication failed");
            return Err(e);
        }
    };
    req.extensions_mut().insert(kind);
    Ok(next.run(req).await)
}

async fn authenticate(
    state: &AppState,
    params: &RawPathParams,
    headers: &axum::http::HeaderMap,
    peer: Option<std::net::IpAddr>,
) -> Result<AuthKind, AppError> {
    if state.cfg.ddi.anonymous {
        return Ok(AuthKind::Anonymous);
    }
    // Like hawkBit's filter chain: a header-auth miss falls through to tokens.
    let h = &state.cfg.ddi.header_auth;
    if h.enabled
        && (h.trusted_proxies.is_empty() || peer.is_some_and(|p| h.trusted_proxies.contains(&p)))
        && let Some(cid) = controller_id(params)
        && header_authenticates(h, headers, cid)
    {
        return Ok(AuthKind::Certificate);
    }
    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    if let Some(token) = auth.strip_prefix("GatewayToken ") {
        if state.cfg.ddi.gateway_token.as_deref() == Some(token) {
            Ok(AuthKind::Gateway)
        } else {
            Err(AppError::Unauthorized)
        }
    } else if let Some(token) = auth.strip_prefix("TargetToken ") {
        let cid = controller_id(params).ok_or(AppError::Unauthorized)?;
        let t = target::Entity::find()
            .filter(target::Column::ControllerId.eq(cid))
            .one(&state.db)
            .await?
            .ok_or(AppError::Unauthorized)?;
        if t.security_token == token {
            Ok(AuthKind::Target)
        } else {
            Err(AppError::Unauthorized)
        }
    } else {
        Err(AppError::Unauthorized)
    }
}

fn controller_id(params: &RawPathParams) -> Option<&str> {
    params
        .iter()
        .find(|(k, _)| *k == "controllerId")
        .map(|(_, v)| v)
}

/// hawkBit 1.2 `SecurityHeaderAuthenticator`: the controller-id header must
/// equal the path's id, and some authority header must name a trusted
/// authority — the bare header as a `,`/`;` list, else `<name>-0`, `-1`, ...
/// until one is missing (starting at 1 when `-0` is absent).
fn header_authenticates(
    h: &crate::config::HeaderAuthConfig,
    headers: &axum::http::HeaderMap,
    cid: &str,
) -> bool {
    let get = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    if get(&h.controller_id_header) != Some(cid) {
        return false;
    }
    let trusted = |a: &str| {
        let a = a.trim();
        !a.is_empty()
            && h.trusted_authorities
                .iter()
                .any(|t| t.trim().eq_ignore_ascii_case(a))
    };
    if let Some(list) = get(&h.authority_header) {
        return list.split([',', ';']).any(trusted);
    }
    let mut i = 0;
    loop {
        match get(&format!("{}-{i}", h.authority_header)) {
            Some(a) if trusted(a) => return true,
            Some(_) => {}
            None if i == 0 => {}
            None => return false,
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    fn cfg() -> crate::config::HeaderAuthConfig {
        crate::config::HeaderAuthConfig {
            enabled: true,
            trusted_authorities: vec!["ca-a".into()],
            ..Default::default()
        }
    }

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut m = HeaderMap::new();
        for (k, v) in pairs {
            m.insert(*k, v.parse().expect("valid header value"));
        }
        m
    }

    #[test]
    fn list_header_any_entry_case_insensitive() {
        let h = headers(&[("x-controller-id", "dev-1"), ("x-authority", "ca-x; CA-A")]);
        assert!(header_authenticates(&cfg(), &h, "dev-1"));
    }

    #[test]
    fn controller_id_must_match_exactly() {
        let h = headers(&[("x-controller-id", "dev-2"), ("x-authority", "ca-a")]);
        assert!(!header_authenticates(&cfg(), &h, "dev-1"));
    }

    #[test]
    fn indexed_headers_from_zero_or_one() {
        let from0 = headers(&[
            ("x-controller-id", "dev-1"),
            ("x-authority-0", "ca-x"),
            ("x-authority-1", "ca-a"),
        ]);
        assert!(header_authenticates(&cfg(), &from0, "dev-1"));
        let from1 = headers(&[("x-controller-id", "dev-1"), ("x-authority-1", "ca-a")]);
        assert!(header_authenticates(&cfg(), &from1, "dev-1"));
        // A gap ends the scan, as upstream.
        let gap = headers(&[("x-controller-id", "dev-1"), ("x-authority-2", "ca-a")]);
        assert!(!header_authenticates(&cfg(), &gap, "dev-1"));
    }

    #[test]
    fn blank_authority_never_matches() {
        let mut c = cfg();
        c.trusted_authorities.push(String::new());
        let h = headers(&[("x-controller-id", "dev-1"), ("x-authority", "")]);
        assert!(!header_authenticates(&c, &h, "dev-1"));
    }
}
