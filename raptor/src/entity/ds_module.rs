use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "ds_module")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub ds_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub module_id: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
