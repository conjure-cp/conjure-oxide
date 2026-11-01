use super::super::FunctionExplicit;
use crate::types::function::horizontal::membership_from_positions;
use conjure_cp::ast::{Atom, Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

#[register_rule("Base", 8700, [In])]
fn membership_in_function_explicit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::In(_, member, collection) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Defined(_, function) = collection.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = function.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let representation = reference
        .ptr()
        .get_repr::<FunctionExplicit>()
        .ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::pure(membership_from_positions(
        member,
        representation.domain_values.len(),
        |index| Expr::from(representation.domain_values[(index - 1) as usize].clone()),
        |index| representation.defined_expr(index),
    )))
}
