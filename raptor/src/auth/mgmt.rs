//! Management API auth middleware: resolves a session cookie or HTTP Basic
//! auth (the `[mgmt]` config admin, then enabled local users) to a
//! [`Principal`] request extension, then applies the role gate.

use crate::auth::principal::{Principal, Role, Source};
use crate::entity::user as users;
use crate::error::AppError;
use crate::state::AppState;
use argon2::{Argon2, PasswordVerifier};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, header};
use axum::middleware::Next;
use axum::response::Response;
use base64::Engine;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

pub async fn mgmt_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let req_path = req.uri().path().to_string();
    let principal = match check_auth(&state, req.headers()).await {
        Ok(p) => p,
        Err(e) => {
            state.metrics.auth_failure("mgmt");
            // The console's own session probe (quiet) fails on every
            // logged-out page load; that is not worth a warning.
            if !matches!(e, AppError::UnauthorizedQuiet) {
                tracing::warn!(path = req_path, "mgmt authentication failed");
            }
            return Err(e);
        }
    };
    if !permitted(principal.role, req.method(), &req_path) {
        tracing::info!(
            user = principal.username,
            method = %req.method(),
            path = req_path,
            "mgmt request denied by role"
        );
        return Err(AppError::Forbidden("insufficient permission".into()));
    }
    req.extensions_mut().insert(principal);
    Ok(next.run(req).await)
}

/// The role gate: user administration is admin-only; otherwise a role that
/// cannot write gets `GET`/`HEAD` only. Every route is covered by default.
pub fn permitted(role: Role, method: &Method, path: &str) -> bool {
    if path == "/rest/v1/users" || path.starts_with("/rest/v1/users/") {
        return role == Role::Admin;
    }
    role.can_write() || method == Method::GET || method == Method::HEAD
}

async fn check_auth(state: &AppState, headers: &HeaderMap) -> Result<Principal, AppError> {
    // The SPA tags its own requests so a failed session check doesn't trigger
    // the browser's native Basic-Auth dialog (see raptor-ui/src/api.rs).
    let quiet = headers.contains_key("x-requested-with");
    let unauthorized = || {
        if quiet {
            AppError::UnauthorizedQuiet
        } else {
            AppError::Unauthorized
        }
    };

    if let Some(tok) = crate::auth::session::session_cookie(headers)
        && let Some(p) = state.sessions.validate(&tok)
    {
        return Ok(p);
    }

    let header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(unauthorized)?;
    let b64 = header.strip_prefix("Basic ").ok_or_else(unauthorized)?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|_| unauthorized())?;
    let decoded = String::from_utf8(decoded).map_err(|_| unauthorized())?;
    let (user, pass) = decoded.split_once(':').ok_or_else(unauthorized)?;

    authenticate(state, user, pass)
        .await?
        .ok_or_else(unauthorized)
}

/// Password check for Basic auth and `POST /rest/v1/login`: the config admin
/// first (its name is reserved, so it never falls through to the table), then
/// enabled local users.
pub async fn authenticate(
    state: &AppState,
    user: &str,
    pass: &str,
) -> Result<Option<Principal>, AppError> {
    if user == state.cfg.mgmt.username {
        return Ok(
            verify_creds(&state.cfg.mgmt, user, pass).then(|| Principal {
                username: user.to_string(),
                role: Role::Admin,
                source: Source::Config,
            }),
        );
    }
    let Some(row) = users::Entity::find()
        .filter(users::Column::Username.eq(user))
        .filter(users::Column::Source.eq(Source::Local.as_str()))
        .filter(users::Column::Enabled.eq(true))
        .one(&state.db)
        .await?
    else {
        return Ok(None);
    };
    let (Some(hash), Some(role)) = (row.password_hash.as_deref(), Role::parse(&row.role)) else {
        return Ok(None);
    };
    Ok(verify_hash(hash, pass).then_some(Principal {
        username: row.username,
        role,
        source: Source::Local,
    }))
}

pub fn verify_creds(cfg: &crate::config::MgmtConfig, user: &str, pass: &str) -> bool {
    user == cfg.username && verify_hash(&cfg.password_hash, pass)
}

fn verify_hash(hash: &str, pass: &str) -> bool {
    // argon2 0.6 verifies straight from the PHC string, so an unparseable hash
    // falls out as a verification error rather than needing its own branch —
    // either way, nobody authenticates against it.
    Argon2::default()
        .verify_password(pass.as_bytes(), hash)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_gets_reads_but_never_user_admin() {
        let ro = Role::ReadOnly;
        assert!(permitted(ro, &Method::GET, "/rest/v1/targets"));
        assert!(permitted(ro, &Method::HEAD, "/rest/v1/targets"));
        assert!(!permitted(ro, &Method::POST, "/rest/v1/targets"));
        assert!(!permitted(ro, &Method::DELETE, "/rest/v1/targets/x"));
        assert!(!permitted(ro, &Method::GET, "/rest/v1/users"));
        assert!(!permitted(ro, &Method::GET, "/rest/v1/users/1"));
        assert!(permitted(ro, &Method::GET, "/rest/v1/usersettings"));
        assert!(permitted(Role::Admin, &Method::DELETE, "/rest/v1/users/1"));
    }
}
