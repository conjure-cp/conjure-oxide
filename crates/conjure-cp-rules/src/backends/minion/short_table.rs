//! Lower sparse short-table rows to their specified equalities.
use conjure_cp::ast::{Expression as Expr, Literal, Metadata, Moo, SymbolTable, eval_constant};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

use crate::shared::utils::{table_ordered_entries, table_rows, tuple_expr_entries};

/// A short row matches when all specified positions match; omitted positions are unrestricted.
#[register_rule("Minion", 4050, [ShortTable])]
fn expand_short_table(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::ShortTable(_, inputs, rows) = expr else {
        return Err(RuleNotApplicable);
    };
    let inputs = table_ordered_entries(inputs).ok_or(RuleNotApplicable)?;
    let rows = table_rows(rows).ok_or(RuleNotApplicable)?;
    let mut alternatives = Vec::with_capacity(rows.len());
    for row in rows {
        let row = super::materialise_matrix_operand(&row).unwrap_or(row);
        let pairs = crate::shared::utils::short_table_row_entries(&row).ok_or(RuleNotApplicable)?;
        let mut positions = std::collections::HashSet::new();
        let mut equalities = Vec::with_capacity(pairs.len());
        for pair in pairs {
            let entries = tuple_expr_entries(&pair).ok_or(RuleNotApplicable)?;
            let [position, value] = entries.as_slice() else {
                return Err(RuleNotApplicable);
            };
            let Some(Literal::Int(position)) = eval_constant(position) else {
                return Err(RuleNotApplicable);
            };
            let position = position
                .checked_sub(1)
                .and_then(|position| usize::try_from(position).ok())
                .ok_or(RuleNotApplicable)?;
            let input = inputs.get(position).ok_or(RuleNotApplicable)?;
            if !positions.insert(position) {
                return Err(RuleNotApplicable);
            }
            equalities.push(Expr::Eq(
                Metadata::new(),
                Moo::new(input.clone()),
                Moo::new(value.clone()),
            ));
        }
        alternatives.push(Expr::And(
            Metadata::new(),
            Moo::new(conjure_cp::into_matrix_expr!(equalities)),
        ));
    }
    Ok(RuleEffect::pure(Expr::Or(
        Metadata::new(),
        Moo::new(conjure_cp::into_matrix_expr!(alternatives)),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::AbstractLiteral;

    #[test]
    fn table_collections_lower_to_numeric_minion_rows() {
        let sequence =
            |values| Expr::AbstractLiteral(Metadata::new(), AbstractLiteral::Sequence(values));
        let rows = Expr::AbstractLiteral(
            Metadata::new(),
            AbstractLiteral::Set(vec![
                sequence(vec![true.into(), (-2).into()]),
                sequence(vec![false.into(), 3.into()]),
            ]),
        );
        for negative in [false, true] {
            let inputs = Moo::new(sequence(vec![1.into(), (-2).into()]));
            let expression = if negative {
                Expr::NegativeTable(Metadata::new(), inputs, Moo::new(rows.clone()))
            } else {
                Expr::Table(Metadata::new(), inputs, Moo::new(rows.clone()))
            };
            let effect = super::super::flatten_table(&expression, &SymbolTable::new()).unwrap();
            assert!(
                matches!(effect.new_expression, Expr::FlatTable(_, inputs, rows, sign) if inputs.len() == 2 && rows == vec![vec![1,-2], vec![0,3]] && sign == negative)
            );
        }
        let empty = Expr::Table(
            Metadata::new(),
            Moo::new(sequence(vec![1.into()])),
            Moo::new(Expr::AbstractLiteral(
                Metadata::new(),
                AbstractLiteral::Set(vec![]),
            )),
        );
        assert!(
            matches!(super::super::flatten_table(&empty, &SymbolTable::new()).unwrap().new_expression, Expr::FlatTable(_, _, rows, false) if rows.is_empty())
        );
        let wrong_width = Expr::Table(
            Metadata::new(),
            Moo::new(sequence(vec![1.into()])),
            Moo::new(rows),
        );
        assert!(super::super::flatten_table(&wrong_width, &SymbolTable::new()).is_err());
    }

    #[test]
    fn short_table_sets_preserve_empty_and_sparse_rows() {
        let set = |values| Expr::AbstractLiteral(Metadata::new(), AbstractLiteral::Set(values));
        let pair = |position, value| {
            Expr::AbstractLiteral(
                Metadata::new(),
                AbstractLiteral::Tuple(vec![Expr::from(position), Expr::from(value)]),
            )
        };
        for (rows, expected) in [
            (vec![], false),
            (vec![set(vec![])], true),
            (vec![set(vec![pair(2, 9)])], true),
            (vec![set(vec![pair(1, 8)])], false),
        ] {
            let expression = Expr::ShortTable(
                Metadata::new(),
                Moo::new(conjure_cp::matrix_expr![7.into(), 9.into()]),
                Moo::new(set(rows)),
            );
            let effect = expand_short_table(&expression, &SymbolTable::new()).unwrap();
            assert_eq!(eval_constant(&effect.new_expression), Some(expected.into()));
        }
        // A two-element matrix is not a position/value tuple.
        let expression = Expr::ShortTable(
            Metadata::new(),
            Moo::new(conjure_cp::matrix_expr![7.into()]),
            Moo::new(set(vec![set(vec![conjure_cp::matrix_expr![
                1.into(),
                7.into()
            ]])])),
        );
        assert!(expand_short_table(&expression, &SymbolTable::new()).is_err());
    }

    #[test]
    fn sparse_rows_preserve_wildcards_and_empty_table_semantics() {
        let pair = |position, value| {
            Expr::AbstractLiteral(
                Metadata::new(),
                AbstractLiteral::Tuple(vec![Expr::from(position), Expr::from(value)]),
            )
        };
        let table = |rows: Vec<Vec<Expr>>| {
            Expr::ShortTable(
                Metadata::new(),
                Moo::new(conjure_cp::matrix_expr![7.into(), 9.into()]),
                Moo::new(conjure_cp::into_matrix_expr!(
                    rows.into_iter()
                        .map(|row| conjure_cp::into_matrix_expr!(row))
                        .collect::<Vec<_>>()
                )),
            )
        };
        for (rows, expected) in [
            (vec![], false),
            (vec![vec![]], true),
            (vec![vec![pair(1, 7)]], true),
            (vec![vec![pair(1, 8)]], false),
            (vec![vec![pair(1, 8)], vec![pair(2, 9)]], true),
        ] {
            let effect = expand_short_table(&table(rows), &SymbolTable::new()).unwrap();
            assert_eq!(eval_constant(&effect.new_expression), Some(expected.into()));
        }
        for rows in [
            vec![vec![pair(0, 7)]],
            vec![vec![pair(3, 7)]],
            vec![vec![pair(1, 7), pair(1, 8)]],
        ] {
            assert!(expand_short_table(&table(rows), &SymbolTable::new()).is_err());
        }
        let sequence =
            Expr::AbstractLiteral(Metadata::new(), AbstractLiteral::Sequence(vec![pair(1, 7)]));
        let expression = Expr::ShortTable(
            Metadata::new(),
            Moo::new(conjure_cp::matrix_expr![7.into(), 9.into()]),
            Moo::new(conjure_cp::into_matrix_expr!(vec![sequence])),
        );
        let effect = expand_short_table(&expression, &SymbolTable::new()).unwrap();
        assert_eq!(eval_constant(&effect.new_expression), Some(true.into()));
    }
}
