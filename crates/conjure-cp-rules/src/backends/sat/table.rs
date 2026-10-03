//! Preserve constant table relations until library clause generation.
use conjure_cp::ast::{Atom, Expression as Expr, Literal, SatEncodingDecision, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

fn materialise(expression: &Expr) -> Expr {
    if let Expr::Atomic(_, Atom::Reference(reference)) = expression {
        if let Some(value) = reference.resolve_expression() {
            return value;
        }
        if let Some(value) = reference.resolve_constant() {
            return value.into();
        }
    }
    expression.clone()
}
fn cell_value(expression: &Expr) -> Option<i64> {
    let expression = materialise(expression);
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
    let tuple = materialise(tuple);
    let (tuple, _) = tuple.unwrap_matrix_unchecked().ok_or(RuleNotApplicable)?;
    let inputs = tuple
        .iter()
        .map(super::pseudo_boolean::integer_view)
        .collect::<Option<Vec<_>>>()
        .ok_or(RuleNotApplicable)?;
    let rows = materialise(rows);
    let (rows, _) = rows.unwrap_matrix_unchecked().ok_or(RuleNotApplicable)?;
    let rows = rows
        .iter()
        .map(|row| {
            let row = materialise(row);
            let (row, _) = row.unwrap_matrix_unchecked()?;
            if row.len() != inputs.len() {
                return None;
            }
            row.iter().map(cell_value).collect::<Option<Vec<_>>>()
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(RuleNotApplicable)?;
    let mut symbols = symbols.clone();
    let output = super::boolean::create_bool_aux(&mut symbols);
    Ok(RuleEffect::sat(
        output.clone(),
        vec![SatEncodingDecision::Table {
            output,
            inputs,
            rows,
            negative,
            encoding: None,
            pb_encoding: None,
        }],
        symbols,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{
        AbstractLiteral, DeclarationPtr, Domain, Metadata, Moo, Name, Reference,
    };
    #[test]
    fn table_extracts_signed_boolean_rows_and_rejects_wrong_width() {
        let bit: Expr = Reference::new(DeclarationPtr::new_find(
            Name::User("table_bit".into()),
            Domain::bool(),
        ))
        .into();
        let matrix = |values: Vec<Expr>| {
            let length = values.len() as i32;
            Expr::AbstractLiteral(
                Metadata::new(),
                AbstractLiteral::Matrix(
                    values,
                    Domain::int(vec![conjure_cp::ast::Range::Bounded(1, length)]),
                ),
            )
        };
        let tuple = matrix(vec![bit, (-2).into()]);
        let rows = matrix(vec![
            matrix(vec![true.into(), (-2).into()]),
            matrix(vec![false.into(), 3.into()]),
        ]);
        let expression =
            Expr::NegativeTable(Metadata::new(), Moo::new(tuple.clone()), Moo::new(rows));
        let effect = select_table(&expression, &SymbolTable::new()).unwrap();
        assert!(
            matches!(&effect.new_sat_decisions[0], SatEncodingDecision::Table { rows, negative: true, encoding: None, .. } if *rows == vec![vec![1, -2], vec![0, 3]])
        );
        let wrong = Expr::Table(
            Metadata::new(),
            Moo::new(tuple),
            Moo::new(matrix(vec![matrix(vec![1.into()])])),
        );
        assert!(select_table(&wrong, &SymbolTable::new()).is_err());
    }
}
