use super::{RewriteError, RuleSet};
use crate::{
    ast::Expression as Expr,
    bug,
    rule_engine::{
        get_rule_priorities,
        rewriter_common::{log_rule_application, RuleResult},
        Rule,
    },
    Model,
};

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use uniplate::Biplate;

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

    while let Some(()) = try_rewrite_model(
        &mut model,
        &rules_by_priority,
        prop_multiple_equally_applicable,
    ) {}

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
    type CtxFn = Arc<dyn Fn(Expr) -> Model>;
    let mut results: Vec<(RuleResult<'_>, u16, Expr, CtxFn)> = vec![];

    // Iterate over rules by priority in descending order.
    'top: for (priority, rule_set) in rules_by_priority.iter().rev() {
        // Using Biplate, rewrite both the expression tree, and any value lettings in the symbol
        // table.
        for (expr, ctx) in <_ as Biplate<Expr>>::contexts_bi(model) {
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
            *model = ctx(result.reduction.new_expression.clone());

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
