use sea_orm_migration::prelude::*;

#[derive(DeriveIden)]
enum RolloutGroup {
    Table,
    PurgedFinished,
    PurgedError,
    PurgedCancelled,
    PurgedRunning,
}

/// Per-group tally of actions that automatic cleanup (#134) has deleted,
/// bucketed the same way `totalTargetsPerStatus` buckets live ones.
///
/// A rollout group's progress is derived by counting its actions and treating
/// members without one as still pending. Deleting a finished action would
/// therefore move that target from `finished` back to `scheduled` — a
/// long-completed rollout would start reporting as though it had never run.
/// These counters remember the outcome the deleted rows carried, so cleanup
/// reclaims the storage without rewriting history.
///
/// Four columns rather than six because only *closed* actions are ever
/// eligible: `finished`, `error`, `canceled`, and a `downloadonly` action's
/// `downloaded` (which buckets as `running`). Nothing can be purged out of
/// `scheduled` or `notstarted`, which are the absence of an action, not a
/// state one can be in.
#[derive(DeriveMigrationName)]
pub struct Migration;

const COLUMNS: [RolloutGroup; 4] = [
    RolloutGroup::PurgedFinished,
    RolloutGroup::PurgedError,
    RolloutGroup::PurgedCancelled,
    RolloutGroup::PurgedRunning,
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for col in COLUMNS {
            m.alter_table(
                Table::alter()
                    .table(RolloutGroup::Table)
                    .add_column(
                        ColumnDef::new(col)
                            .big_integer()
                            .not_null()
                            .default(0)
                            .to_owned(),
                    )
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for col in COLUMNS {
            m.alter_table(
                Table::alter()
                    .table(RolloutGroup::Table)
                    .drop_column(col)
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
