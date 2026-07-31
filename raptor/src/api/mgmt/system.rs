//! System endpoints: tenant configuration (read-only, file-driven) and fleet
//! statistics.

use crate::config::Config;
use crate::entity::{action, distribution_set, rollout, software_module, target};
use crate::error::AppError;
use crate::state::AppState;
use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::routing::get;
use raptor_api_types::{SystemStatistics, TenantConfigValue};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/rest/v1/system/configs", get(get_configs))
        .route(
            "/rest/v1/system/configs/{key}",
            get(get_config)
                .put(config_read_only)
                .delete(config_read_only),
        )
        .route("/rest/v1/system/statistics", get(statistics))
}

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

/// Query params for [`statistics`]. `q` is the same FIQL the target list and
/// saved target filters accept, so a console can ask for one saved filter's
/// numbers with the query it already has.
#[derive(Debug, Deserialize)]
pub struct StatsParams {
    pub q: Option<String>,
}

pub async fn statistics(
    State(st): State<AppState>,
    Query(p): Query<StatsParams>,
) -> Result<Json<SystemStatistics>, AppError> {
    // Only the target counters can be scoped by a target filter. The catalogue
    // and action totals below keep their fleet-wide meaning whether or not `q`
    // is set — scoping "how many distribution sets exist" to a set of targets
    // has no meaning, and zeroing them would read as data loss. Documented in
    // docs/src/reference/management-api.md.
    let filter = match p.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        Some(q) => Some(super::targets::condition(q)?),
        None => None,
    };
    let scoped = || {
        let sel = target::Entity::find();
        match &filter {
            Some(c) => sel.filter(c.clone()),
            None => sel,
        }
    };

    let total_targets = scoped().count(&st.db).await?;
    let total_distribution_sets = distribution_set::Entity::find().count(&st.db).await?;
    let total_software_modules = software_module::Entity::find().count(&st.db).await?;
    let total_actions = action::Entity::find().count(&st.db).await?;
    let total_rollouts = rollout::Entity::find().count(&st.db).await?;
    let active_actions = action::Entity::find()
        .filter(action::Column::Active.eq(true))
        .count(&st.db)
        .await?;

    let rows: Vec<(String, i64)> = scoped()
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
