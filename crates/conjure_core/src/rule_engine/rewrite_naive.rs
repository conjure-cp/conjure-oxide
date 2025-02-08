use super::{resolve_rules::RuleData, RewriteError, RuleSet};
use crate::{
    ast::Expression as Expr,
    bug,
    rule_engine::{
        get_rules_grouped,
        rewriter_common::{log_rule_application, RuleResult},
    },
    Model,
};

use itertools::Itertools;
use std::sync::Arc;
use uniplate::Biplate;

/// A naive, exhaustive rewriter for development purposes. Applies rules in priority order,
/// favouring expressions found earlier during preorder traversal of the tree.
pub fn rewrite_naive<'a>(
    model: &Model,
    rule_sets: &Vec<&'a RuleSet<'a>>,
    prop_multiple_equally_applicable: bool,
) -> Result<Model, RewriteError> {
    let rules_grouped = get_rules_grouped(rule_sets)
        .unwrap_or_else(|_| bug!("get_rule_priorities() failed!"))
        .into_iter()
        .collect_vec();

    let mut model = model.clone();

    // Rewrite until there are no more rules left to apply.
    while let Some(()) =
        try_rewrite_model(&mut model, &rules_grouped, prop_multiple_equally_applicable)
    {}

    Ok(model)
}

// Tries to do a single rewrite on the model.
//
// Returns None if no change was made.
fn try_rewrite_model(
    model: &mut Model,
    rules_grouped: &Vec<(u16, Vec<RuleData<'_>>)>,
    prop_multiple_equally_applicable: bool,
) -> Option<()> {
    type CtxFn = Arc<dyn Fn(Expr) -> Model>;
    let mut results: Vec<(RuleResult<'_>, u16, Expr, CtxFn)> = vec![];

    // Iterate over rules by priority in descending order.
    'top: for (priority, rules) in rules_grouped.iter() {
        // Using Biplate, rewrite both the expression tree, and any value lettings in the symbol
        // table.
        for (expr, ctx) in <_ as Biplate<Expr>>::contexts_bi(model) {
            // Clone expr and ctx so they can be reused
            let expr = expr.clone();
            let ctx = ctx.clone();
            for rd in rules {
                match (rd.rule.application)(&expr, model) {
                    Ok(red) => {
                        // Collect applicable rules
                        results.push((
                            RuleResult {
                                rule_data: rd.clone(),
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
                            "Rule attempted but not applied: {} (priority {}, rule set {}), to expression: {}",
                            rd.rule.name,
                            priority,
                            rd.rule_set.name,
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
                assert_no_multiple_equally_applicable_rules(&results, rules_grouped);
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
fn assert_no_multiple_equally_applicable_rules<CtxFnType>(
    results: &Vec<(RuleResult<'_>, u16, Expr, CtxFnType)>,
    rules_grouped: &Vec<(u16, Vec<RuleData<'_>>)>,
) {
    if results.len() <= 1 {
        return;
    }

    let names: Vec<_> = results
        .iter()
        .map(|(result, _, _, _)| result.rule_data.rule.name)
        .collect();

    // Extract the expression from the first result
    let expr = results[0].2.clone();

    // Construct a single string to display the names of the rules grouped by priority
    let mut rules_by_priority_string = String::new();
    rules_by_priority_string.push_str("Rules grouped by priority:\n");
    for (priority, rules) in rules_grouped.iter() {
        rules_by_priority_string.push_str(&format!("Priority {}:\n", priority));
        for rd in rules {
            rules_by_priority_string.push_str(&format!(
                "  - {} (from {})\n",
                rd.rule.name, rd.rule_set.name
            ));
        }
    }
    bug!("Multiple equally applicable rules for {expr}: {names:#?}\n\n{rules_by_priority_string}");
}
