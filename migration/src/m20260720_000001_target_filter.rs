use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum TargetFilter {
    Table,
    Id,
    Name,
    Query,
    AutoAssignDsId,
    AutoAssignActionType,
    CreatedAt,
    UpdatedAt,
}

fn pk_i64<T: IntoIden + 'static>(col: T) -> ColumnDef {
    ColumnDef::new(col)
        .big_integer()
        .not_null()
        .auto_increment()
        .primary_key()
        .to_owned()
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.create_table(
            Table::create()
                .table(TargetFilter::Table)
                .col(pk_i64(TargetFilter::Id))
                .col(
                    ColumnDef::new(TargetFilter::Name)
                        .string()
                        .not_null()
                        .unique_key(),
                )
                .col(ColumnDef::new(TargetFilter::Query).text().not_null())
                .col(ColumnDef::new(TargetFilter::AutoAssignDsId).big_integer())
                .col(ColumnDef::new(TargetFilter::AutoAssignActionType).string())
                .col(
                    ColumnDef::new(TargetFilter::CreatedAt)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(TargetFilter::UpdatedAt)
                        .big_integer()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(TargetFilter::Table, TargetFilter::AutoAssignDsId)
                        .to(
                            sea_orm_migration::prelude::Alias::new("distribution_set"),
                            sea_orm_migration::prelude::Alias::new("id"),
                        ),
                )
                .to_owned(),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(Table::drop().table(TargetFilter::Table).to_owned())
            .await?;
        Ok(())
    }
}
