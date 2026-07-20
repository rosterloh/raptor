use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "target_filter")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub name: String,
    /// FIQL query string evaluated against targets.
    pub query: String,
    /// When set, targets matching `query` are auto-assigned this distribution set.
    pub auto_assign_ds_id: Option<i64>,
    /// Action type for the auto-assignment ("forced" or "soft").
    pub auto_assign_action_type: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
