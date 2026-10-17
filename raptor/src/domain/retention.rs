//! Release retention: keep the newest `keep_releases` valid distribution sets
//! of each name and hard-delete everything older, reclaiming artifact storage.
//!
//! A raptor extension — hawkBit can only soft-delete a set that was ever
//! assigned, and never deletes it on its own. Here a retired set goes
//! entirely:
//!
//! 1. it is force-invalidated (rollouts stopped, actions closed, auto-assign
//!    filters detached), exactly as `POST .../invalidate` with `force` would;
//! 2. its rollouts are deleted;
//! 3. in one transaction: its actions and their history, every target
//!    reference to it, its own rows, and every software module no remaining
//!    set uses;
//! 4. after commit, the blobs those modules held, unless another artifact
//!    still shares the hash.
//!
//! Steps 1 and 2 commit on their own, so a failure part-way leaves the set
//! invalid but present. It is still older than the newest valid sets, so the
//! next sweep picks it up again: every step is safe to repeat.
//!
//! Nothing is held back for a target still running a retired release. A
//! device that has not taken a newer image in N releases is the anomaly; it
//! shows as `registered` until assigned something current.

use crate::domain::distribution_set::{CancelMode, invalidate};
use crate::entity::{
    action, distribution_set, ds_metadata, ds_module, ds_tag_assignment, rollout, target,
    target_filter,
};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::now_ms;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, EntityTrait, PaginatorTrait,
    QueryFilter, QuerySelect, TransactionTrait,
};
use std::collections::HashMap;

/// Sets retired per sweep. Each one is several statements plus blob deletes,
/// so the first sweep over an instance with years of releases is spread over
/// several intervals rather than run in one go.
const MAX_SETS_PER_SWEEP: usize = 50;

/// Action ids per `delete_actions` call, bounding its `IN (...)` lists.
const ACTION_BATCH: usize = 1000;

/// What one sweep removed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Retired {
    pub sets: u64,
    pub modules: u64,
    pub blobs: u64,
}

/// Retires every set older than the newest `keep_releases` valid ones of its
/// name, up to [`MAX_SETS_PER_SWEEP`]. Does nothing unless `keep_releases`
/// is set.
pub async fn run_sweep(st: &AppState) -> Result<Retired, AppError> {
    let keep = match st.cfg.retention.keep_releases {
        Some(n) if n > 0 => n as usize,
        _ => return Ok(Retired::default()),
    };
    let mut total = Retired::default();
    for id in expired_sets(st, keep)
        .await?
        .into_iter()
        .take(MAX_SETS_PER_SWEEP)
    {
        let r = retire(st, id).await?;
        total.sets += r.sets;
        total.modules += r.modules;
        total.blobs += r.blobs;
    }
    Ok(total)
}

/// Ids of the sets due for retirement, oldest first.
///
/// Per `(tenant, name)`, sets are ranked newest first by creation time (id
/// breaks ties). Version strings are not reliably comparable, so they play no
/// part. The newest `keep` *valid* sets are kept, so `keep` really is the
/// number of rollback targets left; every set older than the oldest of those
/// goes, invalid or not. An invalid set newer than that stays until enough
/// valid releases have followed it. A name with fewer than `keep` valid sets
/// loses nothing.
async fn expired_sets(st: &AppState, keep: usize) -> Result<Vec<i64>, AppError> {
    let mut by_name: HashMap<(String, String), Vec<distribution_set::Model>> = HashMap::new();
    for ds in distribution_set::Entity::find().all(&st.db).await? {
        by_name
            .entry((ds.tenant.clone(), ds.name.clone()))
            .or_default()
            .push(ds);
    }
    let mut out: Vec<(i64, i64)> = Vec::new();
    for sets in by_name.values_mut() {
        sets.sort_by_key(|d| std::cmp::Reverse((d.created_at, d.id)));
        let Some((cut, _)) = sets
            .iter()
            .enumerate()
            .filter(|(_, d)| !d.invalid)
            .nth(keep - 1)
        else {
            continue;
        };
        out.extend(sets[cut + 1..].iter().map(|d| (d.created_at, d.id)));
    }
    out.sort();
    Ok(out.into_iter().map(|(_, id)| id).collect())
}

