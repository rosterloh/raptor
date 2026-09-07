//! Automatic action cleanup (#134): deletes closed actions past a retention
//! window, together with the status history hanging off them.
//!
//! Mirrors hawkBit's `AutoActionCleanup`, which is off unless configured and
//! is documented as being for terminated actions only. Two things it does
//! *not* do are deliberate here:
//!
//! - It deletes in bounded batches. hawkBit does the same, via a `LIMIT` on
//!   its native delete, "to reduce the overall load of the database" — a first
//!   sweep over a long-neglected instance could otherwise be a very large
//!   single statement.
//! - It records what it deleted from a rollout group before deleting it. See
//!   [`crate::domain::rollout::counts_by_group`]: progress is derived by
//!   counting actions, so silently removing them would walk a finished
//!   rollout's targets back to `scheduled`. hawkBit has this exact drift —
//!   its `TotalTargetCountStatus` derives `NOTSTARTED` as
//!   `total - sum(action counts)` — and tolerates it; raptor does not.

use crate::entity::{action, action_status, action_status_message, rollout_group};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::now_ms;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use std::collections::HashMap;

/// Actions deleted per batch. Bounds both the `IN (...)` lists below and how
/// long any one statement holds the database busy.
const BATCH: u64 = 1000;

/// Batches per sweep. Without a cap a first run against a long-neglected
/// instance would delete unboundedly in one go; with only a single batch (what
/// hawkBit does per run) a million-row backlog at an hourly cadence would take
/// weeks to drain. This clears up to 10k per sweep and picks the rest up on the
/// next one.
const MAX_BATCHES_PER_SWEEP: usize = 10;

/// Runs batches until nothing is left to delete or the per-sweep cap is hit.
/// This is what the background task calls; [`cleanup_actions`] is one batch.
pub async fn run_sweep(st: &AppState) -> Result<u64, AppError> {
    let mut total = 0;
    for _ in 0..MAX_BATCHES_PER_SWEEP {
        let n = cleanup_actions(st).await?;
        total += n;
        if n < BATCH {
            break;
        }
    }
    Ok(total)
}

/// Deletes one batch of expired closed actions, returning how many went.
///
/// Eligibility is deliberately narrower than the configured status list: an
/// action must also be **inactive**. A device can sit in a status the operator
/// listed while its action is still live (an action being `canceling` is the
/// obvious case), and deleting from under it would strand the device holding an
/// action id the server no longer knows.
pub async fn cleanup_actions(st: &AppState) -> Result<u64, AppError> {
    let cfg = &st.cfg.cleanup;
    if !cfg.enabled || cfg.action_statuses.is_empty() {
        return Ok(0);
    }
    let cutoff = now_ms() - cfg.expiry_millis();

    let expired = action::Entity::find()
        .filter(action::Column::Active.eq(false))
        .filter(action::Column::Status.is_in(cfg.action_statuses.clone()))
        .filter(action::Column::UpdatedAt.lt(cutoff))
        .order_by_asc(action::Column::Id)
        .limit(BATCH)
        .all(&st.db)
        .await?;
    if expired.is_empty() {
        return Ok(0);
    }

    // Tally per rollout group *before* the rows go, so the group can keep
    // reporting the outcome these actions carried.
    let mut purged: HashMap<i64, crate::domain::rollout::TargetsPerStatus> = HashMap::new();
    for a in &expired {
        if let Some(gid) = a.rollout_group_id {
            crate::domain::rollout::bucket(purged.entry(gid).or_default(), &a.status, 1);
        }
    }

    let ids: Vec<i64> = expired.iter().map(|a| a.id).collect();
    let status_ids: Vec<i64> = action_status::Entity::find()
        .filter(action_status::Column::ActionId.is_in(ids.clone()))
        .all(&st.db)
        .await?
        .into_iter()
        .map(|s| s.id)
        .collect();

    // Innermost first: raptor's foreign keys carry no `ON DELETE CASCADE`, and
    // SQLite would not enforce one anyway, so deleting the parent first would
    // orphan exactly the rows this sweep exists to reclaim.
    if !status_ids.is_empty() {
        action_status_message::Entity::delete_many()
            .filter(action_status_message::Column::ActionStatusId.is_in(status_ids.clone()))
            .exec(&st.db)
            .await?;
        action_status::Entity::delete_many()
            .filter(action_status::Column::Id.is_in(status_ids))
            .exec(&st.db)
            .await?;
    }
    let deleted = action::Entity::delete_many()
        .filter(action::Column::Id.is_in(ids))
        .exec(&st.db)
        .await?
        .rows_affected;

    for (gid, counts) in purged {
        let Some(g) = rollout_group::Entity::find_by_id(gid).one(&st.db).await? else {
            continue; // group deleted with its rollout since the tally
        };
        let mut gm: rollout_group::ActiveModel = g.clone().into();
        gm.purged_finished = Set(g.purged_finished + counts.finished);
        gm.purged_error = Set(g.purged_error + counts.error);
        gm.purged_cancelled = Set(g.purged_cancelled + counts.cancelled);
        gm.purged_running = Set(g.purged_running + counts.running);
        gm.update(&st.db).await?;
    }

    Ok(deleted)
}
