//! Normalising rules for `Sum`

use conjure_core::ast::Expression as Expr;
use conjure_core::rule_engine::{register_rule, ApplicationError, ApplicationResult, Reduction};
use conjure_core::Model;

use Expr::*;

/// Removes sums with a single argument.
///
/// ```text
/// sum([a]) ~> a
/// ```
#[register_rule(("Base", 8800))]
fn remove_unit_vector_sum(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Sum(_, exprs) if (exprs.len() == 1) => Ok(Reduction::pure(exprs[0].clone())),
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}
