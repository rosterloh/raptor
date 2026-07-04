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
    pub forced: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
