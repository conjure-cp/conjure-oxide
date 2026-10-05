//! Scalar lexicographic comparisons reuse numeric relation and Boolean decisions.
use conjure_cp::ast::{Expression, Metadata, Moo, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

#[register_rule("SAT", 18500, [LexLt, LexLeq])]
fn scalar_lex_comparison(expr: &Expression, symbols: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expression::LexLt(_, lhs, rhs) | Expression::LexLeq(_, lhs, rhs) => (lhs, rhs),
        _ => return Err(RuleNotApplicable),
    };
    for operand in [lhs, rhs] {
        let entries = super::super::smt::lex::lex_operand_elements(operand)?;
        if entries.iter().any(|entry| {
            !entry
                .domain_of()
                .is_some_and(|domain| domain.is_int() || domain.is_bool())
        }) {
            // Compound comparisons promote back to lex; their representation rules must run first.
            return Err(RuleNotApplicable);
        }
    }
    super::super::smt::lex::expand_lex_lt_leq(expr, symbols)
}

/// Representation ordering constraints may already be flattened into scalar atoms.
#[register_rule("SAT", 18500, [FlatLexLt, FlatLexLeq])]
fn flat_scalar_lex_comparison(expr: &Expression, symbols: &SymbolTable) -> ApplicationResult {
    let (left, right, strict) = match expr {
        Expression::FlatLexLt(_, left, right) => (left, right, true),
        Expression::FlatLexLeq(_, left, right) => (left, right, false),
        _ => return Err(RuleNotApplicable),
    };
    let matrix = |atoms: &[conjure_cp::ast::Atom]| {
        Moo::new(conjure_cp::into_matrix_expr!(
            atoms
                .iter()
                .cloned()
                .map(Expression::from)
                .collect::<Vec<_>>()
        ))
    };
    let expression = if strict {
        Expression::LexLt(Metadata::new(), matrix(left), matrix(right))
    } else {
        Expression::LexLeq(Metadata::new(), matrix(left), matrix(right))
    };
    scalar_lex_comparison(&expression, symbols)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Metadata, Moo, eval_constant};
    use conjure_cp::into_matrix_expr;

    #[test]
    fn scalar_lex_matches_sequence_order_for_empty_prefixes_and_first_differences() {
        let lists: &[&[i32]] = &[&[], &[0], &[1], &[0, 0], &[0, 1], &[1, 0]];
        for lhs in lists {
            for rhs in lists {
                for strict in [false, true] {
                    let left = Moo::new(into_matrix_expr!(
                        lhs.iter()
                            .copied()
                            .map(Expression::from)
                            .collect::<Vec<_>>()
                    ));
                    let right = Moo::new(into_matrix_expr!(
                        rhs.iter()
                            .copied()
                            .map(Expression::from)
                            .collect::<Vec<_>>()
                    ));
                    let expr = if strict {
                        Expression::LexLt(Metadata::new(), left, right)
                    } else {
                        Expression::LexLeq(Metadata::new(), left, right)
                    };
                    let expanded = scalar_lex_comparison(&expr, &SymbolTable::new())
                        .unwrap()
                        .new_expression;
                    assert_eq!(
                        eval_constant(&expanded),
                        Some((if strict { lhs < rhs } else { lhs <= rhs }).into()),
                        "{lhs:?} {rhs:?} strict={strict}"
                    );
                }
            }
        }
    }

    #[test]
    fn scalar_lex_waits_for_compound_entry_representations() {
        let tuple = Expression::from(conjure_cp::ast::Literal::AbstractLiteral(
            conjure_cp::ast::AbstractLiteral::Tuple(vec![1.into(), 2.into()]),
        ));
        let matrix = Moo::new(into_matrix_expr!(vec![tuple]));
        for expr in [
            Expression::LexLt(Metadata::new(), matrix.clone(), matrix.clone()),
            Expression::LexLeq(Metadata::new(), matrix.clone(), matrix.clone()),
        ] {
            assert!(scalar_lex_comparison(&expr, &SymbolTable::new()).is_err());
        }
    }
}
