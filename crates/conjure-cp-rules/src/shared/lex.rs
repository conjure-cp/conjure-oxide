//! Ordered scalar lex operands and their recursive Boolean comparison.
use conjure_cp::ast::{Expression as Expr, matrix::safe_index_optimised};
use conjure_cp::essence_expr;
use conjure_cp::rule_engine::ApplicationError::{DomainError, RuleNotApplicable};

/// Read a one-dimensional lex operand in its declared order.
pub(crate) fn lex_operand_elements(
    expr: &Expr,
) -> Result<Vec<Expr>, conjure_cp::rule_engine::ApplicationError> {
    // Representation rules commonly turn a slice into an explicit matrix literal. Consume that
    // literal directly: after an element is replaced by its BV representation its inferred
    // Essence domain may temporarily be unavailable, but its list order is still explicit and is
    // all lexicographic expansion needs.
    if let Some(elements) = expr.unwrap_list() {
        return Ok(elements);
    }

    let domain = expr.domain_of().ok_or(RuleNotApplicable)?;
    let Some((_, indices)) = domain.as_matrix_ground() else {
        return Err(RuleNotApplicable);
    };
    if indices.len() != 1 {
        return Err(RuleNotApplicable);
    }

    indices[0]
        .values()
        .map_err(|_| DomainError)?
        .map(|index| safe_index_optimised(expr.clone(), index).ok_or(DomainError))
        .collect()
}

/// Compare ordered scalar entries, including unequal lengths.
pub(crate) fn lex_elements_to_recursive_or(
    lhs_elements: &[Expr],
    rhs_elements: &[Expr],
    allow_equality: bool,
) -> Expr {
    match (lhs_elements, rhs_elements) {
        ([], []) => allow_equality.into(),
        ([..], []) => false.into(),
        ([], [..]) => true.into(),
        ([lhs_element, lhs_tail @ ..], [rhs_element, rhs_tail @ ..]) => {
            let tail = lex_elements_to_recursive_or(lhs_tail, rhs_tail, allow_equality);
            essence_expr!(r"&lhs_element < &rhs_element \/ (&lhs_element = &rhs_element /\ &tail)")
        }
    }
}
