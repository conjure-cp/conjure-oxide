//! Normalising rules for Lt and Gt.
//!
//! For Minion, these normalise into Leq and Geq respectively.

use conjure_cp_core::{
    ast::{Expression as Expr, SymbolTable},
    rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, Reduction},
};
use conjure_cp_rule_macros::register_rule;
use conjure_essence_macros::essence_expr;

/// Converts Lt to Leq
///
/// Minion only for now, but this could be useful for other solvers too.
///
/// # Rationale
///
/// Minion supports Leq directly in some constraints, such as SumLeq, WeightedSumLeq, ...
/// This transformation makes Lt work with these constraints too without needing special
/// cases in the Minion conversion rules.
#[register_rule(("Minion", 8400))]
fn lt_to_leq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Lt(_, lhs, rhs) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    // add to rhs so that this is in the correct form for ineq ( x <= y + k)
    // todo (gs248) - tests expect a Sum([rhs, -1]) so we generate that; maybe just use a subtraction instead?
    Ok(Reduction::pure(essence_expr!(&lhs <= (&rhs + (-1)))))
}

/// Converts Gt to Geq
///
/// Minion only for now, but this could be useful for other solvers too.
///
/// # Rationale
///
/// Minion supports Geq directly in some constraints, such as SumGeq, WeightedSumGeq, ...
/// This transformation makes Gt work with these constraints too without needing special
/// cases in the Minion conversion rules.
#[register_rule(("Minion", 8400))]
fn gt_to_geq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Gt(_, lhs, rhs) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(essence_expr!((&lhs + (-1)) >= &rhs)))
}
