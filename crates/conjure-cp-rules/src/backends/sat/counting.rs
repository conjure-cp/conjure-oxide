//! Connect occurrence constraints to the shared cardinality and pseudo-Boolean decisions.
use conjure_cp::ast::{Expression as Expr, Metadata, Moo, SymbolTable};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

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
    let entries = |expression: &Expr| {
        super::table::materialise(expression)
            .unwrap_matrix_unchecked()
            .map(|(values, _)| values)
            .ok_or(RuleNotApplicable)
    };
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
