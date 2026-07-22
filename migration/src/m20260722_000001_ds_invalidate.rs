use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum DistributionSet {
    Table,
    Invalid,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(DistributionSet::Table)
                .add_column(
                    ColumnDef::new(DistributionSet::Invalid)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .to_owned(),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(DistributionSet::Table)
                .drop_column(DistributionSet::Invalid)
                .to_owned(),
        )
        .await?;
        Ok(())
    }
}
