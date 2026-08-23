use sea_orm_migration::prelude::*;

#[derive(DeriveIden)]
enum Target {
    Table,
    GroupName,
}

/// hawkBit's single-valued organisational placement for a target (`group` on
/// the wire), distinct from tags: one group per target, free-form and
/// `/`-separated for hierarchy, versus tags' many-per-target labels.
///
/// The column is `group_name` rather than `group` because `GROUP` is a reserved
/// word in both SQLite and Postgres. SeaQuery quotes identifiers so a bare
/// `group` would likely work, but the wire contract is what has to match
/// hawkBit — the storage name is internal, and `target_type` → `targetType`
/// already sets that precedent — so there is nothing to gain from betting on
/// quoting behaviour in two backends.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Target::Table)
                .add_column(ColumnDef::new(Target::GroupName).string().null())
                .to_owned(),
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Target::Table)
                .drop_column(Target::GroupName)
                .to_owned(),
        )
        .await
    }
}
