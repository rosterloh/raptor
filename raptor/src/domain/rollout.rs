//! Rollout lifecycle: creating groups from a target-filter query, starting
//! the first group, and the success/error-threshold evaluation that advances
//! or pauses subsequent groups, plus the stop operation. `evaluate_rollouts`
//! is the entry point called by the background sweep in `main`.

use crate::entity::{
    action, distribution_set, rollout, rollout_group, rollout_target_group, target,
};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::now_ms;
use raptor_api_types::{RolloutCreate, RolloutTargetsPerStatus as TargetsPerStatus};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, ModelTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use std::collections::HashMap;

fn parse_percent(expr: &str) -> Result<i64, AppError> {
    expr.parse::<i64>()
        .ok()
        .filter(|v| (0..=100).contains(v))
        .ok_or_else(|| AppError::BadRequest(format!("invalid threshold expression: {expr}")))
}

pub async fn create_rollout(
    st: &AppState,
    req: &RolloutCreate,
) -> Result<rollout::Model, AppError> {
    let ds = distribution_set::Entity::find_by_id(req.distribution_set_id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set"))?;
    if !ds.complete {
        return Err(AppError::BadRequest(
            "distribution set is incomplete".into(),
        ));
    }
    if req.amount_groups < 1 {
        return Err(AppError::BadRequest("amountGroups must be >= 1".into()));
    }

    let cond = crate::api::mgmt::targets::condition(&req.target_filter_query)?;
    let targets = target::Entity::find()
        .filter(cond)
        .order_by_asc(target::Column::Id)
        .all(&st.db)
        .await?;
    if targets.is_empty() {
        return Err(AppError::BadRequest(
            "target filter matches no targets".into(),
        ));
    }

    let action_type = crate::domain::deployment::parse_action_type(req.rollout_type.as_deref())?;
    let success_threshold = parse_percent(&req.success_condition.expression)?;
    let error_threshold = match &req.error_condition {
        Some(c) => parse_percent(&c.expression)?,
        None => 101, // never triggers
    };

    // hawkBit gates a new rollout behind an operator decision when the tenant's
    // `rollout.approval.enabled` flag is set; otherwise it is startable at once.
    let initial_status = if st.cfg.rollout_approval_enabled {
        "waiting_for_approval"
    } else {
        "ready"
    };

    let txn = st.db.begin().await?;
    let now = now_ms();
    let r = rollout::ActiveModel {
        name: Set(req.name.clone()),
        description: Set(req.description.clone()),
        ds_id: Set(ds.id),
        target_filter: Set(req.target_filter_query.clone()),
        status: Set(initial_status.into()),
        action_type: Set(action_type.into()),
        forced_time: Set(req.forcetime),
        total_targets: Set(targets.len() as i64),
        group_count: Set(req.amount_groups),
        success_threshold: Set(success_threshold),
        error_threshold: Set(error_threshold),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    let per_group = targets.len().div_ceil(req.amount_groups as usize).max(1);
    for (idx, chunk) in targets.chunks(per_group).enumerate() {
        let g = rollout_group::ActiveModel {
            rollout_id: Set(r.id),
            name: Set(format!("group-{}", idx + 1)),
            order_index: Set(idx as i64),
            status: Set("ready".into()),
            total_targets: Set(chunk.len() as i64),
            success_threshold: Set(success_threshold),
            error_threshold: Set(error_threshold),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&txn)
        .await?;
        for t in chunk {
            rollout_target_group::ActiveModel {
                rollout_group_id: Set(g.id),
                target_id: Set(t.id),
                ..Default::default()
            }
            .insert(&txn)
            .await?;
        }
    }
    txn.commit().await?;
    Ok(r)
}

async fn schedule_group(st: &AppState, group: &rollout_group::Model) -> Result<(), AppError> {
    let r = rollout::Entity::find_by_id(group.rollout_id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("rollout"))?;
    let members = rollout_target_group::Entity::find()
        .filter(rollout_target_group::Column::RolloutGroupId.eq(group.id))
        .all(&st.db)
        .await?;
    for m in members {
        let t = target::Entity::find_by_id(m.target_id)
            .one(&st.db)
            .await?
            .ok_or(AppError::NotFound("target"))?;
        // Rollouts carry no maintenance window: hawkBit's own Management API
        // has no maintenanceWindow field on rollout creation to be at parity
        // with (verified against MgmtRolloutRestRequestBodyPost/Put and
        // AbstractMgmtRolloutConditionsEntity — see #116).
        let res = crate::domain::deployment::assign_ds(
            st,
            &t,
            r.ds_id,
            Some(&r.action_type),
            r.forced_time,
            None,
        )
        .await?;
        if let Some(action_id) = res.action_id {
            let a = action::Entity::find_by_id(action_id)
                .one(&st.db)
                .await?
                .ok_or(AppError::NotFound("action"))?;
            let mut am: action::ActiveModel = a.into();
            am.rollout_id = Set(Some(r.id));
            am.rollout_group_id = Set(Some(group.id));
            am.update(&st.db).await?;
        }
    }
    let mut gm: rollout_group::ActiveModel = group.clone().into();
    gm.status = Set("running".into());
    gm.updated_at = Set(now_ms());
    gm.update(&st.db).await?;
    Ok(())
}

/// An operator's verdict on a rollout waiting for approval (hawkBit's
/// `Rollout.ApprovalDecision`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approved,
    Denied,
}

/// Records an approve/deny decision on a rollout in `waiting_for_approval`.
///
/// Approval takes the rollout to `ready` — the same state a rollout is created
/// in when the approval gate is off — so everything downstream (`start`, the
/// evaluator, the target counters) needs no notion of approval at all. Denial
/// is terminal: `approval_denied` is not a startable status, and nothing
/// transitions out of it, so a denied rollout can only be deleted. That
/// matches hawkBit, which likewise offers no "undeny" and expects the operator
/// to create a fresh rollout.
///
/// `remark` is optional and only overwrites a previous note when given,
/// mirroring hawkBit's `approveOrDeny0`.
pub async fn decide_approval(
    st: &AppState,
    r: rollout::Model,
    decision: ApprovalDecision,
    remark: Option<String>,
) -> Result<rollout::Model, AppError> {
    if r.status != "waiting_for_approval" {
        return Err(AppError::BadRequest(format!(
            "cannot approve or deny rollout in status {}",
            r.status
        )));
    }
    let status = match decision {
        ApprovalDecision::Approved => "ready",
        ApprovalDecision::Denied => "approval_denied",
    };
    // raptor authenticates a single configured operator account, so the
    // decider is that account — there is no per-request identity to carry.
    let decided_by = st.cfg.mgmt.username.clone();

    let mut rm: rollout::ActiveModel = r.into();
    rm.status = Set(status.into());
    rm.approval_decided_by = Set(Some(decided_by));
    if let Some(remark) = remark {
        rm.approval_remark = Set(Some(remark));
    }
    rm.updated_at = Set(now_ms());
    Ok(rm.update(&st.db).await?)
}

pub async fn start_rollout(st: &AppState, r: rollout::Model) -> Result<rollout::Model, AppError> {
    if r.status != "ready" {
        return Err(AppError::BadRequest(format!(
            "cannot start rollout in status {}",
            r.status
        )));
    }
    let mut rm: rollout::ActiveModel = r.into();
    rm.status = Set("running".into());
    rm.updated_at = Set(now_ms());
    let r = rm.update(&st.db).await?;

    let first = rollout_group::Entity::find()
        .filter(rollout_group::Column::RolloutId.eq(r.id))
        .order_by_asc(rollout_group::Column::OrderIndex)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("rollout group"))?;
    schedule_group(st, &first).await?;
    Ok(r)
}

