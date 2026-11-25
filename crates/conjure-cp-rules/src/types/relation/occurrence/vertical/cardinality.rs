use super::super::RelationOccurrence;
use crate::guard;
use conjure_cp::ast::{Atom, Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

#[register_rule("ReprGeneral", 9500, [Card])]
fn cardinality_relation_occurrence(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Card(_, rel) = expr &&
        let Expr::Atomic(_, Atom::Reference(reference)) = rel.as_ref() &&
        let Some(repr) = reference.get_repr_as::<RelationOccurrence>()
        else { return Err(RuleNotApplicable) }
    );

    Ok(RuleEffect::pure(repr.cardinality_expr()))
}
