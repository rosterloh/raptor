use super::deployment::find_target_action;
use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub struct Feedback {
    pub status: FeedbackStatus,
}

#[derive(Deserialize)]
pub struct FeedbackStatus {
    pub execution: String,
    #[serde(default)]
    pub result: FeedbackResult,
    #[serde(default)]
    pub details: Vec<String>,
}

#[derive(Deserialize, Default)]
pub struct FeedbackResult {
    #[serde(default = "none_str")]
    pub finished: String,
}

fn none_str() -> String { "none".into() }

pub async fn deployment_feedback(
    State(st): State<AppState>,
    Extension(_auth): Extension<crate::auth::ddi::AuthKind>,
    Path((_tenant, cid, action_id)): Path<(String, String, i64)>,
    Json(fb): Json<Feedback>,
) -> Result<StatusCode, AppError> {
    let (t, a) = find_target_action(&st, &cid, action_id).await?;
    if !a.active {
        return Err(AppError::Gone);
    }
    crate::domain::deployment::apply_feedback(&st, &t, &a, &fb.status.execution, &fb.status.result.finished, &fb.status.details).await?;
    Ok(StatusCode::OK)
}

pub async fn cancel_action(
    State(st): State<AppState>,
    Extension(_auth): Extension<crate::auth::ddi::AuthKind>,
    Path((_tenant, cid, action_id)): Path<(String, String, i64)>,
) -> Result<Json<Value>, AppError> {
    let (_t, a) = find_target_action(&st, &cid, action_id).await?;
    if !a.active || a.status != "canceling" {
        return Err(AppError::NotFound("cancel action"));
    }
    Ok(Json(json!({"id": a.id.to_string(), "cancelAction": {"stopId": a.id.to_string()}})))
}

pub async fn cancel_feedback(
    State(st): State<AppState>,
    Extension(_auth): Extension<crate::auth::ddi::AuthKind>,
    Path((_tenant, cid, action_id)): Path<(String, String, i64)>,
    Json(fb): Json<Feedback>,
) -> Result<StatusCode, AppError> {
    let (t, a) = find_target_action(&st, &cid, action_id).await?;
    if !a.active {
        return Err(AppError::Gone);
    }
    crate::domain::deployment::apply_cancel_feedback(&st, &t, &a, &fb.status.execution, &fb.status.details).await?;
    Ok(StatusCode::OK)
}
