use sea_orm::entity::prelude::*;

/// Composition of a distribution-set type: which software-module types it is
/// made of, and whether each is mandatory (required for a set to be `complete`)
/// or optional.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "ds_type_module")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub ds_type_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub module_type_id: i64,
    pub mandatory: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
