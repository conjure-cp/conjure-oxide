use conjure_core::{ast::Expression, rule::RuleApplicationError};
use conjure_rules::register_rule;

#[register_rule]
fn identity(expr: &Expression) -> Result<Expression, RuleApplicationError> {
    Ok(expr.clone())
}
