//! Auto-assignment: saved target filters (`target_filter`) that optionally carry
//! a distribution set which is assigned to every matching target — including
//! targets that register or change attributes after the filter is created.
//!
//! Assignment is deliberately non-disruptive: it never supersedes a target's
//! in-flight action and never re-assigns a DS the target already has assigned.

use crate::api::mgmt::targets::fiql_map;
use crate::domain::deployment::{active_action, assign_ds};
use crate::entity::{distribution_set, target, target_filter};
use crate::error::AppError;
use crate::state::AppState;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// Assigns `ds_id` to `target` unless it already has that DS assigned or has an
/// active action (auto-assignment must not disturb an in-flight deployment).
async fn maybe_assign(
    st: &AppState,
    target: &target::Model,
    ds_id: i64,
    forced: bool,
) -> Result<(), AppError> {
    if target.assigned_ds_id == Some(ds_id) {
        return Ok(());
    }
    if active_action(&st.db, target.id).await?.is_some() {
        return Ok(());
    }
    assign_ds(st, target, ds_id, forced).await?;
    Ok(())
}

fn forced_from(action_type: &Option<String>) -> bool {
    action_type.as_deref() != Some("soft")
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
    let expr = crate::fiql::parse(&filter.query).map_err(AppError::BadRequest)?;
    let cond = crate::fiql::to_condition(&expr, &fiql_map)?;
    let targets = target::Entity::find()
        .filter(cond)
        .order_by_asc(target::Column::Id)
        .all(&st.db)
        .await?;
    let forced = forced_from(&filter.auto_assign_action_type);
    for t in targets {
        maybe_assign(st, &t, ds_id, forced).await?;
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
        run_auto_assign(st, &f).await?;
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
        // longer parses (e.g. a field map changed), skip rather than fail the poll.
        let Ok(expr) = crate::fiql::parse(&f.query) else {
            continue;
        };
        let Ok(cond) = crate::fiql::to_condition(&expr, &fiql_map) else {
            continue;
        };
        let matched = target::Entity::find()
            .filter(cond)
            .filter(target::Column::Id.eq(target.id))
            .one(&st.db)
            .await?
            .is_some();
        if matched && assignable_ds(st, ds_id).await?.is_some() {
            maybe_assign(st, target, ds_id, forced_from(&f.auto_assign_action_type)).await?;
        }
    }
    Ok(())
}
