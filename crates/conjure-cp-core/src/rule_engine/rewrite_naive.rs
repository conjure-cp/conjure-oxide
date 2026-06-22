use super::{RewriteError, RuleSet, resolve_rules::RuleData};
use crate::{
    Model,
    ast::{Expression as Expr, discriminant_from_value},
    bug,
    objective::introduce_objective_auxiliary,
    rule_engine::{
        get_rules_grouped,
        rewriter_common::{
            RuleResult, log_rule_application, snapshot_variable_declarations,
            try_rewrite_value_letting_once,
        },
        submodel_zipper::expression_ctx,
    },
    settings::{
        RewriteConfig, Rewriter, default_rule_trace_enabled, rule_trace_enabled,
        rule_trace_verbose_enabled, set_current_rewriter,
    },
    stats::RewriterStats,
};

use itertools::Itertools;
use std::{collections::HashMap, sync::Arc, time::Instant};
use tracing::trace;

// debug imports
#[cfg(debug_assertions)]
use {
    crate::ast::assertions::debug_assert_model_well_formed,
    tracing::{Level, span},
};

type ApplicableRule<'a, CtxFnType> = (RuleResult<'a>, u16, Expr, CtxFnType);

#[derive(Clone)]
struct RuleGroup<'a> {
    priority: u16,
    rules: Vec<RuleData<'a>>,
    rules_by_discriminant: HashMap<usize, Vec<RuleData<'a>>>,
    universal_rules: Vec<RuleData<'a>>,
}

impl<'a> RuleGroup<'a> {
    fn new(priority: u16, rules: Vec<RuleData<'a>>) -> Self {
        let discriminants = rules
            .iter()
            .filter_map(|rd| rd.rule.applicable_to)
            .flatten()
            .copied()
            .collect_vec();

        let rules_by_discriminant = discriminants
            .into_iter()
            .unique()
            .map(|discriminant| {
                let bucket = rules
                    .iter()
                    .filter(|rd| rule_applies_to_discriminant(rd, discriminant))
                    .cloned()
                    .collect();
                (discriminant, bucket)
            })
            .collect();

        let universal_rules = rules
            .iter()
            .filter(|rd| rd.rule.applicable_to.is_none())
            .cloned()
            .collect();

        Self {
            priority,
            rules,
            rules_by_discriminant,
            universal_rules,
        }
    }

    fn candidates(&self, config: RewriteConfig, expr: &Expr) -> &[RuleData<'a>] {
        if !config.prefilter {
            return &self.rules;
        }

        let discriminant = discriminant_from_value(expr);
        self.rules_by_discriminant
            .get(&discriminant)
            .unwrap_or(&self.universal_rules)
    }
}

/// A naive, exhaustive rewriter for development purposes. Applies rules in priority order,
/// favouring expressions found earlier during preorder traversal of the tree.
pub fn rewrite_naive<'a>(
    model: &Model,
    rule_sets: &Vec<&'a RuleSet<'a>>,
    prop_multiple_equally_applicable: bool,
    config: RewriteConfig,
) -> Result<Model, RewriteError> {
    set_current_rewriter(Rewriter::Rewrite(config));

    let rules_grouped = get_rules_grouped(rule_sets)
        .unwrap_or_else(|_| bug!("get_rule_priorities() failed!"))
        .into_iter()
        .collect_vec();
    let bucketed_rules = rules_grouped
        .iter()
        .map(|(priority, rules)| RuleGroup::new(*priority, rules.clone()))
        .collect_vec();

    let mut model = introduce_objective_auxiliary(model.clone());
    let mut done_something = true;

    let mut rewriter_stats = RewriterStats::new();
    rewriter_stats.is_optimization_enabled = Some(config.prefilter);
    let run_start = Instant::now();

    if rule_trace_enabled() && default_rule_trace_enabled() {
        trace!(
            target: "rule_engine_rule_trace",
            "Model before rewriting:\n\n{}\n--\n",
            model
        );
    }
    if rule_trace_enabled() && rule_trace_verbose_enabled() {
        trace!(
            target: "rule_engine_rule_trace_verbose",
            "elapsed_s,rule_level,rule_name,rule_set,status,expression"
        );
    }

    // Rewrite until there are no more rules left to apply.
    while done_something {
        done_something = try_rewrite_model(
            &mut model,
            &rules_grouped,
            &bucketed_rules,
            prop_multiple_equally_applicable,
            &mut rewriter_stats,
            &run_start,
            config,
        )
        .is_some();
    }

    let run_end = Instant::now();
    rewriter_stats.rewriter_run_time = Some(run_end - run_start);

    model
        .context
        .write()
        .unwrap()
        .stats
        .add_rewriter_run(rewriter_stats);

    if rule_trace_enabled() && default_rule_trace_enabled() {
        trace!(
            target: "rule_engine_rule_trace",
            "Final model:\n\n{}",
            model
        );
    }
    Ok(model)
}

