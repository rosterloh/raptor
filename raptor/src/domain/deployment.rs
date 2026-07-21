use crate::entity::{action, action_status, action_status_message, distribution_set, target};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::now_ms;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};

pub struct AssignResult {
    pub action_id: Option<i64>,
    pub already_assigned: bool,
}

pub async fn add_action_status(
    db: &DatabaseConnection,
    action_id: i64,
    status: &str,
    messages: &[String],
) -> Result<(), AppError> {
    let row = action_status::ActiveModel {
        action_id: Set(action_id),
        status: Set(status.to_string()),
        created_at: Set(now_ms()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    for m in messages {
        action_status_message::ActiveModel {
            action_status_id: Set(row.id),
            message: Set(m.clone()),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

pub async fn active_action(
    db: &DatabaseConnection,
    target_id: i64,
) -> Result<Option<action::Model>, AppError> {
    Ok(action::Entity::find()
        .filter(action::Column::TargetId.eq(target_id))
        .filter(action::Column::Active.eq(true))
        .one(db)
        .await?)
}

#[tracing::instrument(skip_all, fields(target_id = target.id, ds_id, forced))]
pub async fn assign_ds(
    st: &AppState,
    target: &target::Model,
    ds_id: i64,
    forced: bool,
) -> Result<AssignResult, AppError> {
    let ds = distribution_set::Entity::find_by_id(ds_id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set"))?;
    if !ds.complete {
        return Err(AppError::BadRequest(
            "distribution set is incomplete".into(),
        ));
    }
    if let Some(current) = active_action(&st.db, target.id).await? {
        if current.ds_id == ds.id {
            return Ok(AssignResult {
                action_id: None,
                already_assigned: true,
            });
        }
        // v1: hard-cancel the superseded action (hawkBit soft-cancels; devices tolerate both)
        let cid = current.id;
        let mut am: action::ActiveModel = current.into();
        am.status = Set("canceled".into());
        am.active = Set(false);
        am.updated_at = Set(now_ms());
        am.update(&st.db).await?;
        add_action_status(
            &st.db,
            cid,
            "canceled",
            &["superseded by new assignment".into()],
        )
        .await?;
    }
    // With the confirmation flow enabled, an assignment waits for confirmation
    // before becoming an active deployment — unless the target has auto-confirm on.
    let initial = if st.cfg.ddi.confirmation_flow && !target.auto_confirm {
        "wait_for_confirmation"
    } else {
        "running"
    };
    let now = now_ms();
    let a = action::ActiveModel {
        target_id: Set(target.id),
        ds_id: Set(ds.id),
        status: Set(initial.into()),
        active: Set(true),
        forced: Set(forced),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&st.db)
    .await?;
    add_action_status(&st.db, a.id, initial, &[]).await?;
    st.metrics.action_created();

    let mut tm: target::ActiveModel = target.clone().into();
    tm.assigned_ds_id = Set(Some(ds.id));
    tm.update_status = Set("pending".into());
    tm.updated_at = Set(now);
    tm.update(&st.db).await?;

    Ok(AssignResult {
        action_id: Some(a.id),
        already_assigned: false,
    })
}

#[tracing::instrument(skip_all, fields(action_id = a.id, execution, finished))]
pub async fn apply_feedback(
    st: &AppState,
    t: &target::Model,
    a: &action::Model,
    execution: &str,
    finished: &str,
    details: &[String],
) -> Result<(), AppError> {
    add_action_status(&st.db, a.id, execution, details).await?;
    match (execution, finished) {
        ("closed", "failure") => {
            set_action(st, a, "error", false).await?;
            set_target_status(st, t, None, "error").await?;
            st.metrics.action_failed();
        }
        ("closed", _) => {
            set_action(st, a, "finished", false).await?;
            set_target_status(st, t, Some(a.ds_id), "in_sync").await?;
            st.metrics.action_finished();
        }
        ("canceled", _) => {
            set_action(st, a, "canceled", false).await?;
            let status = if t.installed_ds_id.is_some() {
                "in_sync"
            } else {
                "registered"
            };
            set_target_status(st, t, None, status).await?;
            st.metrics.action_canceled();
        }
        _ => {} // proceeding/download/downloaded/resumed/scheduled/rejected: history only
    }
    Ok(())
}

pub async fn apply_cancel_feedback(
    st: &AppState,
    t: &target::Model,
    a: &action::Model,
    execution: &str,
    details: &[String],
) -> Result<(), AppError> {
    add_action_status(&st.db, a.id, &format!("cancel_{execution}"), details).await?;
    match execution {
        "closed" => {
            set_action(st, a, "canceled", false).await?;
            let status = if t.installed_ds_id.is_some() {
                "in_sync"
            } else {
                "registered"
            };
            // assigned reverts to installed on confirmed cancel
            let mut tm: target::ActiveModel = t.clone().into();
            tm.assigned_ds_id = Set(t.installed_ds_id);
            tm.update_status = Set(status.into());
            tm.updated_at = Set(now_ms());
            tm.update(&st.db).await?;
            st.metrics.action_canceled();
        }
        "rejected" => {
            set_action(st, a, "running", true).await?;
        }
        _ => {}
    }
    Ok(())
}

/// Confirms a waiting action, transitioning it to `running` so the next poll
/// yields a deploymentBase link.
pub async fn confirm_action(
    st: &AppState,
    a: &action::Model,
    details: &[String],
) -> Result<(), AppError> {
    if a.status != "wait_for_confirmation" {
        return Err(AppError::BadRequest(
            "action is not waiting for confirmation".into(),
        ));
    }
    add_action_status(&st.db, a.id, "confirmed", details).await?;
    set_action(st, a, "running", true).await?;
    Ok(())
}

/// Records a denial; the action stays in `wait_for_confirmation` (device may
/// confirm later, or an operator can cancel it).
pub async fn deny_action(
    st: &AppState,
    a: &action::Model,
    details: &[String],
) -> Result<(), AppError> {
    if a.status != "wait_for_confirmation" {
        return Err(AppError::BadRequest(
            "action is not waiting for confirmation".into(),
        ));
    }
    add_action_status(&st.db, a.id, "denied", details).await?;
    Ok(())
}

/// Confirms every waiting action for a target — used when auto-confirm is
/// activated so already-pending assignments proceed immediately.
pub async fn confirm_waiting_actions(st: &AppState, target_id: i64) -> Result<(), AppError> {
    let waiting = action::Entity::find()
        .filter(action::Column::TargetId.eq(target_id))
        .filter(action::Column::Status.eq("wait_for_confirmation"))
        .all(&st.db)
        .await?;
    for a in &waiting {
        confirm_action(st, a, &["auto-confirmed".into()]).await?;
    }
    Ok(())
}

async fn set_action(
    st: &AppState,
    a: &action::Model,
    status: &str,
    active: bool,
) -> Result<(), AppError> {
    let mut am: action::ActiveModel = a.clone().into();
    am.status = Set(status.into());
    am.active = Set(active);
    am.updated_at = Set(now_ms());
    am.update(&st.db).await?;
    Ok(())
}

async fn set_target_status(
    st: &AppState,
    t: &target::Model,
    installed: Option<i64>,
    status: &str,
) -> Result<(), AppError> {
    let mut tm: target::ActiveModel = t.clone().into();
    if let Some(ds) = installed {
        tm.installed_ds_id = Set(Some(ds));
    }
    tm.update_status = Set(status.into());
    tm.updated_at = Set(now_ms());
    tm.update(&st.db).await?;
    Ok(())
}

pub fn action_rest(
    a: &action::Model,
    target_cid: Option<&str>,
    base: &str,
) -> raptor_api_types::ActionRest {
    let is_cancel = matches!(a.status.as_str(), "canceling" | "canceled");
    raptor_api_types::ActionRest {
        id: a.id,
        action_type: if is_cancel { "cancel" } else { "update" }.to_string(),
        status: if a.active { "pending" } else { "finished" }.to_string(),
        detail_status: a.status.clone(),
        force_type: if a.forced { "forced" } else { "soft" }.to_string(),
        created_at: a.created_at,
        last_modified_at: a.updated_at,
        target: target_cid.map(str::to_string),
        links: serde_json::json!({
            "self": {"href": format!("{base}/rest/v1/actions/{}", a.id)},
            "distributionset": {"href": format!("{base}/rest/v1/distributionsets/{}", a.ds_id)}
        }),
    }
}
