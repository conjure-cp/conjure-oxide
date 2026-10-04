//! Retain allDifferent as a semantic decision until library clause generation.
use conjure_cp::ast::{
    Expression as Expr, Metadata, Moo, ReturnType, SatEncodingDecision, SymbolTable, Typeable,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Preserve ready numeric operands, including nested Boolean uses.
#[register_rule("SAT", 18500, [AllDiff, AllDifferentExcept])]
fn select_alldifferent(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let (matrix, except) = match expr {
        Expr::AllDiff(_, matrix) => (matrix, None),
        Expr::AllDifferentExcept(_, matrix, except) => {
            let value = super::pseudo_boolean::integer_view(except).ok_or(RuleNotApplicable)?;
            (matrix, Some(value))
        }
        _ => return Err(RuleNotApplicable),
    };
    let elements = matrix.unwrap_list_cow().ok_or(RuleNotApplicable)?;
    let inputs = elements
        .iter()
        .map(super::pseudo_boolean::integer_view)
        .collect::<Option<Vec<_>>>()
        .ok_or(RuleNotApplicable)?;
    let mut symbols = symbols.clone();
    let output = super::boolean::create_bool_aux(&mut symbols);
    Ok(RuleEffect::sat(
        output.clone(),
        vec![SatEncodingDecision::AllDifferent {
            output,
            inputs,
            comparisons: None,
            except,
            encoding: None,
            amo_encoding: None,
            pb_encoding: None,
        }],
        symbols,
    ))
}

/// Construct whole-value comparisons before their representation-specific lowering.
#[register_rule("SAT", 18400, [AllDiff, AllDifferentExcept])]
fn introduce_compound_alldifferent(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let (matrix, except) = match expr {
        Expr::AllDiff(_, matrix) => (matrix, None),
        Expr::AllDifferentExcept(_, matrix, except) => (matrix, Some(except)),
        _ => return Err(RuleNotApplicable),
    };
    let inputs = matrix.unwrap_list_cow().ok_or(RuleNotApplicable)?;
    if inputs.is_empty()
        || inputs.iter().any(|input| {
            matches!(
                input.return_type(),
                ReturnType::Int | ReturnType::Bool | ReturnType::Unknown
            )
        })
    {
        return Err(RuleNotApplicable);
    }
    let mut symbols = symbols.clone();
    let mut definitions = Vec::new();
    let mut conditions = Vec::new();
    for (index, left) in inputs.iter().enumerate() {
        let exempt = except.filter(|_| index + 1 < inputs.len()).map(|except| {
            let output = super::boolean::create_bool_aux(&mut symbols);
            definitions.push(Expr::Iff(
                Metadata::new(),
                Moo::new(output.clone()),
                Moo::new(Expr::Eq(
                    Metadata::new(),
                    Moo::new(left.clone()),
                    except.clone(),
                )),
            ));
            output
        });
        for right in &inputs[index + 1..] {
            let unequal = Expr::Neq(
                Metadata::new(),
                Moo::new(left.clone()),
                Moo::new(right.clone()),
            );
            conditions.push(if let Some(exempt) = &exempt {
                Expr::Or(
                    Metadata::new(),
                    Moo::new(conjure_cp::into_matrix_expr!(vec![unequal, exempt.clone()])),
                )
            } else {
                unequal
            });
        }
    }
    Ok(RuleEffect::new(
        Expr::SatAllDifferentComparisons(
            Metadata::new(),
            Moo::new(conjure_cp::into_matrix_expr!(conditions)),
        ),
        definitions,
        symbols,
    ))
}

/// Keep the allDifferent family choice after whole-value comparisons become Boolean literals.
#[register_rule("SAT", 18500, [SatAllDifferentComparisons])]
fn select_compound_alldifferent(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SatAllDifferentComparisons(_, comparisons) = expr else {
        return Err(RuleNotApplicable);
    };
    let comparisons = comparisons.unwrap_list_cow().ok_or(RuleNotApplicable)?;
    if !comparisons.iter().all(|condition| {
        condition.return_type() == ReturnType::Bool && crate::shared::utils::is_literal(condition)
    }) {
        return Err(RuleNotApplicable);
    }
    let mut symbols = symbols.clone();
    let output = super::boolean::create_bool_aux(&mut symbols);
    Ok(RuleEffect::sat(
        output.clone(),
        vec![SatEncodingDecision::AllDifferent {
            output,
            inputs: vec![],
            comparisons: Some(comparisons.into_owned()),
            except: None,
            encoding: None,
            amo_encoding: None,
            pb_encoding: None,
        }],
        symbols,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{
        AbstractLiteral, DeclarationPtr, Domain, Metadata, Moo, Name, Reference, SATIntEncoding,
    };

    #[test]
    fn exception_views_are_retained_and_pending_variables_wait() {
        let matrix = Expr::AbstractLiteral(
            Metadata::new(),
            AbstractLiteral::matrix_implied_indices(vec![0.into(), 0.into()]),
        );
        let input = Expr::AllDifferentExcept(
            Metadata::new(),
            Moo::new(matrix.clone()),
            Moo::new(0.into()),
        );
        let effect = select_alldifferent(&input, &SymbolTable::new()).unwrap();
        assert!(matches!(
            &effect.new_sat_decisions[0],
            SatEncodingDecision::AllDifferent { except: Some(value), inputs, .. }
            if inputs.len() == 2 && value.constant == 0 && value.terms.is_empty()
        ));
        let pending: Expr = Reference::new(DeclarationPtr::new_find(
            Name::User("exception".into()),
            Domain::int(vec![conjure_cp::ast::Range::Bounded(0, 1)]),
        ))
        .into();
        let input = Expr::AllDifferentExcept(Metadata::new(), Moo::new(matrix), Moo::new(pending));
        assert!(select_alldifferent(&input, &SymbolTable::new()).is_err());
        let bit: Expr = Reference::new(DeclarationPtr::new_find(
            Name::User("exception_bit".into()),
            Domain::bool(),
        ))
        .into();
        let exception = Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(conjure_cp::into_matrix_expr!(vec![bit.clone()])),
            (0, 1),
        );
        let input = Expr::AllDifferentExcept(
            Metadata::new(),
            Moo::new(conjure_cp::into_matrix_expr!(vec![0.into(), 0.into()])),
            Moo::new(exception),
        );
        let effect = select_alldifferent(&input, &SymbolTable::new()).unwrap();
        assert!(effect.new_sat_decisions[0].expressions().contains(&&bit));
    }

    #[test]
    fn alldifferent_retains_numeric_views_and_stages_whole_value_comparisons() {
        let bit: Expr = Reference::new(DeclarationPtr::new_find(
            Name::User("bit".into()),
            Domain::bool(),
        ))
        .into();
        let matrix = |inputs| {
            Expr::AbstractLiteral(
                Metadata::new(),
                AbstractLiteral::matrix_implied_indices(inputs),
            )
        };
        let direct = Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Direct,
            Moo::new(matrix(vec![
                bit.clone(),
                Expr::Not(Metadata::new(), Moo::new(bit.clone())),
            ])),
            (-1, 0),
        );
        let input = Expr::AllDiff(Metadata::new(), Moo::new(matrix(vec![direct, 0.into()])));
        let effect = select_alldifferent(&input, &SymbolTable::new()).unwrap();
        assert!(
            matches!(&effect.new_sat_decisions[0], SatEncodingDecision::AllDifferent { inputs, encoding: None, .. }
            if inputs[0].choices.as_ref().unwrap().iter().map(|(value, _)| *value).collect::<Vec<_>>() == vec![-1, 0]
                && inputs[1].constant == 0)
        );
        let constant = Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(matrix(vec![Expr::from(false)])),
            (42, 42),
        );
        let view = super::super::pseudo_boolean::integer_view(&constant).unwrap();
        assert_eq!(view.constant, 0, "Read literal bits, not the bounds");
        assert!(view.choices.is_some());
        let compound = Expr::AllDiff(
            Metadata::new(),
            Moo::new(matrix(vec![matrix(vec![bit.clone()]), matrix(vec![bit])])),
        );
        assert!(select_alldifferent(&compound, &SymbolTable::new()).is_err());
        let effect = introduce_compound_alldifferent(&compound, &SymbolTable::new()).unwrap();
        assert!(matches!(
            effect.new_expression,
            Expr::SatAllDifferentComparisons(_, _)
        ));
        assert!(select_compound_alldifferent(&effect.new_expression, &SymbolTable::new()).is_err());
        let ready = Expr::SatAllDifferentComparisons(
            Metadata::new(),
            Moo::new(matrix(vec![true.into(), false.into()])),
        );
        let effect = select_compound_alldifferent(&ready, &SymbolTable::new()).unwrap();
        assert!(
            matches!(&effect.new_sat_decisions[0], SatEncodingDecision::AllDifferent { comparisons: Some(conditions), .. } if conditions.len() == 2)
        );
    }
}
