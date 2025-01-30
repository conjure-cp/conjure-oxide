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

    type CtxFn = Arc<dyn Fn(Expr) -> Vec<Expr>>;
    let mut model = model.clone();

    loop {
        let mut results: Vec<(RuleResult<'_>, u16, Expr, CtxFn)> = vec![];

        // Iterate over rules by priority in descending order.
        'top: for (priority, rule_set) in rules_by_priority.iter().rev() {
            for (expr, ctx) in <_ as Biplate<Expr>>::contexts_bi(&model.get_constraints_vec()) {
                // Clone expr and ctx so they can be reused
                let expr = expr.clone();
                let ctx = ctx.clone();
                for rule in rule_set {
                    match (rule.application)(&expr, &model) {
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
            [] => break, // Exit if no rules are applicable.
            [(result, _priority, expr, ctx), ..] => {
                // Extract the single applicable rule and apply it

                log_rule_application(result, expr, &model);

                // Replace expr with new_expression
                model.set_constraints(ctx(result.reduction.new_expression.clone()));

                // Apply new symbols and top level
                result.reduction.clone().apply(&mut model);

                if results.len() > 1 && prop_multiple_equally_applicable {
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
            }
        }
    }

    Ok(model)
}
