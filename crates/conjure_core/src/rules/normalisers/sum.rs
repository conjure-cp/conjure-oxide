//! Normalising rules for `Sum`

use conjure_core::ast::Expression as Expr;
use conjure_core::rule_engine::{register_rule, ApplicationResult, Reduction};

use crate::ast::SymbolTable;
use crate::rule_engine::ApplicationError::RuleNotApplicable;

/// Removes sums with a single argument.
///
/// ```text
/// sum([a]) ~> a
/// ```
#[register_rule(("Base", 8800))]
fn remove_unit_vector_sum(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Sum(_, e) = expr else {
        return Err(RuleNotApplicable);
    };

    let exprs = e.as_ref().clone().unwrap_list().ok_or(RuleNotApplicable)?;

    if exprs.len() == 1 {
        Ok(Reduction::pure(exprs[0].clone()))
    } else {
        Err(RuleNotApplicable)
    }
}