pub async fn pause_rollout(st: &AppState, r: rollout::Model) -> Result<rollout::Model, AppError> {
    if r.status != "running" {
        return Err(AppError::BadRequest(format!(
            "cannot pause rollout in status {}",
            r.status
        )));
    }
    let mut rm: rollout::ActiveModel = r.into();
    rm.status = Set("paused".into());
    rm.updated_at = Set(now_ms());
    Ok(rm.update(&st.db).await?)
}

pub async fn resume_rollout(st: &AppState, r: rollout::Model) -> Result<rollout::Model, AppError> {
    if r.status != "paused" {
        return Err(AppError::BadRequest(format!(
            "cannot resume rollout in status {}",
            r.status
        )));
    }
    let mut rm: rollout::ActiveModel = r.into();
    rm.status = Set("running".into());
    rm.updated_at = Set(now_ms());
    let r = rm.update(&st.db).await?;
    evaluate_rollout(st, &r).await?;
    Ok(r)
}

/// Stops a rollout: soft-cancels the actions it issued and takes the rollout to
/// the terminal `stopped` status. This is the "abort this bad update now" lever
/// — unlike `pause_rollout`, which leaves already-issued actions running, and
/// unlike `delete_rollout`, which destroys the record an operator needs for the
/// post-mortem.
///
/// Stoppable from any status that is not already terminal or draining, matching
/// hawkBit's own `ROLLOUT_STATUS_STOPPABLE`. That includes the pre-start states:
/// a rollout that was never started has nothing to cancel, but stopping it is
/// how an operator retires it while keeping the record — the alternative is
/// deleting it, which throws that record away.
///
/// hawkBit models this as `STOPPING` -> `STOPPED`: the rollout sits in
/// `stopping` while the cancellations propagate to devices over DDI, and
/// reaches `stopped` once none of its actions is active any more.
/// `evaluate_rollouts` performs that second transition on each sweep.
///
/// Cancellation is soft: actions go to `canceling` and are served to the device
/// as `cancelAction`, so a device that has already downloaded can stop cleanly
/// and report the outcome back. `delete_rollout` hard-cancels instead, because
/// it is removing the very record that feedback would attach to.
pub async fn stop_rollout(st: &AppState, r: rollout::Model) -> Result<rollout::Model, AppError> {
    if !matches!(
        r.status.as_str(),
        "ready" | "waiting_for_approval" | "approval_denied" | "running" | "paused"
    ) {
        return Err(AppError::BadRequest(format!(
            "cannot stop rollout in status {}",
            r.status
        )));
    }

    // Actions already in `canceling` are left alone: re-issuing the cancel would
    // add a duplicate status entry without changing what the device is told.
    let actions = action::Entity::find()
        .filter(action::Column::RolloutId.eq(r.id))
        .filter(action::Column::Active.eq(true))
        .filter(action::Column::Status.ne("canceling"))
        .all(&st.db)
        .await?;
    for a in actions {
        let aid = a.id;
        let mut am: action::ActiveModel = a.into();
        am.status = Set("canceling".into());
        am.updated_at = Set(now_ms());
        am.update(&st.db).await?;
        crate::domain::deployment::add_action_status(
            &st.db,
            aid,
            "canceling",
            &["rollout stopped".into()],
        )
        .await?;
    }

    // Groups that never ran, or were mid-flight, are stopped with the rollout.
    // Groups that already finished keep their outcome.
    for g in rollout_group::Entity::find()
        .filter(rollout_group::Column::RolloutId.eq(r.id))
        .filter(rollout_group::Column::Status.ne("finished"))
        .all(&st.db)
        .await?
    {
        let mut gm: rollout_group::ActiveModel = g.into();
        gm.status = Set("stopped".into());
        gm.updated_at = Set(now_ms());
        gm.update(&st.db).await?;
    }

    let mut rm: rollout::ActiveModel = r.into();
    rm.status = Set("stopping".into());
    rm.updated_at = Set(now_ms());
    let r = rm.update(&st.db).await?;

    // A rollout with nothing in flight (never started, or every device already
    // done) settles now rather than waiting for the next evaluator sweep.
    settle_stopping(st, r).await
}

