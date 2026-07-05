use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "target")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub controller_id: String,
    pub name: String,
    pub description: Option<String>,
    pub security_token: String,
    pub update_status: String,
    pub last_poll_at: Option<i64>,
    pub address: Option<String>,
    pub assigned_ds_id: Option<i64>,
    pub installed_ds_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
