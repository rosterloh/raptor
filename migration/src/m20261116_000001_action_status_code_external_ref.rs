//! DDI feedback `status.code` (issue #216) and assignment `externalRef`
//! (issue #212). One column per `ALTER TABLE`: SQLite can't add several at once.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Action {
    Table,
    LastStatusCode,
    ExternalRef,
}

#[derive(DeriveIden)]
enum ActionStatus {
    Table,
    Code,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Action::Table)
                .add_column(ColumnDef::new(Action::LastStatusCode).integer().null())
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Action::Table)
                .add_column(ColumnDef::new(Action::ExternalRef).string_len(512).null())
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(ActionStatus::Table)
                .add_column(ColumnDef::new(ActionStatus::Code).integer().null())
                .to_owned(),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(ActionStatus::Table)
                .drop_column(ActionStatus::Code)
                .to_owned(),
        )
        .await?;
        for col in [Action::ExternalRef, Action::LastStatusCode] {
            m.alter_table(
                Table::alter()
                    .table(Action::Table)
                    .drop_column(col)
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
