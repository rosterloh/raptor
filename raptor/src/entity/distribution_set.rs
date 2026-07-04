use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "distribution_set")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub type_id: i64,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub required_migration_step: bool,
    pub complete: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
