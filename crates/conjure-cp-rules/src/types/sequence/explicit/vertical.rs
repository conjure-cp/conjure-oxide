use super::SequenceExplicit;
use crate::guard;
use conjure_cp::ast::{Atom, Expression as Expr, SymbolTable};
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
/// Positions past the active length hold `padding`, so this reads a padding value rather than
/// going undefined when `i` is beyond `|s|`. That matches how the rest of this representation
/// already treats inactive positions; giving out-of-range application its own undefinedness
/// semantics needs a design of its own, and is deferred until a case in scope needs it.
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

    Ok(Reduction::pure(repr.slot_expr_at(index.as_ref().clone())))
}
