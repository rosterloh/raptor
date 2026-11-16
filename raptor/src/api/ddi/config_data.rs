//! `PUT .../configData`: device-reported attributes, applied as merge
//! (default), replace, or remove against `target_attribute`. May trigger a
//! re-evaluation of auto-assign target filters since attributes changed.

use crate::auth::ddi::AuthKind;
use crate::entity::target_attribute;
use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub struct ConfigData {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub data: BTreeMap<String, String>,
    // legacy fields (id, time, status) intentionally ignored
}

fn default_mode() -> String {
    "merge".into()
}

pub async fn put_config_data(
    State(st): State<AppState>,
    Extension(auth): Extension<AuthKind>,
    peer: Option<Extension<axum::extract::ConnectInfo<std::net::SocketAddr>>>,
    headers: axum::http::HeaderMap,
    Path((_tenant, cid)): Path<(String, String)>,
    Json(body): Json<ConfigData>,
) -> Result<StatusCode, AppError> {
    let addr = crate::util::client_address(
        &st.cfg,
        &headers,
        peer.map(|Extension(axum::extract::ConnectInfo(p))| p),
    );
    let t = super::root::get_or_register(&st, &cid, auth, addr.as_deref()).await?;
    check_attribute_lengths(&body.data)?;

    // `replace` starts from an empty set, so only what the request carries
    // counts; `merge` adds to what is already there. `remove` only deletes.
    if body.mode != "remove" {
        let existing = if body.mode == "replace" {
            0
        } else {
            target_attribute::Entity::find()
                .filter(target_attribute::Column::TargetId.eq(t.id))
                .count(&st.db)
                .await?
        };
        crate::domain::quota::assert_quota(
            existing,
            body.data.len() as u64,
            st.cfg.quota.max_attribute_entries_per_target,
            "attribute",
            &format!("target {cid}"),
        )?;
    }

    match body.mode.as_str() {
        "replace" => {
            target_attribute::Entity::delete_many()
                .filter(target_attribute::Column::TargetId.eq(t.id))
                .exec(&st.db)
                .await?;
            insert_all(&st, t.id, &body.data).await?;
        }
        "remove" => {
            target_attribute::Entity::delete_many()
                .filter(target_attribute::Column::TargetId.eq(t.id))
                .filter(target_attribute::Column::Key.is_in(body.data.keys().cloned()))
                .exec(&st.db)
                .await?;
        }
        _ => {
            // merge: upsert each key
            for (k, v) in &body.data {
                let existing = target_attribute::Entity::find()
                    .filter(target_attribute::Column::TargetId.eq(t.id))
                    .filter(target_attribute::Column::Key.eq(k))
                    .one(&st.db)
                    .await?;
                match existing {
                    Some(row) => {
                        let mut am: target_attribute::ActiveModel = row.into();
                        am.value = Set(v.clone());
                        am.update(&st.db).await?;
                    }
                    None => {
                        target_attribute::ActiveModel {
                            target_id: Set(t.id),
                            key: Set(k.clone()),
                            value: Set(v.clone()),
                            ..Default::default()
                        }
                        .insert(&st.db)
                        .await?;
                    }
                }
            }
        }
    }
    // Attributes have arrived: stop advertising the configData link until an
    // operator asks for them again (hawkBit `requestAttributes`).
    if t.request_attributes {
        let mut am: crate::entity::target::ActiveModel = t.clone().into();
        am.request_attributes = Set(false);
        am.update(&st.db).await?;
    }
    // Attribute changes can flip a target into a saved filter's match set;
    // re-evaluate auto-assign filters against it (no-op when none apply).
    crate::domain::target_filter::auto_assign_for_target(&st, &t).await?;
    Ok(StatusCode::OK)
}

/// hawkBit's per-key and per-value limit on controller attributes.
const MAX_ATTRIBUTE_LEN: usize = 128;

fn check_attribute_lengths(data: &BTreeMap<String, String>) -> Result<(), AppError> {
    for (k, v) in data {
        let (what, len) = if k.chars().count() > MAX_ATTRIBUTE_LEN {
            ("key", k.chars().count())
        } else if v.chars().count() > MAX_ATTRIBUTE_LEN {
            ("value", v.chars().count())
        } else {
            continue;
        };
        return Err(AppError::BadRequest(format!(
            "attribute {what} for {k:.32?} is {len} characters; the limit is {MAX_ATTRIBUTE_LEN}"
        )));
    }
    Ok(())
}

async fn insert_all(
    st: &AppState,
    target_id: i64,
    data: &BTreeMap<String, String>,
) -> Result<(), AppError> {
    for (k, v) in data {
        target_attribute::ActiveModel {
            target_id: Set(target_id),
            key: Set(k.clone()),
            value: Set(v.clone()),
            ..Default::default()
        }
        .insert(&st.db)
        .await?;
    }
    Ok(())
}
