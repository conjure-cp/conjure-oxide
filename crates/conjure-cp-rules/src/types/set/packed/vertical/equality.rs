use super::super::SetPacked;
use crate::guard;
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Literal, Metadata, Moo, SymbolTable,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Lower equality between a packed set and a set literal to packed-rank equality.
#[register_rule("ReprGeneral", 9500, [Eq])]
fn eq_packed_literal(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Eq(_, lhs, rhs) = expr else {
            return Err(RuleNotApplicable);
        }
    );

    let (set, literal) = match (lhs.as_ref(), rhs.as_ref()) {
        (
            Expr::Atomic(_, Atom::Reference(reference)),
            Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Set(elems)))),
        ) => (reference, elems),
        (
            Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Set(elems)))),
            Expr::Atomic(_, Atom::Reference(reference)),
        ) => (reference, elems),
        _ => return Err(RuleNotApplicable),
    };
    let Some(repr) = set.get_repr_as::<SetPacked>() else {
        return Err(RuleNotApplicable);
    };

    Ok(RuleEffect::pure(
        repr.equality_to_literal_expr(literal)
            .ok_or(RuleNotApplicable)?,
    ))
}

/// Lower equality between two packed sets to equality of their ranks.
///
/// Rank equality is only sound when both sides share the same inner-domain
/// ordering and cardinality bounds; otherwise fall through to horizontal
/// `a ⊆ b ∧ b ⊆ a` (via membership), which handles overlapping domains.
#[register_rule("ReprGeneral", 9500, [Eq])]
fn eq_packed(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Eq(_, lhs, rhs) = expr &&
        let Expr::Atomic(_, Atom::Reference(lhs)) = lhs.as_ref() &&
        let Expr::Atomic(_, Atom::Reference(rhs)) = rhs.as_ref() &&
        let Some(lhs_repr) = lhs.get_repr_as::<SetPacked>() &&
        let Some(rhs_repr) = rhs.get_repr_as::<SetPacked>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    if lhs_repr.elements.as_ref() != rhs_repr.elements.as_ref()
        || lhs_repr.cardinality != rhs_repr.cardinality
    {
        return Err(RuleNotApplicable);
    }

    Ok(RuleEffect::pure(Expr::Eq(
        Metadata::new(),
        Moo::new(lhs_repr.packed_expr()),
        Moo::new(rhs_repr.packed_expr()),
    )))
}
