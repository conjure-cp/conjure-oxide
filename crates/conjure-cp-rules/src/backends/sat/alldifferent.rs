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
    let elements = super::counting::matrix_entries(matrix).ok_or(RuleNotApplicable)?;
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
    let inputs = super::counting::matrix_entries(matrix).ok_or(RuleNotApplicable)?;
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
