//! Scalar lexicographic comparisons reuse numeric relation and Boolean decisions.
use conjure_cp::ast::{Expression, SymbolTable};
use conjure_cp::rule_engine::{ApplicationResult, register_rule};

#[register_rule("SAT", 18500, [LexLt, LexLeq])]
fn scalar_lex_comparison(expr: &Expression, symbols: &SymbolTable) -> ApplicationResult {
    super::super::smt::lex::expand_lex_lt_leq(expr, symbols)
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
}
