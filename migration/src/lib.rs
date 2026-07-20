pub use sea_orm_migration::prelude::*;

mod m20260704_000001_initial;
mod m20260712_000001_rollout;
mod m20260720_000001_target_filter;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260704_000001_initial::Migration),
            Box::new(m20260712_000001_rollout::Migration),
            Box::new(m20260720_000001_target_filter::Migration),
        ]
    }
}
