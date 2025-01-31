use super::{RewriteError, RuleSet};
use crate::{
    ast::{Expression as Expr, Name},
    bug,
    rule_engine::{
        get_rule_priorities,
        rewriter_common::{log_rule_application, RuleResult},
        Rule,
    },
    Model,
};

use crate::ast::pretty::pretty_value_letting_declaration;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use uniplate::{Biplate, Uniplate as _};

/// A naive, exhaustive rewriter for development purposes. Applies rules in priority order,
/// favouring expressions found earlier during preorder traversal of the tree.
pub fn rewrite_naive<'a>(
    model: &Model,
    rule_sets: &Vec<&'a RuleSet<'a>>,
    prop_multiple_equally_applicable: bool,
) -> Result<Model, RewriteError> {
    let priorities =
        get_rule_priorities(rule_sets).unwrap_or_else(|_| bug!("get_rule_priorities() failed!"));

    // Group rules by priority in descending order.
    let mut grouped: BTreeMap<u16, HashSet<&'a Rule<'a>>> = BTreeMap::new();
    for (rule, priority) in priorities {
        grouped.entry(priority).or_default().insert(rule);
    }
    let rules_by_priority: Vec<(u16, HashSet<&'a Rule<'a>>)> = grouped.into_iter().collect();

    let mut model = model.clone();

    // Rewrite until there are no more rules left to apply.
    //
    // 1. try to rewrite value lettings.
    // 2. try to rewrite the model (the main event).

    while let Some(()) = try_rewrite_value_lettings(
        &mut model,
        &rules_by_priority,
        prop_multiple_equally_applicable,
    )
    .or_else(|| {
        try_rewrite_model(
            &mut model,
            &rules_by_priority,
            prop_multiple_equally_applicable,
        )
    }) {}

    Ok(model)
}

// Tries to do a single rewrite on the model.
//
// Returns None if no change was made.
fn try_rewrite_model<'a>(
    model: &mut Model,
    rules_by_priority: &Vec<(u16, HashSet<&'a Rule<'a>>)>,
    prop_multiple_equally_applicable: bool,
) -> Option<()> {
    type CtxFn = Arc<dyn Fn(Expr) -> Vec<Expr>>;
    let mut results: Vec<(RuleResult<'_>, u16, Expr, CtxFn)> = vec![];

    // Iterate over rules by priority in descending order.
    'top: for (priority, rule_set) in rules_by_priority.iter().rev() {
        for (expr, ctx) in <_ as Biplate<Expr>>::contexts_bi(&model.get_constraints_vec()) {
            // Clone expr and ctx so they can be reused
            let expr = expr.clone();
            let ctx = ctx.clone();
            for rule in rule_set {
                match (rule.application)(&expr, model) {
                    Ok(red) => {
                        // Collect applicable rules
                        results.push((
                            RuleResult {
                                rule,
                                reduction: red,
                            },
                            *priority,
                            expr.clone(),
                            ctx.clone(),
                        ));
                    }
                    Err(_) => {
                        // when called a lot, this becomes very expensive!
                        #[cfg(debug_assertions)]
                        tracing::trace!(
                            "Rule attempted but not applied: {} (priority {}), to expression: {}",
                            rule.name,
                            priority,
                            expr
                        );
                    }
                }
            }
            // This expression has the highest rule priority so far, so this is what we want to
            // rewrite.
            if !results.is_empty() {
                break 'top;
            }
        }
    }

    match results.as_slice() {
        [] => {
            return None;
        } // no rules are applicable.
        [(result, _priority, expr, ctx), ..] => {
            if prop_multiple_equally_applicable {
                assert_no_multiple_equally_applicable_rules(&results, rules_by_priority);
            }

            // Extract the single applicable rule and apply it
            log_rule_application(result, expr, model);

            // Replace expr with new_expression
            model.set_constraints(ctx(result.reduction.new_expression.clone()));

            // Apply new symbols and top level
            result.reduction.clone().apply(model);
        }
    }

    Some(())
}

