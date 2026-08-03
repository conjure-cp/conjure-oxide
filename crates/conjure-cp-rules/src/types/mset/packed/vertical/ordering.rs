use super::super::MSetPacked;
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
#[register_rule("ReprGeneral", 9500, [LexLt, LexLeq])]
fn order_packed_msets(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expr::LexLt(_, lhs, rhs) | Expr::LexLeq(_, lhs, rhs) => (lhs, rhs),
        _ => return Err(RuleNotApplicable),
    };
    let lhs = singleton_reference(lhs)
        .and_then(|reference| {
            reference
                .get_repr_as::<MSetPacked>()
                .map(|value| value.clone())
        })
        .ok_or(RuleNotApplicable)?;
    let rhs = singleton_reference(rhs)
        .and_then(|reference| {
            reference
                .get_repr_as::<MSetPacked>()
                .map(|value| value.clone())
        })
        .ok_or(RuleNotApplicable)?;
    if lhs.elements.as_ref() != rhs.elements.as_ref() || lhs.occurrence != rhs.occurrence {
        return Err(RuleNotApplicable);
    }
    Ok(RuleEffect::pure(match expr {
        Expr::LexLt(..) => Expr::Lt(
            Metadata::new(),
            Moo::new(lhs.packed_expr()),
            Moo::new(rhs.packed_expr()),
        ),
        Expr::LexLeq(..) => Expr::Leq(
            Metadata::new(),
            Moo::new(lhs.packed_expr()),
            Moo::new(rhs.packed_expr()),
        ),
        _ => unreachable!(),
    }))
}
