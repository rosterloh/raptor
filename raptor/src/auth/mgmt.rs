use crate::error::AppError;
use crate::state::AppState;
use argon2::password_hash::PasswordHash;
use argon2::{Argon2, PasswordVerifier};
use axum::extract::{Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use base64::Engine;

pub async fn mgmt_auth(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    // The SPA tags its own requests so a failed session check doesn't trigger
    // the browser's native Basic-Auth dialog (see raptor-ui/src/api.rs).
    let quiet = req.headers().contains_key("x-requested-with");
    let unauthorized = || {
        if quiet {
            AppError::UnauthorizedQuiet
        } else {
            AppError::Unauthorized
        }
    };

    if let Some(tok) = crate::auth::session::session_cookie(req.headers()) {
        if state.sessions.validate(&tok) {
            return Ok(next.run(req).await);
        }
    }

    let header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(unauthorized)?;
    let b64 = header.strip_prefix("Basic ").ok_or_else(unauthorized)?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|_| unauthorized())?;
    let decoded = String::from_utf8(decoded).map_err(|_| unauthorized())?;
    let (user, pass) = decoded.split_once(':').ok_or_else(unauthorized)?;

    if !verify_creds(&state.cfg.mgmt, user, pass) {
        return Err(unauthorized());
    }
    Ok(next.run(req).await)
}

pub fn verify_creds(cfg: &crate::config::MgmtConfig, user: &str, pass: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(&cfg.password_hash) else {
        return false;
    };
    user == cfg.username
        && Argon2::default()
            .verify_password(pass.as_bytes(), &parsed)
            .is_ok()
}
