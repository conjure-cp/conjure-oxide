use conjure_macros::register_rule;

use crate::{
    ast::{Atom, Expression as Expr},
    rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, Reduction},
    Model,
};

/// Substitutes value lettings for their values.
///
/// # Priority
///
/// This rule must have a higher priority than solver-flattening rules (which should be priority 4000).
///
/// Otherwise, the letting may be put into a flat constraint, as it is a reference. At this point
/// it ceases to be an expression, so we cannot match over it.
#[register_rule(("Base", 5000))]
fn substitute_value_lettings(expr: &Expr, m: &Model) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Reference(name)) = expr else {
        return Err(RuleNotApplicable);
    };

    let value = m
        .symbols()
        .get_value_letting(name)
        .ok_or(RuleNotApplicable)?;

    Ok(Reduction::pure(value.clone()))
}
