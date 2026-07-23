use sea_orm::entity::prelude::*;

/// Distribution-set types a target type is compatible with. A typed target may
/// only be assigned distribution sets whose type appears here.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "target_type_ds_type")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub target_type_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub ds_type_id: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
