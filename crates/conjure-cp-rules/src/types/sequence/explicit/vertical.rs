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
