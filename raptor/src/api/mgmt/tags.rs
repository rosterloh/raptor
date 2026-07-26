//! Target and distribution-set tags (hawkBit `/rest/v1/targettags` and
//! `/rest/v1/distributionsettags`): free-form labels used for fleet
//! organisation and as the `tag==` FIQL term on target/DS lists.

use super::mappers::{DsRest, TargetRest};
use super::targets::find_by_cid;
use crate::api::paging::{apply_sort, page, ListParams, Paged};
use crate::entity::{
    distribution_set, ds_tag, ds_tag_assignment, target, target_tag, target_tag_assignment,
};
use crate::error::AppError;
use crate::fiql::Op;
use crate::state::AppState;
use crate::util::{base_url, now_ms};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::Json;
use axum::Router;
use raptor_api_types::{TagCreate, TagRest, TagUpdate};
use sea_orm::sea_query::{Expr as SqlExpr, Query as SqlQuery, SelectStatement, SimpleExpr};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, EntityTrait, QueryFilter,
};
use serde_json::json;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/rest/v1/targettags",
            get(target_tag_list).post(target_tag_create),
        )
        .route(
            "/rest/v1/targettags/{id}",
            get(target_tag_one)
                .put(target_tag_update)
                .delete(target_tag_delete),
        )
        .route(
            "/rest/v1/targettags/{id}/assigned",
            get(target_tag_assigned)
                .post(target_tag_assign_many)
                .delete(target_tag_unassign_many),
        )
        .route(
            "/rest/v1/targettags/{id}/assigned/{cid}",
            post(target_tag_assign_one).delete(target_tag_unassign_one),
        )
        .route("/rest/v1/targets/{cid}/tags", get(tags_of_target))
        .route(
            "/rest/v1/distributionsettags",
            get(ds_tag_list).post(ds_tag_create),
        )
        .route(
            "/rest/v1/distributionsettags/{id}",
            get(ds_tag_one).put(ds_tag_update).delete(ds_tag_delete),
        )
        .route(
            "/rest/v1/distributionsettags/{id}/assigned",
            get(ds_tag_assigned)
                .post(ds_tag_assign_many)
                .delete(ds_tag_unassign_many),
        )
        .route(
            "/rest/v1/distributionsettags/{id}/assigned/{dsid}",
            post(ds_tag_assign_one).delete(ds_tag_unassign_one),
        )
        .route("/rest/v1/distributionsets/{id}/tags", get(tags_of_ds))
}

// --------------------------------------------------------------------------
// JSON rendering (hawkBit `MgmtTag` shape)
// --------------------------------------------------------------------------

fn target_tag_rest(t: &target_tag::Model) -> TagRest {
    let base = format!("/rest/v1/targettags/{}", t.id);
    TagRest {
        id: t.id,
        name: t.name.clone(),
        description: t.description.clone(),
        colour: t.colour.clone(),
        created_at: t.created_at,
        last_modified_at: t.updated_at,
        links: json!({
            "self": {"href": base},
            "assignedTargets": {"href": format!("{base}/assigned")},
        }),
    }
}

fn ds_tag_rest(t: &ds_tag::Model) -> TagRest {
    let base = format!("/rest/v1/distributionsettags/{}", t.id);
    TagRest {
        id: t.id,
        name: t.name.clone(),
        description: t.description.clone(),
        colour: t.colour.clone(),
        created_at: t.created_at,
        last_modified_at: t.updated_at,
        links: json!({
            "self": {"href": base},
            "assignedDistributionSets": {"href": format!("{base}/assigned")},
        }),
    }
}

fn target_tag_fiql(f: &str) -> Option<target_tag::Column> {
    match f {
        "id" => Some(target_tag::Column::Id),
        "name" => Some(target_tag::Column::Name),
        "description" => Some(target_tag::Column::Description),
        "colour" => Some(target_tag::Column::Colour),
        _ => None,
    }
}

fn ds_tag_fiql(f: &str) -> Option<ds_tag::Column> {
    match f {
        "id" => Some(ds_tag::Column::Id),
        "name" => Some(ds_tag::Column::Name),
        "description" => Some(ds_tag::Column::Description),
        "colour" => Some(ds_tag::Column::Colour),
        _ => None,
    }
}