/// Hard-deletes one set and everything hanging off it; see the module docs
/// for the order and why it is safe to repeat.
pub async fn retire(st: &AppState, id: i64) -> Result<Retired, AppError> {
    let Some(ds) = distribution_set::Entity::find_by_id(id).one(&st.db).await? else {
        return Ok(Retired::default());
    };
    if !ds.invalid {
        invalidate(st, id, CancelMode::Force, true).await?;
    }
    for r in rollout::Entity::find()
        .filter(rollout::Column::DsId.eq(id))
        .all(&st.db)
        .await?
    {
        crate::domain::rollout::delete_rollout(st, r).await?;
    }

    let txn = st.db.begin().await?;

    // Actions and their history. Any still active (a set invalidated earlier
    // with `soft`, say) go too; the target fix-up below settles their devices.
    let action_ids: Vec<i64> = action::Entity::find()
        .select_only()
        .column(action::Column::Id)
        .filter(action::Column::DsId.eq(id))
        .into_tuple()
        .all(&txn)
        .await?;
    for chunk in action_ids.chunks(ACTION_BATCH) {
        crate::domain::cleanup::delete_actions(&txn, chunk.to_vec()).await?;
    }

    // Targets pointing at the set: an installed reference is cleared, and an
    // assignment falls back to whatever is still installed. A target left
    // with a different set assigned than installed is still mid-update on a
    // kept release and keeps its status.
    let mut touched = Vec::new();
    for t in target::Entity::find()
        .filter(
            Condition::any()
                .add(target::Column::AssignedDsId.eq(id))
                .add(target::Column::InstalledDsId.eq(id)),
        )
        .all(&txn)
        .await?
    {
        let installed = t.installed_ds_id.filter(|&i| i != id);
        let assigned = t.assigned_ds_id.filter(|&a| a != id).or(installed);
        let status = if assigned.is_some() && assigned != installed {
            t.update_status.clone()
        } else if installed.is_some() {
            "in_sync".to_string()
        } else {
            "registered".to_string()
        };
        touched.push(t.controller_id.clone());
        let mut tm: target::ActiveModel = t.into();
        tm.installed_ds_id = Set(installed);
        tm.assigned_ds_id = Set(assigned);
        tm.update_status = Set(status);
        tm.updated_at = Set(now_ms());
        tm.update(&txn).await?;
    }

    // Invalidation already detached these; repeated in case an earlier,
    // interrupted retirement left one behind.
    target_filter::Entity::update_many()
        .col_expr(
            target_filter::Column::AutoAssignDsId,
            sea_orm::sea_query::Expr::value(Option::<i64>::None),
        )
        .col_expr(
            target_filter::Column::AutoAssignActionType,
            sea_orm::sea_query::Expr::value(Option::<String>::None),
        )
        .filter(target_filter::Column::AutoAssignDsId.eq(id))
        .exec(&txn)
        .await?;

    // The set's own rows, remembering its modules for the orphan check.
    let module_ids: Vec<i64> = ds_module::Entity::find()
        .filter(ds_module::Column::DsId.eq(id))
        .all(&txn)
        .await?
        .into_iter()
        .map(|m| m.module_id)
        .collect();
    ds_module::Entity::delete_many()
        .filter(ds_module::Column::DsId.eq(id))
        .exec(&txn)
        .await?;
    ds_metadata::Entity::delete_many()
        .filter(ds_metadata::Column::DsId.eq(id))
        .exec(&txn)
        .await?;
    ds_tag_assignment::Entity::delete_many()
        .filter(ds_tag_assignment::Column::DsId.eq(id))
        .exec(&txn)
        .await?;
    distribution_set::Entity::delete_by_id(id)
        .exec(&txn)
        .await?;

    // Modules no remaining set uses. One shared with a kept release stays.
    let mut modules = 0;
    let mut shas = Vec::new();
    for mid in module_ids {
        let refs = ds_module::Entity::find()
            .filter(ds_module::Column::ModuleId.eq(mid))
            .count(&txn)
            .await?;
        if refs == 0 {
            shas.extend(crate::domain::artifact::delete_module_rows(&txn, mid).await?);
            modules += 1;
        }
    }
    txn.commit().await?;

    for aid in action_ids {
        st.events.clear_action(aid);
    }
    for cid in &touched {
        crate::domain::deployment::publish_target(st, cid);
    }
    shas.sort();
    shas.dedup();
    let mut blobs = 0;
    for sha in &shas {
        if crate::domain::artifact::gc_blob(st, sha).await? {
            blobs += 1;
        }
    }
    tracing::info!(
        ds_id = id,
        name = %ds.name,
        version = %ds.version,
        modules,
        blobs,
        "release retired"
    );
    Ok(Retired {
        sets: 1,
        modules,
        blobs,
    })
}
