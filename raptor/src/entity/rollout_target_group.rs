use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "rollout_target_group")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub rollout_group_id: i64,
    pub target_id: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
