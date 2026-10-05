//! Inverse lookups retain actual index labels and first-match semantics.
use conjure_cp::ast::{
    Domain, Expression as Expr, Literal, Metadata, Moo, Range, Reference, ReturnType, SymbolTable,
    Typeable, eval_constant,
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

/// Compound and variable entries compare whole values before representation lowering.
#[register_rule("SAT", 18400, [IndexOf])]
fn index_of_general_matrix(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::IndexOf(_, matrix, value) = expr else {
        return Err(RuleNotApplicable);
    };
    let (entries, labels) = super::element::matrix_data(matrix).ok_or(RuleNotApplicable)?;
    if entries.is_empty() {
        return Err(RuleNotApplicable);
    }
    let value_type = value.return_type();
    let labels = labels
        .into_iter()
        .map(i32::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| RuleNotApplicable)?;
    let domain = Domain::int(labels.iter().copied().map(Range::Single).collect());
    let domain = if value_type == ReturnType::Int {
        domain
            .union(&value.domain_of().ok_or(RuleNotApplicable)?)
            .map_err(|_| RuleNotApplicable)?
    } else if value_type == ReturnType::Bool {
        domain
            .union(&Domain::int(vec![Range::Bounded(0, 1)]))
            .map_err(|_| RuleNotApplicable)?
    } else {
        domain
    };
    let mut symbols = symbols.clone();
    let output: Expr = Reference::new(symbols.gen_find_auxiliary(&domain)).into();
    let mut definitions = Vec::new();
    let mut remaining: Expr = true.into();
    for (entry, label) in entries.into_iter().zip(labels) {
        let matches = super::boolean::create_bool_aux(&mut symbols);
        definitions.push(Expr::Iff(
            Metadata::new(),
            Moo::new(matches.clone()),
            Moo::new(Expr::Eq(Metadata::new(), value.clone(), Moo::new(entry))),
        ));
        let first = vec![remaining.clone(), matches.clone()];
        definitions.push(Expr::Imply(
            Metadata::new(),
            Moo::new(Expr::And(
                Metadata::new(),
                Moo::new(conjure_cp::into_matrix_expr!(first)),
            )),
            Moo::new(Expr::Eq(
                Metadata::new(),
                Moo::new(output.clone()),
                Moo::new(label.into()),
            )),
        ));
        let next = super::boolean::create_bool_aux(&mut symbols);
        definitions.push(Expr::Iff(
            Metadata::new(),
            Moo::new(next.clone()),
            Moo::new(Expr::And(
                Metadata::new(),
                Moo::new(conjure_cp::into_matrix_expr!(vec![
                    remaining,
                    Expr::Not(Metadata::new(), Moo::new(matches)),
                ])),
            )),
        ));
        remaining = next;
    }
    // Only scalar values have an identity fallback. Compound absence is guarded by callers.
    if matches!(value_type, ReturnType::Int | ReturnType::Bool) {
        let fallback = if value_type == ReturnType::Bool {
            Expr::ToInt(Metadata::new(), value.clone())
        } else {
            (**value).clone()
        };
        definitions.push(Expr::Imply(
            Metadata::new(),
            Moo::new(remaining),
            Moo::new(Expr::Eq(
                Metadata::new(),
                Moo::new(output.clone()),
                Moo::new(fallback),
            )),
        ));
    }
    Ok(RuleEffect::new(output, definitions, symbols))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{AbstractLiteral, Atom};
    use uniplate::Uniplate;

    fn general_inverse_values(matrix: Expr, value: Expr) -> Vec<i32> {
        let effect = index_of_general_matrix(
            &Expr::IndexOf(Metadata::new(), Moo::new(matrix), Moo::new(value)),
            &SymbolTable::new(),
        )
        .unwrap();
        let Expr::Atomic(_, Atom::Reference(output)) = &effect.new_expression else {
            panic!("expected inverse lookup auxiliary");
        };
        let mut booleans = std::collections::BTreeSet::new();
        for definition in &effect.new_top {
            for expression in definition.universe() {
                if let Expr::Atomic(_, Atom::Reference(reference)) = expression
                    && reference.domain().unwrap().is_bool()
                {
                    booleans.insert(reference);
                }
            }
        }
        let booleans = booleans.into_iter().collect::<Vec<_>>();
        output
            .resolved_domain()
            .unwrap()
            .values_i32()
            .unwrap()
            .into_iter()
            .filter(|value| {
                (0..(1 << booleans.len())).any(|bits| {
                    let mut assignments = BTreeMap::new();
                    assignments.insert(output.clone(), Expr::from(*value));
                    for (position, reference) in booleans.iter().enumerate() {
                        assignments
                            .insert(reference.clone(), Expr::from(bits & (1 << position) != 0));
                    }
                    effect.new_top.iter().all(|definition| {
                        let expression = definition.transform(&|expression| {
                            if let Expr::Atomic(_, Atom::Reference(reference)) = &expression {
                                assignments.get(reference).cloned().unwrap_or(expression)
                            } else {
                                expression
                            }
                        });
                        eval_constant(&expression) == Some(Literal::Bool(true))
                    })
                })
            })
            .collect()
    }

    #[test]
    fn general_inverse_preserves_first_match_and_scalar_fallbacks() {
        let matrix = conjure_cp::matrix_expr![7.into(), 7.into();
            Domain::int(vec![Range::Single(-2), Range::Single(3)])];
        assert_eq!(general_inverse_values(matrix.clone(), 7.into()), vec![-2]);
        assert_eq!(general_inverse_values(matrix, 42.into()), vec![42]);
        let matrix = conjure_cp::matrix_expr![true.into(); Domain::int(vec![Range::Single(3)])];
        assert_eq!(general_inverse_values(matrix, false.into()), vec![0]);
    }

    #[test]
    fn absent_compound_inverse_does_not_constrain_a_masked_caller() {
        let tuple = |value| {
            Expr::from(Literal::AbstractLiteral(AbstractLiteral::Tuple(vec![
                Literal::Int(value),
            ])))
        };
        let matrix = conjure_cp::matrix_expr![tuple(7), tuple(7);
            Domain::int(vec![Range::Single(-2), Range::Single(3)])];
        assert_eq!(general_inverse_values(matrix.clone(), tuple(7)), vec![-2]);
        assert_eq!(general_inverse_values(matrix, tuple(42)), vec![-2, 3]);
    }

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
