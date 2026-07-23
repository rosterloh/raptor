use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "sm_metadata")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub module_id: i64,
    pub key: String,
    pub value: String,
    /// hawkBit `targetVisible`: surfaces the entry to devices in DDI
    /// `deploymentBase` chunks.
    pub target_visible: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
