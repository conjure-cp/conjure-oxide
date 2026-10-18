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