// --------------------------------------------------------------------------
// `tag==x` on target / distribution-set lists
// --------------------------------------------------------------------------

/// Splits a `tag` comparison into (membership is positive, operator to apply to
/// the tag *name*). `tag!=beta` means "not tagged beta", so the negation lives
/// on the membership test, not inside the subquery — otherwise a target
/// carrying both `beta` and `stable` would match.
fn tag_op(op: &Op) -> Result<(bool, Op), AppError> {
    match op {
        Op::Eq => Ok((true, Op::Eq)),
        Op::Ne => Ok((false, Op::Eq)),
        Op::In => Ok((true, Op::In)),
        Op::Out => Ok((false, Op::In)),
        _ => Err(AppError::BadRequest(
            "unsupported operator for query field: tag".into(),
        )),
    }
}

fn membership(owner: SqlExpr, owners: SelectStatement, positive: bool) -> SimpleExpr {
    if positive {
        owner.in_subquery(owners)
    } else {
        owner.not_in_subquery(owners)
    }
}

/// Compiles `tag==x` on `/rest/v1/targets` into
/// `target.id IN (SELECT target_id FROM target_tag_assignment
///                WHERE tag_id IN (SELECT id FROM target_tag WHERE name = 'x'))`.
/// Two plain subqueries rather than a join keep the SQL identical on SQLite and
/// Postgres and leave the outer query's paging untouched.
pub fn target_tag_field(
    field: &str,
    op: &Op,
    values: &[String],
) -> Option<Result<SimpleExpr, AppError>> {
    if field != "tag" {
        return None;
    }
    Some(tag_op(op).map(|(positive, name_op)| {
        let tag_ids = SqlQuery::select()
            .column(target_tag::Column::Id)
            .from(target_tag::Entity)
            .and_where(crate::fiql::cmp_expr(
                target_tag::Column::Name,
                &name_op,
                values,
            ))
            .to_owned();
        let owners = SqlQuery::select()
            .column(target_tag_assignment::Column::TargetId)
            .from(target_tag_assignment::Entity)
            .and_where(SqlExpr::col(target_tag_assignment::Column::TagId).in_subquery(tag_ids))
            .to_owned();
        membership(
            SqlExpr::col((target::Entity, target::Column::Id)),
            owners,
            positive,
        )
    }))
}

/// The distribution-set counterpart of [`target_tag_field`].
pub fn ds_tag_field(
    field: &str,
    op: &Op,
    values: &[String],
) -> Option<Result<SimpleExpr, AppError>> {
    if field != "tag" {
        return None;
    }
    Some(tag_op(op).map(|(positive, name_op)| {
        let tag_ids = SqlQuery::select()
            .column(ds_tag::Column::Id)
            .from(ds_tag::Entity)
            .and_where(crate::fiql::cmp_expr(
                ds_tag::Column::Name,
                &name_op,
                values,
            ))
            .to_owned();
        let owners = SqlQuery::select()
            .column(ds_tag_assignment::Column::DsId)
            .from(ds_tag_assignment::Entity)
            .and_where(SqlExpr::col(ds_tag_assignment::Column::TagId).in_subquery(tag_ids))
            .to_owned();
        membership(
            SqlExpr::col((distribution_set::Entity, distribution_set::Column::Id)),
            owners,
            positive,
        )
    }))
}

/// `owner_id IN (SELECT ... WHERE tag_id = <id>)` — the "entities carrying this
/// tag" filter behind the `/assigned` listings.
fn tagged_by(owner: SqlExpr, owners: SelectStatement) -> Condition {
    Condition::all().add(owner.in_subquery(owners))
}

fn targets_with_tag(tag_id: i64) -> Condition {
    let owners = SqlQuery::select()
        .column(target_tag_assignment::Column::TargetId)
        .from(target_tag_assignment::Entity)
        .and_where(SqlExpr::col(target_tag_assignment::Column::TagId).eq(tag_id))
        .to_owned();
    tagged_by(SqlExpr::col((target::Entity, target::Column::Id)), owners)
}

