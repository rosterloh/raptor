use migration::{Migrator, MigratorTrait};
use raptor::entity::{distribution_set_type, software_module_type};
use sea_orm::{Database, EntityTrait};

#[tokio::test]
async fn migrations_apply_and_seed_types() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    // idempotent
    Migrator::up(&db, None).await.unwrap();

    let sm_types = software_module_type::Entity::find().all(&db).await.unwrap();
    let keys: Vec<_> = sm_types.iter().map(|t| t.key.as_str()).collect();
    assert_eq!(keys, ["os", "firmware", "runtime", "application"]);

    let ds_types = distribution_set_type::Entity::find()
        .all(&db)
        .await
        .unwrap();
    let keys: Vec<_> = ds_types.iter().map(|t| t.key.as_str()).collect();
    assert_eq!(keys, ["os", "os_app", "app"]);
}
