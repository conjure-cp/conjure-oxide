use conjure_cp::ast::{Expression as Expr, SymbolTable, matrix::safe_index_optimised};
use conjure_cp::essence_expr;
use conjure_cp::rule_engine::{
    ApplicationError::{DomainError, RuleNotApplicable},
    ApplicationResult, RuleEffect, register_rule,
};

fn lex_operand_elements(
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

#[register_rule("Smt", 2001, [LexLt, LexLeq])]
fn expand_lex_lt_leq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expr::LexLt(_, lhs, rhs) | Expr::LexLeq(_, lhs, rhs) => (lhs, rhs),
        _ => return Err(RuleNotApplicable),
    };

    let lhs_elements = lex_operand_elements(lhs)?;
    let rhs_elements = lex_operand_elements(rhs)?;
    let allow_equality = matches!(expr, Expr::LexLeq(..));

    Ok(RuleEffect::pure(lex_elements_to_recursive_or(
        &lhs_elements,
        &rhs_elements,
        allow_equality,
    )))
}

fn lex_elements_to_recursive_or(
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

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Metadata, Moo};
    use conjure_cp::matrix_expr;

    #[test]
    fn expands_explicit_lists_even_when_element_domains_are_unavailable() {
        let lhs = matrix_expr![
            Expr::Metavar(Metadata::new(), "a".into()),
            Expr::Metavar(Metadata::new(), "b".into())
        ];
        let rhs = matrix_expr![
            Expr::Metavar(Metadata::new(), "c".into()),
            Expr::Metavar(Metadata::new(), "d".into())
        ];
        assert!(lhs.domain_of().is_none());
        assert!(rhs.domain_of().is_none());

        let comparison = Expr::LexLeq(Metadata::new(), Moo::new(lhs), Moo::new(rhs));
        let result = expand_lex_lt_leq(&comparison, &SymbolTable::new())
            .expect("explicit list order is enough to expand lex");

        assert!(!matches!(result.new_expression, Expr::LexLeq(..)));
    }
}
