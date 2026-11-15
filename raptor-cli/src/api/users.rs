//! Operator users (`/rest/v1/users`), addressed by username on the command
//! line; [`find_id`] resolves the name with a filtered list call.

use super::urlencode;
use crate::client::Client;
use anyhow::{Result, anyhow};
use raptor_api_types::{PagedList, UserCreate, UserRest, UserUpdate};

pub async fn list(c: &Client) -> Result<Vec<UserRest>> {
    let paged: PagedList<UserRest> = c.get("/rest/v1/users?limit=1000").await?;
    Ok(paged.content)
}

pub async fn find_id(c: &Client, username: &str) -> Result<i64> {
    let q = urlencode(&format!("username=={username}"));
    let paged: PagedList<UserRest> = c.get(&format!("/rest/v1/users?q={q}")).await?;
    paged
        .content
        .first()
        .map(|u| u.id)
        .ok_or_else(|| anyhow!("no user named {username}"))
}

pub async fn create(c: &Client, body: &UserCreate) -> Result<UserRest> {
    c.post("/rest/v1/users", body).await
}

pub async fn update(c: &Client, id: i64, body: &UserUpdate) -> Result<UserRest> {
    c.put(&format!("/rest/v1/users/{id}"), body).await
}

pub async fn delete(c: &Client, id: i64) -> Result<()> {
    c.delete(&format!("/rest/v1/users/{id}")).await
}
