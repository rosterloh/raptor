use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Target {
    Table,
    Id,
}
#[derive(DeriveIden)]
enum DistributionSet {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum TargetTag {
    Table,
    Id,
    Name,
    Description,
    Colour,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum DsTag {
    Table,
    Id,
    Name,
    Description,
    Colour,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum TargetTagAssignment {
    Table,
    TargetId,
    TagId,
}
#[derive(DeriveIden)]
enum DsTagAssignment {
    Table,
    DsId,
    TagId,
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
                .table(TargetTag::Table)
                .col(pk_i64(TargetTag::Id))
                .col(
                    ColumnDef::new(TargetTag::Name)
                        .string()
                        .not_null()
                        .unique_key(),
                )
                .col(ColumnDef::new(TargetTag::Description).string())
                .col(ColumnDef::new(TargetTag::Colour).string())
                .col(
                    ColumnDef::new(TargetTag::CreatedAt)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(TargetTag::UpdatedAt)
                        .big_integer()
                        .not_null(),
                )
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(DsTag::Table)
                .col(pk_i64(DsTag::Id))
                .col(ColumnDef::new(DsTag::Name).string().not_null().unique_key())
                .col(ColumnDef::new(DsTag::Description).string())
                .col(ColumnDef::new(DsTag::Colour).string())
                .col(ColumnDef::new(DsTag::CreatedAt).big_integer().not_null())
                .col(ColumnDef::new(DsTag::UpdatedAt).big_integer().not_null())
                .to_owned(),
        )
        .await?;

        // Join tables: a composite primary key makes an assignment idempotent at
        // the schema level, and deleting a tag only ever removes rows here.
        m.create_table(
            Table::create()
                .table(TargetTagAssignment::Table)
                .col(
                    ColumnDef::new(TargetTagAssignment::TargetId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(TargetTagAssignment::TagId)
                        .big_integer()
                        .not_null(),
                )
                .primary_key(
                    Index::create()
                        .col(TargetTagAssignment::TargetId)
                        .col(TargetTagAssignment::TagId),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(TargetTagAssignment::Table, TargetTagAssignment::TargetId)
                        .to(Target::Table, Target::Id),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(TargetTagAssignment::Table, TargetTagAssignment::TagId)
                        .to(TargetTag::Table, TargetTag::Id),
                )
                .to_owned(),
        )
        .await?;
        // `tag==x` resolves tag ids first, then looks up assignments by tag.
        m.create_index(
            Index::create()
                .name("ix_target_tag_assignment_tag")
                .table(TargetTagAssignment::Table)
                .col(TargetTagAssignment::TagId)
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(DsTagAssignment::Table)
                .col(
                    ColumnDef::new(DsTagAssignment::DsId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(DsTagAssignment::TagId)
                        .big_integer()
                        .not_null(),
                )
                .primary_key(
                    Index::create()
                        .col(DsTagAssignment::DsId)
                        .col(DsTagAssignment::TagId),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(DsTagAssignment::Table, DsTagAssignment::DsId)
                        .to(DistributionSet::Table, DistributionSet::Id),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(DsTagAssignment::Table, DsTagAssignment::TagId)
                        .to(DsTag::Table, DsTag::Id),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("ix_ds_tag_assignment_tag")
                .table(DsTagAssignment::Table)
                .col(DsTagAssignment::TagId)
                .to_owned(),
        )
        .await?;

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for t in [
            TableRef::Table(TargetTagAssignment::Table.into_iden()),
            TableRef::Table(DsTagAssignment::Table.into_iden()),
            TableRef::Table(TargetTag::Table.into_iden()),
            TableRef::Table(DsTag::Table.into_iden()),
        ] {
            m.drop_table(Table::drop().table(t).to_owned()).await?;
        }
        Ok(())
    }
}
