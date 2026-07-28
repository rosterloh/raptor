use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Rollout {
    Table,
    #[sea_orm(iden = "action_type")]
    Type,
    ForcedTime,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // A rollout carries the action type its generated actions inherit.
        // Existing rollouts kept assigning forced actions, which is the default.
        // One column per statement: SQLite rejects multiple ALTER options.
        m.alter_table(
            Table::alter()
                .table(Rollout::Table)
                .add_column(
                    ColumnDef::new(Rollout::Type)
                        .string()
                        .not_null()
                        .default("forced"),
                )
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Rollout::Table)
                .add_column(ColumnDef::new(Rollout::ForcedTime).big_integer().null())
                .to_owned(),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Rollout::Table)
                .drop_column(Rollout::Type)
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Rollout::Table)
                .drop_column(Rollout::ForcedTime)
                .to_owned(),
        )
        .await?;
        Ok(())
    }
}