/// `stopping` -> `stopped`, once no action the rollout issued is still active.
/// Devices leave `canceling` by reporting cancel feedback over DDI (or by being
/// force-canceled), so this is driven by the evaluator rather than by a timer.
async fn settle_stopping(st: &AppState, r: rollout::Model) -> Result<rollout::Model, AppError> {
    let in_flight = action::Entity::find()
        .filter(action::Column::RolloutId.eq(r.id))
        .filter(action::Column::Active.eq(true))
        .count(&st.db)
        .await?;
    if in_flight > 0 {
        return Ok(r);
    }
    let mut rm: rollout::ActiveModel = r.into();
    rm.status = Set("stopped".into());
    rm.updated_at = Set(now_ms());
    Ok(rm.update(&st.db).await?)
}

pub async fn delete_rollout(st: &AppState, r: rollout::Model) -> Result<(), AppError> {
    let groups = rollout_group::Entity::find()
        .filter(rollout_group::Column::RolloutId.eq(r.id))
        .all(&st.db)
        .await?;
    for g in &groups {
        let actions = action::Entity::find()
            .filter(action::Column::RolloutGroupId.eq(g.id))
            .filter(action::Column::Active.eq(true))
            .all(&st.db)
            .await?;
        for a in actions {
            let aid = a.id;
            let mut am: action::ActiveModel = a.into();
            am.status = Set("canceled".into());
            am.active = Set(false);
            am.updated_at = Set(now_ms());
            am.update(&st.db).await?;
            crate::domain::deployment::add_action_status(
                &st.db,
                aid,
                "canceled",
                &["rollout deleted".into()],
            )
            .await?;
        }
        rollout_target_group::Entity::delete_many()
            .filter(rollout_target_group::Column::RolloutGroupId.eq(g.id))
            .exec(&st.db)
            .await?;
    }
    rollout_group::Entity::delete_many()
        .filter(rollout_group::Column::RolloutId.eq(r.id))
        .exec(&st.db)
        .await?;
    r.delete(&st.db).await?;
    Ok(())
}

