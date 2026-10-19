use super::super::RelationPacked;
use crate::guard;
use conjure_cp::ast::{AbstractLiteral, Atom, Expression as Expr, Literal, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Lower equality between a packed relation and a relation literal to equality on the packed rank.
#[register_rule("ReprGeneral", 9500, [Eq])]
fn eq_relation_packed_literal(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Eq(_, lhs, rhs) = expr else {
            return Err(RuleNotApplicable);
        }
    );

    let (rel, tuples) = match (lhs.as_ref(), rhs.as_ref()) {
        (
            Expr::Atomic(_, Atom::Reference(reference)),
            Expr::Atomic(
                _,
                Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Relation(tuples))),
            ),
        ) => (reference, tuples),
        (
            Expr::Atomic(
                _,
                Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Relation(tuples))),
            ),
            Expr::Atomic(_, Atom::Reference(reference)),
        ) => (reference, tuples),
        _ => return Err(RuleNotApplicable),
    };
    let Some(repr) = rel.get_repr_as::<RelationPacked>() else {
        return Err(RuleNotApplicable);
    };
    let Some(constraint) = repr.equality_to_literal_expr(tuples) else {
        return Err(RuleNotApplicable);
    };

    Ok(RuleEffect::pure(constraint))
}
