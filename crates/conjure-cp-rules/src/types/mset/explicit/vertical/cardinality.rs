use super::super::MSetExplicit;
use crate::guard;
use conjure_cp::ast::{Atom, Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

#[register_rule("ReprGeneral", 9500, [Card])]
fn cardinality_explicit_mset(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(let Expr::Card(_, value) = expr &&
        let Expr::Atomic(_, Atom::Reference(reference)) = value.as_ref() &&
        let Some(representation) = reference.get_repr_as::<MSetExplicit>()
        else { return Err(RuleNotApplicable) });
    Ok(RuleEffect::pure(representation.cardinality_expr()))
}
