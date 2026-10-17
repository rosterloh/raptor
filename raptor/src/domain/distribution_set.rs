//! Distribution-set lifecycle rules that outlive a single request: today,
//! invalidation (hawkBit `MgmtInvalidateDistributionSetRequestBody`).
//!
//! Lives here rather than in the mgmt handler so background tasks can drive
//! the same transition the REST endpoint does.

use crate::entity::{action, distribution_set, rollout, target, target_filter};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::now_ms;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

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
                let installed = t.installed_ds_id.is_some();
                let cid = t.controller_id.clone();
                let mut tm: target::ActiveModel = t.into();
                tm.update_status = Set(if installed { "in_sync" } else { "registered" }.into());
                tm.updated_at = Set(now_ms());
                tm.update(&st.db).await?;
                crate::domain::deployment::publish_target(st, &cid);
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
