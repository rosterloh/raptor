pub use sea_orm_migration::prelude::*;

mod m20260704_000001_initial;
mod m20260712_000001_rollout;
mod m20260720_000001_target_filter;
mod m20260721_000001_confirmation;
mod m20260722_000001_ds_invalidate;
mod m20260723_000001_types_crud;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260704_000001_initial::Migration),
            Box::new(m20260712_000001_rollout::Migration),
            Box::new(m20260720_000001_target_filter::Migration),
            Box::new(m20260721_000001_confirmation::Migration),
            Box::new(m20260722_000001_ds_invalidate::Migration),
            Box::new(m20260723_000001_types_crud::Migration),
        ]
    }
}