/// Scans all running rollouts and advances/pauses their current group based on
/// action outcomes. Called from the background evaluator and after resume.
pub async fn evaluate_rollouts(st: &AppState) -> Result<(), AppError> {
    let running = rollout::Entity::find()
        .filter(rollout::Column::Status.eq("running"))
        .all(&st.db)
        .await?;
    for r in running {
        evaluate_rollout(st, &r).await?;
    }

    // Rollouts an operator stopped stay in `stopping` until the cancels they
    // issued have drained out of the devices.
    for r in rollout::Entity::find()
        .filter(rollout::Column::Status.eq("stopping"))
        .all(&st.db)
        .await?
    {
        settle_stopping(st, r).await?;
    }
    Ok(())
}

async fn evaluate_rollout(st: &AppState, r: &rollout::Model) -> Result<(), AppError> {
    let Some(group) = rollout_group::Entity::find()
        .filter(rollout_group::Column::RolloutId.eq(r.id))
        .filter(rollout_group::Column::Status.eq("running"))
        .one(&st.db)
        .await?
    else {
        return Ok(());
    };

    let actions = action::Entity::find()
        .filter(action::Column::RolloutGroupId.eq(group.id))
        .all(&st.db)
        .await?;
    let total = actions.len().max(1) as i64;
    let success = actions.iter().filter(|a| a.status == "finished").count() as i64;
    let error = actions
        .iter()
        .filter(|a| matches!(a.status.as_str(), "error" | "canceled"))
        .count() as i64;

    if error * 100 / total >= group.error_threshold {
        let mut gm: rollout_group::ActiveModel = group.clone().into();
        gm.status = Set("paused".into());
        gm.updated_at = Set(now_ms());
        gm.update(&st.db).await?;
        let mut rm: rollout::ActiveModel = r.clone().into();
        rm.status = Set("paused".into());
        rm.updated_at = Set(now_ms());
        rm.update(&st.db).await?;
        return Ok(());
    }

    if success * 100 / total >= group.success_threshold {
        let order_index = group.order_index;
        let mut gm: rollout_group::ActiveModel = group.into();
        gm.status = Set("finished".into());
        gm.updated_at = Set(now_ms());
        gm.update(&st.db).await?;

        let next = rollout_group::Entity::find()
            .filter(rollout_group::Column::RolloutId.eq(r.id))
            .filter(rollout_group::Column::OrderIndex.gt(order_index))
            .order_by_asc(rollout_group::Column::OrderIndex)
            .one(&st.db)
            .await?;
        match next {
            Some(g) => schedule_group(st, &g).await?,
            None => {
                let mut rm: rollout::ActiveModel = r.clone().into();
                rm.status = Set("finished".into());
                rm.updated_at = Set(now_ms());
                rm.update(&st.db).await?;
            }
        }
    }
    Ok(())
}

