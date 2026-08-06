use super::super::RelationPacked;
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// `member in rel` on a packed relation tests the corresponding tuple's occurrence bit. `SafeIndex`
/// is valid on any tuple-shaped expression regardless of whether `member` is a literal tuple or an
/// arbitrary tuple-typed expression (e.g. a reference into another representation's own
/// declaration); `tuple_membership_expr` itself fast-paths to a direct bit test once every field
/// resolves to a constant.
#[register_rule("Base", 8650, [In])]
fn membership_relation_packed(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::In(_, member, collection) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = collection.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Some(representation) = reference.ptr().get_repr::<RelationPacked>() else {
        return Err(RuleNotApplicable);
    };

    let fields: Vec<Expr> = (1..=representation.arity as i32)
        .map(|i| {
            Expr::SafeIndex(
                Metadata::new(),
                Moo::new((**member).clone()),
                vec![i.into()],
            )
        })
        .collect();

    Ok(RuleEffect::pure(
        representation.tuple_membership_expr(&fields),
    ))
}
