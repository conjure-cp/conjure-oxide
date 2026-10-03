//! Constant scalar inverse lookup reuses identity-default element selection.
use conjure_cp::ast::{
    Domain, Expression as Expr, Literal, Metadata, Moo, Range, SymbolTable, eval_constant,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
use std::collections::BTreeMap;

#[register_rule("SAT", 18500, [IndexOf])]
fn index_of_constant_scalar_matrix(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::IndexOf(_, matrix, value) = expr else {
        return Err(RuleNotApplicable);
    };
    let value_domain = value.domain_of().ok_or(RuleNotApplicable)?;
    if !value_domain.is_int() && !value_domain.is_bool() {
        return Err(RuleNotApplicable);
    }
    let (entries, labels) = super::element::matrix_data(matrix).ok_or(RuleNotApplicable)?;
    let mut inverse = BTreeMap::new();
    for (entry, label) in entries.iter().zip(labels) {
        let entry = match eval_constant(entry) {
            Some(Literal::Int(value)) => value,
            Some(Literal::Bool(value)) => i32::from(value),
            _ => return Err(RuleNotApplicable),
        };
        let label = i32::try_from(label).map_err(|_| RuleNotApplicable)?;
        // Constant folding of the existing inverse lookup chooses the first matching label.
        inverse.entry(entry).or_insert(label);
    }
    let domain = Domain::int(inverse.keys().copied().map(Range::Single).collect());
    let entries = inverse.into_values().map(Expr::from).collect();
    let matrix = conjure_cp::into_matrix_expr![entries; domain];
    let value = if value_domain.is_bool() {
        Moo::new(Expr::ToInt(Metadata::new(), value.clone()))
    } else {
        value.clone()
    };
    Ok(RuleEffect::pure(Expr::ElementId(
        Metadata::new(),
        Moo::new(matrix),
        value,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inverse(matrix: Expr, value: Expr) -> Expr {
        index_of_constant_scalar_matrix(
            &Expr::IndexOf(Metadata::new(), Moo::new(matrix), Moo::new(value)),
            &SymbolTable::new(),
        )
        .unwrap()
        .new_expression
    }

    #[test]
    fn inverse_preserves_sparse_labels_first_matches_and_identity_fallback() {
        let matrix = conjure_cp::matrix_expr![20.into(), 10.into(), 20.into();
            Domain::int(vec![Range::Single(-2), Range::Single(3), Range::Single(7)])];
        for value in [-3, -2, 0, 3, 7, 10, 20, 21] {
            let Expr::ElementId(_, matrix, index) = inverse(matrix.clone(), value.into()) else {
                panic!("expected identity-default inverse matrix");
            };
            assert_eq!(*index, Expr::from(value));
            let (entries, labels) = super::super::element::matrix_data(&matrix).unwrap();
            let result = labels
                .iter()
                .position(|label| *label == i64::from(value))
                .map(|position| eval_constant(&entries[position]).unwrap())
                .unwrap_or(Literal::Int(value));
            assert_eq!(
                result,
                Literal::Int(match value {
                    10 => 3,
                    20 => -2,
                    _ => value,
                })
            );
        }
    }

    #[test]
    fn inverse_boolean_entries_use_numeric_zero_and_one() {
        let inverse = inverse(
            conjure_cp::matrix_expr![true.into(), false.into()],
            false.into(),
        );
        let Expr::ElementId(_, matrix, value) = inverse else {
            panic!("expected identity-default inverse matrix");
        };
        let (entries, labels) = super::super::element::matrix_data(&matrix).unwrap();
        assert_eq!(labels, vec![0, 1]);
        assert_eq!(entries, vec![2.into(), 1.into()]);
        assert!(matches!(value.as_ref(), Expr::ToInt(..)));
    }
}
