//! Controller poll base (`GET .../controller/v1/{controllerId}`):
//! registers/looks up the target and reports the actionable `_links`
//! (deploymentBase, confirmationBase, cancelAction, installedBase) for its
//! current state. `get_or_register` is also the shared target-lookup used
//! by every other DDI handler.

use crate::auth::ddi::AuthKind;
use crate::domain::deployment::active_action;
use crate::entity::{action, target};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::{base_url, client_address, now_ms, random_token};
use axum::extract::{ConnectInfo, Path, State};
use axum::http::HeaderMap;
use axum::{Extension, Json};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder,
};
use serde_json::{Map, Value, json};
use std::net::SocketAddr;

/// Resolves this poll's advertised sleep interval. Override rules (from
/// `[ddi] polling_interval`, hawkBit's `pollingTime` grammar — see
/// `Config::polling_schedule`) are evaluated in written order via one
/// indexed point-query each — `target.id = ? AND <rule>` — reusing the same
/// FIQL → SeaORM compiler the Management API's `q=` filters use
/// (`api::mgmt::targets::condition`), so `group==`, `tag==` and
/// `attribute.<key>==` mean the same thing here as everywhere else raptor
/// accepts a filter, rather than a second, parallel evaluator that could
/// drift from it. First match wins; with no rules configured (the common
/// case) this issues no extra query at all.
async fn resolve_polling_interval(st: &AppState, t: &target::Model) -> Result<String, AppError> {
    let schedule = st.cfg.ddi.polling_schedule().unwrap_or_else(|_| {
        // Startup validation (main.rs) already rejects a malformed
        // `polling_interval`; this only guards handlers built directly on a
        // `Config` that skipped that step (e.g. some test harnesses).
        crate::config::PollingSchedule {
            default: crate::config::PollingInterval {
                duration: std::time::Duration::from_secs(300),
                deviation_percent: 0,
            },
            rules: Vec::new(),
        }
    });

    for rule in &schedule.rules {
        let cond = match crate::api::mgmt::targets::condition(&rule.filter) {
            Ok(c) => c,
            Err(e) => {
                // Shouldn't happen — startup validation compiles every rule
                // the same way — but a query failure here must not 500 every
                // device's poll, so skip the rule rather than propagate.
                tracing::warn!(
                    filter = %rule.filter,
                    error = ?e,
                    "invalid pollingTime override filter, skipping"
                );
                continue;
            }
        };
        let matched = target::Entity::find()
            .filter(target::Column::Id.eq(t.id))
            .filter(cond)
            .one(&st.db)
            .await?
            .is_some();
        if matched {
            return Ok(rule.interval.resolve());
        }
    }
    Ok(schedule.default.resolve())
}

/// Looks up (or auto-registers) the polling target and stamps `last_poll_at`.
/// `address` is the device's source address, recorded so a DDI-registered target
/// shows a last-seen address without an operator setting one by hand.
pub async fn get_or_register(
    st: &AppState,
    cid: &str,
    auth: AuthKind,
    address: Option<&str>,
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
            let created = target::ActiveModel {
                controller_id: Set(cid.to_string()),
                name: Set(cid.to_string()),
                security_token: Set(random_token()),
                update_status: Set("registered".into()),
                auto_confirm: Set(st.cfg.ddi.auto_confirm_default),
                address: Set(address.map(str::to_string)),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            }
            .insert(&st.db)
            .await?;
            // A freshly registered target may match a saved filter with an
            // attached auto-assign DS; assign it now rather than waiting for the
            // periodic sweep so this very poll can return a deploymentBase link.
            crate::domain::target_filter::auto_assign_for_target(st, &created).await?;
            target::Entity::find_by_id(created.id)
                .one(&st.db)
                .await?
                .ok_or(AppError::NotFound("target"))?
        }
    };
    let mut am: target::ActiveModel = t.clone().into();
    am.last_poll_at = Set(Some(now_ms()));
    // Only touch address/updated_at when it actually moved, so a stable fleet
    // doesn't rewrite updated_at on every poll.
    if let Some(a) = address
        && t.address.as_deref() != Some(a)
    {
        am.address = Set(Some(a.to_string()));
        am.updated_at = Set(now_ms());
    }
    Ok(am.update(&st.db).await?)
}

pub async fn poll(
    State(st): State<AppState>,
    Extension(auth): Extension<AuthKind>,
    // Option<Extension<..>> rather than Option<ConnectInfo<..>>: axum 0.8 has no
    // optional impl for ConnectInfo, and `into_make_service_with_connect_info`
    // puts the same value in extensions. Absent under `Router::oneshot` in tests.
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Path((_tenant, cid)): Path<(String, String)>,
) -> Result<Json<Value>, AppError> {
    let addr = client_address(&st.cfg, &headers, peer.map(|Extension(ConnectInfo(p))| p));
    let t = get_or_register(&st, &cid, auth, addr.as_deref()).await?;
    let base = super::ddi_base(&base_url(&st.cfg, &headers), &st.cfg.tenant, &cid);
    let sleep = resolve_polling_interval(&st, &t).await?;

    let mut links = Map::new();
    // Only ask for attributes when we actually want them: clients such as the
    // Zephyr hawkbit client re-upload their whole attribute set on every poll
    // that carries this link.
    if t.request_attributes {
        links.insert(
            "configData".into(),
            json!({"href": format!("{base}/configData")}),
        );
    }
    if let Some(a) = active_action(&st.db, t.id).await? {
        match a.status.as_str() {
            "running" => {
                links.insert(
                    "deploymentBase".into(),
                    json!({"href": format!("{base}/deploymentBase/{}", a.id)}),
                );
            }
            "wait_for_confirmation" => {
                links.insert(
                    "confirmationBase".into(),
                    json!({"href": format!("{base}/confirmationBase/{}", a.id)}),
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
        "config": {"polling": {"sleep": sleep}},
        "_links": Value::Object(links)
    })))
}
