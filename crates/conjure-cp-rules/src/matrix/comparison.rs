use crate::guard;
use crate::utils::{
    as_eq_or_neq, as_lex_comparison_op, collect_cmp_exprs, collect_eq_or_neq, try_flatten_matrix,
};
use conjure_cp::ast::{Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::rule_engine::{ApplicationResult, Reduction, register_rule};
use itertools::{Itertools, izip};

/// Equality of matrix literals:
/// ```plain
/// [a, b, c] = [1, 2, 3]
/// ~>
/// and([a = 1, b = 2, c = 3])
/// ```
#[register_rule(("Base", 2000))]
fn matrix_eq_matrix(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Some(lhs_elems) = try_flatten_matrix(lhs).map(Itertools::collect_vec) &&
        let Some(rhs_elems) = try_flatten_matrix(rhs).map(Itertools::collect_vec) &&
        lhs_elems.len() == rhs_elems.len()
        else {
            return Err(RuleNotApplicable);
        }
    );

    let new_expr = collect_eq_or_neq(neq, izip!(lhs_elems, rhs_elems));
    Ok(Reduction::pure(new_expr))
}

/// Lex comparison of matrix literals:
/// ```plain
/// [a, b, c] lex> [1, 2, 3]
/// ~>
/// or([
///     a lex> 1,
///     a = 1 /\ b lex> 2,
///     a = 1 /\ b = 2 /\ c lex> 3
/// ])
/// ```
#[register_rule(("Base", 2000))]
fn matrix_cmp_matrix(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Some((lhs, rhs)) = as_lex_comparison_op(expr)                                  &&
        let Some(lhs_elems) = try_flatten_matrix(lhs.as_ref()).map(Itertools::collect_vec) &&
        let Some(rhs_elems) = try_flatten_matrix(rhs.as_ref()).map(Itertools::collect_vec) &&
        lhs_elems.len() == rhs_elems.len()
        else {
            return Err(RuleNotApplicable);
        }
    );

    let new_expr = collect_cmp_exprs(expr, lhs_elems, rhs_elems);
    Ok(Reduction::pure(new_expr))
}
