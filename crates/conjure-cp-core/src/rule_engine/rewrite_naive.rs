use super::{RewriteError, RuleSet, resolve_rules::RuleData};
use crate::{
    Model,
    ast::{Expression as Expr, Metadata, discriminant_from_value},
    bug,
    objective::introduce_objective_auxiliary,
    rule_engine::{
        expression_zipper::ExpressionZipper,
        get_rules_grouped,
        rewriter_common::{
            RuleResult, VariableDeclarationSnapshot, log_rule_application,
            snapshot_symbols_after_effect, snapshot_variable_declarations,
            try_rewrite_value_letting_once,
        },
    },
    settings::{
        RewriteConfig, Rewriter, default_rule_trace_enabled, rule_trace_enabled,
        rule_trace_verbose_enabled, set_current_rewriter,
    },
    stats::RewriterStats,
};

use itertools::Itertools;
use std::{
    collections::{BTreeMap, HashMap},
    time::Instant,
};
use tracing::trace;
use uniplate::Biplate;

// debug imports
#[cfg(debug_assertions)]
use {
    crate::ast::assertions::debug_assert_model_well_formed,
    tracing::{Level, span},
};

type ApplicableRule<'a, CtxFnType> = (
    RuleResult<'a>,
    u16,
    Expr,
    CtxFnType,
    Option<VariableDeclarationSnapshot>,
);

#[derive(Default)]
struct DirtyTrace {
    enabled: bool,
    passes: usize,
    priority_scans: usize,
    expression_visits: usize,
    dirty_hits: usize,
    clean_marks: usize,
    attempted_expressions: usize,
    rule_attempts: usize,
    rewrites: usize,
    value_letting_rewrites: usize,
    whole_model_clears_after_value_letting: usize,
    whole_model_clears_after_side_effects: usize,
    replacement_subtree_clears: usize,
    ancestor_clears: usize,
    dirty_hits_by_priority: BTreeMap<u16, usize>,
    clean_marks_by_priority: BTreeMap<u16, usize>,
    rule_attempts_by_priority: BTreeMap<u16, usize>,
    rewrites_by_rule: BTreeMap<String, usize>,
    side_effect_rewrites_by_rule: BTreeMap<String, usize>,
}

impl DirtyTrace {
    fn from_env() -> Self {
        Self {
            enabled: std::env::var_os("CONJURE_DIRTY_TRACE").is_some(),
            ..Self::default()
        }
    }

    fn record_dirty_hit(&mut self, priority: u16) {
        self.dirty_hits += 1;
        *self.dirty_hits_by_priority.entry(priority).or_default() += 1;
    }

    fn record_clean_mark(&mut self, priority: u16) {
        self.clean_marks += 1;
        *self.clean_marks_by_priority.entry(priority).or_default() += 1;
    }

    fn record_rewrite(&mut self, rule_name: &str, side_effects: bool) {
        self.rewrites += 1;
        *self
            .rewrites_by_rule
            .entry(rule_name.to_owned())
            .or_default() += 1;
        if side_effects {
            *self
                .side_effect_rewrites_by_rule
                .entry(rule_name.to_owned())
                .or_default() += 1;
        }
    }

    fn finish(&self, stats: &RewriterStats) {
        if !self.enabled {
            return;
        }

        eprintln!("[dirty-trace] passes={}", self.passes);
        eprintln!("[dirty-trace] priority_scans={}", self.priority_scans);
        eprintln!("[dirty-trace] expression_visits={}", self.expression_visits);
        eprintln!(
            "[dirty-trace] attempted_expressions={}",
            self.attempted_expressions
        );
        eprintln!("[dirty-trace] rule_attempts_counted={}", self.rule_attempts);
        eprintln!(
            "[dirty-trace] stats_rule_attempts={}",
            stats.rewriter_rule_application_attempts.unwrap_or(0)
        );
        eprintln!("[dirty-trace] clean_marks={}", self.clean_marks);
        eprintln!("[dirty-trace] dirty_hits={}", self.dirty_hits);
        eprintln!("[dirty-trace] rewrites={}", self.rewrites);
        eprintln!(
            "[dirty-trace] value_letting_rewrites={}",
            self.value_letting_rewrites
        );
        eprintln!(
            "[dirty-trace] whole_model_clears_after_value_letting={}",
            self.whole_model_clears_after_value_letting
        );
        eprintln!(
            "[dirty-trace] whole_model_clears_after_side_effects={}",
            self.whole_model_clears_after_side_effects
        );
        eprintln!(
            "[dirty-trace] replacement_subtree_clears={}",
            self.replacement_subtree_clears
        );
        eprintln!("[dirty-trace] ancestor_clears={}", self.ancestor_clears);
        eprintln!(
            "[dirty-trace] dirty_hits_by_priority={:?}",
            self.dirty_hits_by_priority
        );
        eprintln!(
            "[dirty-trace] clean_marks_by_priority={:?}",
            self.clean_marks_by_priority
        );
        eprintln!(
            "[dirty-trace] rule_attempts_by_priority={:?}",
            self.rule_attempts_by_priority
        );
        eprintln!("[dirty-trace] rewrites_by_rule={:?}", self.rewrites_by_rule);
        eprintln!(
            "[dirty-trace] side_effect_rewrites_by_rule={:?}",
            self.side_effect_rewrites_by_rule
        );
    }
}

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