// Tries to do a single rewrite on the value lettings of the model.
//
// Returns None if no change was made.
fn try_rewrite_value_lettings<'a>(
    model: &mut Model,
    rules_by_priority: &Vec<(u16, HashSet<&'a Rule<'a>>)>,
    prop_multiple_equally_applicable: bool,
) -> Option<()> {
    // I don't like this clone, but otherwise I run into borrow checker issues with multiple
    // borrows of model.
    for (name, _) in model.symbols().clone().iter_value_letting() {
        if try_rewrite_value_letting(
            name,
            model,
            rules_by_priority,
            prop_multiple_equally_applicable,
        )
        .is_some()
        {
            return Some(());
        }
    }

    None
}

// Tries to do a single rewrite on the given value letting.
//
// Returns None if no change was made.
fn try_rewrite_value_letting<'a>(
    name: &Name,
    model: &mut Model,
    rules_by_priority: &Vec<(u16, HashSet<&'a Rule<'a>>)>,
    prop_multiple_equally_applicable: bool,
) -> Option<()> {
    type CtxFn = Arc<dyn Fn(Expr) -> Expr>;
    let mut results: Vec<(RuleResult<'_>, u16, Expr, CtxFn)> = vec![];

    // Originally this was a &mut Expression.
    //
    // However, this counted as a mutable borrow of the model (as this expression is in the model), meaning
    // we couldn't borrow the model again later on.
    let root_expr = model.symbols().get_value_letting(name)?.clone();
    'top: for (priority, rule_set) in rules_by_priority.iter().rev() {
        for (expr, ctx) in root_expr.contexts() {
            // Clone expr and ctx so they can be reused
            let expr = expr.clone();
            let ctx = ctx.clone();
            for rule in rule_set {
                match (rule.application)(&expr, model) {
                    Ok(red) => {
                        // Collect applicable rules
                        results.push((
                            RuleResult {
                                rule,
                                reduction: red,
                            },
                            *priority,
                            expr.clone(),
                            ctx.clone(),
                        ));
                    }
                    #[allow(clippy::unwrap_used)]
                    Err(_) => {
                        // when called a lot, this becomes very expensive!
                        #[cfg(debug_assertions)]
                        tracing::trace!(
                            "Rule attempted but not applied: {} (priority {}), to declaration {}",
                            rule.name,
                            priority,
                            pretty_value_letting_declaration(model.symbols(), name).unwrap()
                        );
                    }
                }
            }
            // This expression has the highest rule priority so far, so this is what we want to
            // rewrite.
            if !results.is_empty() {
                break 'top;
            }
        }
    }

    match results.as_slice() {
        [] => {
            return None;
        } // no rules are applicable.
        [(result, _priority, expr, ctx), ..] => {
            if prop_multiple_equally_applicable {
                assert_no_multiple_equally_applicable_rules(&results, rules_by_priority);
            }

            // Extract the single applicable rule and apply it
            log_rule_application(result, expr, model);

            // Replace expr with new_expression
            model.symbols_mut().update_add_value_letting(
                name.clone(),
                ctx(result.reduction.new_expression.clone()),
            )?;

            // Apply new symbols and top level
            result.reduction.clone().apply(model);
        }
    }

    Some(())
}

// Exits with a bug if there are multiple equally applicable rules for an expression.
fn assert_no_multiple_equally_applicable_rules<'a, CtxFnType>(
    results: &Vec<(RuleResult<'_>, u16, Expr, CtxFnType)>,
    rules_by_priority: &Vec<(u16, HashSet<&'a Rule<'a>>)>,
) {
    if results.len() <= 1 {
        return;
    }

    let names: Vec<_> = results
        .iter()
        .map(|(result, _, _, _)| result.rule.name)
        .collect();

    // Extract the expression from the first result
    let expr = results[0].2.clone();

    // Construct a single string to display the names of the rules grouped by priority
    let mut rules_by_priority_string = String::new();
    rules_by_priority_string.push_str("Rules grouped by priority:\n");
    for (priority, rule_set) in rules_by_priority.iter().rev() {
        rules_by_priority_string.push_str(&format!("Priority {}:\n", priority));
        for rule in rule_set {
            rules_by_priority_string.push_str(&format!("  - {}\n", rule.name));
        }
    }
    bug!("Multiple equally applicable rules for {expr}: {names:#?}\n\n{rules_by_priority_string}");
}
