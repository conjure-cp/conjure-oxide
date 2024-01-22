use conjure_core::ast::Expression;
use conjure_core::rule::{Rule, RuleApplicationError};

fn identity(expr: &Expression) -> Result<Expression, RuleApplicationError> {
    Ok(expr.clone())
}

pub static IDENTITY_RULE: Rule = Rule {
    name: "identity",
    application: identity,
};
