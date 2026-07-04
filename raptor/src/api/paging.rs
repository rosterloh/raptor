use crate::error::AppError;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, Order, QueryOrder, QuerySelect, Select};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize)]
pub struct Paged<T: Serialize> {
    pub content: Vec<T>,
    pub total: u64,
    pub size: usize,
}

impl<T: Serialize> Paged<T> {
    pub fn new(content: Vec<T>, total: u64) -> Self {
        let size = content.len();
        Self { content, total, size }
    }
}

pub fn apply_sort<E: EntityTrait, C: ColumnTrait>(
    sel: Select<E>,
    sort: &Option<String>,
    map: &dyn Fn(&str) -> Option<C>,
) -> Result<Select<E>, AppError> {
    let Some(s) = sort else { return Ok(sel) };
    let (field, dir) = s.split_once(':').unwrap_or((s.as_str(), "ASC"));
    let col = map(field)
        .ok_or_else(|| AppError::BadRequest(format!("unknown sort field: {field}")))?;
    let order = match dir.to_ascii_uppercase().as_str() {
        "ASC" => Order::Asc,
        "DESC" => Order::Desc,
        other => return Err(AppError::BadRequest(format!("invalid sort direction: {other}"))),
    };
    Ok(sel.order_by(col, order))
}

/// hawkBit list convention: filter (q) applied by caller; this does count + offset/limit.
pub async fn page<E: EntityTrait>(
    db: &DatabaseConnection,
    sel: Select<E>,
    p: &ListParams,
) -> Result<(Vec<E::Model>, u64), AppError> {
    // Clone the select for counting - we need to count all results before applying offset/limit
    let count_sel = sel.clone();

    // The number of total items is obtained by fetching with a very high limit and counting
    // This is a workaround since Select doesn't directly support count()
    let all_rows = count_sel.all(db).await?;
    let total = all_rows.len() as u64;

    // Now fetch with offset/limit
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
        assert!(
            s.contains("ORDER BY \"target\".\"name\" DESC"),
            "{s}"
        );
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
