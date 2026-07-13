use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Rollout {
    Table,
    Id,
    Name,
    Description,
    DsId,
    TargetFilter,
    Status,
    TotalTargets,
    GroupCount,
    SuccessThreshold,
    ErrorThreshold,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum RolloutGroup {
    Table,
    Id,
    RolloutId,
    Name,
    OrderIndex,
    Status,
    TotalTargets,
    SuccessThreshold,
    ErrorThreshold,
    CreatedAt,
    UpdatedAt,
}
#[derive(DeriveIden)]
enum RolloutTargetGroup {
    Table,
    Id,
    RolloutGroupId,
    TargetId,
}
#[derive(DeriveIden)]
enum Action {
    Table,
    RolloutId,
    RolloutGroupId,
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
                .table(Rollout::Table)
                .col(pk_i64(Rollout::Id))
                .col(
                    ColumnDef::new(Rollout::Name)
                        .string()
                        .not_null()
                        .unique_key(),
                )
                .col(ColumnDef::new(Rollout::Description).string())
                .col(ColumnDef::new(Rollout::DsId).big_integer().not_null())
                .col(ColumnDef::new(Rollout::TargetFilter).text().not_null())
                .col(ColumnDef::new(Rollout::Status).string().not_null())
                .col(
                    ColumnDef::new(Rollout::TotalTargets)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(Rollout::GroupCount).big_integer().not_null())
                .col(
                    ColumnDef::new(Rollout::SuccessThreshold)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Rollout::ErrorThreshold)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(Rollout::CreatedAt).big_integer().not_null())
                .col(ColumnDef::new(Rollout::UpdatedAt).big_integer().not_null())
                .foreign_key(ForeignKey::create().from(Rollout::Table, Rollout::DsId).to(
                    sea_orm_migration::prelude::Alias::new("distribution_set"),
                    sea_orm_migration::prelude::Alias::new("id"),
                ))
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(RolloutGroup::Table)
                .col(pk_i64(RolloutGroup::Id))
                .col(
                    ColumnDef::new(RolloutGroup::RolloutId)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(RolloutGroup::Name).string().not_null())
                .col(
                    ColumnDef::new(RolloutGroup::OrderIndex)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(RolloutGroup::Status).string().not_null())
                .col(
                    ColumnDef::new(RolloutGroup::TotalTargets)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RolloutGroup::SuccessThreshold)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RolloutGroup::ErrorThreshold)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RolloutGroup::CreatedAt)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RolloutGroup::UpdatedAt)
                        .big_integer()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(RolloutGroup::Table, RolloutGroup::RolloutId)
                        .to(Rollout::Table, Rollout::Id),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("ux_rollout_group_order")
                .table(RolloutGroup::Table)
                .col(RolloutGroup::RolloutId)
                .col(RolloutGroup::OrderIndex)
                .unique()
                .to_owned(),
        )
        .await?;

        m.create_table(
            Table::create()
                .table(RolloutTargetGroup::Table)
                .col(pk_i64(RolloutTargetGroup::Id))
                .col(
                    ColumnDef::new(RolloutTargetGroup::RolloutGroupId)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(RolloutTargetGroup::TargetId)
                        .big_integer()
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(
                            RolloutTargetGroup::Table,
                            RolloutTargetGroup::RolloutGroupId,
                        )
                        .to(RolloutGroup::Table, RolloutGroup::Id),
                )
                .foreign_key(
                    ForeignKey::create()
                        .from(RolloutTargetGroup::Table, RolloutTargetGroup::TargetId)
                        .to(
                            sea_orm_migration::prelude::Alias::new("target"),
                            sea_orm_migration::prelude::Alias::new("id"),
                        ),
                )
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("ux_rollout_target_group")
                .table(RolloutTargetGroup::Table)
                .col(RolloutTargetGroup::RolloutGroupId)
                .col(RolloutTargetGroup::TargetId)
                .unique()
                .to_owned(),
        )
        .await?;
        m.create_index(
            Index::create()
                .name("ix_rollout_target_group_target")
                .table(RolloutTargetGroup::Table)
                .col(RolloutTargetGroup::TargetId)
                .to_owned(),
        )
        .await?;

        m.alter_table(
            Table::alter()
                .table(Action::Table)
                .add_column(ColumnDef::new(Action::RolloutId).big_integer())
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Action::Table)
                .add_column(ColumnDef::new(Action::RolloutGroupId).big_integer())
                .to_owned(),
        )
        .await?;

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Action::Table)
                .drop_column(Action::RolloutGroupId)
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Action::Table)
                .drop_column(Action::RolloutId)
                .to_owned(),
        )
        .await?;
        for t in [
            TableRef::Table(RolloutTargetGroup::Table.into_iden()),
            TableRef::Table(RolloutGroup::Table.into_iden()),
            TableRef::Table(Rollout::Table.into_iden()),
        ] {
            m.drop_table(Table::drop().table(t).to_owned()).await?;
        }
        Ok(())
    }
}
