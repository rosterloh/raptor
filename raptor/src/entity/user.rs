//! `users`: operator accounts beyond the `[mgmt]` config admin, which is never
//! stored here. `source` is `local` (argon2id `password_hash`) or `oidc`
//! (no password, created on first sign-in).

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(default_value = "DEFAULT")]
    pub tenant: String,
    pub username: String,
    pub password_hash: Option<String>,
    pub role: String,
    pub source: String,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
