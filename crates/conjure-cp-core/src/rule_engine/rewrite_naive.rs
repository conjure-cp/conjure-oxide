use super::{RewriteError, RuleSet, resolve_rules::RuleData};
use crate::{
    Model,
    ast::{Expression as Expr, SubModel, comprehension::Comprehension},
    bug,
    rule_engine::{
        get_rules_grouped,
        rewriter_common::{RuleResult, log_rule_application},
        submodel_zipper::submodel_ctx,
    },
    stats::RewriterStats,
};

use itertools::Itertools;
use std::{process::exit, sync::Arc, time::Instant};
use tracing::{Level, span, trace};
use uniplate::{Biplate, Uniplate};

/// A naive, exhaustive rewriter for development purposes. Applies rules in priority order,
/// favouring expressions found earlier during preorder traversal of the tree.
pub fn rewrite_naive<'a>(
    model: &Model,
    rule_sets: &Vec<&'a RuleSet<'a>>,
    prop_multiple_equally_applicable: bool,
    exit_after_unrolling: bool,
) -> Result<Model, RewriteError> {
    let rules_grouped = get_rules_grouped(rule_sets)
        .unwrap_or_else(|_| bug!("get_rule_priorities() failed!"))
        .into_iter()
        .collect_vec();

    let mut model = model.clone();
    let mut done_something = true;

    let mut rewriter_stats = RewriterStats::new();
    rewriter_stats.is_optimization_enabled = Some(false);
    let run_start = Instant::now();

    trace!(
        target: "rule_engine_human",
        "Model before rewriting:\n\n{}\n--\n",
        model
    );

    // Rewrite until there are no more rules left to apply.
    while done_something {
        let mut new_model = None;
        done_something = false;

        // Rewrite each sub-model in the tree, largest first.
        for (mut submodel, ctx) in <_ as Biplate<SubModel>>::contexts_bi(&model) {
            if try_rewrite_model(
                &mut submodel,
                &rules_grouped,
                prop_multiple_equally_applicable,
                &mut rewriter_stats,
            )
            .is_some()
            {
                new_model = Some(ctx(submodel));
                done_something = true;
                break;
            }
        }
        if let Some(new_model) = new_model {
            model = new_model;
        }

        if Biplate::<Comprehension>::universe_bi(model.as_submodel()).is_empty()
            && exit_after_unrolling
        {
            println!("{}", model.as_submodel().root().universe().len());
            exit(0);
        }
    }

    let run_end = Instant::now();
    rewriter_stats.rewriter_run_time = Some(run_end - run_start);

    model
        .context
        .write()
        .unwrap()
        .stats
        .add_rewriter_run(rewriter_stats);

    trace!(
        target: "rule_engine_human",
        "Final model:\n\n{}",
        model
    );
    Ok(model)
}

// Tries to do a single rewrite on the model.
//
// Returns None if no change was made.
fn try_rewrite_model(
    submodel: &mut SubModel,
    rules_grouped: &Vec<(u16, Vec<RuleData<'_>>)>,
    prop_multiple_equally_applicable: bool,
    stats: &mut RewriterStats,
) -> Option<()> {
    type CtxFn = Arc<dyn Fn(Expr) -> SubModel>;
    let mut results: Vec<(RuleResult<'_>, u16, Expr, CtxFn)> = vec![];

    // Iterate over rules by priority in descending order.
    'top: for (priority, rules) in rules_grouped.iter() {
        // Using Biplate, rewrite both the expression tree, and any value lettings in the symbol
        // table.
        for (expr, ctx) in submodel_ctx(submodel.clone()) {
            // Clone expr and ctx so they can be reused
            let expr = expr.clone();
            let ctx = ctx.clone();
            for rd in rules {
                // Count rule application attempts
                stats.rewriter_rule_application_attempts =
                    Some(stats.rewriter_rule_application_attempts.unwrap_or(0) + 1);

                #[cfg(debug_assertions)]
                let span = span!(Level::TRACE,"trying_rule_application",rule_name=rd.rule.name,rule_target_expression=%expr);

                #[cfg(debug_assertions)]
                let _guard = span.enter();

                #[cfg(debug_assertions)]
                tracing::trace!(rule_name = rd.rule.name, "Trying rule");

                match (rd.rule.application)(&expr, &submodel.symbols()) {
                    Ok(red) => {
                        // Count successful rule applications
                        stats.rewriter_rule_applications =
                            Some(stats.rewriter_rule_applications.unwrap_or(0) + 1);

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
            log_rule_application(result, expr, submodel);

            // Replace expr with new_expression
            *submodel = ctx(result.reduction.new_expression.clone());

            // Apply new symbols and top level
            result.reduction.clone().apply(submodel);
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
        rules_by_priority_string.push_str(&format!("Priority {priority}:\n"));
        for rd in rules {
            rules_by_priority_string.push_str(&format!(
                "  - {} (from {})\n",
                rd.rule.name, rd.rule_set.name
            ));
        }
    }
    bug!("Multiple equally applicable rules for {expr}: {names:#?}\n\n{rules_by_priority_string}");
}
