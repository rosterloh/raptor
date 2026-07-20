use super::deployment::{deployment_json_keyed, find_target_action};
use crate::auth::ddi::AuthKind;
use crate::entity::target;
use crate::error::AppError;
use crate::state::AppState;
use crate::util::{base_url, now_ms};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
pub struct ConfirmationFeedback {
    /// "confirmed" or "denied".
    pub confirmation: String,
    #[serde(default)]
    pub details: Vec<String>,
}

/// `GET .../confirmationBase/{actionId}` — the action awaiting confirmation,
/// rendered like a deploymentBase but under a `confirmation` key.
pub async fn confirmation_base(
    State(st): State<AppState>,
    Extension(_auth): Extension<AuthKind>,
    headers: axum::http::HeaderMap,
    Path((_tenant, cid, action_id)): Path<(String, String, i64)>,
) -> Result<Json<Value>, AppError> {
    let (_t, a) = find_target_action(&st, &cid, action_id).await?;
    if !a.active || a.status != "wait_for_confirmation" {
        return Err(AppError::NotFound("action"));
    }
    let base = base_url(&st.cfg, &headers);
    Ok(Json(
        deployment_json_keyed(&st, &cid, &a, &base, "confirmation").await?,
    ))
}

/// `POST .../confirmationBase/{actionId}/feedback` — confirm or deny a waiting action.
pub async fn confirmation_feedback(
    State(st): State<AppState>,
    Extension(_auth): Extension<AuthKind>,
    Path((_tenant, cid, action_id)): Path<(String, String, i64)>,
    Json(fb): Json<ConfirmationFeedback>,
) -> Result<StatusCode, AppError> {
    let (_t, a) = find_target_action(&st, &cid, action_id).await?;
    if !a.active {
        return Err(AppError::Gone);
    }
    match fb.confirmation.as_str() {
        "confirmed" => {
            crate::domain::deployment::confirm_action(&st, &a, &fb.details).await?;
        }
        "denied" => {
            crate::domain::deployment::deny_action(&st, &a, &fb.details).await?;
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "invalid confirmation value: {other}"
            )));
        }
    }
    Ok(StatusCode::OK)
}

async fn set_auto_confirm(st: &AppState, cid: &str, on: bool) -> Result<target::Model, AppError> {
    let t = target::Entity::find()
        .filter(target::Column::ControllerId.eq(cid))
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("target"))?;
    let target_id = t.id;
    let mut am: target::ActiveModel = t.into();
    am.auto_confirm = Set(on);
    am.updated_at = Set(now_ms());
    let t = am.update(&st.db).await?;
    if on {
        crate::domain::deployment::confirm_waiting_actions(st, target_id).await?;
    }
    Ok(t)
}

/// `POST .../confirmationBase/activateAutoConfirm` — device opts into auto-confirm.
pub async fn activate_auto_confirm(
    State(st): State<AppState>,
    Extension(_auth): Extension<AuthKind>,
    Path((_tenant, cid)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    set_auto_confirm(&st, &cid, true).await?;
    Ok(StatusCode::OK)
}

/// `POST .../confirmationBase/deactivateAutoConfirm` — device opts back out.
pub async fn deactivate_auto_confirm(
    State(st): State<AppState>,
    Extension(_auth): Extension<AuthKind>,
    Path((_tenant, cid)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    set_auto_confirm(&st, &cid, false).await?;
    Ok(StatusCode::OK)
}