/// Bucket an action status into the hawkBit `totalTargetsPerStatus` field it
/// contributes to. Everything still in flight (`running`, `canceling`,
/// `wait_for_confirmation`) counts as running.
fn bucket(counts: &mut TargetsPerStatus, action_status: &str, n: i64) {
    match action_status {
        "finished" => counts.finished += n,
        "error" => counts.error += n,
        "canceled" => counts.cancelled += n,
        _ => counts.running += n,
    }
}

/// Whether a rollout has left its pre-start states — which is what decides
/// whether a target with no action yet counts as `scheduled` (waiting for its
/// group's turn) or `notstarted`.
///
/// `waiting_for_approval` and `approval_denied` sit alongside `ready` here: an
/// unapproved rollout has issued nothing, and a denied one never will.
fn is_started(status: &str) -> bool {
    !matches!(status, "ready" | "waiting_for_approval" | "approval_denied")
}

/// Per-group target counts by deployment outcome, keyed by rollout group id.
/// Groups with no members are absent from the map (callers default to zeroes).
///
/// Targets without an action yet have not been deployed to: they are `notstarted`
/// while the rollout itself has not been started, and `scheduled` once it is
/// running and they are only waiting for their group's turn.
async fn counts_by_group(
    st: &AppState,
    groups: &[rollout_group::Model],
    started: &HashMap<i64, bool>,
) -> Result<HashMap<i64, TargetsPerStatus>, AppError> {
    let ids: Vec<i64> = groups.iter().map(|g| g.id).collect();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }

    let members: Vec<(i64, i64)> = rollout_target_group::Entity::find()
        .select_only()
        .column(rollout_target_group::Column::RolloutGroupId)
        .column_as(rollout_target_group::Column::Id.count(), "count")
        .filter(rollout_target_group::Column::RolloutGroupId.is_in(ids.clone()))
        .group_by(rollout_target_group::Column::RolloutGroupId)
        .into_tuple()
        .all(&st.db)
        .await?;
    let by_group: HashMap<i64, i64> = members.into_iter().collect();

    let actions: Vec<(Option<i64>, String, i64)> = action::Entity::find()
        .select_only()
        .column(action::Column::RolloutGroupId)
        .column(action::Column::Status)
        .column_as(action::Column::Id.count(), "count")
        .filter(action::Column::RolloutGroupId.is_in(ids))
        .group_by(action::Column::RolloutGroupId)
        .group_by(action::Column::Status)
        .into_tuple()
        .all(&st.db)
        .await?;

    let mut out: HashMap<i64, TargetsPerStatus> = HashMap::new();
    for (gid, status, n) in actions {
        let Some(gid) = gid else { continue };
        bucket(out.entry(gid).or_default(), &status, n);
    }
    for g in groups {
        let Some(members) = by_group.get(&g.id).copied() else {
            continue;
        };
        let c = out.entry(g.id).or_default();
        // Members the group has no action for yet. Clamped at zero because a
        // target whose rollout action was superseded by a later assignment
        // leaves the canceled one behind, counted against the same group.
        let pending = (members - (c.finished + c.error + c.cancelled + c.running)).max(0);
        if started.get(&g.rollout_id).copied().unwrap_or(false) {
            c.scheduled += pending;
        } else {
            c.notstarted += pending;
        }
    }
    Ok(out)
}

