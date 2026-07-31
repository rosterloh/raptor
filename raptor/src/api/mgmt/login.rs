//! Session login/logout (`/rest/v1/login`, `/rest/v1/logout`). Deliberately
//! NOT merged into `mgmt::router()`'s auth-gated `Router` — these two routes
//! must stay reachable without a session, so `app::build_app` merges
//! [`routes`] directly, ahead of the `mgmt_auth` layer.
//!
//! [`gated_routes`] is the exception: `/rest/v1/session` only means anything
//! behind the auth layer, so `mgmt::router()` merges that one instead.

use crate::auth::session::{COOKIE, session_cookie};
use crate::error::AppError;
use crate::state::AppState;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use raptor_api_types::LoginRequest;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/rest/v1/login", post(login))
        .route("/rest/v1/logout", post(logout))
}

/// Session routes that belong *behind* `mgmt_auth`, merged by [`super::router`].
pub fn gated_routes() -> Router<AppState> {
    Router::new().route("/rest/v1/session", axum::routing::get(session))
}

/// "Is my session still valid?" — for the web console to ask before it paints
/// a logged-in shell. Reaching this handler at all means `mgmt_auth` accepted
/// the request, so it answers unconditionally; the informative response is the
/// 401 the middleware returns in its place.
pub async fn session() -> StatusCode {
    StatusCode::NO_CONTENT
}

pub async fn login(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> Result<Response, AppError> {
    if !crate::auth::mgmt::verify_creds(&st.cfg.mgmt, &req.username, &req.password) {
        let body = raptor_api_types::ErrorBody {
            exception_class: "org.springframework.security.authentication.BadCredentialsException"
                .into(),
            error_code: "hawkbit.server.error.unauthorized".into(),
            message: "unauthorized".into(),
        };
        return Ok((StatusCode::UNAUTHORIZED, Json(body)).into_response());
    }
    let token = st.sessions.create();
    let secure = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        == Some("https");
    let mut cookie = format!("{COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/");
    if secure {
        cookie.push_str("; Secure");
    }
    Ok((StatusCode::NO_CONTENT, [(header::SET_COOKIE, cookie)]).into_response())
}

pub async fn logout(State(st): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(tok) = session_cookie(&headers) {
        st.sessions.remove(&tok);
    }
    let secure = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        == Some("https");
    let mut clear = format!("{COOKIE}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0");
    if secure {
        clear.push_str("; Secure");
    }
    (StatusCode::NO_CONTENT, [(header::SET_COOKIE, clear)]).into_response()
}
