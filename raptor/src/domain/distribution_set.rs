//! Distribution-set lifecycle rules that outlive a single request:
//! invalidation (hawkBit `MgmtInvalidateDistributionSetRequestBody`) and
//! deletion.
//!
//! Lives here rather than in the mgmt handler so background tasks can drive
//! the same transition the REST endpoint does.

use crate::entity::{
    action, distribution_set, ds_metadata, ds_module, ds_tag_assignment, rollout, target,
    target_filter,
};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::now_ms;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseTransaction, EntityTrait,
    QueryFilter,
};

/// How invalidation treats the set's in-flight actions. hawkBit spells these
/// `none`, `soft` and `force` in `actionCancelationType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancelMode {
    /// Leave active actions running to completion.
    None,
    /// Move active actions to `canceling`; the device confirms the cancel.
    Soft,
    /// Close active actions as `canceled` immediately and reset their targets.
    Force,
}

impl CancelMode {
    /// Parses hawkBit's wire spelling; `None` input means the default (`none`).
    pub fn parse(s: Option<&str>) -> Result<Self, AppError> {
        match s.unwrap_or("none") {
            "none" => Ok(Self::None),
            "soft" => Ok(Self::Soft),
            "force" => Ok(Self::Force),
            other => Err(AppError::BadRequest(format!(
                "invalid actionCancelationType: {other}"
            ))),
        }
    }
}

/// Marks a set invalid so it can no longer be assigned or rolled out, detaches
/// auto-assignments that reference it, and optionally stops its rollouts and
/// cancels its in-flight actions.
pub async fn invalidate(
    st: &AppState,
    id: i64,
    mode: CancelMode,
    cancel_rollouts: bool,
) -> Result<(), AppError> {
    let ds = distribution_set::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set"))?;
    if ds.invalid {
        return Err(AppError::Conflict(
            "distribution set already invalidated".into(),
        ));
    }

    // 1. Mark invalid — blocks any further assignment (see domain::assign_ds).
    let mut am: distribution_set::ActiveModel = ds.into();
    am.invalid = Set(true);
    am.updated_at = Set(now_ms());
    am.update(&st.db).await?;

    // 2. Detach auto-assignments that point at this set.
    for f in target_filter::Entity::find()
        .filter(target_filter::Column::AutoAssignDsId.eq(id))
        .all(&st.db)
        .await?
    {
        let mut fm: target_filter::ActiveModel = f.into();
        fm.auto_assign_ds_id = Set(None);
        fm.auto_assign_action_type = Set(None);
        fm.updated_at = Set(now_ms());
        fm.update(&st.db).await?;
    }

    // 3. Optionally stop rollouts deploying this set. "stopped" is terminal —
    //    the evaluator only advances "running" rollouts.
    if cancel_rollouts {
        for r in rollout::Entity::find()
            .filter(rollout::Column::DsId.eq(id))
            .filter(rollout::Column::Status.is_not_in(["finished", "stopped"]))
            .all(&st.db)
            .await?
        {
            let rid = r.id;
            let mut rm: rollout::ActiveModel = r.into();
            rm.status = Set("stopped".into());
            rm.updated_at = Set(now_ms());
            rm.update(&st.db).await?;
            crate::domain::rollout::publish_rollout(st, rid);
        }
    }

    // 4. Cancel in-flight actions referencing this set.
    if mode == CancelMode::None {
        return Ok(());
    }
    let reason = ["distribution set invalidated".to_string()];
    for a in action::Entity::find()
        .filter(action::Column::DsId.eq(id))
        .filter(action::Column::Active.eq(true))
        .all(&st.db)
        .await?
    {
        let target_id = a.target_id;
        let mut aam: action::ActiveModel = a.clone().into();
        if mode == CancelMode::Force {
            aam.status = Set("canceled".into());
            aam.active = Set(false);
            aam.updated_at = Set(now_ms());
            aam.update(&st.db).await?;
            crate::domain::deployment::add_action_status(st, &a, "canceled", &reason).await?;
            // Reset the target so it isn't left "pending" forever.
            if let Some(t) = target::Entity::find_by_id(target_id).one(&st.db).await? {
                crate::domain::deployment::revert_to_installed(&st.db, &t).await?;
                crate::domain::deployment::publish_target(st, &t.controller_id);
            }
            st.events.clear_action(a.id);
        } else {
            aam.status = Set("canceling".into());
            aam.updated_at = Set(now_ms());
            aam.update(&st.db).await?;
            crate::domain::deployment::add_action_status(st, &a, "canceling", &reason).await?;
        }
    }
    Ok(())
}

/// Deletes a set's own rows and releases everything pointing at it: targets,
/// auto-assign filters, tags, metadata, module links. Leaves its actions,
/// rollouts and modules to the caller. Returns the controller ids touched and
/// the set's module ids, for the caller's events and orphan check.
pub async fn delete_rows(
    txn: &DatabaseTransaction,
    id: i64,
) -> Result<(Vec<String>, Vec<i64>), AppError> {
    // Targets pointing at the set: an installed reference is cleared, and an
    // assignment falls back to whatever is still installed. A target left
    // with a different set assigned than installed is still mid-update on
    // another set and keeps its status.
    let mut touched = Vec::new();
    for t in target::Entity::find()
        .filter(
            Condition::any()
                .add(target::Column::AssignedDsId.eq(id))
                .add(target::Column::InstalledDsId.eq(id)),
        )
        .all(txn)
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
        tm.update(txn).await?;
    }

    // Retirement invalidated first, which detached these; a manual delete
    // did not.
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
        .exec(txn)
        .await?;

    // The set's own rows, remembering its modules for the orphan check.
    let module_ids: Vec<i64> = ds_module::Entity::find()
        .filter(ds_module::Column::DsId.eq(id))
        .all(txn)
        .await?
        .into_iter()
        .map(|m| m.module_id)
        .collect();
    ds_module::Entity::delete_many()
        .filter(ds_module::Column::DsId.eq(id))
        .exec(txn)
        .await?;
    ds_metadata::Entity::delete_many()
        .filter(ds_metadata::Column::DsId.eq(id))
        .exec(txn)
        .await?;
    ds_tag_assignment::Entity::delete_many()
        .filter(ds_tag_assignment::Column::DsId.eq(id))
        .exec(txn)
        .await?;
    distribution_set::Entity::delete_by_id(id).exec(txn).await?;

    Ok((touched, module_ids))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_mode_parses_hawkbit_spellings() {
        assert_eq!(CancelMode::parse(None).unwrap(), CancelMode::None);
        assert_eq!(CancelMode::parse(Some("none")).unwrap(), CancelMode::None);
        assert_eq!(CancelMode::parse(Some("soft")).unwrap(), CancelMode::Soft);
        assert_eq!(CancelMode::parse(Some("force")).unwrap(), CancelMode::Force);
        assert!(CancelMode::parse(Some("FORCE")).is_err());
    }
}
