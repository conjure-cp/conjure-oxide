use super::super::RelationAsSet;
use conjure_cp::ast::{Atom, Expression as Expr, Moo, Reference, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// A relation channelled through [`RelationAsSet`] is, value-for-value, the same set of tuples,
/// so cardinality delegates straight to whichever set representation `set_decl` ends up with.
#[register_rule("Base", 8650, [Card])]
fn cardinality_relation_as_set(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Card(metadata, rel) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = rel.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Some(representation) = reference.ptr().get_repr::<RelationAsSet>() else {
        return Err(RuleNotApplicable);
    };

    let set_ref = Expr::from(Reference::new(representation.set_decl.clone()));
    Ok(RuleEffect::pure(Expr::Card(
        metadata.clone(),
        Moo::new(set_ref),
    )))
}
