use crate::entity::target;
use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{RawPathParams, Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AuthKind {
    Anonymous,
    Gateway,
    Target,
}

pub async fn ddi_auth(
    State(state): State<AppState>,
    params: RawPathParams,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let kind = if state.cfg.ddi.anonymous {
        AuthKind::Anonymous
    } else {
        let auth = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized)?;

        if let Some(token) = auth.strip_prefix("GatewayToken ") {
            if state.cfg.ddi.gateway_token.as_deref() == Some(token) {
                AuthKind::Gateway
            } else {
                return Err(AppError::Unauthorized);
            }
        } else if let Some(token) = auth.strip_prefix("TargetToken ") {
            let cid = params
                .iter()
                .find(|(k, _)| *k == "controllerId")
                .map(|(_, v)| v.to_string())
                .ok_or(AppError::Unauthorized)?;
            let t = target::Entity::find()
                .filter(target::Column::ControllerId.eq(cid))
                .one(&state.db)
                .await?
                .ok_or(AppError::Unauthorized)?;
            if t.security_token == token {
                AuthKind::Target
            } else {
                return Err(AppError::Unauthorized);
            }
        } else {
            return Err(AppError::Unauthorized);
        }
    };
    req.extensions_mut().insert(kind);
    Ok(next.run(req).await)
}
