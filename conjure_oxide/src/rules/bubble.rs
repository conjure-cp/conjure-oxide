use conjure_core::{
    ast::{Constant, Expression, Model},
    metadata::Metadata,
    rule::{ApplicationError, ApplicationResult, Reduction},
};
use conjure_rules::{register_rule, register_rule_set};

register_rule_set!("Bubble", 254, ());

/*
    Reduce bubbles with a boolean expression to a conjunction with their condition.

    e.g. (a / b = c) @ (b != 0) => (a / b = c) & (b != 0)
*/
#[register_rule(("Bubble", 100))]
fn expand_bubble(expr: &Expression, _: &Model) -> ApplicationResult {
    match expr {
        // TODO: change "false" to check return type
        Expression::Bubble(_, a, b) if false => Ok(Reduction::pure(Expression::And(
            Metadata::new(),
            vec![*a.clone(), *b.clone()],
        ))),
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/*
    Bring bubbles not caught by the above rule higher up the tree.

    E.g. ((a / b) @ (b != 0)) = c => (a / b = c) @ (b != 0)
*/
#[register_rule(("Bubble", 100))]
fn bubble_up(expr: &Expression, _: &Model) -> ApplicationResult {
    todo!();
}

#[register_rule(("Bubble", 100))]
fn div_to_bubble(expr: &Expression, _: &Model) -> ApplicationResult {
    if let Expression::Div(m, a, b) = expr {
        return Ok(Reduction::pure(Expression::Bubble(
            Metadata::new(),
            Box::new(Expression::SafeDiv(m.clone(), a.clone(), b.clone())),
            Box::new(Expression::Neq(
                Metadata::new(),
                b.clone(),
                Box::new(Expression::Constant(Metadata::new(), Constant::Int(0))),
            )),
        )));
    }
    return Err(ApplicationError::RuleNotApplicable);
}
