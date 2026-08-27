//! The three type catalogues: software-module types
//! (`/rest/v1/softwaremoduletypes`), distribution-set types
//! (`/rest/v1/distributionsettypes`) and target types
//! (`/rest/v1/targettypes`). Read-only from the CLI's side — commands consult
//! them to validate a `--type` *before* writing anything, since a rejected
//! type halfway through `publish` leaves an orphaned module and artifact
//! behind and there is no CLI command to delete either.

use crate::client::Client;
use anyhow::Result;
use raptor_api_types::{DsTypeRest, PagedList, SmTypeRest};

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
