//! Normalising rules for `Sum`

use conjure_core::ast::Expression as Expr;
use conjure_core::rule_engine::{register_rule, ApplicationError, ApplicationResult, Reduction};

use Expr::*;

use crate::ast::SymbolTable;

/// Removes sums with a single argument.
///
/// ```text
/// sum([a]) ~> a
/// ```
#[register_rule(("Base", 8800))]
fn remove_unit_vector_sum(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Sum(_, exprs) if (exprs.len() == 1) => Ok(Reduction::pure(exprs[0].clone())),
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}
