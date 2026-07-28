//! `Migrator`: the ordered list of schema migrations, applied by `raptor`'s
//! `main` on startup via `Migrator::up`. Each `mNNN...` submodule is one
//! migration; add new ones here in chronological order.

pub use sea_orm_migration::prelude::*;

mod m20260704_000001_initial;
mod m20260712_000001_rollout;
mod m20260720_000001_target_filter;
mod m20260721_000001_confirmation;
mod m20260722_000001_ds_invalidate;
mod m20260723_000001_metadata;
mod m20260723_000001_types_crud;
mod m20260726_000001_tags;
mod m20260727_000001_request_attributes;
mod m20260727_000002_action_type;
mod m20260727_000003_rollout_action_type;

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
            Box::new(m20260723_000001_metadata::Migration),
            Box::new(m20260723_000001_types_crud::Migration),
            Box::new(m20260726_000001_tags::Migration),
            Box::new(m20260727_000001_request_attributes::Migration),
            Box::new(m20260727_000002_action_type::Migration),
            Box::new(m20260727_000003_rollout_action_type::Migration),
        ]
    }
}
