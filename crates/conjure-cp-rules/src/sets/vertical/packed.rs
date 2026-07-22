use crate::guard;
use crate::representation::SetPacked;
use conjure_cp::ast::{Atom, Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

#[register_rule("ReprGeneral", 9500, [In])]
fn membership_packed(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::In(_, member, set) = expr &&
        let Expr::Atomic(_, Atom::Reference(reference)) = set.as_ref() &&
        let Some(repr) = reference.get_repr_as::<SetPacked>()
        else { return Err(RuleNotApplicable) }
    );

    Ok(RuleEffect::pure(repr.membership_expr((**member).clone())))
}

#[register_rule("ReprGeneral", 9500, [Card])]
fn cardinality_packed(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Card(_, set) = expr &&
        let Expr::Atomic(_, Atom::Reference(reference)) = set.as_ref() &&
        let Some(repr) = reference.get_repr_as::<SetPacked>()
        else { return Err(RuleNotApplicable) }
    );

    Ok(RuleEffect::pure(repr.cardinality_expr()))
}
