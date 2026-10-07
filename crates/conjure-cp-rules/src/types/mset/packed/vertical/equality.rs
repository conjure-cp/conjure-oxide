use super::super::MSetPacked;
use crate::guard;
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Literal, Metadata, Moo, SymbolTable,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
#[register_rule("ReprGeneral", 9500, [Eq])]
fn equality_packed_mset_literal(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    };
    let (reference, elems) = match (lhs.as_ref(), rhs.as_ref()) {
        (
            Expr::Atomic(_, Atom::Reference(reference)),
            Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::MSet(elems)))),
        )
        | (
            Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::MSet(elems)))),
            Expr::Atomic(_, Atom::Reference(reference)),
        ) => (reference, elems),
        _ => return Err(RuleNotApplicable),
    };
    let representation = reference
        .get_repr_as::<MSetPacked>()
        .ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::pure(
        representation
            .equality_to_literal_expr(elems)
            .ok_or(RuleNotApplicable)?,
    ))
}
#[register_rule("ReprGeneral", 9500, [Eq])]
fn equality_packed_msets(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(let Expr::Eq(_, lhs, rhs) = expr && let Expr::Atomic(_, Atom::Reference(lhs)) = lhs.as_ref() && let Expr::Atomic(_, Atom::Reference(rhs)) = rhs.as_ref() && let Some(lhs) = lhs.get_repr_as::<MSetPacked>() && let Some(rhs) = rhs.get_repr_as::<MSetPacked>() else { return Err(RuleNotApplicable) });
    if lhs.elements.as_ref() != rhs.elements.as_ref() || lhs.occurrence != rhs.occurrence {
        return Err(RuleNotApplicable);
    }
    Ok(RuleEffect::pure(Expr::Eq(
        Metadata::new(),
        Moo::new(lhs.packed_expr()),
        Moo::new(rhs.packed_expr()),
    )))
}
