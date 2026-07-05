use crate::error::AppError;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait, QueryOrder, QuerySelect,
    Select,
};
use serde::Deserialize;

pub use raptor_api_types::PagedList as Paged;

fn default_limit() -> u64 {
    50
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    #[serde(default)]
    pub offset: u64,
    #[serde(default = "default_limit")]
    pub limit: u64,
    pub sort: Option<String>,
    pub q: Option<String>,
}

pub fn apply_sort<E: EntityTrait, C: ColumnTrait>(
    sel: Select<E>,
    sort: &Option<String>,
    map: &dyn Fn(&str) -> Option<C>,
) -> Result<Select<E>, AppError> {
    let Some(s) = sort else { return Ok(sel) };
    let (field, dir) = s.split_once(':').unwrap_or((s.as_str(), "ASC"));
    let col =
        map(field).ok_or_else(|| AppError::BadRequest(format!("unknown sort field: {field}")))?;
    let order = match dir.to_ascii_uppercase().as_str() {
        "ASC" => Order::Asc,
        "DESC" => Order::Desc,
        other => {
            return Err(AppError::BadRequest(format!(
                "invalid sort direction: {other}"
            )))
        }
    };
    Ok(sel.order_by(col, order))
}

/// hawkBit list convention: filter (q) applied by caller; this does count + offset/limit.
pub async fn page<E: EntityTrait + Send>(
    db: &DatabaseConnection,
    sel: Select<E>,
    p: &ListParams,
) -> Result<(Vec<E::Model>, u64), AppError>
where
    E::Model: Send + Sync,
{
    let count_sel = sel.clone();
    let total = count_sel.count(db).await?;
    let rows = sel.offset(p.offset).limit(p.limit).all(db).await?;
    Ok((rows, total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::target;
    use sea_orm::QueryTrait;

    fn map(f: &str) -> Option<target::Column> {
        (f == "name").then_some(target::Column::Name)
    }

    #[test]
    fn sort_parses_direction() {
        let sel = apply_sort(target::Entity::find(), &Some("name:DESC".into()), &map).unwrap();
        let s = sel.build(sea_orm::DatabaseBackend::Sqlite).to_string();
        assert!(s.contains("ORDER BY \"target\".\"name\" DESC"), "{s}");
    }

    #[test]
    fn bad_sort_field_rejected() {
        assert!(apply_sort(target::Entity::find(), &Some("nope:ASC".into()), &map).is_err());
    }

    #[test]
    fn paged_envelope_shape() {
        let p = Paged::new(vec![1, 2, 3], 10);
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["total"], 10);
        assert_eq!(v["size"], 3);
        assert_eq!(v["content"], serde_json::json!([1, 2, 3]));
    }
}
