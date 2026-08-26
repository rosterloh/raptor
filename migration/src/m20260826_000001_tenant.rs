//! Adds a `tenant` column (default `'DEFAULT'`) to every query-root table and
//! widens the tables that own a user-visible unique name into a composite
//! `(tenant, name)` key. Behaviour stays single-tenant — nothing queries on
//! `tenant` yet — this only lays the schema groundwork so real isolation can
//! land later without a table rebuild. See
//! docs/superpowers/specs/2026-08-26-multi-tenancy-design.md.
//!
//! Eight tables declare their unique name inline on `CREATE TABLE`
//! (`unique_key()`), which Postgres and SQLite both implement as a table
//! constraint rather than a droppable index. SQLite in particular has no
//! `ALTER TABLE ... DROP CONSTRAINT`, so those eight need a full rebuild
//! (create new table, copy rows, drop old, rename). `software_module` and
//! `distribution_set` already declare their uniqueness as a named `CREATE
//! INDEX ... UNIQUE`, so those just get the column plus a swapped index.
//! `action` has no unique constraint at all and only needs the column.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Tables whose uniqueness is declared as a named index rather than inline —
/// only the index needs replacing, no table rebuild.
const INDEXED_UNIQUE: &[(&str, &str, &[&str])] = &[
    (
        "software_module",
        "ux_sm_name_version_type",
        &["name", "version", "type_id"],
    ),
    (
        "distribution_set",
        "ux_ds_name_version",
        &["name", "version"],
    ),
];

/// (table, single unique column) for the eight inline-`unique_key()` tables,
/// with the full column set (in declaration order, after every later
/// migration's `ADD COLUMN`s) needed to rebuild them on SQLite.
struct RebuildTable {
    name: &'static str,
    unique_col: &'static str,
    /// `(column, sqlite type + constraint)`, in final column order.
    columns: &'static [(&'static str, &'static str)],
}

