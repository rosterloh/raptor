use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Action {
    Table,
    DeploymentFetchCount,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Action::Table)
                .add_column(
                    ColumnDef::new(Action::DeploymentFetchCount)
                        .integer()
                        .not_null()
                        .default(0),
                )
                .to_owned(),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Action::Table)
                .drop_column(Action::DeploymentFetchCount)
                .to_owned(),
        )
        .await?;
        Ok(())
    }
}
