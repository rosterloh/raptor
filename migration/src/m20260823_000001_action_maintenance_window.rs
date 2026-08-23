use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Action {
    Table,
    MaintenanceSchedule,
    MaintenanceDuration,
    MaintenanceTimezone,
}

/// The three columns are written and read as a set — an action either carries a
/// full maintenance window or none at all. They are added one statement at a
/// time because SQLite's `ALTER TABLE` takes a single `ADD COLUMN` per
/// statement, unlike Postgres.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for col in [
            Action::MaintenanceSchedule,
            Action::MaintenanceDuration,
            Action::MaintenanceTimezone,
        ] {
            m.alter_table(
                Table::alter()
                    .table(Action::Table)
                    .add_column(ColumnDef::new(col).string().null())
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        for col in [
            Action::MaintenanceSchedule,
            Action::MaintenanceDuration,
            Action::MaintenanceTimezone,
        ] {
            m.alter_table(
                Table::alter()
                    .table(Action::Table)
                    .drop_column(col)
                    .to_owned(),
            )
            .await?;
        }
        Ok(())
    }
}