const REBUILD_TABLES: &[RebuildTable] = &[
    RebuildTable {
        name: "target",
        unique_col: "controller_id",
        columns: &[
            ("id", "INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT"),
            ("controller_id", "TEXT NOT NULL"),
            ("name", "TEXT NOT NULL"),
            ("description", "TEXT"),
            ("security_token", "TEXT NOT NULL"),
            ("update_status", "TEXT NOT NULL DEFAULT 'unknown'"),
            ("last_poll_at", "BIGINT"),
            ("address", "TEXT"),
            ("assigned_ds_id", "BIGINT"),
            ("installed_ds_id", "BIGINT"),
            ("created_at", "BIGINT NOT NULL"),
            ("updated_at", "BIGINT NOT NULL"),
            ("auto_confirm", "BOOLEAN NOT NULL DEFAULT 0"),
            ("type_id", "BIGINT"),
            ("request_attributes", "BOOLEAN NOT NULL DEFAULT 1"),
            ("group_name", "TEXT"),
        ],
    },
    RebuildTable {
        name: "software_module_type",
        unique_col: "key",
        columns: &[
            ("id", "INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT"),
            ("key", "TEXT NOT NULL"),
            ("name", "TEXT NOT NULL"),
            ("description", "TEXT"),
            ("max_assignments", "INTEGER NOT NULL DEFAULT 1"),
        ],
    },
    RebuildTable {
        name: "distribution_set_type",
        unique_col: "key",
        columns: &[
            ("id", "INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT"),
            ("key", "TEXT NOT NULL"),
            ("name", "TEXT NOT NULL"),
            ("description", "TEXT"),
        ],
    },
    RebuildTable {
        name: "target_type",
        unique_col: "name",
        columns: &[
            ("id", "INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT"),
            ("name", "TEXT NOT NULL"),
            ("description", "TEXT"),
            ("colour", "TEXT"),
        ],
    },
    RebuildTable {
        name: "target_tag",
        unique_col: "name",
        columns: &[
            ("id", "INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT"),
            ("name", "TEXT NOT NULL"),
            ("description", "TEXT"),
            ("colour", "TEXT"),
            ("created_at", "BIGINT NOT NULL"),
            ("updated_at", "BIGINT NOT NULL"),
        ],
    },
    RebuildTable {
        name: "ds_tag",
        unique_col: "name",
        columns: &[
            ("id", "INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT"),
            ("name", "TEXT NOT NULL"),
            ("description", "TEXT"),
            ("colour", "TEXT"),
            ("created_at", "BIGINT NOT NULL"),
            ("updated_at", "BIGINT NOT NULL"),
        ],
    },
    RebuildTable {
        name: "rollout",
        unique_col: "name",
        columns: &[
            ("id", "INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT"),
            ("name", "TEXT NOT NULL"),
            ("description", "TEXT"),
            ("ds_id", "BIGINT NOT NULL"),
            ("target_filter", "TEXT NOT NULL"),
            ("status", "TEXT NOT NULL"),
            ("total_targets", "BIGINT NOT NULL"),
            ("group_count", "BIGINT NOT NULL"),
            ("success_threshold", "BIGINT NOT NULL"),
            ("error_threshold", "BIGINT NOT NULL"),
            ("created_at", "BIGINT NOT NULL"),
            ("updated_at", "BIGINT NOT NULL"),
            ("action_type", "TEXT NOT NULL DEFAULT 'forced'"),
            ("forced_time", "BIGINT"),
        ],
    },
    RebuildTable {
        name: "target_filter",
        unique_col: "name",
        columns: &[
            ("id", "INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT"),
            ("name", "TEXT NOT NULL"),
            ("query", "TEXT NOT NULL"),
            ("auto_assign_ds_id", "BIGINT"),
            ("auto_assign_action_type", "TEXT"),
            ("created_at", "BIGINT NOT NULL"),
            ("updated_at", "BIGINT NOT NULL"),
        ],
    },
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // `action` has no unique constraint — just gains the column.
        m.alter_table(
            Table::alter()
                .table(Alias::new("action"))
                .add_column(
                    ColumnDef::new(Alias::new("tenant"))
                        .string()
                        .not_null()
                        .default("DEFAULT"),
                )
                .to_owned(),
        )
        .await?;

        match m.get_database_backend() {
            sea_orm::DbBackend::Sqlite => {
                // Rebuilding `target` etc. drops and recreates tables that
                // other tables hold a live FK against (action, target_attribute,
                // ...). `defer_foreign_keys` postpones *checking* violations
                // to COMMIT, but SQLite's deferred-violation count is a
                // running counter bumped by individual DML, not a fresh scan
                // at commit — dropping a parent table bumps it for every
                // referencing child row and the counter is never reconciled
                // by the later CREATE+RENAME, so commit fails even though the
                // final schema is fully consistent. The documented fix is the
                // other pragma: disable FK enforcement outright so no
                // counting happens at all. `PRAGMA foreign_keys` can only be
                // changed outside a transaction, so it brackets the
                // transaction rather than living inside it.
                m.get_connection()
                    .execute_unprepared("PRAGMA foreign_keys = OFF")
                    .await?;
                let txn = m.begin().await?;
                for t in REBUILD_TABLES {
                    sqlite_rebuild_add_tenant(&txn, t).await?;
                }
                for &(table, index_name, cols) in INDEXED_UNIQUE {
                    let db = txn.get_connection();
                    db.execute_unprepared(&format!(
                        "ALTER TABLE {table} ADD COLUMN tenant TEXT NOT NULL DEFAULT 'DEFAULT'"
                    ))
                    .await?;
                    db.execute_unprepared(&format!("DROP INDEX {index_name}"))
                        .await?;
                    let col_list = cols.join(", ");
                    db.execute_unprepared(&format!(
                        "CREATE UNIQUE INDEX {index_name} ON {table} (tenant, {col_list})"
                    ))
                    .await?;
                }
                txn.commit().await?;
                m.get_connection()
                    .execute_unprepared("PRAGMA foreign_keys = ON")
                    .await?;
            }
            sea_orm::DbBackend::Postgres => {
                for t in REBUILD_TABLES {
                    pg_add_tenant(m, t.name, t.unique_col).await?;
                }
                for &(table, index_name, cols) in INDEXED_UNIQUE {
                    let db = m.get_connection();
                    db.execute_unprepared(&format!(
                        "ALTER TABLE {table} ADD COLUMN tenant TEXT NOT NULL DEFAULT 'DEFAULT'"
                    ))
                    .await?;
                    db.execute_unprepared(&format!("DROP INDEX {index_name}"))
                        .await?;
                    let col_list = cols.join(", ");
                    db.execute_unprepared(&format!(
                        "CREATE UNIQUE INDEX {index_name} ON {table} (tenant, {col_list})"
                    ))
                    .await?;
                }
            }
            other => panic!("unsupported backend: {other:?}"),
        }

        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        match m.get_database_backend() {
            sea_orm::DbBackend::Sqlite => {
                m.get_connection()
                    .execute_unprepared("PRAGMA foreign_keys = OFF")
                    .await?;
                let txn = m.begin().await?;
                for &(table, index_name, cols) in INDEXED_UNIQUE {
                    let db = txn.get_connection();
                    db.execute_unprepared(&format!("DROP INDEX {index_name}"))
                        .await?;
                    let col_list = cols.join(", ");
                    db.execute_unprepared(&format!(
                        "CREATE UNIQUE INDEX {index_name} ON {table} ({col_list})"
                    ))
                    .await?;
                    // SQLite has no DROP COLUMN pre-3.35 semantics issue here —
                    // modern SQLite (bundled by sqlx) supports it directly.
                    db.execute_unprepared(&format!("ALTER TABLE {table} DROP COLUMN tenant"))
                        .await?;
                }
                for t in REBUILD_TABLES.iter().rev() {
                    sqlite_rebuild_drop_tenant(&txn, t).await?;
                }
                txn.commit().await?;
                m.get_connection()
                    .execute_unprepared("PRAGMA foreign_keys = ON")
                    .await?;
            }
            sea_orm::DbBackend::Postgres => {
                for &(table, index_name, cols) in INDEXED_UNIQUE {
                    let db = m.get_connection();
                    db.execute_unprepared(&format!("DROP INDEX {index_name}"))
                        .await?;
                    let col_list = cols.join(", ");
                    db.execute_unprepared(&format!(
                        "CREATE UNIQUE INDEX {index_name} ON {table} ({col_list})"
                    ))
                    .await?;
                    db.execute_unprepared(&format!("ALTER TABLE {table} DROP COLUMN tenant"))
                        .await?;
                }
                for t in REBUILD_TABLES.iter().rev() {
                    pg_drop_tenant(m, t.name, t.unique_col).await?;
                }
            }
            other => panic!("unsupported backend: {other:?}"),
        }
        Ok(())
    }
}

