//! `action_status`: one status-history entry for an `action`.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "action_status")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub action_id: i64,
    pub status: String,
    /// Device-reported `status.code`, if any.
    pub code: Option<i32>,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
