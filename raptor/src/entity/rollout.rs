//! `rollout`: a staged, group-by-group deployment of a distribution set
//! across the targets matched by a filter query.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "rollout")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub ds_id: i64,
    pub target_filter: String,
    pub status: String,
    /// Action type inherited by every action this rollout creates — one of
    /// hawkBit's `forced`, `soft`, `timeforced`, `downloadonly`.
    #[sea_orm(default_value = "forced")]
    pub action_type: String,
    /// `timeforced` deadline passed on to the created actions.
    pub forced_time: Option<i64>,
    pub total_targets: i64,
    pub group_count: i64,
    pub success_threshold: i64,
    pub error_threshold: i64,
    pub created_at: i64,
    pub updated_at: i64,
    /// Operator who approved or denied this rollout, set by the approval
    /// workflow (#17). `None` until a decision is taken — and forever, on a
    /// rollout created while `rollout_approval_enabled` was off.
    pub approval_decided_by: Option<String>,
    /// Free-form note left with the approve/deny decision.
    pub approval_remark: Option<String>,
    /// Tenant this row belongs to — see `target::Model::tenant` for the rationale.
    #[sea_orm(default_value = "DEFAULT")]
    pub tenant: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
