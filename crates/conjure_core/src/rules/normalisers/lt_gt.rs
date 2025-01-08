//! Normalising rules for Lt and Gt.
//!
//! These normalise into Leq and Geq respectively.

use conjure_macros::register_rule;

use crate::{
    ast::{Atom, Expression as Expr, Literal as Lit},
    metadata::Metadata,
    rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, Reduction},
    Model,
};

/// Converts Lt to Leq
///
/// # Rationale
///
/// Minion supports Leq directly in some constraints, such as SumLeq, WeightedSumLeq, ...
/// This transformation makes Lt work with these constraints too without needing special
/// cases in the Minion conversion rules.
#[register_rule(("Base", 8400))]
fn lt_to_leq(expr: &Expr, _: &Model) -> ApplicationResult {
    let Expr::Lt(_, lhs, rhs) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    // add to rhs so that this is in the correct form for ineq ( x <= y + k)
    Ok(Reduction::pure(Expr::Leq(
        Metadata::new(),
        lhs,
        Box::new(Expr::Sum(
            Metadata::new(),
            vec![
                *rhs,
                Expr::Atomic(Metadata::new(), Atom::Literal(Lit::Int(-1))),
            ],
        )),
    )))
}

/// Converts Gt to Geq
///
/// # Rationale
///
/// Minion supports Geq directly in some constraints, such as SumGeq, WeightedSumGeq, ...
/// This transformation makes Gt work with these constraints too without needing special
/// cases in the Minion conversion rules.
#[register_rule(("Base", 8400))]
fn gt_to_geq(expr: &Expr, _: &Model) -> ApplicationResult {
    let Expr::Gt(_, lhs, total) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::Geq(
        Metadata::new(),
        Box::new(Expr::Sum(
            Metadata::new(),
            vec![
                *lhs,
                Expr::Atomic(Metadata::new(), Atom::Literal(Lit::Int(-1))),
            ],
        )),
        total,
    )))
}
