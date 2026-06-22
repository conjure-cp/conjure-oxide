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
    hash::{DefaultHasher, Hash, Hasher},
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
    usize,
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
    cache_hits: usize,
    cache_misses: usize,
    cache_terminal_hits: usize,
    cache_rewrite_hits: usize,
    cache_inserts: usize,
    cache_ancestor_mappings: usize,
    cache_resets: usize,
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
        eprintln!("[dirty-trace] cache_hits={}", self.cache_hits);
        eprintln!("[dirty-trace] cache_misses={}", self.cache_misses);
        eprintln!(
            "[dirty-trace] cache_terminal_hits={}",
            self.cache_terminal_hits
        );
        eprintln!(
            "[dirty-trace] cache_rewrite_hits={}",
            self.cache_rewrite_hits
        );
        eprintln!("[dirty-trace] cache_inserts={}", self.cache_inserts);
        eprintln!(
            "[dirty-trace] cache_ancestor_mappings={}",
            self.cache_ancestor_mappings
        );
        eprintln!("[dirty-trace] cache_resets={}", self.cache_resets);
    }
}

enum CacheResult {
    Unknown,
    Terminal(usize),
    Rewrite(CachedRewrite),
}

#[derive(Clone)]
struct CachedRewrite {
    expr: Expr,
    has_applied_effect: bool,
}

enum CacheEntry {
    Terminal,
    Rewrite(CachedRewrite),
}

#[derive(Default)]
struct RewriteCache {
    map: HashMap<u64, CacheEntry>,
    predecessors: HashMap<u64, Vec<u64>>,
    clean_levels: HashMap<u64, usize>,
}

impl RewriteCache {
    fn node_hash(expr: &Expr) -> u64 {
        expr.get_cached_hash()
    }

    fn combine(node_hash: u64, level: usize) -> u64 {
        let mut hasher = DefaultHasher::new();
        node_hash.hash(&mut hasher);
        level.hash(&mut hasher);
        hasher.finish()
    }

    fn key(expr: &Expr, level: usize) -> u64 {
        Self::combine(Self::node_hash(expr), level)
    }

    fn clear(&mut self) {
        self.map.clear();
        self.predecessors.clear();
        self.clean_levels.clear();
    }

    fn clear_context_dependent(&mut self) {
        self.map.retain(
            |_, entry| matches!(entry, CacheEntry::Rewrite(rewrite) if rewrite.has_applied_effect),
        );
        self.predecessors.clear();
        self.clean_levels.clear();
    }

    fn get(&self, subtree: &Expr, level: usize) -> CacheResult {
        let node_hash = Self::node_hash(subtree);
        if let Some(&max_clean) = self.clean_levels.get(&node_hash)
            && max_clean >= level
        {
            return CacheResult::Terminal(max_clean);
        }

        match self.map.get(&Self::combine(node_hash, level)) {
            None => CacheResult::Unknown,
            Some(CacheEntry::Rewrite(rewrite)) => CacheResult::Rewrite(rewrite.clone()),
            Some(CacheEntry::Terminal) => CacheResult::Terminal(level),
        }
    }

    fn insert(&mut self, from: &Expr, to: Option<Expr>, level: usize, has_applied_effect: bool) {
        self.insert_from_hash(Self::node_hash(from), to, level, has_applied_effect);
    }

    fn insert_from_hash(
        &mut self,
        from_hash: u64,
        to: Option<Expr>,
        level: usize,
        has_applied_effect: bool,
    ) {
        let from_key = Self::combine(from_hash, level);

        let Some(to_expr) = to else {
            self.map.insert(from_key, CacheEntry::Terminal);
            self.clean_levels
                .entry(from_hash)
                .and_modify(|l| *l = (*l).max(level))
                .or_insert(level);
            return;
        };

        let to_key = Self::key(&to_expr, level);
        if from_key == to_key {
            return;
        }

        if let Some(existing) = self.map.get(&from_key) {
            if matches!(existing, CacheEntry::Rewrite(_)) {
                return;
            }
            self.map.remove(&from_key);
        }

        let resolved = match self.map.get(&to_key) {
            Some(CacheEntry::Rewrite(rewrite)) => CachedRewrite {
                expr: rewrite.expr.clone(),
                has_applied_effect: has_applied_effect || rewrite.has_applied_effect,
            },
            Some(CacheEntry::Terminal) => {
                self.map.insert(from_key, CacheEntry::Terminal);
                return;
            }
            None => CachedRewrite {
                expr: to_expr,
                has_applied_effect,
            },
        };

        let resolved_key = Self::key(&resolved.expr, level);
        self.map
            .insert(from_key, CacheEntry::Rewrite(resolved.clone()));

        if let Some(mut predecessors) = self.predecessors.remove(&from_key) {
            for &predecessor in &predecessors {
                let predecessor_had_applied_effect = matches!(self.map.get(&predecessor), Some(CacheEntry::Rewrite(rewrite)) if rewrite.has_applied_effect);
                self.map.insert(
                    predecessor,
                    CacheEntry::Rewrite(CachedRewrite {
                        expr: resolved.expr.clone(),
                        has_applied_effect: resolved.has_applied_effect
                            || predecessor_had_applied_effect,
                    }),
                );
            }

            self.predecessors
                .entry(resolved_key)
                .or_default()
                .append(&mut predecessors);
        }

        self.predecessors
            .entry(resolved_key)
            .or_default()
            .push(from_key);
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
    cache: Option<RewriteCache>,
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
            cache: config.cache.then(RewriteCache::default),
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
        if let Some(cache) = ctx.cache.as_mut() {
            cache.clear();
            ctx.dirty_trace.cache_resets += 1;
        }
        if ctx.config.dirty {
            ctx.dirty_trace.whole_model_clears_after_value_letting += 1;
            clear_model_clean_rule_metadata(submodel);
        }
        return Some(result);
    }

