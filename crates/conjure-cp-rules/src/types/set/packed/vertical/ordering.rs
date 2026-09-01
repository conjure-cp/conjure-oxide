use super::super::SetPacked;
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, Reference, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

fn singleton_reference(expr: &Expr) -> Option<Reference> {
    let values = expr.unwrap_list()?;
    let [Expr::Atomic(_, Atom::Reference(reference))] = values.as_slice() else {
        return None;
    };
    Some(reference.clone())
}

/// Compare packed-set values through their integer ranks.
///
/// Outer explicit-set structural constraints compare adjacent inner values as singleton
/// matrices. Once those values are packed, `<lex` / `<=lex` over the singleton lowers to
/// ordinary integer comparison of the packed ranks.
///
/// Rank order is only comparable when both sides share the same inner-domain ordering
/// and cardinality bounds (as in a matrix of identically typed packed sets).
#[register_rule("ReprGeneral", 9500, [LexLt, LexLeq])]
fn lex_packed_sets(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expr::LexLt(_, lhs, rhs) | Expr::LexLeq(_, lhs, rhs) => (lhs, rhs),
        _ => return Err(RuleNotApplicable),
    };

    let lhs_reference = singleton_reference(lhs).ok_or(RuleNotApplicable)?;
    let rhs_reference = singleton_reference(rhs).ok_or(RuleNotApplicable)?;
    let lhs_repr = lhs_reference
        .get_repr_as::<SetPacked>()
        .ok_or(RuleNotApplicable)?;
    let rhs_repr = rhs_reference
        .get_repr_as::<SetPacked>()
        .ok_or(RuleNotApplicable)?;

    if lhs_repr.elements.as_ref() != rhs_repr.elements.as_ref()
        || lhs_repr.cardinality != rhs_repr.cardinality
    {
        return Err(RuleNotApplicable);
    }

    let lhs_packed = lhs_repr.packed_expr();
    let rhs_packed = rhs_repr.packed_expr();
    let rewritten = match expr {
        Expr::LexLt(..) => Expr::Lt(Metadata::new(), Moo::new(lhs_packed), Moo::new(rhs_packed)),
        Expr::LexLeq(..) => Expr::Leq(Metadata::new(), Moo::new(lhs_packed), Moo::new(rhs_packed)),
        _ => unreachable!(),
    };
    Ok(RuleEffect::pure(rewritten))
}
