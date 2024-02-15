use conjure_core::ast::Expression;
use conjure_core::rule::RuleApplicationError;
use conjure_rule_sets::register_rule_set;
use conjure_rules::register_rule;

register_rule_set!("TestRS", ());

#[register_rule(("TestRS", 0))]
fn identity(expr: &Expression) -> Result<Expression, RuleApplicationError> {
    Ok(expr.clone())
}