/// Postgres: drop the auto-named single-column unique constraint (found via
/// `pg_constraint` rather than assumed from naming convention), add `tenant`,
/// then create the composite replacement.
async fn pg_add_tenant(m: &SchemaManager<'_>, table: &str, unique_col: &str) -> Result<(), DbErr> {
    let db = m.get_connection();
    let row = db
        .query_one_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT conname FROM pg_constraint WHERE conrelid = $1::regclass AND contype = 'u'",
            [table.into()],
        ))
        .await?
        .expect("expected exactly one unique constraint");
    let conname: String = row.try_get("", "conname")?;

    db.execute_unprepared(&format!("ALTER TABLE {table} DROP CONSTRAINT {conname}"))
        .await?;
    db.execute_unprepared(&format!(
        "ALTER TABLE {table} ADD COLUMN tenant TEXT NOT NULL DEFAULT 'DEFAULT'"
    ))
    .await?;
    db.execute_unprepared(&format!(
        "CREATE UNIQUE INDEX ux_{table}_tenant_{unique_col} ON {table} (tenant, {unique_col})"
    ))
    .await?;
    Ok(())
}

async fn pg_drop_tenant(m: &SchemaManager<'_>, table: &str, unique_col: &str) -> Result<(), DbErr> {
    let db = m.get_connection();
    db.execute_unprepared(&format!("DROP INDEX ux_{table}_tenant_{unique_col}"))
        .await?;
    db.execute_unprepared(&format!("ALTER TABLE {table} DROP COLUMN tenant"))
        .await?;
    db.execute_unprepared(&format!(
        "ALTER TABLE {table} ADD CONSTRAINT {table}_{unique_col}_key UNIQUE ({unique_col})"
    ))
    .await?;
    Ok(())
}

/// SQLite: no `DROP CONSTRAINT`, so rebuild the table with `tenant` in the
/// column list and the unique key expressed as a composite `(tenant, col)`
/// table constraint instead of inline on the column.
async fn sqlite_rebuild_add_tenant(m: &SchemaManager<'_>, t: &RebuildTable) -> Result<(), DbErr> {
    let db = m.get_connection();
    let old_cols: Vec<&str> = t.columns.iter().map(|(c, _)| *c).collect();
    let new_defs: Vec<String> = t
        .columns
        .iter()
        .map(|(c, ty)| format!("{c} {ty}"))
        .collect();

    db.execute_unprepared(&format!(
        "CREATE TABLE {table}_new ({defs}, tenant TEXT NOT NULL DEFAULT 'DEFAULT', \
         UNIQUE (tenant, {unique_col}))",
        table = t.name,
        defs = new_defs.join(", "),
        unique_col = t.unique_col,
    ))
    .await?;
    db.execute_unprepared(&format!(
        "INSERT INTO {table}_new ({cols}, tenant) SELECT {cols}, 'DEFAULT' FROM {table}",
        table = t.name,
        cols = old_cols.join(", "),
    ))
    .await?;
    db.execute_unprepared(&format!("DROP TABLE {}", t.name))
        .await?;
    db.execute_unprepared(&format!("ALTER TABLE {}_new RENAME TO {}", t.name, t.name))
        .await?;
    Ok(())
}

async fn sqlite_rebuild_drop_tenant(m: &SchemaManager<'_>, t: &RebuildTable) -> Result<(), DbErr> {
    let db = m.get_connection();
    let old_cols: Vec<&str> = t.columns.iter().map(|(c, _)| *c).collect();
    let mut defs: Vec<String> = t
        .columns
        .iter()
        .map(|(c, ty)| format!("{c} {ty}"))
        .collect();
    // Re-inline the original single-column unique constraint on its own type,
    // matching what the originating CREATE TABLE declared.
    for (i, (c, ty)) in t.columns.iter().enumerate() {
        if *c == t.unique_col {
            defs[i] = format!("{c} {ty} UNIQUE");
        }
    }

    db.execute_unprepared(&format!(
        "CREATE TABLE {table}_new ({defs})",
        table = t.name,
        defs = defs.join(", "),
    ))
    .await?;
    db.execute_unprepared(&format!(
        "INSERT INTO {table}_new ({cols}) SELECT {cols} FROM {table}",
        table = t.name,
        cols = old_cols.join(", "),
    ))
    .await?;
    db.execute_unprepared(&format!("DROP TABLE {}", t.name))
        .await?;
    db.execute_unprepared(&format!("ALTER TABLE {}_new RENAME TO {}", t.name, t.name))
        .await?;
    Ok(())
}
