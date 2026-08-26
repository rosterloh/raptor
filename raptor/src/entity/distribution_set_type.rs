//! `distribution_set_type`: the type catalogue entry a `distribution_set` belongs to.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "distribution_set_type")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    /// Tenant this row belongs to — see `target::Model::tenant` for the rationale.
    #[sea_orm(default_value = "DEFAULT")]
    pub tenant: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
