//! Lower sparse modelling rows to Minion's native short-table relation.
use conjure_cp::ast::{Expression as Expr, Literal, Metadata, Moo, SymbolTable, eval_constant};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

use crate::shared::utils::{table_ordered_entries, table_rows, tuple_expr_entries};

/// Use native short tuples, introducing equality indicators for variable-valued cells.
#[register_rule("Minion", 4050, [ShortTable])]
fn flatten_short_table(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::ShortTable(_, inputs, rows) = expr else {
        return Err(RuleNotApplicable);
    };
    let inputs = table_ordered_entries(inputs).ok_or(RuleNotApplicable)?;
    let rows = table_rows(rows).ok_or(RuleNotApplicable)?;
    let width = inputs.len();
    let mut symbols = symbols.clone();
    let mut top = vec![];
    let mut inputs = inputs
        .into_iter()
        .map(|input| super::flatten_expression_to_atom(input, &mut symbols, &mut top))
        .collect::<Result<Vec<_>, _>>()?;
    let mut short_rows = Vec::with_capacity(rows.len());
    for row in rows {
        let pairs = crate::shared::utils::short_table_row_entries(&row).ok_or(RuleNotApplicable)?;
        let mut positions = std::collections::HashSet::new();
        let mut short_row = Vec::with_capacity(pairs.len());
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
            if position >= width || !positions.insert(position) {
                return Err(RuleNotApplicable);
            }
            match eval_constant(value) {
                Some(Literal::Int(value)) => short_row.push((position, value)),
                Some(Literal::Bool(value)) => short_row.push((position, i32::from(value))),
                Some(_) => return Err(RuleNotApplicable),
                None => {
                    let equality = Expr::Eq(
                        Metadata::new(),
                        Moo::new(Expr::Atomic(Metadata::new(), inputs[position].clone())),
                        Moo::new(value.clone()),
                    );
                    let indicator =
                        super::flatten_expression_to_atom(equality, &mut symbols, &mut top)?;
                    short_row.push((inputs.len(), 1));
                    inputs.push(indicator);
                }
            }
        }
        short_rows.push(short_row);
    }
    Ok(RuleEffect::new(
        Expr::FlatShortTable(Metadata::new(), inputs, short_rows),
        top,
        symbols,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::AbstractLiteral;

    fn flat_matches(expression: &Expr) -> bool {
        let Expr::FlatShortTable(_, inputs, rows) = expression else {
            panic!("Expected a native short table");
        };
        let values = inputs
            .iter()
            .map(|atom| {
                match eval_constant(&Expr::Atomic(Metadata::new(), atom.clone())).unwrap() {
                    Literal::Int(value) => value,
                    Literal::Bool(value) => i32::from(value),
                    _ => panic!("Expected scalar inputs"),
                }
            })
            .collect::<Vec<_>>();
        rows.iter().any(|row| {
            row.iter()
                .all(|(position, value)| values[*position] == *value)
        })
    }

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
            (vec![set(vec![pair(2, 9), pair(2, 9)])], true),
            (vec![set(vec![pair(1, 8)])], false),
        ] {
            let expression = Expr::ShortTable(
                Metadata::new(),
                Moo::new(conjure_cp::matrix_expr![7.into(), 9.into()]),
                Moo::new(set(rows)),
            );
            let effect = flatten_short_table(&expression, &SymbolTable::new()).unwrap();
            assert_eq!(flat_matches(&effect.new_expression), expected);
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
        assert!(flatten_short_table(&expression, &SymbolTable::new()).is_err());
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
            let effect = flatten_short_table(&table(rows), &SymbolTable::new()).unwrap();
            assert_eq!(flat_matches(&effect.new_expression), expected);
        }
        for rows in [
            vec![vec![pair(0, 7)]],
            vec![vec![pair(3, 7)]],
            vec![vec![pair(1, 7), pair(1, 8)]],
        ] {
            assert!(flatten_short_table(&table(rows), &SymbolTable::new()).is_err());
        }
        let sequence =
            Expr::AbstractLiteral(Metadata::new(), AbstractLiteral::Sequence(vec![pair(1, 7)]));
        let expression = Expr::ShortTable(
            Metadata::new(),
            Moo::new(conjure_cp::matrix_expr![7.into(), 9.into()]),
            Moo::new(conjure_cp::into_matrix_expr!(vec![sequence])),
        );
        let effect = flatten_short_table(&expression, &SymbolTable::new()).unwrap();
        assert!(flat_matches(&effect.new_expression));
    }
}
