use crate::_RULES_DISTRIBUTED_SLICE;
use conjure_core::{ast::Expression, rule::RuleApplicationError};
use conjure_macros::register_rule;

#[register_rule]
fn identity(expr: &Expression) -> Result<Expression, RuleApplicationError> {
    Ok(expr.clone())
}
