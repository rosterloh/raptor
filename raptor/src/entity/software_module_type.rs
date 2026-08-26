//! `software_module_type`: the type catalogue entry a `software_module` belongs to.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "software_module_type")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    /// How many modules of this type a distribution set may contain.
    pub max_assignments: i32,
    /// Tenant this row belongs to — see `target::Model::tenant` for the rationale.
    #[sea_orm(default_value = "DEFAULT")]
    pub tenant: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
