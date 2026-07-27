//! `action`: a device's in-progress or historical deployment/cancel action.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "action")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub target_id: i64,
    pub ds_id: i64,
    pub status: String,
    pub active: bool,
    /// hawkBit assignment type: `forced`, `soft`, `timeforced` or
    /// `downloadonly`. See [`crate::domain::deployment::ddi_modes`].
    #[sea_orm(default_value = "forced")]
    pub action_type: String,
    /// For `timeforced` only: soft until this instant, forced after it. `None`
    /// behaves as already-reached, matching hawkBit's default of 0.
    pub forced_time: Option<i64>,
    pub rollout_id: Option<i64>,
    pub rollout_group_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
