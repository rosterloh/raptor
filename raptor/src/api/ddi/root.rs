use crate::auth::ddi::AuthKind;
use crate::domain::deployment::active_action;
use crate::entity::{action, target};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::{base_url, now_ms, random_token};
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::{Extension, Json};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder,
};
use serde_json::{json, Map, Value};

pub async fn get_or_register(
    st: &AppState,
    cid: &str,
    auth: AuthKind,
) -> Result<target::Model, AppError> {
    let existing = target::Entity::find()
        .filter(target::Column::ControllerId.eq(cid))
        .one(&st.db)
        .await?;
    let t = match existing {
        Some(t) => t,
        None => {
            if auth == AuthKind::Target {
                return Err(AppError::NotFound("target")); // middleware already guards this
            }
            let now = now_ms();
            target::ActiveModel {
                controller_id: Set(cid.to_string()),
                name: Set(cid.to_string()),
                security_token: Set(random_token()),
                update_status: Set("registered".into()),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            }
            .insert(&st.db)
            .await?
        }
    };
    let mut am: target::ActiveModel = t.clone().into();
    am.last_poll_at = Set(Some(now_ms()));
    Ok(am.update(&st.db).await?)
}

pub async fn poll(
    State(st): State<AppState>,
    Extension(auth): Extension<AuthKind>,
    headers: HeaderMap,
    Path((_tenant, cid)): Path<(String, String)>,
) -> Result<Json<Value>, AppError> {
    let t = get_or_register(&st, &cid, auth).await?;
    let base = super::ddi_base(&base_url(&st.cfg, &headers), &cid);

    let mut links = Map::new();
    links.insert(
        "configData".into(),
        json!({"href": format!("{base}/configData")}),
    );
    if let Some(a) = active_action(&st.db, t.id).await? {
        match a.status.as_str() {
            "running" => {
                links.insert(
                    "deploymentBase".into(),
                    json!({"href": format!("{base}/deploymentBase/{}", a.id)}),
                );
            }
            "canceling" => {
                links.insert(
                    "cancelAction".into(),
                    json!({"href": format!("{base}/cancelAction/{}", a.id)}),
                );
            }
            _ => {} // unknown active status: no actionable link
        }
    }
    if let Some(installed) = action::Entity::find()
        .filter(action::Column::TargetId.eq(t.id))
        .filter(action::Column::Status.eq("finished"))
        .order_by(action::Column::Id, Order::Desc)
        .one(&st.db)
        .await?
    {
        links.insert(
            "installedBase".into(),
            json!({"href": format!("{base}/installedBase/{}", installed.id)}),
        );
    }

    Ok(Json(json!({
        "config": {"polling": {"sleep": st.cfg.ddi.polling_interval}},
        "_links": Value::Object(links)
    })))
}
