use super::super::RelationAsSet;
use crate::guard;
use conjure_cp::ast::{AbstractLiteral, Atom, Expression as Expr, Literal, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Lower equality between a sparse relation and a relation literal.
#[register_rule("Base", 8650, [Eq])]
fn eq_relation_as_set_literal(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
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
    let Some(repr) = rel.ptr().get_repr::<RelationAsSet>() else {
        return Err(RuleNotApplicable);
    };

    Ok(RuleEffect::pure(repr.equality_to_literal_expr(tuples)))
}
