//! `rollout_group`: one ordered batch of a `rollout`, with its own success/error thresholds.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "rollout_group")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub rollout_id: i64,
    pub name: String,
    pub order_index: i64,
    pub status: String,
    pub total_targets: i64,
    pub success_threshold: i64,
    pub error_threshold: i64,
    pub created_at: i64,
    pub updated_at: i64,
    /// Outcomes of actions automatic cleanup has deleted, kept so the group's
    /// `totalTargetsPerStatus` still reflects what happened — see the
    /// `m20260907_000001_rollout_group_purged_counts` migration.
    #[sea_orm(default_value = 0)]
    pub purged_finished: i64,
    #[sea_orm(default_value = 0)]
    pub purged_error: i64,
    #[sea_orm(default_value = 0)]
    pub purged_cancelled: i64,
    #[sea_orm(default_value = 0)]
    pub purged_running: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
