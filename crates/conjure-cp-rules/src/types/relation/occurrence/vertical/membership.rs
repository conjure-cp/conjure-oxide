use super::super::RelationOccurrence;
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, Reference, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// `member in rel` on a dense relation is a direct matrix lookup, `matrix[member[1],...,member[N]]`.
/// `SafeIndex` is valid on any tuple-shaped expression regardless of whether `member` is a literal
/// tuple or an arbitrary tuple-typed expression (e.g. a reference into another representation's own
/// declaration, as `FunctionAsRelation`'s witness-based surjectivity constraint builds) -- later
/// indexing/representation rules resolve each field access independently either way.
#[register_rule("Base", 8650, [In])]
fn membership_relation_occurrence(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::In(_, member, collection) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = collection.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Some(representation) = reference.ptr().get_repr::<RelationOccurrence>() else {
        return Err(RuleNotApplicable);
    };

    let arity = representation.inner_domains.len() as i32;
    let fields: Vec<Expr> = (1..=arity)
        .map(|i| {
            Expr::SafeIndex(
                Metadata::new(),
                Moo::new((**member).clone()),
                vec![i.into()],
            )
        })
        .collect();

    let matrix_ref = Expr::from(Reference::new(representation.matrix_decl.clone()));
    Ok(RuleEffect::pure(Expr::SafeIndex(
        Metadata::new(),
        Moo::new(matrix_ref),
        fields,
    )))
}
