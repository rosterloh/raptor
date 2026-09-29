//! Auto-assignment: saved target filters (`target_filter`) that optionally carry
//! a distribution set which is assigned to every matching target — including
//! targets that register or change attributes after the filter is created.
//!
//! Assignment is deliberately non-disruptive: it never supersedes a target's
//! in-flight action and never re-assigns a DS the target already has assigned.

use crate::api::mgmt::targets::condition;
use crate::domain::deployment::{active_action, assign_ds};
use crate::entity::{distribution_set, target, target_filter};
use crate::error::AppError;
use crate::metrics::{SWEEP_SKIP_AUTO_ASSIGN_FILTER, SWEEP_SKIP_AUTO_ASSIGN_TARGET};
use crate::state::AppState;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// Assigns `ds_id` to `target` unless it already has that DS assigned or has an
/// active action (auto-assignment must not disturb an in-flight deployment).
///
/// A target that cannot take the DS (typically an incompatible target type) is
/// logged and skipped: every caller — the sweep, a device's own poll, attaching
/// the DS — would otherwise fail on account of that one target (#148).
async fn maybe_assign(
    st: &AppState,
    target: &target::Model,
    ds_id: i64,
    action_type: Option<&str>,
) -> Result<(), AppError> {
    if target.assigned_ds_id == Some(ds_id) {
        return Ok(());
    }
    if active_action(&st.db, target.id).await?.is_some() {
        return Ok(());
    }
    // Auto-assignment carries no maintenance window: hawkBit's own
    // Management API has no such field on target-filter auto-assignment to
    // be at parity with (verified against MgmtTargetFilterQuery and its
    // request body — see #116).
    match assign_ds(st, target, ds_id, action_type, None, None).await {
        Err(e) if !e.is_infrastructure() => {
            tracing::warn!(
                error = ?e,
                target_id = target.id,
                ds_id,
                "auto-assignment skipped a target that cannot take the distribution set"
            );
            st.metrics.sweep_skipped(SWEEP_SKIP_AUTO_ASSIGN_TARGET);
        }
        r => {
            r?;
        }
    }
    Ok(())
}

/// Loads the auto-assign DS for a filter, returning it only if it is complete
/// (an incomplete DS cannot be assigned, so we skip rather than error the sweep).
async fn assignable_ds(
    st: &AppState,
    ds_id: i64,
) -> Result<Option<distribution_set::Model>, AppError> {
    Ok(distribution_set::Entity::find_by_id(ds_id)
        .one(&st.db)
        .await?
        .filter(|ds| ds.complete))
}

/// Runs one filter's auto-assignment against every currently matching target.
pub async fn run_auto_assign(st: &AppState, filter: &target_filter::Model) -> Result<(), AppError> {
    let Some(ds_id) = filter.auto_assign_ds_id else {
        return Ok(());
    };
    if assignable_ds(st, ds_id).await?.is_none() {
        return Ok(());
    }
    let cond = condition(&filter.query)?;
    let targets = target::Entity::find()
        .filter(cond)
        .order_by_asc(target::Column::Id)
        .all(&st.db)
        .await?;
    let action_type = filter.auto_assign_action_type.as_deref();
    for t in targets {
        maybe_assign(st, &t, ds_id, action_type).await?;
    }
    Ok(())
}

/// Periodic sweep: runs every filter that has an auto-assign DS attached. Shared
/// with the rollout evaluator's background task.
pub async fn auto_assign_all(st: &AppState) -> Result<(), AppError> {
    let filters = target_filter::Entity::find()
        .filter(target_filter::Column::AutoAssignDsId.is_not_null())
        .order_by_asc(target_filter::Column::Id)
        .all(&st.db)
        .await?;
    for f in filters {
        // One broken filter (say, a stored query that no longer compiles) must
        // not starve the filters behind it (#148).
        match run_auto_assign(st, &f).await {
            Err(e) if !e.is_infrastructure() => {
                tracing::warn!(error = ?e, filter_id = f.id, "auto-assignment skipped a filter");
                st.metrics.sweep_skipped(SWEEP_SKIP_AUTO_ASSIGN_FILTER);
            }
            r => r?,
        }
    }
    Ok(())
}

/// Evaluates every auto-assign filter against a single target (usually one that
/// just registered or changed attributes), assigning the first matching filter's
/// DS so it lands without waiting for the periodic sweep.
pub async fn auto_assign_for_target(st: &AppState, target: &target::Model) -> Result<(), AppError> {
    let filters = target_filter::Entity::find()
        .filter(target_filter::Column::AutoAssignDsId.is_not_null())
        .order_by_asc(target_filter::Column::Id)
        .all(&st.db)
        .await?;
    for f in filters {
        let Some(ds_id) = f.auto_assign_ds_id else {
            continue;
        };
        // The stored query was validated at create/update time; if it somehow no
        // longer compiles (e.g. a field map changed), skip rather than fail the poll.
        let Ok(cond) = condition(&f.query) else {
            continue;
        };
        let matched = target::Entity::find()
            .filter(cond)
            .filter(target::Column::Id.eq(target.id))
            .one(&st.db)
            .await?
            .is_some();
        if matched && assignable_ds(st, ds_id).await?.is_some() {
            maybe_assign(st, target, ds_id, f.auto_assign_action_type.as_deref()).await?;
        }
    }
    Ok(())
}
