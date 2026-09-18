use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Rollout {
    Table,
    Dynamic,
    DynamicGroupSize,
}

#[derive(DeriveIden)]
enum RolloutGroup {
    Table,
    Dynamic,
}

/// hawkBit's dynamic rollouts: a trailing group that keeps absorbing targets
/// which start matching the filter after the rollout was created.
///
/// `dynamic_group_size` is the capacity of each dynamic group, resolved once at
/// creation time from `dynamicGroupTemplate.targetCount` or, failing that, the
/// size of the last static group — hawkBit resolves the same value but stores
/// it by reusing the group's `target_percentage` column, which raptor has no
/// equivalent of. It doubles as the denominator for the dynamic group's
/// success/error thresholds, which would otherwise be evaluated against a
/// membership count that grows under them (hawkBit builds a reflection proxy
/// over `getTotalTargets` for exactly this reason — `JpaRolloutExecutor`).
///
/// The per-group flag is stored rather than derived from "is this the last
/// group of a dynamic rollout": once one dynamic group fills and the next is
/// created behind it, the filled one is still dynamic but no longer last.
///
/// No column for the template's `nameSuffix`. hawkBit re-derives it from the
/// previous group's name each time it creates another dynamic group, so the
/// first group's name carries it forward on its own.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.alter_table(
            Table::alter()
                .table(Rollout::Table)
                .add_column(
                    ColumnDef::new(Rollout::Dynamic)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Rollout::Table)
                .add_column(
                    ColumnDef::new(Rollout::DynamicGroupSize)
                        .big_integer()
                        .null(),
                )
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(RolloutGroup::Table)
                .add_column(
                    ColumnDef::new(RolloutGroup::Dynamic)
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
                .table(RolloutGroup::Table)
                .drop_column(RolloutGroup::Dynamic)
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Rollout::Table)
                .drop_column(Rollout::DynamicGroupSize)
                .to_owned(),
        )
        .await?;
        m.alter_table(
            Table::alter()
                .table(Rollout::Table)
                .drop_column(Rollout::Dynamic)
                .to_owned(),
        )
        .await?;
        Ok(())
    }
}
