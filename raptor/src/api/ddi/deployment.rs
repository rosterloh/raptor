use crate::entity::{
    action, action_status, action_status_message, artifact, ds_module, sm_metadata,
    software_module, target,
};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::base_url;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::{Extension, Json};
use sea_orm::{ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder};
use serde_json::{json, Value};

const HISTORY_LIMIT: usize = 10;

fn part_for(type_key: &str) -> &str {
    match type_key {
        "os" | "firmware" => "os",
        "runtime" => "jvm",
        "application" => "bApp",
        other => other,
    }
}

pub fn ddi_artifact_json(ar: &artifact::Model, ddi: &str, module_id: i64, https: bool) -> Value {
    let dl = format!(
        "{ddi}/softwaremodules/{module_id}/artifacts/{}",
        ar.filename
    );
    let mut l = json!({
        "download-http": {"href": dl},
        "md5sum-http": {"href": format!("{dl}.MD5SUM")}
    });
    if https {
        l["download"] = json!({"href": dl});
        l["md5sum"] = json!({"href": format!("{dl}.MD5SUM")});
    }
    json!({
        "filename": ar.filename,
        "hashes": {"sha1": ar.sha1, "md5": ar.md5, "sha256": ar.sha256},
        "size": ar.size,
        "_links": l
    })
}

pub async fn deployment_json(
    st: &AppState,
    cid: &str,
    a: &action::Model,
    base: &str,
) -> Result<Value, AppError> {
    deployment_json_keyed(st, cid, a, base, "deployment").await
}

/// Builds the DDI deployment/confirmation payload. `top_key` is `"deployment"`
/// for deploymentBase/installedBase and `"confirmation"` for confirmationBase —
/// the chunk/mode/history shape is identical (hawkBit parity).
pub async fn deployment_json_keyed(
    st: &AppState,
    cid: &str,
    a: &action::Model,
    base: &str,
    top_key: &str,
) -> Result<Value, AppError> {
    let ddi = super::ddi_base(base, cid);
    let https = base.starts_with("https://");
    let keys = crate::api::mgmt::software_modules::type_keys(&st.db).await?;

    let links = ds_module::Entity::find()
        .filter(ds_module::Column::DsId.eq(a.ds_id))
        .all(&st.db)
        .await?;
    let ids: Vec<i64> = links.iter().map(|l| l.module_id).collect();
    let modules = if ids.is_empty() {
        vec![]
    } else {
        software_module::Entity::find()
            .filter(software_module::Column::Id.is_in(ids))
            .all(&st.db)
            .await?
    };

    let mut chunks = Vec::with_capacity(modules.len());
    for m in &modules {
        let arts = artifact::Entity::find()
            .filter(artifact::Column::ModuleId.eq(m.id))
            .all(&st.db)
            .await?;
        let artifacts: Vec<Value> = arts
            .iter()
            .map(|ar| ddi_artifact_json(ar, &ddi, m.id, https))
            .collect();
        let key = keys.get(&m.type_id).map(String::as_str).unwrap_or("os");
        let mut chunk = json!({
            "part": part_for(key),
            "version": m.version,
            "name": m.name,
            "artifacts": artifacts
        });
        // targetVisible metadata surfaces to the device (hawkBit parity);
        // non-visible entries stay Management-API only.
        let visible = sm_metadata::Entity::find()
            .filter(sm_metadata::Column::ModuleId.eq(m.id))
            .filter(sm_metadata::Column::TargetVisible.eq(true))
            .order_by(sm_metadata::Column::Key, Order::Asc)
            .all(&st.db)
            .await?;
        if !visible.is_empty() {
            chunk["metadata"] = Value::Array(
                visible
                    .iter()
                    .map(|md| json!({"key": md.key, "value": md.value}))
                    .collect(),
            );
        }
        chunks.push(chunk);
    }

    // action history: last few status entries
    let statuses = action_status::Entity::find()
        .filter(action_status::Column::ActionId.eq(a.id))
        .order_by(action_status::Column::Id, Order::Desc)
        .all(&st.db)
        .await?;
    let mut messages = Vec::new();
    for s in statuses.iter().take(HISTORY_LIMIT) {
        for m in action_status_message::Entity::find()
            .filter(action_status_message::Column::ActionStatusId.eq(s.id))
            .order_by(action_status_message::Column::Id, Order::Desc)
            .all(&st.db)
            .await?
        {
            messages.push(m.message);
        }
    }
    let history_status = statuses
        .first()
        .map(|s| s.status.to_uppercase())
        .unwrap_or_else(|| "RUNNING".into());

    let mode = if a.forced { "forced" } else { "attempt" };
    let mut out = serde_json::Map::new();
    out.insert("id".into(), json!(a.id.to_string()));
    out.insert(
        top_key.to_string(),
        json!({"download": mode, "update": mode, "chunks": chunks}),
    );
    out.insert(
        "actionHistory".into(),
        json!({"status": history_status, "messages": messages}),
    );
    Ok(Value::Object(out))
}

pub async fn find_target_action(
    st: &AppState,
    cid: &str,
    action_id: i64,
) -> Result<(target::Model, action::Model), AppError> {
    let t = target::Entity::find()
        .filter(target::Column::ControllerId.eq(cid))
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("target"))?;
    let a = action::Entity::find_by_id(action_id)
        .one(&st.db)
        .await?
        .filter(|a| a.target_id == t.id)
        .ok_or(AppError::NotFound("action"))?;
    Ok((t, a))
}

pub async fn installed_base(
    State(st): State<AppState>,
    Extension(_auth): Extension<crate::auth::ddi::AuthKind>,
    headers: HeaderMap,
    Path((_tenant, cid, action_id)): Path<(String, String, i64)>,
) -> Result<Json<Value>, AppError> {
    let (_t, a) = find_target_action(&st, &cid, action_id).await?;
    if a.status != "finished" {
        return Err(AppError::NotFound("installed action"));
    }
    let base = base_url(&st.cfg, &headers);
    Ok(Json(deployment_json(&st, &cid, &a, &base).await?))
}

pub async fn deployment_base(
    State(st): State<AppState>,
    Extension(_auth): Extension<crate::auth::ddi::AuthKind>,
    headers: HeaderMap,
    Path((_tenant, cid, action_id)): Path<(String, String, i64)>,
) -> Result<Json<Value>, AppError> {
    let (_t, a) = find_target_action(&st, &cid, action_id).await?;
    if !a.active || a.status != "running" {
        return Err(AppError::NotFound("action"));
    }
    let base = base_url(&st.cfg, &headers);
    Ok(Json(deployment_json(&st, &cid, &a, &base).await?))
}
