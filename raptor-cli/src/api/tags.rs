//! Tags. The target-tag and distribution-set-tag resources are identical
//! apart from their path, so [`Kind`] selects between them rather than
//! duplicating every function.
//!
//! `raptorctl target tag add/rm` takes a tag *name*, but the assignment
//! endpoint wants an id — `find_id` resolves it via a list call, same as the
//! web console would.

use crate::client::Client;
use anyhow::Result;
use raptor_api_types::{TagCreate, TagRest};

/// Which tag resource a command addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Target,
    Ds,
}

impl Kind {
    fn path(self) -> &'static str {
        match self {
            Kind::Target => "targettags",
            Kind::Ds => "distributionsettags",
        }
    }

    /// How the kind reads inside an error message.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Target => "target",
            Kind::Ds => "distribution set",
        }
    }
}

pub async fn list(c: &Client, kind: Kind) -> Result<Vec<TagRest>> {
    let paged: raptor_api_types::PagedList<TagRest> = c
        .get(&format!("/rest/v1/{}?limit=1000", kind.path()))
        .await?;
    Ok(paged.content)
}

pub async fn create(c: &Client, kind: Kind, body: &TagCreate) -> Result<TagRest> {
    c.post(&format!("/rest/v1/{}", kind.path()), &vec![body.clone()])
        .await
        .map(|mut v: Vec<TagRest>| v.remove(0))
}

pub async fn delete(c: &Client, kind: Kind, tag_id: i64) -> Result<()> {
    c.delete(&format!("/rest/v1/{}/{tag_id}", kind.path()))
        .await
}

/// Resolve a tag name to its id. The failure names the tags that *do* exist —
/// without that, a typo and a not-yet-created tag look identical, and until
/// `tag create` existed neither could be acted on from the CLI at all.
pub async fn find_id(c: &Client, kind: Kind, name: &str) -> Result<i64> {
    let tags = list(c, kind).await?;
    match tags.iter().find(|t| t.name == name) {
        Some(t) => Ok(t.id),
        None => Err(missing_tag_error(
            kind,
            name,
            &tags.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
        )),
    }
}

fn missing_tag_error(kind: Kind, name: &str, existing: &[&str]) -> anyhow::Error {
    let flag = match kind {
        Kind::Target => "",
        Kind::Ds => " --ds",
    };
    anyhow::anyhow!(
        "no {} tag named '{name}' — existing: {}. Create it with 'raptorctl tag create{flag} {name}'.",
        kind.label(),
        if existing.is_empty() {
            "(none)".to_string()
        } else {
            existing.join(", ")
        },
    )
}

pub async fn assign(c: &Client, kind: Kind, tag_id: i64, id: &str) -> Result<()> {
    c.post_empty(
        &format!("/rest/v1/{}/{tag_id}/assigned/{id}", kind.path()),
        &(),
    )
    .await
}

pub async fn unassign(c: &Client, kind: Kind, tag_id: i64, id: &str) -> Result<()> {
    c.delete(&format!("/rest/v1/{}/{tag_id}/assigned/{id}", kind.path()))
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_tag_names_the_tags_that_do_exist() {
        let e = missing_tag_error(Kind::Target, "zephyr", &["prod", "canary"]);
        let msg = e.to_string();
        assert!(msg.contains("no target tag named 'zephyr'"), "{msg}");
        assert!(msg.contains("prod, canary"), "{msg}");
        assert!(msg.contains("raptorctl tag create zephyr"), "{msg}");
    }

    #[test]
    fn missing_tag_on_an_empty_catalogue_still_points_at_create() {
        // The reported case: nothing exists yet and `add` was the only verb.
        let msg = missing_tag_error(Kind::Ds, "lts", &[]).to_string();
        assert!(msg.contains("no distribution set tag named 'lts'"), "{msg}");
        assert!(msg.contains("existing: (none)"), "{msg}");
        assert!(msg.contains("raptorctl tag create --ds lts"), "{msg}");
    }
}
