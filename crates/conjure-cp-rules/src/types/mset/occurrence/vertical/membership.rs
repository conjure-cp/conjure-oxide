use super::super::MSetOccurrence;
use crate::guard;
use conjure_cp::ast::{Atom, Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

#[register_rule("ReprGeneral", 9500, [In])]
fn membership_occurrence_mset(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(let Expr::In(_, member, value) = expr && let Expr::Atomic(_, Atom::Reference(reference)) = value.as_ref() && let Some(representation) = reference.get_repr_as::<MSetOccurrence>() else { return Err(RuleNotApplicable) });
    Ok(RuleEffect::pure(
        representation.membership_expr((**member).clone()),
    ))
}