fn ds_with_tag(tag_id: i64) -> Condition {
    let owners = SqlQuery::select()
        .column(ds_tag_assignment::Column::DsId)
        .from(ds_tag_assignment::Entity)
        .and_where(SqlExpr::col(ds_tag_assignment::Column::TagId).eq(tag_id))
        .to_owned();
    tagged_by(
        SqlExpr::col((distribution_set::Entity, distribution_set::Column::Id)),
        owners,
    )
}

/// `tag.id IN (SELECT tag_id FROM <join> WHERE <owner_col> = <owner_id>)` — the
/// tags carried by one entity, the inverse of [`targets_with_tag`].
fn tags_of(tag_id_col: SqlExpr, link: SelectStatement) -> Condition {
    Condition::all().add(tag_id_col.in_subquery(link))
}

// --------------------------------------------------------------------------
// Target tags
// --------------------------------------------------------------------------

async fn load_target_tag(st: &AppState, id: i64) -> Result<target_tag::Model, AppError> {
    target_tag::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("target tag"))
}

pub async fn target_tag_list(
    State(st): State<AppState>,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<TagRest>>, AppError> {
    let mut sel = target_tag::Entity::find();
    if let Some(q) = &p.q {
        let expr = crate::fiql::parse(q).map_err(AppError::BadRequest)?;
        sel = sel.filter(crate::fiql::to_condition(&expr, &target_tag_fiql)?);
    }
    sel = apply_sort(sel, &p.sort, &target_tag_fiql)?;
    let (rows, total) = page(&st.db, sel, &p).await?;
    Ok(Json(Paged::new(
        rows.iter().map(target_tag_rest).collect(),
        total,
    )))
}

pub async fn target_tag_one(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<TagRest>, AppError> {
    Ok(Json(target_tag_rest(&load_target_tag(&st, id).await?)))
}

pub async fn target_tag_create(
    State(st): State<AppState>,
    Json(body): Json<Vec<TagCreate>>,
) -> Result<(StatusCode, Json<Vec<TagRest>>), AppError> {
    // Validate every item before writing so a clash can't leave a partial batch.
    let mut seen = std::collections::HashSet::new();
    for c in &body {
        if !seen.insert(c.name.as_str()) {
            return Err(AppError::Conflict(format!(
                "duplicate tag {} in request",
                c.name
            )));
        }
        if target_tag::Entity::find()
            .filter(target_tag::Column::Name.eq(&c.name))
            .one(&st.db)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict(format!(
                "target tag {} already exists",
                c.name
            )));
        }
    }
    let mut out = Vec::with_capacity(body.len());
    for c in body {
        let now = now_ms();
        let t = target_tag::ActiveModel {
            name: Set(c.name),
            description: Set(c.description),
            colour: Set(c.colour),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&st.db)
        .await?;
        out.push(target_tag_rest(&t));
    }
    Ok((StatusCode::CREATED, Json(out)))
}

pub async fn target_tag_update(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(u): Json<TagUpdate>,
) -> Result<Json<TagRest>, AppError> {
    let t = load_target_tag(&st, id).await?;
    if let Some(name) = &u.name {
        if target_tag::Entity::find()
            .filter(target_tag::Column::Name.eq(name))
            .one(&st.db)
            .await?
            .is_some_and(|other| other.id != t.id)
        {
            return Err(AppError::Conflict(format!(
                "target tag {name} already exists"
            )));
        }
    }
    let mut am: target_tag::ActiveModel = t.into();
    if let Some(n) = u.name {
        am.name = Set(n);
    }
    if let Some(d) = u.description {
        am.description = Set(Some(d));
    }
    if let Some(c) = u.colour {
        am.colour = Set(Some(c));
    }
    am.updated_at = Set(now_ms());
    Ok(Json(target_tag_rest(&am.update(&st.db).await?)))
}

/// Deleting a tag drops its assignments; the tagged targets stay.
pub async fn target_tag_delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let t = load_target_tag(&st, id).await?;
    target_tag_assignment::Entity::delete_many()
        .filter(target_tag_assignment::Column::TagId.eq(t.id))
        .exec(&st.db)
        .await?;
    target_tag::Entity::delete_by_id(t.id).exec(&st.db).await?;
    Ok(StatusCode::OK)
}

