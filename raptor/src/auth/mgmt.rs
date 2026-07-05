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
    let header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    let b64 = header
        .strip_prefix("Basic ")
        .ok_or(AppError::Unauthorized)?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|_| AppError::Unauthorized)?;
    let decoded = String::from_utf8(decoded).map_err(|_| AppError::Unauthorized)?;
    let (user, pass) = decoded.split_once(':').ok_or(AppError::Unauthorized)?;

    let cfg = &state.cfg.mgmt;
    let parsed = PasswordHash::new(&cfg.password_hash).map_err(|_| AppError::Unauthorized)?;
    let ok = user == cfg.username
        && Argon2::default()
            .verify_password(pass.as_bytes(), &parsed)
            .is_ok();
    if !ok {
        return Err(AppError::Unauthorized);
    }
    Ok(next.run(req).await)
}
