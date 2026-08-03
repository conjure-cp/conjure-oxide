use super::super::MSetExplicit;
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, Reference, SymbolTable};
use conjure_cp::into_matrix_expr;
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
fn order_explicit_msets(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expr::LexLt(_, lhs, rhs) | Expr::LexLeq(_, lhs, rhs) => (lhs, rhs),
        _ => return Err(RuleNotApplicable),
    };
    let lhs = singleton_reference(lhs)
        .and_then(|reference| {
            reference
                .get_repr_as::<MSetExplicit>()
                .map(|value| value.clone())
        })
        .ok_or(RuleNotApplicable)?;
    let rhs = singleton_reference(rhs)
        .and_then(|reference| {
            reference
                .get_repr_as::<MSetExplicit>()
                .map(|value| value.clone())
        })
        .ok_or(RuleNotApplicable)?;
    if lhs.cardinality != rhs.cardinality || lhs.elements.as_ref() != rhs.elements.as_ref() {
        return Err(RuleNotApplicable);
    }
    let max = lhs.cardinality.1;
    let lhs_order = into_matrix_expr!(
        std::iter::once(lhs.cardinality_expr())
            .chain((1..=max).map(|index| lhs.slot_expr(index)))
            .collect::<Vec<_>>()
    );
    let rhs_order = into_matrix_expr!(
        std::iter::once(rhs.cardinality_expr())
            .chain((1..=max).map(|index| rhs.slot_expr(index)))
            .collect::<Vec<_>>()
    );
    Ok(RuleEffect::pure(match expr {
        Expr::LexLt(..) => Expr::LexLt(Metadata::new(), Moo::new(lhs_order), Moo::new(rhs_order)),
        Expr::LexLeq(..) => Expr::LexLeq(Metadata::new(), Moo::new(lhs_order), Moo::new(rhs_order)),
        _ => unreachable!(),
    }))
}