pub async fn target_tag_assigned(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<TargetRest>>, AppError> {
    let t = load_target_tag(&st, id).await?;
    let mut sel = target::Entity::find().filter(targets_with_tag(t.id));
    if let Some(q) = &p.q {
        sel = sel.filter(super::targets::condition(q)?);
    }
    sel = apply_sort(sel, &p.sort, &super::targets::fiql_map)?;
    let (rows, total) = page(&st.db, sel, &p).await?;
    let base = base_url(&st.cfg, &headers);
    let interval = st.cfg.ddi.polling_duration();
    Ok(Json(Paged::new(
        rows.iter()
            .map(|t| super::mappers::target_rest(t, interval, &base))
            .collect(),
        total,
    )))
}

/// `GET /rest/v1/targets/{cid}/tags` — the tags carried by one target.
///
/// raptor extension (additive, not in hawkBit): the reverse lookup exists only
/// as `/rest/v1/targettags/{id}/assigned`, so showing one target's tags would
/// otherwise cost a request per tag.
pub async fn tags_of_target(
    State(st): State<AppState>,
    Path(cid): Path<String>,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<TagRest>>, AppError> {
    let t = find_by_cid(&st.db, &cid).await?;
    let link = SqlQuery::select()
        .column(target_tag_assignment::Column::TagId)
        .from(target_tag_assignment::Entity)
        .and_where(SqlExpr::col(target_tag_assignment::Column::TargetId).eq(t.id))
        .to_owned();
    let sel = target_tag::Entity::find().filter(tags_of(
        SqlExpr::col((target_tag::Entity, target_tag::Column::Id)),
        link,
    ));
    let (rows, total) = page(&st.db, apply_sort(sel, &p.sort, &target_tag_fiql)?, &p).await?;
    Ok(Json(Paged::new(
        rows.iter().map(target_tag_rest).collect(),
        total,
    )))
}

/// Assignment is idempotent: re-assigning an already-tagged target is a no-op
/// rather than a conflict, so bulk calls can be retried safely.
async fn assign_target(st: &AppState, tag_id: i64, cid: &str) -> Result<target::Model, AppError> {
    let t = find_by_cid(&st.db, cid).await?;
    if target_tag_assignment::Entity::find_by_id((t.id, tag_id))
        .one(&st.db)
        .await?
        .is_none()
    {
        target_tag_assignment::ActiveModel {
            target_id: Set(t.id),
            tag_id: Set(tag_id),
        }
        .insert(&st.db)
        .await?;
    }
    Ok(t)
}

/// `POST /rest/v1/targettags/{id}/assigned` — bulk assign, body is a JSON array
/// of controller ids. Returns the affected targets.
pub async fn target_tag_assign_many(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(cids): Json<Vec<String>>,
) -> Result<Json<Vec<TargetRest>>, AppError> {
    let tag = load_target_tag(&st, id).await?;
    // Resolve every controller id first: an unknown one fails the whole call.
    for cid in &cids {
        find_by_cid(&st.db, cid).await?;
    }
    let base = base_url(&st.cfg, &headers);
    let interval = st.cfg.ddi.polling_duration();
    let mut out = Vec::with_capacity(cids.len());
    for cid in &cids {
        let t = assign_target(&st, tag.id, cid).await?;
        out.push(super::mappers::target_rest(&t, interval, &base));
    }
    Ok(Json(out))
}

/// `POST /rest/v1/targettags/{id}/assigned/{cid}` — assign a single target.
pub async fn target_tag_assign_one(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path((id, cid)): Path<(i64, String)>,
) -> Result<Json<TargetRest>, AppError> {
    let tag = load_target_tag(&st, id).await?;
    let t = assign_target(&st, tag.id, &cid).await?;
    Ok(Json(super::mappers::target_rest(
        &t,
        st.cfg.ddi.polling_duration(),
        &base_url(&st.cfg, &headers),
    )))
}

/// `DELETE /rest/v1/targettags/{id}/assigned/{cid}` — unassign a single target.
pub async fn target_tag_unassign_one(
    State(st): State<AppState>,
    Path((id, cid)): Path<(i64, String)>,
) -> Result<StatusCode, AppError> {
    let tag = load_target_tag(&st, id).await?;
    let t = find_by_cid(&st.db, &cid).await?;
    target_tag_assignment::Entity::find_by_id((t.id, tag.id))
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("target tag assignment"))?;
    target_tag_assignment::Entity::delete_by_id((t.id, tag.id))
        .exec(&st.db)
        .await?;
    Ok(StatusCode::OK)
}

/// `DELETE /rest/v1/targettags/{id}/assigned` — bulk unassign, body is a JSON
/// array of controller ids. Ids that aren't tagged are ignored.
pub async fn target_tag_unassign_many(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(cids): Json<Vec<String>>,
) -> Result<StatusCode, AppError> {
    let tag = load_target_tag(&st, id).await?;
    let mut ids = Vec::with_capacity(cids.len());
    for cid in &cids {
        ids.push(find_by_cid(&st.db, cid).await?.id);
    }
    if !ids.is_empty() {
        target_tag_assignment::Entity::delete_many()
            .filter(target_tag_assignment::Column::TagId.eq(tag.id))
            .filter(target_tag_assignment::Column::TargetId.is_in(ids))
            .exec(&st.db)
            .await?;
    }
    Ok(StatusCode::OK)
}

// --------------------------------------------------------------------------
// Distribution set tags
// --------------------------------------------------------------------------

async fn load_ds_tag(st: &AppState, id: i64) -> Result<ds_tag::Model, AppError> {
    ds_tag::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set tag"))
}

async fn load_ds(st: &AppState, id: i64) -> Result<distribution_set::Model, AppError> {
    distribution_set::Entity::find_by_id(id)
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set"))
}

pub async fn ds_tag_list(
    State(st): State<AppState>,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<TagRest>>, AppError> {
    let mut sel = ds_tag::Entity::find();
    if let Some(q) = &p.q {
        let expr = crate::fiql::parse(q).map_err(AppError::BadRequest)?;
        sel = sel.filter(crate::fiql::to_condition(&expr, &ds_tag_fiql)?);
    }
    sel = apply_sort(sel, &p.sort, &ds_tag_fiql)?;
    let (rows, total) = page(&st.db, sel, &p).await?;
    Ok(Json(Paged::new(
        rows.iter().map(ds_tag_rest).collect(),
        total,
    )))
}

pub async fn ds_tag_one(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<TagRest>, AppError> {
    Ok(Json(ds_tag_rest(&load_ds_tag(&st, id).await?)))
}

pub async fn ds_tag_create(
    State(st): State<AppState>,
    Json(body): Json<Vec<TagCreate>>,
) -> Result<(StatusCode, Json<Vec<TagRest>>), AppError> {
    let mut seen = std::collections::HashSet::new();
    for c in &body {
        if !seen.insert(c.name.as_str()) {
            return Err(AppError::Conflict(format!(
                "duplicate tag {} in request",
                c.name
            )));
        }
        if ds_tag::Entity::find()
            .filter(ds_tag::Column::Name.eq(&c.name))
            .one(&st.db)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict(format!(
                "distribution set tag {} already exists",
                c.name
            )));
        }
    }
    let mut out = Vec::with_capacity(body.len());
    for c in body {
        let now = now_ms();
        let t = ds_tag::ActiveModel {
            name: Set(c.name),
            description: Set(c.description),
            colour: Set(c.colour),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&st.db)
        .await?;
        out.push(ds_tag_rest(&t));
    }
    Ok((StatusCode::CREATED, Json(out)))
}

pub async fn ds_tag_update(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(u): Json<TagUpdate>,
) -> Result<Json<TagRest>, AppError> {
    let t = load_ds_tag(&st, id).await?;
    if let Some(name) = &u.name {
        if ds_tag::Entity::find()
            .filter(ds_tag::Column::Name.eq(name))
            .one(&st.db)
            .await?
            .is_some_and(|other| other.id != t.id)
        {
            return Err(AppError::Conflict(format!(
                "distribution set tag {name} already exists"
            )));
        }
    }
    let mut am: ds_tag::ActiveModel = t.into();
    if let Some(n) = u.name {
        am.name = Set(n);
    }
    if let Some(d) = u.description {
        am.description = Set(Some(d));
    }
    if let Some(c) = u.colour {
        am.colour = Set(Some(c));
    }
    am.updated_at = Set(now_ms());
    Ok(Json(ds_tag_rest(&am.update(&st.db).await?)))
}

pub async fn ds_tag_delete(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let t = load_ds_tag(&st, id).await?;
    ds_tag_assignment::Entity::delete_many()
        .filter(ds_tag_assignment::Column::TagId.eq(t.id))
        .exec(&st.db)
        .await?;
    ds_tag::Entity::delete_by_id(t.id).exec(&st.db).await?;
    Ok(StatusCode::OK)
}

pub async fn ds_tag_assigned(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<DsRest>>, AppError> {
    let t = load_ds_tag(&st, id).await?;
    let mut sel = distribution_set::Entity::find().filter(ds_with_tag(t.id));
    if let Some(q) = &p.q {
        sel = sel.filter(super::distribution_sets::condition(q)?);
    }
    sel = apply_sort(sel, &p.sort, &super::distribution_sets::fiql_map)?;
    let (rows, total) = page(&st.db, sel, &p).await?;
    let base = base_url(&st.cfg, &headers);
    let mut content = Vec::with_capacity(rows.len());
    for ds in &rows {
        content.push(super::distribution_sets::ds_with_modules(&st, ds, &base).await?);
    }
    Ok(Json(Paged::new(content, total)))
}

/// `GET /rest/v1/distributionsets/{id}/tags` — the tags carried by one set.
/// raptor extension, see [`tags_of_target`].
pub async fn tags_of_ds(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Query(p): Query<ListParams>,
) -> Result<Json<Paged<TagRest>>, AppError> {
    let ds = load_ds(&st, id).await?;
    let link = SqlQuery::select()
        .column(ds_tag_assignment::Column::TagId)
        .from(ds_tag_assignment::Entity)
        .and_where(SqlExpr::col(ds_tag_assignment::Column::DsId).eq(ds.id))
        .to_owned();
    let sel = ds_tag::Entity::find().filter(tags_of(
        SqlExpr::col((ds_tag::Entity, ds_tag::Column::Id)),
        link,
    ));
    let (rows, total) = page(&st.db, apply_sort(sel, &p.sort, &ds_tag_fiql)?, &p).await?;
    Ok(Json(Paged::new(
        rows.iter().map(ds_tag_rest).collect(),
        total,
    )))
}

async fn assign_ds_tag(
    st: &AppState,
    tag_id: i64,
    ds_id: i64,
) -> Result<distribution_set::Model, AppError> {
    let ds = load_ds(st, ds_id).await?;
    if ds_tag_assignment::Entity::find_by_id((ds.id, tag_id))
        .one(&st.db)
        .await?
        .is_none()
    {
        ds_tag_assignment::ActiveModel {
            ds_id: Set(ds.id),
            tag_id: Set(tag_id),
        }
        .insert(&st.db)
        .await?;
    }
    Ok(ds)
}

/// `POST /rest/v1/distributionsettags/{id}/assigned` — bulk assign, body is a
/// JSON array of distribution set ids.
pub async fn ds_tag_assign_many(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(ds_ids): Json<Vec<i64>>,
) -> Result<Json<Vec<DsRest>>, AppError> {
    let tag = load_ds_tag(&st, id).await?;
    for ds_id in &ds_ids {
        load_ds(&st, *ds_id).await?;
    }
    let base = base_url(&st.cfg, &headers);
    let mut out = Vec::with_capacity(ds_ids.len());
    for ds_id in &ds_ids {
        let ds = assign_ds_tag(&st, tag.id, *ds_id).await?;
        out.push(super::distribution_sets::ds_with_modules(&st, &ds, &base).await?);
    }
    Ok(Json(out))
}

pub async fn ds_tag_assign_one(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path((id, ds_id)): Path<(i64, i64)>,
) -> Result<Json<DsRest>, AppError> {
    let tag = load_ds_tag(&st, id).await?;
    let ds = assign_ds_tag(&st, tag.id, ds_id).await?;
    Ok(Json(
        super::distribution_sets::ds_with_modules(&st, &ds, &base_url(&st.cfg, &headers)).await?,
    ))
}

pub async fn ds_tag_unassign_one(
    State(st): State<AppState>,
    Path((id, ds_id)): Path<(i64, i64)>,
) -> Result<StatusCode, AppError> {
    let tag = load_ds_tag(&st, id).await?;
    let ds = load_ds(&st, ds_id).await?;
    ds_tag_assignment::Entity::find_by_id((ds.id, tag.id))
        .one(&st.db)
        .await?
        .ok_or(AppError::NotFound("distribution set tag assignment"))?;
    ds_tag_assignment::Entity::delete_by_id((ds.id, tag.id))
        .exec(&st.db)
        .await?;
    Ok(StatusCode::OK)
}

pub async fn ds_tag_unassign_many(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(ds_ids): Json<Vec<i64>>,
) -> Result<StatusCode, AppError> {
    let tag = load_ds_tag(&st, id).await?;
    for ds_id in &ds_ids {
        load_ds(&st, *ds_id).await?;
    }
    if !ds_ids.is_empty() {
        ds_tag_assignment::Entity::delete_many()
            .filter(ds_tag_assignment::Column::TagId.eq(tag.id))
            .filter(ds_tag_assignment::Column::DsId.is_in(ds_ids))
            .exec(&st.db)
            .await?;
    }
    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{EntityTrait, QueryFilter, QueryTrait};

    fn target_sql(q: &str) -> String {
        let expr = crate::fiql::parse(q).unwrap();
        let cond = crate::fiql::to_condition_ext(
            &expr,
            &crate::api::mgmt::targets::fiql_map,
            &target_tag_field,
        )
        .unwrap();
        target::Entity::find()
            .filter(cond)
            .build(sea_orm::DatabaseBackend::Sqlite)
            .to_string()
    }

    #[test]
    fn tag_eq_is_an_in_subquery() {
        let s = target_sql("tag==beta");
        assert!(s.contains("\"id\" IN (SELECT \"target_id\""), "{s}");
        assert!(s.contains("\"name\" = 'beta'"), "{s}");
        assert!(!s.contains("NOT IN"), "{s}");
    }

    #[test]
    fn tag_ne_negates_the_membership_not_the_name() {
        let s = target_sql("tag!=beta");
        assert!(s.contains("NOT IN (SELECT \"target_id\""), "{s}");
        // The name comparison inside the subquery stays positive.
        assert!(s.contains("\"name\" = 'beta'"), "{s}");
    }

    #[test]
    fn tag_wildcard_becomes_like() {
        let s = target_sql("tag==be*");
        assert!(s.contains("LIKE 'be%'"), "{s}");
    }

    #[test]
    fn tag_in_list() {
        let s = target_sql("tag=in=(beta,stable)");
        assert!(s.contains("IN ('beta', 'stable')"), "{s}");
    }

    #[test]
    fn tag_combines_with_plain_columns() {
        let s = target_sql("tag==beta;updateStatus==in_sync");
        assert!(s.contains("AND"), "{s}");
        assert!(s.contains("\"update_status\" = 'in_sync'"), "{s}");
    }

    #[test]
    fn ordering_operators_rejected_on_tag() {
        let expr = crate::fiql::parse("tag=gt=beta").unwrap();
        assert!(matches!(
            crate::fiql::to_condition_ext(
                &expr,
                &crate::api::mgmt::targets::fiql_map,
                &target_tag_field
            ),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn ds_tag_targets_the_distribution_set_table() {
        let expr = crate::fiql::parse("tag==beta").unwrap();
        let cond = crate::fiql::to_condition_ext(
            &expr,
            &|f: &str| (f == "name").then_some(distribution_set::Column::Name),
            &ds_tag_field,
        )
        .unwrap();
        let s = distribution_set::Entity::find()
            .filter(cond)
            .build(sea_orm::DatabaseBackend::Sqlite)
            .to_string();
        assert!(s.contains("IN (SELECT \"ds_id\""), "{s}");
    }
}
