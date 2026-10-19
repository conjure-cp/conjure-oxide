use super::SequenceExplicit;
use crate::guard;
use crate::types::sequence::apply_at_position;
use conjure_cp::ast::{Atom, Expression as Expr, Reference, SymbolTable};
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::rule_engine::{ApplicationResult, RuleEffect as Reduction, register_rule};

/// Cardinality of an explicit sequence variable
/// ```plain
/// |s|
/// ~>
/// sLength
/// ```
#[register_rule("ReprGeneral", 9500, [Card])]
fn sequence_explicit_card(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Card(_, subject) = expr &&
        let Expr::Atomic(_, Atom::Reference(re)) = subject.as_ref() &&
        let Some(repr) = re.get_repr_as::<SequenceExplicit>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    Ok(Reduction::pure(repr.length_expr()))
}

/// Application of an explicit sequence variable
/// ```plain
/// s(i)
/// ~>
/// sValues[i]
/// ```
///
/// See [`apply_at_position`] for how the undefinedness of applying a sequence out of range is
/// handled.
#[register_rule("ReprGeneral", 9500, [Image])]
fn sequence_explicit_image(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Image(_, subject, index) = expr &&
        let Expr::Atomic(_, Atom::Reference(re)) = subject.as_ref() &&
        let Some(repr) = re.get_repr_as::<SequenceExplicit>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    Ok(Reduction::pure(apply_at_position(
        Expr::from(Reference::new(repr.values_matrix.clone())),
        index.as_ref().clone(),
        repr.size_bounds,
        || repr.length_expr(),
    )))
}
