//! Normalisation rules for comprehensions.

use std::collections::HashSet;

use conjure_cp::{
    ast::{
        Expression as Expr, Metadata, Moo, Name, SymbolTable,
        ac_operators::ACOperatorKind,
        comprehension::{Comprehension, ComprehensionBuilder},
    },
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
    },
};

/// Merges nested comprehensions inside the same AC operator into a single comprehension.
///
/// ```text
/// op([ op([ body | qs2 ]) | qs1 ]) ~> op([ body | qs1, qs2 ])
/// ```
///
/// where `op` is one of `and`, `or`, `sum`, or `product`.
#[register_rule(("Base", 8900))]
fn merge_nested_ac_comprehensions(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let new_expr = merge_nested_ac_comprehensions_impl(expr).ok_or(RuleNotApplicable)?;
    Ok(Reduction::pure(new_expr))
}

fn merge_nested_ac_comprehensions_impl(expr: &Expr) -> Option<Expr> {
    let ac_operator_kind = expr.to_ac_operator_kind()?;

    let outer_comprehension = match expr {
        Expr::And(_, child)
        | Expr::Or(_, child)
        | Expr::Sum(_, child)
        | Expr::Product(_, child) => {
            let Expr::Comprehension(_, comprehension) = child.as_ref() else {
                return None;
            };
            comprehension.as_ref().clone()
        }
        _ => return None,
    };

    let outer_return_expr = outer_comprehension.clone().return_expression();
    let inner_comprehension = extract_inner_comprehension(ac_operator_kind, &outer_return_expr)?;

    // Avoid changing semantics when inner quantifiers shadow outer ones.
    let outer_names: HashSet<Name> = outer_comprehension
        .quantified_vars
        .iter()
        .cloned()
        .collect();
    if inner_comprehension
        .quantified_vars
        .iter()
        .any(|name| outer_names.contains(name))
    {
        return None;
    }

    let parent_scope = outer_comprehension
        .generator_submodel
        .symbols()
        .parent()
        .clone()?;

    let mut builder = ComprehensionBuilder::new(parent_scope);
    builder = add_generators(builder, &outer_comprehension)?;
    builder = add_generators(builder, &inner_comprehension)?;

    for guard in outer_comprehension.generator_submodel.constraints() {
        builder = builder.guard(guard.clone());
    }
    for guard in inner_comprehension.generator_submodel.constraints() {
        builder = builder.guard(guard.clone());
    }

    let merged = builder.with_return_value(
        inner_comprehension.return_expression(),
        Some(ac_operator_kind),
    );

    let merged_comprehension = Expr::Comprehension(Metadata::new(), Moo::new(merged));
    let wrapped = match ac_operator_kind {
        ACOperatorKind::And => Expr::And(Metadata::new(), Moo::new(merged_comprehension)),
        ACOperatorKind::Or => Expr::Or(Metadata::new(), Moo::new(merged_comprehension)),
        ACOperatorKind::Sum => Expr::Sum(Metadata::new(), Moo::new(merged_comprehension)),
        ACOperatorKind::Product => Expr::Product(Metadata::new(), Moo::new(merged_comprehension)),
    };

    Some(wrapped)
}

fn extract_inner_comprehension(
    ac_operator_kind: ACOperatorKind,
    expr: &Expr,
) -> Option<Comprehension> {
    let wrapped = match (ac_operator_kind, expr) {
        (ACOperatorKind::And, Expr::And(_, child)) => child.as_ref(),
        (ACOperatorKind::Or, Expr::Or(_, child)) => child.as_ref(),
        (ACOperatorKind::Sum, Expr::Sum(_, child)) => child.as_ref(),
        (ACOperatorKind::Product, Expr::Product(_, child)) => child.as_ref(),
        _ => return None,
    };

    as_single_comprehension(wrapped)
}

fn as_single_comprehension(expr: &Expr) -> Option<Comprehension> {
    if let Expr::Comprehension(_, comprehension) = expr {
        return Some(comprehension.as_ref().clone());
    }

    let exprs = expr.clone().unwrap_list()?;
    let [Expr::Comprehension(_, comprehension)] = exprs.as_slice() else {
        return None;
    };

    Some(comprehension.as_ref().clone())
}

fn add_generators(
    mut builder: ComprehensionBuilder,
    comprehension: &Comprehension,
) -> Option<ComprehensionBuilder> {
    let symbols = comprehension.generator_submodel.symbols().clone();

    for quantified_var in &comprehension.quantified_vars {
        let declaration = symbols.lookup_local(quantified_var)?;
        declaration.domain()?;
        builder = builder.generator(declaration);
    }

    Some(builder)
}
