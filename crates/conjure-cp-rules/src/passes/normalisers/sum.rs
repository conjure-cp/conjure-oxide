//! Normalising rules for `Sum`

use ApplicationError::RuleNotApplicable;
use conjure_cp::{
    ast::{Expression as Expr, Metadata, Moo, SymbolTable},
    essence_expr, into_matrix_expr,
    rule_engine::{ApplicationError, ApplicationResult, RuleEffect, register_rule},
};

/// Removes sums with a single argument.
///
/// ```text
/// sum([a]) ~> a
/// ```
#[register_rule("Base", 8800, [Sum])]
fn remove_unit_vector_sum(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Sum(_, e) = expr else {
        return Err(RuleNotApplicable);
    };

    // The failure path is only a shape check and must stay O(1) on a wide sum.
    if e.list_len() != Some(1) {
        return Err(RuleNotApplicable);
    }
    let mut exprs = e.unwrap_list().ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::pure(
        exprs.pop().expect("singleton length checked above"),
    ))
}

/// Moves the terms of a sum comparison onto one side before backend lowering.
/// This avoids auxiliaries for whole sums, including sums of represented boolean terms.
#[register_rule("Base", 8800, [Eq / Sum, Neq / Sum, Lt / Sum, Leq / Sum, Gt / Sum, Geq / Sum])]
fn sum_comparison_to_zero(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    type Comparison = fn(Metadata, Moo<Expr>, Moo<Expr>) -> Expr;
    let (lhs, rhs, comparison): (_, _, Comparison) = match expr {
        Expr::Eq(_, lhs, rhs) => (lhs, rhs, Expr::Eq),
        Expr::Neq(_, lhs, rhs) => (lhs, rhs, Expr::Neq),
        Expr::Lt(_, lhs, rhs) => (lhs, rhs, Expr::Lt),
        Expr::Leq(_, lhs, rhs) => (lhs, rhs, Expr::Leq),
        Expr::Gt(_, lhs, rhs) => (lhs, rhs, Expr::Gt),
        Expr::Geq(_, lhs, rhs) => (lhs, rhs, Expr::Geq),
        _ => return Err(RuleNotApplicable),
    };
    let (Expr::Sum(_, lhs), Expr::Sum(_, rhs)) = (lhs.as_ref(), rhs.as_ref()) else {
        return Err(RuleNotApplicable);
    };
    let lhs_terms = lhs.unwrap_list_cow().ok_or(RuleNotApplicable)?;
    let rhs_terms = rhs.unwrap_list_cow().ok_or(RuleNotApplicable)?;
    // A comprehension contributes several summands; expand it before negating each term.
    if lhs_terms
        .iter()
        .chain(rhs_terms.iter())
        .any(|term| matches!(term, Expr::Comprehension(_, _)))
    {
        return Err(RuleNotApplicable);
    }
    let mut terms = lhs_terms.into_owned();
    terms.extend(
        rhs_terms
            .iter()
            .map(|term| Expr::Neg(Metadata::new(), Moo::new(term.clone()))),
    );
    Ok(RuleEffect::pure(comparison(
        Metadata::new(),
        Moo::new(Expr::Sum(
            Metadata::new(),
            Moo::new(into_matrix_expr!(terms)),
        )),
        Moo::new(essence_expr!(0)),
    )))
}
