//! `ds_tag_assignment`: join table linking a `ds_tag` to a `distribution_set`.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "ds_tag_assignment")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub ds_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub tag_id: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
