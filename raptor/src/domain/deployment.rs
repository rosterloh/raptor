//! The action/deployment state machine: assigning a distribution set
//! (`assign_ds`), applying device feedback (`apply_feedback`,
//! `apply_cancel_feedback`), and the confirmation-flow transitions. This is
//! the single place action status and target `update_status` are mutated, so
//! mgmt and DDI handlers stay consistent.

use crate::entity::{
    action, action_status, action_status_message, distribution_set, target, target_type_ds_type,
};
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

/// The four hawkBit assignment types (`MgmtActionType` JSON names).
pub const ACTION_TYPES: [&str; 4] = ["forced", "soft", "timeforced", "downloadonly"];

/// Normalises an assignment `type` from the wire. `None` means forced, matching
/// raptor's previous behaviour of treating anything but `soft` as forced.
pub fn parse_action_type(t: Option<&str>) -> Result<&'static str, AppError> {
    match t {
        None => Ok("forced"),
        Some(v) => ACTION_TYPES
            .iter()
            .find(|k| **k == v)
            .copied()
            .ok_or_else(|| {
                AppError::BadRequest(format!(
                    "unknown action type {v:?}; expected one of {}",
                    ACTION_TYPES.join(", ")
                ))
            }),
    }
}

/// Whether a `timeforced` action's deadline has passed — and therefore whether
/// it now behaves as forced. A missing `forced_time` counts as reached, which is
/// what hawkBit's default of 0 amounts to.
fn time_forced_hit(a: &action::Model, now: i64) -> bool {
    a.forced_time.is_none_or(|t| now >= t)
}

/// The DDI `deployment.download` / `deployment.update` handling types for an
/// action, mirroring hawkBit's `calculateDownloadType`/`calculateUpdateType`:
/// download is forced for `downloadonly` and for forced-or-time-forced actions,
/// attempt otherwise; update is skipped entirely for `downloadonly`.
pub fn ddi_modes(a: &action::Model, now: i64) -> (&'static str, &'static str) {
    let download = match a.action_type.as_str() {
        "forced" | "downloadonly" => "forced",
        "timeforced" if time_forced_hit(a, now) => "forced",
        _ => "attempt",
    };
    let update = if a.action_type == "downloadonly" {
        "skip"
    } else {
        download
    };
    (download, update)
}

/// The `forceType` reported to operators. `timeforced` keeps its own name until
/// its deadline passes, after which it reads as `forced` — the same collapse the
/// DDI modes do, so the Management API and the device agree.
pub fn effective_force_type(a: &action::Model, now: i64) -> &'static str {
    match a.action_type.as_str() {
        "soft" => "soft",
        "downloadonly" => "downloadonly",
        "timeforced" if !time_forced_hit(a, now) => "timeforced",
        _ => "forced",
    }
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

/// `action_type` is validated here rather than at each call site, so no caller
/// (mgmt assignment, rollout, filter auto-assign) can write an unknown type.
#[tracing::instrument(skip_all, fields(target_id = target.id, ds_id, action_type))]
pub async fn assign_ds(
    st: &AppState,
    target: &target::Model,
    ds_id: i64,
    action_type: Option<&str>,
    forced_time: Option<i64>,
) -> Result<AssignResult, AppError> {
    let action_type = parse_action_type(action_type)?;
    let ds = distribution_set::Entity::find_by_id(ds_id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set"))?;
    if !ds.complete {
        return Err(AppError::BadRequest(
            "distribution set is incomplete".into(),
        ));
    }
    if ds.invalid {
        return Err(AppError::BadRequest(
            "distribution set has been invalidated".into(),
        ));
    }
    // A typed target only accepts distribution sets whose type is compatible.
    if let Some(tt_id) = target.type_id {
        let compatible = target_type_ds_type::Entity::find_by_id((tt_id, ds.type_id))
            .one(&st.db)
            .await?
            .is_some();
        if !compatible {
            return Err(AppError::BadRequest(
                "distribution set type is not compatible with the target type".into(),
            ));
        }
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
        action_type: Set(action_type.into()),
        forced_time: Set(forced_time),
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
    // Any feedback — even the "history only" kinds below — proves the device
    // is still communicating, so it clears the repeated-fetch-with-no-feedback
    // diagnostic counter incremented by DDI's deploymentBase handler.
    if a.deployment_fetch_count != 0 {
        let mut am: action::ActiveModel = a.clone().into();
        am.deployment_fetch_count = Set(0);
        am.update(&st.db).await?;
    }
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
        // A downloadonly action is complete once the bytes are down: nothing is
        // installed, so the action closes as `downloaded` and `installed_ds_id`
        // is deliberately left alone (hawkBit only realigns the *assigned* DS,
        // which assign_ds already did). The target still reads as in_sync
        // because there is no pending work left for it.
        ("downloaded", _) if a.action_type == "downloadonly" => {
            set_action(st, a, "downloaded", false).await?;
            set_target_status(st, t, None, "in_sync").await?;
            st.metrics.action_finished();
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
        force_type: effective_force_type(a, now_ms()).to_string(),
        force_time: a.forced_time,
        created_at: a.created_at,
        last_modified_at: a.updated_at,
        target: target_cid.map(str::to_string),
        deployment_fetch_count: a.deployment_fetch_count,
        links: serde_json::json!({
            "self": {"href": format!("{base}/rest/v1/actions/{}", a.id)},
            "distributionset": {"href": format!("{base}/rest/v1/distributionsets/{}", a.ds_id)}
        }),
    }
}
