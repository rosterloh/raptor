//! Compiles a parsed FIQL `Expr` into a SeaORM `Condition`/`SimpleExpr`
//! against a caller-supplied column map, with an optional `VirtualField` hook
//! for fields that aren't plain columns (e.g. `tag` membership joins).

use super::{Comparison, Expr, Op};
use crate::error::AppError;
use sea_orm::sea_query::{ExprTrait, Func, LikeExpr, SimpleExpr};
use sea_orm::{ColumnTrait, ColumnType, Condition};

/// Resolves a FIQL field that is not a column of the queried entity — `tag`,
/// which compiles to a membership test against the tag join table. Returns
/// `None` for names it does not handle, so they fall through to the column map.
pub type VirtualField = dyn Fn(&str, &Op, &[String]) -> Option<Result<SimpleExpr, AppError>>;

/// Compiles one comparison against a column. Integer columns bind `i64`
/// (non-numbers are `400`), boolean columns `true`/`false`. Strings compare
/// case-insensitively, as hawkBit does, via `LOWER()` on both sides so SQLite
/// and Postgres agree; `*` becomes a `LIKE` wildcard with `%`/`_` escaped.
pub fn cmp_expr<C: ColumnTrait>(
    col: C,
    op: &Op,
    values: &[String],
) -> Result<SimpleExpr, AppError> {
    let first = values.first().map(String::as_str).unwrap_or_default();
    match col.def().get_column_type() {
        ColumnType::TinyInteger
        | ColumnType::SmallInteger
        | ColumnType::Integer
        | ColumnType::BigInteger => {
            let nums = values
                .iter()
                .map(|v| {
                    v.parse::<i64>()
                        .map_err(|_| AppError::BadRequest(format!("not a number: {v}")))
                })
                .collect::<Result<Vec<i64>, _>>()?;
            let n = nums.first().copied().unwrap_or_default();
            Ok(match op {
                Op::Eq => col.eq(n),
                Op::Ne => col.ne(n),
                Op::Lt => col.lt(n),
                Op::Le => col.lte(n),
                Op::Gt => col.gt(n),
                Op::Ge => col.gte(n),
                Op::In => col.is_in(nums),
                Op::Out => col.is_not_in(nums),
            })
        }
        ColumnType::Boolean => {
            let b = match first {
                "true" => true,
                "false" => false,
                _ => return Err(AppError::BadRequest(format!("not a boolean: {first}"))),
            };
            match op {
                Op::Eq => Ok(col.eq(b)),
                Op::Ne => Ok(col.ne(b)),
                _ => Err(AppError::BadRequest(
                    "boolean fields support only == and !=".into(),
                )),
            }
        }
        _ => {
            let lower = || SimpleExpr::from(Func::lower(col.into_expr()));
            let v = first.to_lowercase();
            let all = || values.iter().map(|v| v.to_lowercase());
            Ok(match op {
                Op::Eq if v.contains('*') => lower().like(like_pattern(&v)),
                Op::Ne if v.contains('*') => lower().not_like(like_pattern(&v)),
                Op::Eq => ExprTrait::eq(lower(), v),
                Op::Ne => ExprTrait::ne(lower(), v),
                Op::Lt => col.lt(first),
                Op::Le => col.lte(first),
                Op::Gt => col.gt(first),
                Op::Ge => col.gte(first),
                Op::In => lower().is_in(all()),
                Op::Out => lower().is_not_in(all()),
            })
        }
    }
}

/// `*` → `%`, with the value's own `LIKE` metacharacters escaped.
fn like_pattern(v: &str) -> LikeExpr {
    let escaped = v
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
        .replace('*', "%");
    LikeExpr::new(escaped).escape('\\')
}

pub fn to_condition<C: ColumnTrait>(
    expr: &Expr,
    map: &dyn Fn(&str) -> Option<C>,
) -> Result<Condition, AppError> {
    to_condition_ext(expr, map, &|_, _, _| None)
}

