#![allow(clippy::unwrap_used)] // helpers outside #[test] fns aren't covered by allow-unwrap-in-tests
use migration::{Migrator, MigratorTrait};
use raptor::entity::{distribution_set_type, software_module_type, target};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, Database, EntityTrait};

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

#[tokio::test]
async fn rollout_migration_up_and_down() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    Migrator::down(&db, None).await.unwrap();
    // back to the initial-only schema
    Migrator::up(&db, None).await.unwrap();
    let sm_types = software_module_type::Entity::find().all(&db).await.unwrap();
    assert_eq!(sm_types.len(), 4);
}

/// Proves the tenant migration actually widened `target.controller_id`'s
/// unique key to `(tenant, controller_id)`: before it, a second tenant
/// couldn't register a device sharing a `controllerId` with one in another
/// tenant — the two would silently collapse into the same row.
#[tokio::test]
async fn controller_id_is_unique_per_tenant_not_globally() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    Migrator::up(&db, None).await.unwrap();

    let base = target::ActiveModel {
        controller_id: Set("shared-id".into()),
        name: Set("dev".into()),
        security_token: Set("tok".into()),
        created_at: Set(0),
        updated_at: Set(0),
        ..Default::default()
    };

    let mut a = base.clone();
    a.tenant = Set("acme".into());
    a.insert(&db).await.unwrap();

    let mut b = base;
    b.tenant = Set("globex".into());
    b.insert(&db).await.unwrap();

    let rows = target::Entity::find().all(&db).await.unwrap();
    assert_eq!(rows.len(), 2);
}

/// Usernames are unique per tenant via the named `users_tenant_username`
/// index, so the same name can exist in two tenants but not twice in one.
#[tokio::test]
async fn username_is_unique_per_tenant() {
    use raptor::entity::user;
    let db = Database::connect("sqlite::memory:").await.unwrap();
    Migrator::up(&db, None).await.unwrap();

    let base = user::ActiveModel {
        username: Set("alice".into()),
        role: Set("admin".into()),
        source: Set("local".into()),
        enabled: Set(true),
        created_at: Set(0),
        updated_at: Set(0),
        ..Default::default()
    };
    base.clone().insert(&db).await.unwrap();
    assert!(base.clone().insert(&db).await.is_err());

    let mut other = base;
    other.tenant = Set("acme".into());
    other.insert(&db).await.unwrap();
    assert_eq!(user::Entity::find().all(&db).await.unwrap().len(), 2);
}