struct RewritePassContext<'ctx, 'rules> {
    rules_grouped: &'ctx Vec<(u16, Vec<RuleData<'rules>>)>,
    bucketed_rules: &'ctx Vec<RuleGroup<'rules>>,
    prop_multiple_equally_applicable: bool,
    stats: &'ctx mut RewriterStats,
    dirty_trace: &'ctx mut DirtyTrace,
    config: RewriteConfig,
    #[cfg(debug_assertions)]
    run_start: &'ctx Instant,
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
    rewriter_stats.is_optimization_enabled = Some(!config.is_baseline());
    let mut dirty_trace = DirtyTrace::from_env();
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
    {
        let mut pass_ctx = RewritePassContext {
            rules_grouped: &rules_grouped,
            bucketed_rules: &bucketed_rules,
            prop_multiple_equally_applicable,
            stats: &mut rewriter_stats,
            dirty_trace: &mut dirty_trace,
            config,
            #[cfg(debug_assertions)]
            run_start: &run_start,
        };
        while done_something {
            done_something = try_rewrite_model(&mut model, &mut pass_ctx).is_some();
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
    dirty_trace.finish(
        model
            .context
            .read()
            .unwrap()
            .stats
            .rewriter_runs
            .last()
            .expect("rewriter stats were just added"),
    );

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
fn try_rewrite_model<'ctx, 'rules>(
    submodel: &mut Model,
    ctx: &mut RewritePassContext<'ctx, 'rules>,
) -> Option<()> {
    ctx.dirty_trace.passes += 1;
    if let Some(result) = try_rewrite_value_letting_once(
        submodel,
        ctx.rules_grouped,
        ctx.prop_multiple_equally_applicable,
    ) {
        ctx.dirty_trace.value_letting_rewrites += 1;
        if ctx.config.dirty {
            ctx.dirty_trace.whole_model_clears_after_value_letting += 1;
            clear_model_clean_rule_metadata(submodel);
        }
        return Some(result);
    }

    let mut results: Vec<ApplicableRule<'_, ExpressionZipper>> = vec![];

    // Iterate over rules by priority in descending order.
    'top: for rule_group in ctx.bucketed_rules.iter() {
        ctx.dirty_trace.priority_scans += 1;
        // Rewrite within the current root expression tree.
        let mut zipper = ExpressionZipper::new(submodel.root().clone());
        loop {
            ctx.dirty_trace.expression_visits += 1;
            let expr = zipper.focus().clone();
            if ctx.config.dirty
                && expr
                    .meta_ref()
                    .is_clean_for_rule_priority(rule_group.priority)
            {
                ctx.dirty_trace.record_dirty_hit(rule_group.priority);
                if !move_to_next_expression(&mut zipper) {
                    break;
                }
                continue;
            }

            let mut attempted_rule = false;
            let results_before_expr = results.len();
            for rd in rule_group.candidates(ctx.config, &expr) {
                attempted_rule = true;
                ctx.dirty_trace.rule_attempts += 1;
                *ctx.dirty_trace
                    .rule_attempts_by_priority
                    .entry(rule_group.priority)
                    .or_default() += 1;
                // Count rule application attempts
                ctx.stats.rewriter_rule_application_attempts =
                    Some(ctx.stats.rewriter_rule_application_attempts.unwrap_or(0) + 1);

                #[cfg(debug_assertions)]
                let span = span!(Level::TRACE,"trying_rule_application",rule_name=rd.rule.name,rule_target_expression=%expr);

                #[cfg(debug_assertions)]
                let _guard = span.enter();

                #[cfg(debug_assertions)]
                tracing::trace!(rule_name = rd.rule.name, "Trying rule");

                let variable_snapshot_before = matches!(expr, Expr::Root(_, _))
                    .then(|| snapshot_variable_declarations(&submodel.symbols()));

                match (rd.rule.application)(&expr, &submodel.symbols()) {
                    Ok(red) => {
                        // when called a lot, this becomes very expensive!
                        #[cfg(debug_assertions)]
                        if rule_trace_enabled() && rule_trace_verbose_enabled() {
                            log_verbose_rule_attempt(
                                ctx.run_start,
                                &rule_group.priority,
                                rd.rule.name,
                                rd.rule_set.name,
                                "success",
                                &expr,
                            );
                        }

                        // Count successful rule applications
                        ctx.stats.rewriter_rule_applications =
                            Some(ctx.stats.rewriter_rule_applications.unwrap_or(0) + 1);

                        // Collect applicable rules
                        results.push((
                            RuleResult {
                                rule_data: rd.clone(),
                                effect: red,
                            },
                            rule_group.priority,
                            expr.clone(),
                            zipper.clone(),
                            variable_snapshot_before,
                        ));
                    }
                    Err(_) => {
                        // when called a lot, this becomes very expensive!
                        #[cfg(debug_assertions)]
                        if rule_trace_enabled() && rule_trace_verbose_enabled() {
                            log_verbose_rule_attempt(
                                ctx.run_start,
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
            if ctx.config.dirty && attempted_rule && results.len() == results_before_expr {
                ctx.dirty_trace.record_clean_mark(rule_group.priority);
                zipper
                    .focus()
                    .meta_ref()
                    .mark_clean_for_rule_priority(rule_group.priority);
            }
            if attempted_rule {
                ctx.dirty_trace.attempted_expressions += 1;
            }
            // This expression has the highest rule priority so far, so this is what we want to
            // rewrite.
            if !results.is_empty() {
                break 'top;
            }

            if !move_to_next_expression(&mut zipper) {
                break;
            }
        }

        if ctx.config.dirty {
            submodel.replace_root(zipper.rebuild_root());
        }
    }

    match results.as_slice() {
        [] => return None, // no rules are applicable.
        [(result, _priority, expr, zipper, variable_snapshot_before), ..] => {
            if ctx.prop_multiple_equally_applicable {
                assert_no_multiple_equally_applicable_rules(&results, ctx.rules_grouped);
            }

            let effect = result.effect.materialise(&submodel.symbols());
            let variable_snapshots = variable_snapshot_before.clone().map(|before| {
                let after = snapshot_symbols_after_effect(&submodel.symbols(), &effect.symbols);
                (before, after)
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
            let new_root = replace_focus_and_dirty_ancestors(
                zipper,
                result.effect.new_expression.clone(),
                ctx.dirty_trace,
            );
            submodel.replace_root(new_root);

            // Apply new symbols and top level
            let has_model_side_effects = effect_has_model_side_effects(&result.effect);
            ctx.dirty_trace
                .record_rewrite(result.rule_data.rule.name, has_model_side_effects);
            result.effect.clone().apply(submodel);
            if ctx.config.dirty && has_model_side_effects {
                ctx.dirty_trace.whole_model_clears_after_side_effects += 1;
                clear_model_clean_rule_metadata(submodel);
            }

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

/// Advances the zipper in preorder, respecting [`ExpressionZipper`] traversal boundaries.
fn move_to_next_expression(zipper: &mut ExpressionZipper) -> bool {
    if zipper.go_down().is_some() {
        return true;
    }

    while zipper.go_right().is_none() {
        if zipper.go_up().is_none() {
            return false;
        };
    }

    true
}

/// Replaces the focused expression and clears clean-rule metadata on the changed path to root.
fn replace_focus_and_dirty_ancestors(
    zipper: &ExpressionZipper,
    new_focus: Expr,
    dirty_trace: &mut DirtyTrace,
) -> Expr {
    let mut zipper = zipper.clone();
    dirty_trace.replacement_subtree_clears += 1;
    zipper.replace_focus(clear_expr_clean_rule_metadata(new_focus));

    while zipper.go_up().is_some() {
        dirty_trace.ancestor_clears += 1;
        zipper.focus().meta_ref().clear_clean_rule_priority();
    }

    zipper.rebuild_root()
}

/// Clears clean-rule metadata from every expression in a subtree.
fn clear_expr_clean_rule_metadata(expr: Expr) -> Expr {
    expr.transform_bi(&|metadata: Metadata| {
        metadata.clear_clean_rule_priority();
        metadata
    })
}

/// Clears clean-rule metadata from the model root expression tree.
fn clear_model_clean_rule_metadata(model: &mut Model) {
    model.replace_root(clear_expr_clean_rule_metadata(model.root().clone()));
}

fn effect_has_model_side_effects(effect: &crate::rule_engine::rule::RuleEffect) -> bool {
    !effect.new_top.is_empty()
        || !effect.new_clauses.is_empty()
        || effect.symbols.clone().into_iter_local().next().is_some()
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
        .map(|(result, _, _, _, _)| result.rule_data.rule.name)
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
