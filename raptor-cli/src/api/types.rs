//! The three type catalogues: software-module types
//! (`/rest/v1/softwaremoduletypes`), distribution-set types
//! (`/rest/v1/distributionsettypes`) and target types
//! (`/rest/v1/targettypes`). Read-only from the CLI's side — commands consult
//! them to validate a `--type` *before* writing anything, since a rejected
//! type halfway through `publish` leaves an orphaned module and artifact
//! behind and there is no CLI command to delete either.

use crate::client::Client;
use anyhow::Result;
use raptor_api_types::{DsTypeRest, PagedList, SmTypeRest, TargetTypeRest, TypeRef};

pub async fn sm_types(c: &Client) -> Result<Vec<SmTypeRest>> {
    let paged: PagedList<SmTypeRest> = c.get("/rest/v1/softwaremoduletypes?limit=1000").await?;
    Ok(paged.content)
}

pub async fn ds_types(c: &Client) -> Result<Vec<DsTypeRest>> {
    let paged: PagedList<DsTypeRest> = c.get("/rest/v1/distributionsettypes?limit=1000").await?;
    Ok(paged.content)
}

/// The software-module types a distribution-set type *requires* — a set is
/// only `complete`, and therefore only assignable, once it carries one module
/// of each.
pub async fn ds_type_mandatory(c: &Client, id: i64) -> Result<Vec<SmTypeRest>> {
    let paged: PagedList<SmTypeRest> = c
        .get(&format!(
            "/rest/v1/distributionsettypes/{id}/mandatorymoduletypes"
        ))
        .await?;
    Ok(paged.content)
}

pub async fn target_types(c: &Client) -> Result<Vec<TargetTypeRest>> {
    let paged: PagedList<TargetTypeRest> = c.get("/rest/v1/targettypes?limit=1000").await?;
    Ok(paged.content)
}

/// The distribution-set types a target of this type may be assigned. A typed
/// target rejects anything else, so this is the list worth printing next to a
/// compatibility failure.
pub async fn target_type_compatible(c: &Client, id: i64) -> Result<Vec<DsTypeRest>> {
    let paged: PagedList<DsTypeRest> = c
        .get(&format!(
            "/rest/v1/targettypes/{id}/compatibledistributionsettypes"
        ))
        .await?;
    Ok(paged.content)
}

/// Target types are addressed by name on the command line; the assignment
/// endpoint wants an id, as with tags.
pub async fn find_target_type(c: &Client, name: &str) -> Result<TargetTypeRest> {
    let types = target_types(c).await?;
    match types.iter().find(|t| t.name == name) {
        Some(t) => Ok(t.clone()),
        None => {
            let existing = types
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            Err(anyhow::anyhow!(
                "no target type named '{name}' — existing: {}",
                if existing.is_empty() {
                    "(none)"
                } else {
                    &existing
                },
            ))
        }
    }
}

pub async fn assign_target_type(c: &Client, cid: &str, id: i64) -> Result<()> {
    c.post_empty(
        &format!("/rest/v1/targets/{cid}/targettype"),
        &TypeRef { id },
    )
    .await
}

pub async fn unassign_target_type(c: &Client, cid: &str) -> Result<()> {
    c.delete(&format!("/rest/v1/targets/{cid}/targettype"))
        .await
}
