use super::super::SetOccurrence;
use crate::guard;
use conjure_cp::ast::{AbstractLiteral, Atom, Expression as Expr, Literal, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Lower equality between an occurrence set and a set literal.
///
/// Inactive outer-set slots are padded with `= {}`; Conjure achieves the same via
/// `dontCare` of the occurrence bits. Matching the literal bit-for-bit avoids falling
/// through to horizontal subset comprehensions over a decision-valued set.
#[register_rule("ReprGeneral", 9500, [Eq])]
fn eq_occurrence_literal(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
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
    let Some(repr) = set.get_repr_as::<SetOccurrence>() else {
        return Err(RuleNotApplicable);
    };

    Ok(RuleEffect::pure(repr.equality_to_literal_expr(literal)))
}