    let mut results: Vec<ApplicableRule<'_, ExpressionZipper>> = vec![];

    // Iterate over rules by priority in descending order.
    'top: for (level, rule_group) in ctx.bucketed_rules.iter().enumerate() {
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

            if let Some(cache) = ctx.cache.as_mut() {
                match cache.get(&expr, level) {
                    CacheResult::Terminal(clean_level) => {
                        ctx.dirty_trace.cache_hits += 1;
                        ctx.dirty_trace.cache_terminal_hits += 1;
                        trace!(target: "rule_engine", clean_level, "Rewrite cache terminal hit");
                        if ctx.config.dirty {
                            zipper
                                .focus()
                                .meta_ref()
                                .mark_clean_for_rule_priority(rule_group.priority);
                        }
                        if !move_to_next_expression(&mut zipper) {
                            break;
                        }
                        continue;
                    }
                    CacheResult::Rewrite(cached) => {
                        ctx.dirty_trace.cache_hits += 1;
                        ctx.dirty_trace.cache_rewrite_hits += 1;
                        let new_root = replace_focus_and_dirty_ancestors(
                            &zipper,
                            clear_expr_clean_rule_metadata(cached.expr),
                            ctx.dirty_trace,
                            Some(cache),
                            level,
                            cached.has_applied_effect,
                        );
                        submodel.replace_root(new_root);
                        return Some(());
                    }
                    CacheResult::Unknown => {
                        ctx.dirty_trace.cache_misses += 1;
                    }
                }
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
                            level,
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
            if ctx.config.cache && results.len() == results_before_expr {
                if let Some(cache) = ctx.cache.as_mut() {
                    cache.insert(&expr, None, level, false);
                    ctx.dirty_trace.cache_inserts += 1;
                }
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

        if ctx.config.dirty || ctx.config.cache {
            submodel.replace_root(zipper.rebuild_root());
        }
    }

    match results.as_slice() {
        [] => return None, // no rules are applicable.
        [(result, level, expr, zipper, variable_snapshot_before), ..] => {
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

            let has_model_side_effects = effect_has_model_side_effects(&result.effect);
            let replacement = clear_expr_clean_rule_metadata(result.effect.new_expression.clone());
            if let Some(cache) = ctx.cache.as_mut() {
                if has_model_side_effects {
                    cache.clear_context_dependent();
                    ctx.dirty_trace.cache_resets += 1;
                }
                cache.insert(
                    expr,
                    Some(replacement.clone()),
                    *level,
                    has_model_side_effects,
                );
                ctx.dirty_trace.cache_inserts += 1;
            }

            // Replace expr with new_expression
            let new_root = replace_focus_and_dirty_ancestors(
                zipper,
                replacement,
                ctx.dirty_trace,
                ctx.cache.as_mut(),
                *level,
                has_model_side_effects,
            );
            submodel.replace_root(new_root);

            // Apply new symbols and top level
            ctx.dirty_trace
                .record_rewrite(result.rule_data.rule.name, has_model_side_effects);
            result.effect.clone().apply(submodel);
            if ctx.config.dirty && has_model_side_effects {
                ctx.dirty_trace.whole_model_clears_after_side_effects += 1;
                clear_model_clean_rule_metadata(submodel);
            } else if ctx.config.cache && has_model_side_effects {
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
    mut cache: Option<&mut RewriteCache>,
    cache_level: usize,
    has_applied_effect: bool,
) -> Expr {
    let mut zipper = zipper.clone();
    let ancestor_hashes = cache
        .as_ref()
        .map(|_| ancestor_hashes_to_root(&mut zipper.clone()));
    dirty_trace.replacement_subtree_clears += 1;
    zipper.replace_focus(new_focus);

    let mut ancestor_index = 0;
    while zipper.go_up().is_some() {
        dirty_trace.ancestor_clears += 1;
        zipper.focus().meta_ref().clear_clean_rule_priority();
        zipper.focus().invalidate_cache();
        if let Some(cache) = cache.as_deref_mut()
            && let Some(old_hash) = ancestor_hashes
                .as_ref()
                .and_then(|hashes| hashes.get(ancestor_index))
        {
            cache.insert_from_hash(
                *old_hash,
                Some(zipper.focus().clone()),
                cache_level,
                has_applied_effect,
            );
            dirty_trace.cache_ancestor_mappings += 1;
        }
        ancestor_index += 1;
    }

    zipper.rebuild_root()
}

fn ancestor_hashes_to_root(zipper: &mut ExpressionZipper) -> Vec<u64> {
    let mut hashes = Vec::new();
    while zipper.go_up().is_some() {
        hashes.push(zipper.focus().get_cached_hash());
    }
    hashes
}

/// Clears rewrite metadata from every expression in a subtree.
fn clear_expr_clean_rule_metadata(expr: Expr) -> Expr {
    let expr = expr.transform_bi(&|metadata: Metadata| {
        metadata.clear_clean_rule_priority();
        metadata
    });
    expr.invalidate_cache_recursive();
    expr
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

#[cfg(test)]
mod tests {
    use crate::ast::{Atom, Literal};

    use super::*;

    fn int_lit(value: i32) -> Expr {
        Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Int(value)))
    }

    fn root(exprs: Vec<Expr>) -> Expr {
        Expr::Root(Metadata::new(), exprs)
    }

    #[test]
    fn rewrite_cache_resolves_transitive_rewrites() {
        let a = int_lit(1);
        let b = int_lit(2);
        let c = int_lit(3);
        let d = int_lit(4);
        let mut cache = RewriteCache::default();

        cache.insert(&a, Some(b.clone()), 0, false);
        cache.insert(&b, Some(c.clone()), 0, false);
        cache.insert(&c, Some(d.clone()), 0, false);

        for expr in [&a, &b, &c] {
            match cache.get(expr, 0) {
                CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, d),
                CacheResult::Unknown | CacheResult::Terminal(_) => {
                    panic!("expected transitive rewrite cache hit")
                }
            }
        }
    }

    #[test]
    fn rewrite_cache_tracks_terminal_levels() {
        let a = int_lit(1);
        let mut cache = RewriteCache::default();

        cache.insert(&a, None, 0, false);
        cache.insert(&a, None, 1, false);

        match cache.get(&a, 0) {
            CacheResult::Terminal(level) => assert_eq!(level, 1),
            CacheResult::Unknown | CacheResult::Rewrite(_) => panic!("expected terminal hit"),
        }
        match cache.get(&a, 1) {
            CacheResult::Terminal(level) => assert_eq!(level, 1),
            CacheResult::Unknown | CacheResult::Rewrite(_) => panic!("expected terminal hit"),
        }
        assert!(matches!(cache.get(&a, 2), CacheResult::Unknown));
    }

    #[test]
    fn rewrite_cache_resolves_ancestor_mappings_transitively() {
        let old_parent = root(vec![int_lit(1)]);
        let old_parent_hash = RewriteCache::node_hash(&old_parent);
        let mid_parent = root(vec![int_lit(2)]);
        let final_parent = root(vec![int_lit(3)]);
        let mut cache = RewriteCache::default();

        cache.insert_from_hash(old_parent_hash, Some(mid_parent.clone()), 0, false);
        cache.insert(&mid_parent, Some(final_parent.clone()), 0, false);

        match cache.get(&old_parent, 0) {
            CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, final_parent),
            CacheResult::Unknown | CacheResult::Terminal(_) => {
                panic!("expected ancestor rewrite cache hit")
            }
        }
    }

    #[test]
    fn rewrite_cache_keeps_applied_effect_rewrites_after_context_clear() {
        let pure_from = int_lit(1);
        let pure_to = int_lit(2);
        let effect_from = int_lit(3);
        let effect_to = int_lit(4);
        let terminal = int_lit(5);
        let mut cache = RewriteCache::default();

        cache.insert(&pure_from, Some(pure_to), 0, false);
        cache.insert(&effect_from, Some(effect_to.clone()), 0, true);
        cache.insert(&terminal, None, 0, false);

        cache.clear_context_dependent();

        assert!(matches!(cache.get(&pure_from, 0), CacheResult::Unknown));
        assert!(matches!(cache.get(&terminal, 0), CacheResult::Unknown));
        match cache.get(&effect_from, 0) {
            CacheResult::Rewrite(rewritten) => {
                assert_eq!(rewritten.expr, effect_to);
                assert!(rewritten.has_applied_effect);
            }
            CacheResult::Unknown | CacheResult::Terminal(_) => {
                panic!("expected applied-effect rewrite hit")
            }
        }
    }

    #[test]
    fn rewrite_cache_propagates_applied_effect_marker_through_chains() {
        let a = int_lit(1);
        let b = int_lit(2);
        let c = int_lit(3);
        let mut cache = RewriteCache::default();

        cache.insert(&a, Some(b.clone()), 0, true);
        cache.insert(&b, Some(c.clone()), 0, false);
        cache.clear_context_dependent();

        match cache.get(&a, 0) {
            CacheResult::Rewrite(rewritten) => {
                assert_eq!(rewritten.expr, c);
                assert!(rewritten.has_applied_effect);
            }
            CacheResult::Unknown | CacheResult::Terminal(_) => {
                panic!("expected applied-effect rewrite hit")
            }
        }
        assert!(matches!(cache.get(&b, 0), CacheResult::Unknown));
    }
}
