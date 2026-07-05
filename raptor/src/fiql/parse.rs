use winnow::ascii::multispace0;
use winnow::combinator::{alt, delimited, separated};
use winnow::prelude::*;
use winnow::token::take_while;
use winnow::Result as WResult;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    And(Vec<Expr>),
    Or(Vec<Expr>),
    Cmp(Comparison),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    pub field: String,
    pub op: Op,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    Out,
}

pub fn parse(input: &str) -> Result<Expr, String> {
    let mut i = input;
    let expr = or_expr(&mut i).map_err(|e| format!("invalid FIQL query: {e}"))?;
    if !i.is_empty() {
        return Err(format!("invalid FIQL query: trailing input {i:?}"));
    }
    Ok(expr)
}

fn or_expr(i: &mut &str) -> WResult<Expr> {
    let items: Vec<Expr> = separated(1.., and_expr, ',').parse_next(i)?;
    Ok(if items.len() == 1 {
        items.into_iter().next().unwrap()
    } else {
        Expr::Or(items)
    })
}

fn and_expr(i: &mut &str) -> WResult<Expr> {
    let items: Vec<Expr> = separated(1.., primary, ';').parse_next(i)?;
    Ok(if items.len() == 1 {
        items.into_iter().next().unwrap()
    } else {
        Expr::And(items)
    })
}

fn primary(i: &mut &str) -> WResult<Expr> {
    delimited(multispace0, alt((parens, comparison)), multispace0).parse_next(i)
}

fn parens(i: &mut &str) -> WResult<Expr> {
    delimited('(', or_expr, ')').parse_next(i)
}

fn comparison(i: &mut &str) -> WResult<Expr> {
    let field = take_while(1.., |c: char| {
        c.is_alphanumeric() || matches!(c, '.' | '_' | '-')
    })
    .parse_next(i)?;
    let op = op(i)?;
    let values = match op {
        Op::In | Op::Out => delimited('(', separated(1.., value, ','), ')').parse_next(i)?,
        _ => vec![value(i)?],
    };
    Ok(Expr::Cmp(Comparison {
        field: field.to_string(),
        op,
        values,
    }))
}

fn op(i: &mut &str) -> WResult<Op> {
    alt((
        "==".value(Op::Eq),
        "!=".value(Op::Ne),
        "=lt=".value(Op::Lt),
        "=le=".value(Op::Le),
        "=gt=".value(Op::Gt),
        "=ge=".value(Op::Ge),
        "=in=".value(Op::In),
        "=out=".value(Op::Out),
    ))
    .parse_next(i)
}

fn value(i: &mut &str) -> WResult<String> {
    alt((quoted('"'), quoted('\''), bare)).parse_next(i)
}

fn bare(i: &mut &str) -> WResult<String> {
    take_while(1.., |c: char| {
        !matches!(c, ';' | ',' | '(' | ')' | '"' | '\'') && !c.is_whitespace()
    })
    .map(|s: &str| s.to_string())
    .parse_next(i)
}

fn quoted(q: char) -> impl FnMut(&mut &str) -> WResult<String> {
    move |i: &mut &str| {
        delimited(q, take_while(0.., move |c: char| c != q), q)
            .map(|s: &str| s.to_string())
            .parse_next(i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmp(field: &str, op: Op, values: &[&str]) -> Expr {
        Expr::Cmp(Comparison {
            field: field.into(),
            op,
            values: values.iter().map(|s| s.to_string()).collect(),
        })
    }

    #[test]
    fn simple_eq() {
        assert_eq!(parse("name==foo").unwrap(), cmp("name", Op::Eq, &["foo"]));
    }

    #[test]
    fn wildcard_value_kept_verbatim() {
        assert_eq!(parse("name==foo*").unwrap(), cmp("name", Op::Eq, &["foo*"]));
    }

    #[test]
    fn and_of_two() {
        assert_eq!(
            parse("name==foo*;updateStatus==pending").unwrap(),
            Expr::And(vec![
                cmp("name", Op::Eq, &["foo*"]),
                cmp("updateStatus", Op::Eq, &["pending"])
            ])
        );
    }

    #[test]
    fn and_binds_tighter_than_or() {
        assert_eq!(
            parse("a==1,b==2;c==3").unwrap(),
            Expr::Or(vec![
                cmp("a", Op::Eq, &["1"]),
                Expr::And(vec![cmp("b", Op::Eq, &["2"]), cmp("c", Op::Eq, &["3"])])
            ])
        );
    }

    #[test]
    fn parens_override_precedence() {
        assert_eq!(
            parse("(a==1,b==2);c==3").unwrap(),
            Expr::And(vec![
                Expr::Or(vec![cmp("a", Op::Eq, &["1"]), cmp("b", Op::Eq, &["2"])]),
                cmp("c", Op::Eq, &["3"])
            ])
        );
    }

    #[test]
    fn all_relational_ops() {
        for (s, op) in [
            ("=lt=", Op::Lt),
            ("=le=", Op::Le),
            ("=gt=", Op::Gt),
            ("=ge=", Op::Ge),
            ("!=", Op::Ne),
        ] {
            assert_eq!(
                parse(&format!("x{s}5")).unwrap(),
                cmp("x", op, &["5"]),
                "op {s}"
            );
        }
    }

    #[test]
    fn in_and_out_lists() {
        assert_eq!(
            parse("status=in=(pending,error)").unwrap(),
            cmp("status", Op::In, &["pending", "error"])
        );
        assert_eq!(
            parse("status=out=(unknown)").unwrap(),
            cmp("status", Op::Out, &["unknown"])
        );
    }

    #[test]
    fn quoted_values_allow_specials() {
        assert_eq!(
            parse("name==\"has space;and,comma\"").unwrap(),
            cmp("name", Op::Eq, &["has space;and,comma"])
        );
        assert_eq!(
            parse("name=='single quoted'").unwrap(),
            cmp("name", Op::Eq, &["single quoted"])
        );
    }

    #[test]
    fn dotted_field_names() {
        assert_eq!(
            parse("attribute.hw_revision==2").unwrap(),
            cmp("attribute.hw_revision", Op::Eq, &["2"])
        );
    }

    #[test]
    fn rejects_garbage() {
        for bad in [
            "",
            "name==",
            "==foo",
            "name=zz=1",
            "a==1;;b==2",
            "(a==1",
            "a==1)b==2",
        ] {
            assert!(parse(bad).is_err(), "should reject {bad:?}");
        }
    }
}
