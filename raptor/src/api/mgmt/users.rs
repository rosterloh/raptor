//! Operator users (`/rest/v1/users`, raptor extension) and hawkBit's
//! `GET /rest/v1/userinfo`. User administration is admin-only; the role gate
//! in `auth::mgmt` enforces that before these handlers run.

use super::mappers::user_rest;
use crate::api::paging::{ListParams, Paged, apply_sort, page};
use crate::auth::principal::{Principal, Role, Source};
use crate::entity::user;
use crate::error::AppError;
use crate::state::AppState;
use crate::util::now_ms;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use raptor_api_types::{UserCreate, UserInfo, UserRest, UserUpdate};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/rest/v1/users", get(list).post(create))
        .route("/rest/v1/users/{id}", get(one).put(update).delete(remove))
        .route("/rest/v1/userinfo", get(userinfo))
}

fn fiql(f: &str) -> Option<user::Column> {
    match f {
        "id" => Some(user::Column::Id),
        "username" => Some(user::Column::Username),
        "role" => Some(user::Column::Role),
        "enabled" => Some(user::Column::Enabled),
        "source" => Some(user::Column::Source),
        _ => None,
    }
}

fn parse_role(s: &str) -> Result<Role, AppError> {
    Role::parse(s).ok_or_else(|| AppError::BadRequest(format!("unknown role {s}")))
}

fn hash(pw: &str) -> Result<String, AppError> {
    if pw.is_empty() {
        return Err(AppError::BadRequest("password must not be empty".into()));
    }
    crate::util::hash_password(pw)
        .map_err(|e| AppError::Io(std::io::Error::other(format!("hash password: {e}"))))
}

async fn find(st: &AppState, id: i64) -> Result<user::Model, AppError> {
    user::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("user"))
}

/// Whether `who` is the stored user `u` (the config admin never is).
fn is_self(who: &Principal, u: &user::Model) -> bool {
    who.source != Source::Config && who.username == u.username
}

pub async fn list(
    State(st): State<AppState>,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<UserRest>>, AppError> {
    let mut sel = user::Entity::find();
    if let Some(q) = &p.q {
        let expr = crate::fiql::parse(q).map_err(AppError::BadRequest)?;
        sel = sel.filter(crate::fiql::to_condition(&expr, &fiql)?);
    }
    sel = apply_sort(sel, &p.sort, &fiql)?;
    let (rows, total) = page(&st.db, sel, &p).await?;
    Ok(Json(Paged::new(
        rows.iter().map(user_rest).collect(),
        total,
    )))
}

pub async fn one(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<UserRest>, AppError> {
    Ok(Json(user_rest(&find(&st, id).await?)))
}

pub async fn create(
    State(st): State<AppState>,
    Json(c): Json<UserCreate>,
) -> Result<(StatusCode, Json<UserRest>), AppError> {
    let username = c.username.trim();
    if username.is_empty() {
        return Err(AppError::BadRequest("username must not be empty".into()));
    }
    let role = parse_role(&c.role)?;
    if username == st.cfg.mgmt.username
        || user::Entity::find()
            .filter(user::Column::Username.eq(username))
            .one(&st.db)
            .await?
            .is_some()
    {
        return Err(AppError::Conflict(format!(
            "user {username} already exists"
        )));
    }
    let now = now_ms();
    let u = user::ActiveModel {
        username: Set(username.to_string()),
        password_hash: Set(Some(hash(&c.password)?)),
        role: Set(role.as_str().into()),
        source: Set(Source::Local.as_str().into()),
        enabled: Set(c.enabled.unwrap_or(true)),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&st.db)
    .await?;
    Ok((StatusCode::CREATED, Json(user_rest(&u))))
}

pub async fn update(
    State(st): State<AppState>,
    Extension(who): Extension<Principal>,
    Path(id): Path<i64>,
    Json(u): Json<UserUpdate>,
) -> Result<Json<UserRest>, AppError> {
    let row = find(&st, id).await?;
    let role = u.role.as_deref().map(parse_role).transpose()?;
    if is_self(&who, &row) && (u.enabled == Some(false) || role.is_some_and(|r| r != who.role)) {
        return Err(AppError::BadRequest(
            "cannot disable or change the role of your own account".into(),
        ));
    }
    if u.password.is_some() && row.source != Source::Local.as_str() {
        return Err(AppError::BadRequest(
            "only local users have a password".into(),
        ));
    }
    // Any change to who may do what ends the user's open sessions.
    let revoke = u.enabled == Some(false) || role.is_some_and(|r| r.as_str() != row.role);
    let username = row.username.clone();
    let mut am: user::ActiveModel = row.into();
    if let Some(r) = role {
        am.role = Set(r.as_str().into());
    }
    if let Some(e) = u.enabled {
        am.enabled = Set(e);
    }
    if let Some(pw) = &u.password {
        am.password_hash = Set(Some(hash(pw)?));
    }
    am.updated_at = Set(now_ms());
    let row = am.update(&st.db).await?;
    if revoke {
        st.sessions.remove_user(&username);
    }
    Ok(Json(user_rest(&row)))
}

pub async fn remove(
    State(st): State<AppState>,
    Extension(who): Extension<Principal>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let row = find(&st, id).await?;
    if is_self(&who, &row) {
        return Err(AppError::BadRequest(
            "cannot delete your own account".into(),
        ));
    }
    user::Entity::delete_by_id(row.id).exec(&st.db).await?;
    st.sessions.remove_user(&row.username);
    Ok(StatusCode::OK)
}

/// hawkBit `GET /rest/v1/userinfo` (`MgmtUserInfo`), plus raptor's `role`.
pub async fn userinfo(
    State(st): State<AppState>,
    Extension(who): Extension<Principal>,
) -> Json<UserInfo> {
    Json(UserInfo {
        tenant: st.cfg.tenant.clone(),
        username: who.username,
        permissions: who.role.permissions(),
        role: who.role.as_str().into(),
    })
}