/// Rollout-level target counts (the sum over its groups), keyed by rollout id.
pub async fn counts_for_rollouts(
    st: &AppState,
    rollouts: &[rollout::Model],
) -> Result<HashMap<i64, TargetsPerStatus>, AppError> {
    let ids: Vec<i64> = rollouts.iter().map(|r| r.id).collect();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let started = rollouts
        .iter()
        .map(|r| (r.id, is_started(&r.status)))
        .collect();
    let groups = rollout_group::Entity::find()
        .filter(rollout_group::Column::RolloutId.is_in(ids))
        .all(&st.db)
        .await?;
    let per_group = counts_by_group(st, &groups, &started).await?;

    let mut out: HashMap<i64, TargetsPerStatus> = rollouts
        .iter()
        .map(|r| (r.id, TargetsPerStatus::default()))
        .collect();
    for g in &groups {
        if let (Some(acc), Some(c)) = (out.get_mut(&g.rollout_id), per_group.get(&g.id)) {
            *acc += *c;
        }
    }
    Ok(out)
}

/// Target counts for the groups of one rollout, keyed by rollout group id.
pub async fn counts_for_groups(
    st: &AppState,
    r: &rollout::Model,
    groups: &[rollout_group::Model],
) -> Result<HashMap<i64, TargetsPerStatus>, AppError> {
    let started = HashMap::from([(r.id, is_started(&r.status))]);
    counts_by_group(st, groups, &started).await
}

pub fn rollout_rest(
    r: &rollout::Model,
    counts: TargetsPerStatus,
    base: &str,
) -> raptor_api_types::RolloutRest {
    raptor_api_types::RolloutRest {
        id: r.id,
        name: r.name.clone(),
        description: r.description.clone(),
        distribution_set_id: r.ds_id,
        target_filter_query: r.target_filter.clone(),
        status: r.status.clone(),
        rollout_type: r.action_type.clone(),
        forcetime: r.forced_time,
        total_targets: r.total_targets,
        total_targets_per_status: counts,
        created_at: r.created_at,
        last_modified_at: r.updated_at,
        approve_decided_by: r.approval_decided_by.clone(),
        approval_remark: r.approval_remark.clone(),
        links: serde_json::json!({"self": {"href": format!("{base}/rest/v1/rollouts/{}", r.id)}}),
    }
}

pub fn rollout_group_rest(
    g: &rollout_group::Model,
    rollout_id: i64,
    counts: TargetsPerStatus,
    base: &str,
) -> raptor_api_types::RolloutGroupRest {
    raptor_api_types::RolloutGroupRest {
        id: g.id,
        name: g.name.clone(),
        status: g.status.clone(),
        total_targets: g.total_targets,
        total_targets_per_status: counts,
        success_condition: raptor_api_types::RolloutCondition {
            condition: "THRESHOLD".into(),
            expression: g.success_threshold.to_string(),
        },
        error_condition: raptor_api_types::RolloutCondition {
            condition: "THRESHOLD".into(),
            expression: g.error_threshold.to_string(),
        },
        links: serde_json::json!({
            "self": {"href": format!("{base}/rest/v1/rollouts/{rollout_id}/deploygroups/{}", g.id)}
        }),
    }
}
