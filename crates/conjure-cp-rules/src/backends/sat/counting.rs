//! Connect occurrence constraints to the shared cardinality and pseudo-Boolean decisions.
use conjure_cp::ast::{Expression as Expr, Metadata, Moo, ReturnType, SymbolTable, Typeable};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Matrix entries in index order, ignoring the index domain and expanding `flatten(...)`.
pub(super) fn matrix_entries(expression: &Expr) -> Option<Vec<Expr>> {
    let expression = crate::shared::utils::table_operand(expression);
    if let Expr::Flatten(_, None, inner) = expression {
        fn leaves(expression: &Expr, output: &mut Vec<Expr>) -> Option<()> {
            let expression = crate::shared::utils::table_operand(expression);
            if let Some((entries, _)) = expression.clone().unwrap_matrix_unchecked() {
                for entry in entries {
                    leaves(&entry, output)?;
                }
            } else if matches!(expression.return_type(), ReturnType::Matrix(_)) {
                return None;
            } else {
                output.push(expression);
            }
            Some(())
        }
        let (entries, _) = crate::shared::utils::table_operand(&inner).unwrap_matrix_unchecked()?;
        let mut output = vec![];
        for entry in entries {
            leaves(&entry, &mut output)?;
        }
        Some(output)
    } else {
        expression
            .unwrap_matrix_unchecked()
            .map(|(entries, _)| entries)
    }
}

/// Count equality indicators, keeping the entire constraint Boolean for reified uses.
#[register_rule("SAT", 18500, [AtMost, AtLeast, Gcc, GccWeak])]
fn occurrence_counts(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (vars, counts, values) = match expr {
        Expr::AtMost(_, vars, counts, values) | Expr::AtLeast(_, vars, counts, values) => {
            (vars, counts, values)
        }
        Expr::Gcc(_, vars, values, counts) | Expr::GccWeak(_, vars, values, counts) => {
            (vars, counts, values)
        }
        _ => return Err(RuleNotApplicable),
    };
    let entries = |expression: &Expr| matrix_entries(expression).ok_or(RuleNotApplicable);
    let vars = entries(vars)?;
    let counts = entries(counts)?;
    let values = entries(values)?;
    if counts.len() != values.len() {
        return Err(RuleNotApplicable);
    }
    let comparisons = values
        .into_iter()
        .zip(counts)
        .map(|(value, count)| {
            let indicators = vars
                .iter()
                .map(|var| {
                    Expr::ToInt(
                        Metadata::new(),
                        Moo::new(Expr::Eq(
                            Metadata::new(),
                            Moo::new(var.clone()),
                            Moo::new(value.clone()),
                        )),
                    )
                })
                .collect::<Vec<_>>();
            let total = Moo::new(Expr::Sum(
                Metadata::new(),
                Moo::new(into_matrix_expr!(indicators)),
            ));
            let count = Moo::new(count);
            match expr {
                Expr::AtMost(..) => Expr::Leq(Metadata::new(), total, count),
                Expr::AtLeast(..) => Expr::Geq(Metadata::new(), total, count),
                _ => Expr::Eq(Metadata::new(), total, count),
            }
        })
        .collect::<Vec<_>>();
    Ok(RuleEffect::pure(Expr::And(
        Metadata::new(),
        Moo::new(into_matrix_expr!(comparisons)),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::eval_constant;

    #[test]
    fn flattened_occurrences_count_matrix_leaves() {
        let rows = into_matrix_expr!(vec![
            into_matrix_expr!(vec![1.into(), 2.into()]),
            into_matrix_expr!(vec![1.into()]),
        ]);
        let expr = Expr::AtLeast(
            Metadata::new(),
            Moo::new(Expr::Flatten(Metadata::new(), None, Moo::new(rows))),
            Moo::new(into_matrix_expr!(vec![2.into()])),
            Moo::new(into_matrix_expr!(vec![1.into()])),
        );
        let effect = occurrence_counts(&expr, &SymbolTable::default()).unwrap();
        assert_eq!(eval_constant(&effect.new_expression), Some(true.into()));
    }

    #[test]
    fn occurrences_count_duplicates_and_values_outside_the_input_domain() {
        let vars = Moo::new(into_matrix_expr!(vec![1.into(), 1.into(), 2.into()]));
        let values = Moo::new(into_matrix_expr!(vec![1.into(), 3.into()]));
        for (counts, expected) in [
            (vec![2.into(), 0.into()], true),
            (vec![1.into(), 0.into()], false),
        ] {
            let expr = Expr::Gcc(
                Metadata::new(),
                vars.clone(),
                values.clone(),
                Moo::new(into_matrix_expr!(counts)),
            );
            let lowered = occurrence_counts(&expr, &SymbolTable::default())
                .unwrap()
                .new_expression;
            assert_eq!(eval_constant(&lowered), Some(expected.into()));
        }
    }
}
