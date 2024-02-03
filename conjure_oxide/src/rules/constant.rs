use conjure_core::{ast::Constant as Const, ast::Expression as Expr, rule::RuleApplicationError};
use conjure_rules::register_rule;

#[register_rule]
fn apply_simplify_to_constant(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match simplify_to_constant(expr) {
        Some(c) => Ok(Expr::Constant(c)),
        None => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/// Simplify an expression to a constant if possible
/// Returns:
/// `None` if the expression cannot be simplified to a constant (e.g. if it contains a variable)
/// `Some(Const)` if the expression can be simplified to a constant
pub fn simplify_to_constant(expr: &Expr) -> Option<Const> {
    None
}
