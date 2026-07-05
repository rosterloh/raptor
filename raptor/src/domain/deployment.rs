use crate::entity::{action, action_status, action_status_message, distribution_set, target};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::now_ms;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::json;

pub struct AssignResult {
    pub action_id: Option<i64>,
    pub already_assigned: bool,
}

pub async fn add_action_status(
    db: &DatabaseConnection, action_id: i64, status: &str, messages: &[String],
) -> Result<(), AppError> {
    let row = action_status::ActiveModel {
        action_id: Set(action_id), status: Set(status.to_string()), created_at: Set(now_ms()),
        ..Default::default()
    }.insert(db).await?;
    for m in messages {
        action_status_message::ActiveModel {
            action_status_id: Set(row.id), message: Set(m.clone()),
            ..Default::default()
        }.insert(db).await?;
    }
    Ok(())
}

pub async fn active_action(db: &DatabaseConnection, target_id: i64) -> Result<Option<action::Model>, AppError> {
    Ok(action::Entity::find()
        .filter(action::Column::TargetId.eq(target_id))
        .filter(action::Column::Active.eq(true))
        .one(db).await?)
}

pub async fn assign_ds(
    st: &AppState, target: &target::Model, ds_id: i64, forced: bool,
) -> Result<AssignResult, AppError> {
    let ds = distribution_set::Entity::find_by_id(ds_id).one(&st.db).await?
        .ok_or(AppError::NotFound("distribution set"))?;
    if !ds.complete {
        return Err(AppError::BadRequest("distribution set is incomplete".into()));
    }
    if let Some(current) = active_action(&st.db, target.id).await? {
        if current.ds_id == ds.id {
            return Ok(AssignResult { action_id: None, already_assigned: true });
        }
        // v1: hard-cancel the superseded action (hawkBit soft-cancels; devices tolerate both)
        let cid = current.id;
        let mut am: action::ActiveModel = current.into();
        am.status = Set("canceled".into());
        am.active = Set(false);
        am.updated_at = Set(now_ms());
        am.update(&st.db).await?;
        add_action_status(&st.db, cid, "canceled", &["superseded by new assignment".into()]).await?;
    }
    let now = now_ms();
    let a = action::ActiveModel {
        target_id: Set(target.id), ds_id: Set(ds.id),
        status: Set("running".into()), active: Set(true), forced: Set(forced),
        created_at: Set(now), updated_at: Set(now),
        ..Default::default()
    }.insert(&st.db).await?;
    add_action_status(&st.db, a.id, "running", &[]).await?;

    let mut tm: target::ActiveModel = target.clone().into();
    tm.assigned_ds_id = Set(Some(ds.id));
    tm.update_status = Set("pending".into());
    tm.updated_at = Set(now);
    tm.update(&st.db).await?;

    Ok(AssignResult { action_id: Some(a.id), already_assigned: false })
}

pub fn action_rest(a: &action::Model, base: &str) -> serde_json::Value {
    let is_cancel = matches!(a.status.as_str(), "canceling" | "canceled");
    json!({
        "id": a.id,
        "type": if is_cancel { "cancel" } else { "update" },
        "status": if a.active { "pending" } else { "finished" },
        "detailStatus": a.status,
        "forceType": if a.forced { "forced" } else { "soft" },
        "createdAt": a.created_at,
        "lastModifiedAt": a.updated_at,
        "_links": {
            "self": {"href": format!("{base}/rest/v1/actions/{}", a.id)},
            "distributionset": {"href": format!("{base}/rest/v1/distributionsets/{}", a.ds_id)}
        }
    })
}
