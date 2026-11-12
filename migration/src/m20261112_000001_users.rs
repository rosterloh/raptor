//! Operator accounts beyond the `[mgmt]` config admin (issue #13). The config
//! admin is never stored here, so an empty table means the old behaviour.
//! Uniqueness is a named `(tenant, username)` index, not an inline
//! `unique_key()`, so it can be changed later without a SQLite table rebuild.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
    Tenant,
    Username,
    PasswordHash,
    Role,
    Source,
    Enabled,
    CreatedAt,
    UpdatedAt,
}

const INDEX: &str = "users_tenant_username";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(Users::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Users::Id)
                        .big_integer()
                        .not_null()
                        .auto_increment()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(Users::Tenant)
                        .string()
                        .not_null()
                        .default("DEFAULT"),
                )
                .col(ColumnDef::new(Users::Username).string().not_null())
                .col(ColumnDef::new(Users::PasswordHash).string().null())
                .col(ColumnDef::new(Users::Role).string().not_null())
                .col(ColumnDef::new(Users::Source).string().not_null())
                .col(
                    ColumnDef::new(Users::Enabled)
                        .boolean()
                        .not_null()
                        .default(true),
                )
                .col(ColumnDef::new(Users::CreatedAt).big_integer().not_null())
                .col(ColumnDef::new(Users::UpdatedAt).big_integer().not_null())
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name(INDEX)
                .table(Users::Table)
                .col(Users::Tenant)
                .col(Users::Username)
                .unique()
                .to_owned(),
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(Users::Table).to_owned())
            .await
    }
}
