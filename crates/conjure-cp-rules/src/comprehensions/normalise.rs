//! Normalisation rules for comprehensions.

use std::collections::HashSet;

use conjure_cp::{
    ast::{
        Expression as Expr, Metadata, Moo, Name, SymbolTable, SymbolTablePtr,
        ac_operators::ACOperatorKind, comprehension::Comprehension,
    },
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
    },
};

/// Merges nested comprehensions inside the same AC operator into a single comprehension.
///
/// ```text
/// op([ op([ op([ body | qs3 ]) | qs2 ]) | qs1 ]) ~> op([ body | qs1, qs2, qs3 ])
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

    let parent_scope = outer_comprehension.symbols().parent().clone()?;

    let mut merged_levels = vec![outer_comprehension.clone()];
    let mut merged_names: HashSet<Name> = outer_comprehension
        .quantified_vars()
        .iter()
        .cloned()
        .collect();

    let mut current_return_expression = outer_comprehension.return_expression();
    while let Some(inner_comprehension) =
        extract_inner_comprehension(ac_operator_kind, &current_return_expression)
    {
        // Avoid changing semantics when inner quantifiers shadow outer ones.
        if inner_comprehension
            .quantified_vars()
            .iter()
            .any(|name| merged_names.contains(name))
        {
            break;
        }

        merged_names.extend(inner_comprehension.quantified_vars().iter().cloned());
        current_return_expression = inner_comprehension.clone().return_expression();
        merged_levels.push(inner_comprehension);
    }

    if merged_levels.len() < 2 {
        return None;
    }

    let merged_symbols = merge_symbols(parent_scope, &merged_levels);
    let merged_qualifiers = merged_levels
        .iter()
        .flat_map(|level| level.qualifiers.clone())
        .collect();
    let mut merged = merged_levels.first()?.clone();
    merged.return_expression = current_return_expression;
    merged.qualifiers = merged_qualifiers;
    merged.symbols = merged_symbols;

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

fn merge_symbols(parent_scope: SymbolTablePtr, levels: &[Comprehension]) -> SymbolTablePtr {
    let symbols = SymbolTablePtr::with_parent(parent_scope);
    for level in levels {
        for (_, decl) in level.symbols().clone().into_iter_local() {
            symbols.write().update_insert(decl);
        }
    }
    symbols
}
