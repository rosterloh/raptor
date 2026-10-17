//! Software-module and blob deletion shared by the mgmt DELETE handlers and
//! release retention: the DB rows go first (in the caller's transaction), and
//! the blobs they referenced are collected only after that commits.

use crate::entity::{artifact, sm_metadata, software_module};
use crate::error::AppError;
use crate::state::AppState;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter};

/// Deletes a software module's own rows — artifacts, metadata, the module —
/// and returns the sha256 of every artifact it held, for [`gc_blob`] once the
/// caller has committed. Does not check whether a distribution set still
/// references the module: callers decide that.
pub(crate) async fn delete_module_rows<C: ConnectionTrait>(
    conn: &C,
    module_id: i64,
) -> Result<Vec<String>, AppError> {
    let shas: Vec<String> = artifact::Entity::find()
        .filter(artifact::Column::ModuleId.eq(module_id))
        .all(conn)
        .await?
        .into_iter()
        .map(|a| a.sha256)
        .collect();
    artifact::Entity::delete_many()
        .filter(artifact::Column::ModuleId.eq(module_id))
        .exec(conn)
        .await?;
    sm_metadata::Entity::delete_many()
        .filter(sm_metadata::Column::ModuleId.eq(module_id))
        .exec(conn)
        .await?;
    software_module::Entity::delete_by_id(module_id)
        .exec(conn)
        .await?;
    Ok(shas)
}

/// Remove a blob from the store if no artifact rows reference its sha256
/// anymore, returning whether it went. Irreversible: only call once the DB
/// deletes that dropped the rows are committed.
pub(crate) async fn gc_blob(st: &AppState, sha256: &str) -> Result<bool, AppError> {
    let refs = artifact::Entity::find()
        .filter(artifact::Column::Sha256.eq(sha256))
        .count(&st.db)
        .await?;
    if refs == 0 {
        st.store.remove(sha256).await?;
    }
    Ok(refs == 0)
}
