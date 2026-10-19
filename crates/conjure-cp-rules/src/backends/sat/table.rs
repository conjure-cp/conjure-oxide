//! Preserve numeric table relations until library clause generation.
use crate::shared::utils::{
    checked_short_table_row, table_operand, table_ordered_entries, table_rows,
};
use conjure_cp::ast::{Atom, Expression as Expr, Literal, SatEncodingDecision, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Omitted short-table positions compare the input with itself rather than expanding its domain.
#[register_rule("SAT", 18500, [ShortTable])]
fn expand_short_table(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::ShortTable(_, tuple, rows) = expr else {
        return Err(RuleNotApplicable);
    };
    let inputs = table_ordered_entries(tuple).ok_or(RuleNotApplicable)?;
    let rows = table_rows(rows).ok_or(RuleNotApplicable)?;
    let rows = rows
        .into_iter()
        .map(|row| {
            let mut values = inputs.clone();
            for (position, value) in checked_short_table_row(&row, inputs.len())? {
                values[position] = value;
            }
            Some(conjure_cp::into_matrix_expr!(values))
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::pure(Expr::Table(
        conjure_cp::ast::Metadata::new(),
        tuple.clone(),
        conjure_cp::ast::Moo::new(conjure_cp::into_matrix_expr!(rows)),
    )))
}

fn cell_value(expression: &Expr) -> Option<i64> {
    let expression = table_operand(expression);
    match expression {
        Expr::Atomic(_, Atom::Literal(Literal::Int(value))) => Some(i64::from(value)),
        Expr::Atomic(_, Atom::Literal(Literal::Bool(value))) => Some(i64::from(value)),
        expression => {
            let view = super::pseudo_boolean::integer_view(&expression)?;
            view.terms.is_empty().then_some(view.constant)
        }
    }
}

/// Both signs retain a Boolean output for nested and reified uses.
#[register_rule("SAT", 18500, [Table, NegativeTable])]
fn select_table(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let (tuple, rows, negative) = match expr {
        Expr::Table(_, tuple, rows) => (tuple, rows, false),
        Expr::NegativeTable(_, tuple, rows) => (tuple, rows, true),
        _ => return Err(RuleNotApplicable),
    };
    let tuple = table_ordered_entries(tuple).ok_or(RuleNotApplicable)?;
    let inputs = tuple
        .iter()
        .map(super::pseudo_boolean::integer_view)
        .collect::<Option<Vec<_>>>()
        .ok_or(RuleNotApplicable)?;
    let rows = table_rows(rows).ok_or(RuleNotApplicable)?;
    let row_expressions = rows
        .iter()
        .map(|row| {
            let row = table_ordered_entries(row)?;
            (row.len() == inputs.len()).then_some(row)
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(RuleNotApplicable)?;
    let constants = row_expressions
        .iter()
        .map(|row| row.iter().map(cell_value).collect::<Option<Vec<_>>>())
        .collect::<Option<Vec<_>>>();
    let (rows, row_views) = if let Some(rows) = constants {
        (rows, None)
    } else {
        let views = row_expressions
            .iter()
            .map(|row| {
                row.iter()
                    .map(super::pseudo_boolean::integer_view)
                    .collect::<Option<Vec<_>>>()
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(RuleNotApplicable)?;
        (vec![], Some(views))
    };
    let mut symbols = symbols.clone();
    let output = super::boolean::create_bool_aux(&mut symbols);
    Ok(RuleEffect::sat(
        output.clone(),
        vec![SatEncodingDecision::Table {
            output,
            inputs,
            rows,
            row_views,
            negative,
            encoding: None,
            pb_encoding: None,
        }],
        symbols,
    ))
}
