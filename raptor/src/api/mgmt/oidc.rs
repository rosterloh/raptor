//! Console single sign-on: `/rest/v1/oidc/login` and `/rest/v1/oidc/callback`.
//! Merged by `app::build_app` outside the `mgmt_auth` gate; both answer 404
//! when `[mgmt.oidc]` is not configured.

use crate::auth::oidc::Oidc;
use crate::error::AppError;
use crate::state::AppState;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use std::collections::HashMap;
use std::sync::Arc;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/rest/v1/oidc/login", get(login))
        .route("/rest/v1/oidc/callback", get(callback))
}

fn configured(st: &AppState) -> Result<&Arc<Oidc>, AppError> {
    st.oidc.as_ref().ok_or(AppError::NotFound("oidc login"))
}

/// Redirects to the provider. `?probe` answers 204 instead, so the console
/// can tell whether to offer SSO without starting a login.
async fn login(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response, AppError> {
    let oidc = configured(&st)?;
    if q.contains_key("probe") {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    let url = oidc
        .begin_login()
        .ok_or_else(|| AppError::Unavailable("oidc provider unavailable".into()))?;
    Ok(Redirect::to(&url).into_response())
}

async fn callback(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response, AppError> {
    let oidc = configured(&st)?;
    let pending = q
        .get("state")
        .and_then(|s| oidc.take_pending(s))
        .ok_or(AppError::Unauthorized)?;
    let code = q.get("code").ok_or_else(|| {
        tracing::warn!(
            error = q.get("error").map(String::as_str),
            "oidc login failed"
        );
        AppError::Unauthorized
    })?;
    let id_token = oidc.exchange(code, &pending.verifier).await?;
    let claims = oidc
        .verify(&id_token, &oidc.cfg.client_id, Some(&pending.nonce))
        .await
        .inspect_err(|e| tracing::warn!(reason = ?e, "oidc id token refused"))?;
    let (username, role) = oidc.identity(&claims)?;
    let principal = crate::auth::oidc::resolve_user(&st, &username, role).await?;
    let cookie = super::login::session_set_cookie(&st, &headers, principal);
    Ok((
        StatusCode::SEE_OTHER,
        [
            (header::SET_COOKIE, cookie),
            (header::LOCATION, "/ui".into()),
        ],
    )
        .into_response())
}