/// Like [`to_condition`], but consults `virtual_field` before the column map so
/// callers can compile joined fields (`tag==beta`) into subqueries.
pub fn to_condition_ext<C: ColumnTrait>(
    expr: &Expr,
    map: &dyn Fn(&str) -> Option<C>,
    virtual_field: &VirtualField,
) -> Result<Condition, AppError> {
    Ok(match expr {
        Expr::And(items) => {
            let mut c = Condition::all();
            for i in items {
                c = c.add(to_condition_ext(i, map, virtual_field)?);
            }
            c
        }
        Expr::Or(items) => {
            let mut c = Condition::any();
            for i in items {
                c = c.add(to_condition_ext(i, map, virtual_field)?);
            }
            c
        }
        Expr::Cmp(Comparison { field, op, values }) => {
            let e = match virtual_field(field, op, values) {
                Some(r) => r?,
                None => {
                    let col = map(field).ok_or_else(|| {
                        AppError::BadRequest(format!("unknown query field: {field}"))
                    })?;
                    cmp_expr(col, op, values)?
                }
            };
            Condition::all().add(e)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::target;
    use sea_orm::{Condition, EntityTrait, QueryFilter, QueryTrait};

    fn map(field: &str) -> Option<target::Column> {
        match field {
            "name" => Some(target::Column::Name),
            "controllerId" => Some(target::Column::ControllerId),
            "updateStatus" => Some(target::Column::UpdateStatus),
            _ => None,
        }
    }

    fn sql(cond: Condition) -> String {
        target::Entity::find()
            .filter(cond)
            .build(sea_orm::DatabaseBackend::Sqlite)
            .to_string()
    }

    #[test]
    fn eq_becomes_equals() {
        let e = crate::fiql::parse("name==foo").unwrap();
        let s = sql(to_condition(&e, &map).unwrap());
        assert!(s.contains("LOWER(\"target\".\"name\") = 'foo'"), "{s}");
    }

    #[test]
    fn wildcard_becomes_like() {
        let e = crate::fiql::parse("name==Fo_o%*").unwrap();
        let s = sql(to_condition(&e, &map).unwrap());
        assert!(s.contains(r"LIKE 'fo\_o\%%' ESCAPE '\'"), "{s}");
    }

    #[test]
    fn and_or_nesting() {
        let e = crate::fiql::parse("name==a,name==b;updateStatus==pending").unwrap();
        let s = sql(to_condition(&e, &map).unwrap());
        assert!(s.contains("OR"), "{s}");
        assert!(s.contains("AND"), "{s}");
    }

    #[test]
    fn in_list() {
        let e = crate::fiql::parse("updateStatus=in=(pending,error)").unwrap();
        let s = sql(to_condition(&e, &map).unwrap());
        assert!(s.contains("IN ('pending', 'error')"), "{s}");
    }

    #[test]
    fn unknown_field_is_bad_request() {
        let e = crate::fiql::parse("bogus==1").unwrap();
        assert!(matches!(
            to_condition(&e, &map),
            Err(crate::error::AppError::BadRequest(_))
        ));
    }

    #[test]
    fn bool_values_typed() {
        use crate::entity::action;

        fn action_map(field: &str) -> Option<action::Column> {
            (field == "active").then_some(action::Column::Active)
        }

        let e = crate::fiql::parse("active==true").unwrap();
        let s = action::Entity::find()
            .filter(to_condition(&e, &action_map).unwrap())
            .build(sea_orm::DatabaseBackend::Sqlite)
            .to_string();
        // must compile to a typed bool comparison, not a string literal
        assert!(s.contains("\"active\" = TRUE"), "{s}");
        assert!(!s.contains("'true'"), "{s}");

        let e = crate::fiql::parse("active==false").unwrap();
        let s = action::Entity::find()
            .filter(to_condition(&e, &action_map).unwrap())
            .build(sea_orm::DatabaseBackend::Sqlite)
            .to_string();
        assert!(s.contains("\"active\" = FALSE"), "{s}");
        assert!(!s.contains("'false'"), "{s}");
    }
}
