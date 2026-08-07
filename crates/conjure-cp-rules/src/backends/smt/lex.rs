use conjure_cp::ast::{Expression as Expr, Literal, SymbolTable, matrix::safe_index_optimised};
use conjure_cp::essence_expr;
use conjure_cp::rule_engine::{
    ApplicationError::{DomainError, RuleNotApplicable},
    ApplicationResult, RuleEffect, register_rule,
};
use itertools::Itertools as _;

#[register_rule("Smt", 2001, [LexLt, LexLeq])]
fn expand_lex_lt_leq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expr::LexLt(_, lhs, rhs) | Expr::LexLeq(_, lhs, rhs) => (lhs, rhs),
        _ => return Err(RuleNotApplicable),
    };

    let lhs_domain = lhs.domain_of().ok_or(RuleNotApplicable)?;
    let rhs_domain = rhs.domain_of().ok_or(RuleNotApplicable)?;
    let (Some((_, lhs_indices)), Some((_, rhs_indices))) =
        (lhs_domain.as_matrix_ground(), rhs_domain.as_matrix_ground())
    else {
        return Err(RuleNotApplicable);
    };
    if lhs_indices.len() != 1 || rhs_indices.len() != 1 {
        return Err(RuleNotApplicable);
    }

    let lhs_indices = lhs_indices[0]
        .values()
        .map_err(|_| DomainError)?
        .collect_vec();
    let rhs_indices = rhs_indices[0]
        .values()
        .map_err(|_| DomainError)?
        .collect_vec();
    let allow_equality = matches!(expr, Expr::LexLeq(..));

    Ok(RuleEffect::pure(lex_lt_to_recursive_or(
        lhs,
        rhs,
        &lhs_indices,
        &rhs_indices,
        allow_equality,
    )))
}

fn lex_lt_to_recursive_or(
    lhs: &Expr,
    rhs: &Expr,
    lhs_indices: &[Literal],
    rhs_indices: &[Literal],
    allow_equality: bool,
) -> Expr {
    match (lhs_indices, rhs_indices) {
        ([], []) => allow_equality.into(),
        ([..], []) => false.into(),
        ([], [..]) => true.into(),
        ([lhs_index, lhs_tail @ ..], [rhs_index, rhs_tail @ ..]) => {
            let lhs_element = safe_index_optimised(lhs.clone(), lhs_index.clone()).unwrap();
            let rhs_element = safe_index_optimised(rhs.clone(), rhs_index.clone()).unwrap();
            let tail = lex_lt_to_recursive_or(lhs, rhs, lhs_tail, rhs_tail, allow_equality);
            essence_expr!(r"&lhs_element < &rhs_element \/ (&lhs_element = &rhs_element /\ &tail)")
        }
    }
}
