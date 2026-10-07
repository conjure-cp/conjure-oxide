//! Canonical modelling collections for table constraints.
use crate::shared::utils::{
    short_table_row_entries, table_operand, table_ordered_entries, table_rows,
};
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Literal, Metadata, Moo, SymbolTable,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

fn is_collection(expr: &Expr, set: bool) -> bool {
    matches!(
        (expr, set),
        (Expr::AbstractLiteral(_, AbstractLiteral::Set(_)), true)
            | (
                Expr::Atomic(
                    _,
                    Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Set(_)))
                ),
                true
            )
            | (
                Expr::AbstractLiteral(_, AbstractLiteral::Sequence(_)),
                false
            )
            | (
                Expr::Atomic(
                    _,
                    Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Sequence(_)))
                ),
                false
            )
    )
}

/// Inputs are sequences; relations are sets of sequence rows or sparse sets of tuple pairs.
#[register_rule("Base", 25000, [Table, NegativeTable, ShortTable])]
fn normalise_table_collections(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (meta, inputs, rows, sparse) = match expr {
        Expr::Table(meta, inputs, rows) | Expr::NegativeTable(meta, inputs, rows) => {
            (meta, inputs, rows, false)
        }
        Expr::ShortTable(meta, inputs, rows) => (meta, inputs, rows, true),
        _ => return Err(RuleNotApplicable),
    };
    let input_value = table_operand(inputs);
    let changed_inputs = !is_collection(&input_value, false);
    let inputs = if !changed_inputs {
        inputs.clone()
    } else {
        Moo::new(Expr::AbstractLiteral(
            Metadata::new(),
            AbstractLiteral::Sequence(
                table_ordered_entries(&input_value).ok_or(RuleNotApplicable)?,
            ),
        ))
    };
    let row_value = table_operand(rows);
    let entries = table_rows(&row_value).ok_or(RuleNotApplicable)?;
    let mut changed_rows = !is_collection(&row_value, true);
    let entries = entries
        .into_iter()
        .map(|row| {
            let value = table_operand(&row);
            if is_collection(&value, sparse) {
                return Ok(row);
            }
            changed_rows = true;
            let entries = if sparse {
                short_table_row_entries(&value)
            } else {
                table_ordered_entries(&value)
            }
            .ok_or(RuleNotApplicable)?;
            Ok(Expr::AbstractLiteral(
                Metadata::new(),
                if sparse {
                    AbstractLiteral::Set(entries)
                } else {
                    AbstractLiteral::Sequence(entries)
                },
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let new_rows = if changed_rows {
        Moo::new(Expr::AbstractLiteral(
            Metadata::new(),
            AbstractLiteral::Set(entries),
        ))
    } else {
        rows.clone()
    };
    if !changed_inputs && !changed_rows {
        return Err(RuleNotApplicable);
    }
    Ok(RuleEffect::pure(match expr {
        Expr::Table(..) => Expr::Table(meta.clone(), inputs, new_rows),
        Expr::NegativeTable(..) => Expr::NegativeTable(meta.clone(), inputs, new_rows),
        Expr::ShortTable(..) => Expr::ShortTable(meta.clone(), inputs, new_rows),
        _ => unreachable!(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_tables_have_sequence_inputs_and_set_rows() {
        let inputs = Moo::new(conjure_cp::matrix_expr![1.into(), 2.into()]);
        let rows = Moo::new(conjure_cp::matrix_expr![conjure_cp::matrix_expr![
            1.into(),
            2.into()
        ]]);
        for expr in [
            Expr::Table(Metadata::new(), inputs.clone(), rows.clone()),
            Expr::NegativeTable(Metadata::new(), inputs, rows),
        ] {
            let effect = normalise_table_collections(&expr, &SymbolTable::new()).unwrap();
            let (Expr::Table(_, inputs, rows) | Expr::NegativeTable(_, inputs, rows)) =
                &effect.new_expression
            else {
                panic!("Expected table");
            };
            assert!(is_collection(inputs, false));
            assert!(is_collection(rows, true));
            assert!(
                normalise_table_collections(&effect.new_expression, &SymbolTable::new()).is_err()
            );
            let folded = Expr::NegativeTable(
                Metadata::new(),
                Moo::new(conjure_cp::ast::eval_constant(inputs).unwrap().into()),
                Moo::new(conjure_cp::ast::eval_constant(rows).unwrap().into()),
            );
            assert!(normalise_table_collections(&folded, &SymbolTable::new()).is_err());
        }
    }
}
