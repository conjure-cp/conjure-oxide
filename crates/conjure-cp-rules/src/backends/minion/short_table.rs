//! Lower sparse modelling rows to Minion's native short-table relation.
use conjure_cp::ast::{Expression as Expr, Literal, Metadata, Moo, SymbolTable, eval_constant};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

use crate::shared::utils::{checked_short_table_row, table_ordered_entries, table_rows};

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
        let pairs = checked_short_table_row(&row, width).ok_or(RuleNotApplicable)?;
        let mut short_row = Vec::with_capacity(pairs.len());
        for (position, value) in pairs {
            match eval_constant(&value) {
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
