//! Normalising rules for `Sum`

use ApplicationError::RuleNotApplicable;
use conjure_cp::{
    ast::{Expression as Expr, SymbolTable},
    rule_engine::{ApplicationError, ApplicationResult, RuleEffect, register_rule},
};

/// Removes sums with a single argument.
///
/// ```text
/// sum([a]) ~> a
/// ```
#[register_rule("Base", 8800, [Sum])]
fn remove_unit_vector_sum(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Sum(_, e) = expr else {
        return Err(RuleNotApplicable);
    };

    // The failure path is only a shape check and must stay O(1) on a wide sum.
    if e.list_len() != Some(1) {
        return Err(RuleNotApplicable);
    }
    let mut exprs = e.unwrap_list().ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::pure(
        exprs.pop().expect("singleton length checked above"),
    ))
}