// Tries to do a single rewrite on the model.
//
// Returns None if no change was made.
fn try_rewrite_model(
    submodel: &mut Model,
    rules_grouped: &Vec<(u16, Vec<RuleData<'_>>)>,
    bucketed_rules: &Vec<RuleGroup<'_>>,
    prop_multiple_equally_applicable: bool,
    stats: &mut RewriterStats,
    #[cfg(debug_assertions)] run_start: &Instant,
    #[cfg(not(debug_assertions))] _: &Instant,
    config: RewriteConfig,
) -> Option<()> {
    if let Some(result) =
        try_rewrite_value_letting_once(submodel, rules_grouped, prop_multiple_equally_applicable)
    {
        return Some(result);
    }

    type CtxFn = Arc<dyn Fn(Expr) -> Expr>;
    let mut results: Vec<ApplicableRule<'_, CtxFn>> = vec![];

    // Iterate over rules by priority in descending order.
    'top: for rule_group in bucketed_rules.iter() {
        // Rewrite within the current root expression tree.
        for (expr, ctx) in expression_ctx(submodel.root().clone()) {
            // Clone expr and ctx so they can be reused
            let expr = expr.clone();
            let ctx = ctx.clone();
            for rd in rule_group.candidates(config, &expr) {
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
                        // when called a lot, this becomes very expensive!
                        #[cfg(debug_assertions)]
                        if rule_trace_enabled() && rule_trace_verbose_enabled() {
                            log_verbose_rule_attempt(
                                run_start,
                                &rule_group.priority,
                                rd.rule.name,
                                rd.rule_set.name,
                                "success",
                                &expr,
                            );
                        }

                        // Count successful rule applications
                        stats.rewriter_rule_applications =
                            Some(stats.rewriter_rule_applications.unwrap_or(0) + 1);

                        // Collect applicable rules
                        results.push((
                            RuleResult {
                                rule_data: rd.clone(),
                                effect: red,
                            },
                            rule_group.priority,
                            expr.clone(),
                            ctx.clone(),
                        ));
                    }
                    Err(_) => {
                        // when called a lot, this becomes very expensive!
                        #[cfg(debug_assertions)]
                        if rule_trace_enabled() && rule_trace_verbose_enabled() {
                            log_verbose_rule_attempt(
                                run_start,
                                &rule_group.priority,
                                rd.rule.name,
                                rd.rule_set.name,
                                "fail",
                                &expr,
                            );
                        }
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
        [] => return None, // no rules are applicable.
        [(result, _priority, expr, ctx), ..] => {
            if prop_multiple_equally_applicable {
                assert_no_multiple_equally_applicable_rules(&results, rules_grouped);
            }

            let effect = result.effect.materialize(&submodel.symbols());
            let variable_snapshots = matches!(expr, Expr::Root(_, _)).then(|| {
                (
                    snapshot_variable_declarations(&submodel.symbols()),
                    snapshot_variable_declarations(&effect.symbols),
                )
            });
            let result = RuleResult {
                rule_data: result.rule_data.clone(),
                effect,
            };

            // Extract the single applicable rule and apply it
            log_rule_application(
                &result,
                expr,
                &submodel.symbols(),
                variable_snapshots
                    .as_ref()
                    .map(|(before, after)| (before, after)),
            );

            // Replace expr with new_expression
            let new_root = ctx(result.effect.new_expression.clone());
            submodel.replace_root(new_root);

            // Apply new symbols and top level
            result.effect.clone().apply(submodel);

            #[cfg(debug_assertions)]
            {
                let assertion_context = format!(
                    "naive rewriter after applying rule '{}'",
                    result.rule_data.rule.name
                );
                debug_assert_model_well_formed(submodel, &assertion_context);
            }
        }
    }

    Some(())
}

fn rule_applies_to_discriminant(rule_data: &RuleData<'_>, expr_discriminant: usize) -> bool {
    rule_data
        .rule
        .applicable_to
        .is_none_or(|ids| ids.contains(&expr_discriminant))
}

#[cfg(debug_assertions)]
fn csv_escape(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(debug_assertions)]
fn log_verbose_rule_attempt(
    run_start: &Instant,
    priority: &u16,
    rule_name: &str,
    rule_set_name: &str,
    status: &str,
    expr: &Expr,
) {
    let elapsed_seconds = run_start.elapsed().as_secs_f64();
    let expr_str = expr.to_string();
    trace!(
        target: "rule_engine_rule_trace_verbose",
        "{:.3},{},{},{},{},{}",
        elapsed_seconds,
        priority,
        csv_escape(rule_name),
        csv_escape(rule_set_name),
        status,
        csv_escape(&expr_str)
    );
}

// Exits with a bug if there are multiple equally applicable rules for an expression.
fn assert_no_multiple_equally_applicable_rules<CtxFnType>(
    results: &Vec<ApplicableRule<'_, CtxFnType>>,
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
