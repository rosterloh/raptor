//! System endpoints: tenant configuration (read-only, file-driven) and fleet
//! statistics.

use crate::config::Config;
use crate::entity::{action, distribution_set, rollout, software_module, target};
use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::Json;
use raptor_api_types::{SystemStatistics, TenantConfigValue};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The tenant-configuration keys raptor derives from its config file. raptor is
/// single-tenant and config-file driven, so all values are `global` and
/// read-only. Keys mirror the hawkBit names clients commonly poll.
fn tenant_configs(cfg: &Config) -> BTreeMap<String, TenantConfigValue> {
    let g = |value: Value| TenantConfigValue {
        value,
        global: true,
    };
    let mut m = BTreeMap::new();
    m.insert("pollingTime".into(), g(json!(cfg.ddi.polling_interval)));
    m.insert(
        "authentication.gatewaytoken.enabled".into(),
        g(json!(cfg.ddi.gateway_token.is_some())),
    );
    m.insert("authentication.targettoken.enabled".into(), g(json!(true)));
    m.insert(
        "user.confirmation.flow.enabled".into(),
        g(json!(cfg.ddi.confirmation_flow)),
    );
    // Not implemented yet (#17 / #10); reported so clients see a definite value.
    m.insert("rollout.approval.enabled".into(), g(json!(false)));
    m.insert("multi.assignments.enabled".into(), g(json!(false)));
    m
}

pub async fn get_configs(State(st): State<AppState>) -> Json<BTreeMap<String, TenantConfigValue>> {
    Json(tenant_configs(&st.cfg))
}

pub async fn get_config(
    State(st): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<TenantConfigValue>, AppError> {
    tenant_configs(&st.cfg)
        .remove(&key)
        .map(Json)
        .ok_or(AppError::NotFound("configuration key"))
}

/// PUT/DELETE on a config key: raptor's configuration is file-driven, so writes
/// are refused rather than silently ignored.
pub async fn config_read_only(Path(_key): Path<String>) -> Result<Json<Value>, AppError> {
    Err(AppError::Forbidden(
        "tenant configuration is file-driven and read-only".into(),
    ))
}

pub async fn statistics(State(st): State<AppState>) -> Result<Json<SystemStatistics>, AppError> {
    let total_targets = target::Entity::find().count(&st.db).await?;
    let total_distribution_sets = distribution_set::Entity::find().count(&st.db).await?;
    let total_software_modules = software_module::Entity::find().count(&st.db).await?;
    let total_actions = action::Entity::find().count(&st.db).await?;
    let total_rollouts = rollout::Entity::find().count(&st.db).await?;
    let active_actions = action::Entity::find()
        .filter(action::Column::Active.eq(true))
        .count(&st.db)
        .await?;

    let rows: Vec<(String, i64)> = target::Entity::find()
        .select_only()
        .column(target::Column::UpdateStatus)
        .column_as(target::Column::Id.count(), "cnt")
        .group_by(target::Column::UpdateStatus)
        .into_tuple()
        .all(&st.db)
        .await?;
    let targets_by_status = rows.into_iter().map(|(s, c)| (s, c as u64)).collect();

    Ok(Json(SystemStatistics {
        total_targets,
        total_distribution_sets,
        total_software_modules,
        total_actions,
        total_rollouts,
        targets_by_status,
        active_actions,
    }))
}
