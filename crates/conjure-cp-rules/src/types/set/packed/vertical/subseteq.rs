use super::super::SetPacked;
use crate::guard;
use conjure_cp::ast::{Atom, Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Lower subset inclusion from a packed set.
#[register_rule("ReprGeneral", 9500, [SubsetEq])]
fn subseteq_packed(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::SubsetEq(_, lhs, rhs) = expr &&
        let Expr::Atomic(_, Atom::Reference(lhs)) = lhs.as_ref() &&
        let Some(lhs_repr) = lhs.get_repr_as::<SetPacked>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    Ok(RuleEffect::pure(lhs_repr.subset_expr((**rhs).clone())))
}
