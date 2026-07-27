use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Target {
    Table,
    RequestAttributes,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Defaults to true, including for existing rows: a target we have not
        // (re-)asked yet gets asked once after the upgrade, which is one extra
        // configData upload per device and then silence — cheaper than a
        // backfill that inspects target_attribute.
        m.alter_table(
            Table::alter()
                .table(Target::Table)
                .add_column(
                    ColumnDef::new(Target::RequestAttributes)
                        .boolean()
                        .not_null()
                        .default(true),
                )
                .to_owned(),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Target::Table)
                .drop_column(Target::RequestAttributes)
                .to_owned(),
        )
        .await?;
        Ok(())
    }
}
