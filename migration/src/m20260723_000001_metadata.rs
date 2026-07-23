use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Target {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum SoftwareModule {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum DistributionSet {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum TargetMetadata {
    Table,
    Id,
    TargetId,
    Key,
    Value,
}
#[derive(DeriveIden)]
enum SmMetadata {
    Table,
    Id,
    ModuleId,
    Key,
    Value,
    TargetVisible,
}
#[derive(DeriveIden)]
enum DsMetadata {
    Table,
    Id,
    DsId,
    Key,
    Value,
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
                .table(TargetMetadata::Table)
                .col(pk_i64(TargetMetadata::Id))
                .col(
                    ColumnDef::new(TargetMetadata::TargetId)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(TargetMetadata::Key).string().not_null())
                .col(ColumnDef::new(TargetMetadata::Value).string().not_null())
                .foreign_key(
                    ForeignKey::create()
                        .from(TargetMetadata::Table, TargetMetadata::TargetId)
                        .to(Target::Table, Target::Id),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("ux_target_metadata")
                .table(TargetMetadata::Table)
                .col(TargetMetadata::TargetId)
                .col(TargetMetadata::Key)
                .unique()
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(SmMetadata::Table)
                .col(pk_i64(SmMetadata::Id))
                .col(
                    ColumnDef::new(SmMetadata::ModuleId)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(SmMetadata::Key).string().not_null())
                .col(ColumnDef::new(SmMetadata::Value).string().not_null())
                .col(
                    ColumnDef::new(SmMetadata::TargetVisible)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(SmMetadata::Table, SmMetadata::ModuleId)
                        .to(SoftwareModule::Table, SoftwareModule::Id),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("ux_sm_metadata")
                .table(SmMetadata::Table)
                .col(SmMetadata::ModuleId)
                .col(SmMetadata::Key)
                .unique()
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(DsMetadata::Table)
                .col(pk_i64(DsMetadata::Id))
                .col(ColumnDef::new(DsMetadata::DsId).big_integer().not_null())
                .col(ColumnDef::new(DsMetadata::Key).string().not_null())
                .col(ColumnDef::new(DsMetadata::Value).string().not_null())
                .foreign_key(
                    ForeignKey::create()
                        .from(DsMetadata::Table, DsMetadata::DsId)
                        .to(DistributionSet::Table, DistributionSet::Id),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("ux_ds_metadata")
                .table(DsMetadata::Table)
                .col(DsMetadata::DsId)
                .col(DsMetadata::Key)
                .unique()
                .to_owned(),
        )
        .await?;

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for t in [
            TableRef::Table(TargetMetadata::Table.into_iden()),
            TableRef::Table(SmMetadata::Table.into_iden()),
            TableRef::Table(DsMetadata::Table.into_iden()),
        ] {
            m.drop_table(Table::drop().table(t).to_owned()).await?;
        }
        Ok(())
    }
}
