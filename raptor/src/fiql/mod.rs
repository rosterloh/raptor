//! FIQL query support: `parse` turns a hawkBit-style FIQL string into an
//! `Expr` AST, and `compile`'s `to_condition`/`to_condition_ext` turn that
//! into a SeaORM `Condition` for list-endpoint filtering.

mod compile;
mod parse;
pub use compile::{cmp_expr, to_condition, to_condition_ext};
pub use parse::{Comparison, Expr, Op, parse};
