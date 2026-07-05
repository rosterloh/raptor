use crate::auth::ddi::AuthKind;
use crate::entity::target_attribute;
use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
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
    Path((_tenant, cid)): Path<(String, String)>,
    Json(body): Json<ConfigData>,
) -> Result<StatusCode, AppError> {
    let t = super::root::get_or_register(&st, &cid, auth).await?;
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
                .filter(
                    target_attribute::Column::Key
                        .is_in(body.data.keys().cloned().collect::<Vec<_>>()),
                )
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
    Ok(StatusCode::OK)
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
