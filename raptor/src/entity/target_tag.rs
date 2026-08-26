//! `target_tag`: a named, colour-tagged label assignable to targets.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "target_tag")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub colour: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    /// Tenant this row belongs to — see `target::Model::tenant` for the rationale.
    #[sea_orm(default_value = "DEFAULT")]
    pub tenant: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
