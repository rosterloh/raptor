use super::{Comparison, Expr, Op};
use crate::error::AppError;
use sea_orm::{ColumnTrait, Condition};

pub fn to_condition<C: ColumnTrait>(
    expr: &Expr,
    map: &dyn Fn(&str) -> Option<C>,
) -> Result<Condition, AppError> {
    Ok(match expr {
        Expr::And(items) => {
            let mut c = Condition::all();
            for i in items {
                c = c.add(to_condition(i, map)?);
            }
            c
        }
        Expr::Or(items) => {
            let mut c = Condition::any();
            for i in items {
                c = c.add(to_condition(i, map)?);
            }
            c
        }
        Expr::Cmp(Comparison { field, op, values }) => {
            let col = map(field)
                .ok_or_else(|| AppError::BadRequest(format!("unknown query field: {field}")))?;
            let v = values.first().cloned().unwrap_or_default();
            let has_wild = v.contains('*');
            let like = v.replace('*', "%");
            let e = match op {
                Op::Eq if has_wild => col.like(like),
                Op::Eq => col.eq(v),
                Op::Ne if has_wild => col.not_like(like),
                Op::Ne => col.ne(v),
                Op::Lt => col.lt(v),
                Op::Le => col.lte(v),
                Op::Gt => col.gt(v),
                Op::Ge => col.gte(v),
                Op::In => col.is_in(values.clone()),
                Op::Out => col.is_not_in(values.clone()),
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
        assert!(s.contains("\"name\" = 'foo'"), "{s}");
    }

    #[test]
    fn wildcard_becomes_like() {
        let e = crate::fiql::parse("name==foo*").unwrap();
        let s = sql(to_condition(&e, &map).unwrap());
        assert!(s.contains("LIKE 'foo%'"), "{s}");
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
}
