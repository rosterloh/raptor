use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Action {
    Table,
    Forced,
    // spelled out rather than named `ActionType`, which would repeat the enum
    // name and trip clippy::enum_variant_names
    #[sea_orm(iden = "action_type")]
    Type,
    ForcedTime,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // hawkBit's four assignment types replace the forced/soft boolean.
        // `forced_time` only means anything for `timeforced`: soft until it is
        // reached, forced after. NULL there behaves as "already reached", which
        // is what hawkBit's default of 0 does.
        // One column per statement: SQLite rejects multiple ALTER options.
        m.alter_table(
            Table::alter()
                .table(Action::Table)
                .add_column(
                    ColumnDef::new(Action::Type)
                        .string()
                        .not_null()
                        .default("forced"),
                )
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Action::Table)
                .add_column(ColumnDef::new(Action::ForcedTime).big_integer().null())
                .to_owned(),
        )
        .await?;

        // Backfill from the boolean before it goes away.
        let db = m.get_connection();
        db.execute_unprepared(
            "UPDATE action SET action_type = CASE WHEN forced THEN 'forced' ELSE 'soft' END",
        )
        .await?;

        m.alter_table(
            Table::alter()
                .table(Action::Table)
                .drop_column(Action::Forced)
                .to_owned(),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Action::Table)
                .add_column(
                    ColumnDef::new(Action::Forced)
                        .boolean()
                        .not_null()
                        .default(true),
                )
                .to_owned(),
        )
        .await?;
        m.get_connection()
            .execute_unprepared("UPDATE action SET forced = (action_type <> 'soft')")
            .await?;
        m.alter_table(
            Table::alter()
                .table(Action::Table)
                .drop_column(Action::Type)
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Action::Table)
                .drop_column(Action::ForcedTime)
                .to_owned(),
        )
        .await?;
        Ok(())
    }
}
