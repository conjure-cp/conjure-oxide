use super::{RewriteError, RuleSet, resolve_rules::RuleData};
use crate::{
    Model,
    ast::{Atom, Expression as Expr, Metadata, Moo, Name, discriminant_from_value},
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
    collections::{BTreeMap, BTreeSet, HashMap},
    hash::{DefaultHasher, Hash, Hasher},
    time::Instant,
};
use tracing::trace;
use uniplate::{Biplate, Uniplate};

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
    whole_model_clears_by_rule: BTreeMap<String, usize>,
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

    fn record_whole_model_clear(&mut self, rule_name: &str) {
        self.whole_model_clears_after_side_effects += 1;
        *self
            .whole_model_clears_by_rule
            .entry(rule_name.to_owned())
            .or_default() += 1;
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
        eprintln!(
            "[dirty-trace] whole_model_clears_by_rule={:?}",
            self.whole_model_clears_by_rule
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

/// Result of looking up an expression at a rewrite rule-group level.
enum CacheResult {
    /// The expression has not been cached at this level.
    Unknown,
    /// The expression is known not to rewrite through this maximum cached level.
    Terminal(usize),
    /// The expression rewrites to a cached replacement.
    Rewrite(CachedRewrite),
}

/// Cached rewrite target for a semantic expression/context key.
#[derive(Clone)]
struct CachedRewrite {
    /// Replacement expression to splice into the tree on a cache hit.
    expr: Expr,
}

/// Internal stored cache value for a level-qualified expression hash.
enum CacheEntry {
    /// No rules apply to this expression at the stored level.
    Terminal,
    /// Rules rewrite this expression to the stored replacement.
    Rewrite(CachedRewrite),
}

/// Rewrite cache keyed by expression hash, rule-group level, and symbol context hash.
///
/// Rewrite entries are transitively resolved: inserting `A -> B`, `B -> C`, then `C -> D`
/// updates the observable cache result for `A`, `B`, and `C` to `D`.
#[derive(Default)]
struct RewriteCache {
    /// Level-qualified cache map. Terminal entries are stored here for exact-level lookups.
    map: HashMap<u64, CacheEntry>,
    /// Reverse edges used to update earlier mappings when a target later rewrites again.
    predecessors: HashMap<u64, Vec<u64>>,
    /// Context-qualified terminal shortcut: a subtree clean through level N is clean for <= N.
    clean_levels: HashMap<u64, usize>,
}

impl RewriteCache {
    /// Returns the level-independent structural hash for an expression.
    ///
    /// Symbol-sensitive correctness is provided by mixing `symbol_context_hash` into
    /// [`Self::combine`], not by hashing declaration values into every node key.
    fn node_hash(expr: &Expr, _symbol_context_hash: u64) -> u64 {
        expr.get_cached_hash()
    }

    /// Combines an expression hash, rule-group level, and symbol context hash.
    fn combine(node_hash: u64, level: usize, symbol_context_hash: u64) -> u64 {
        let mut hasher = DefaultHasher::new();
        node_hash.hash(&mut hasher);
        level.hash(&mut hasher);
        symbol_context_hash.hash(&mut hasher);
        hasher.finish()
    }

    /// Returns the level-qualified cache key for an expression.
    fn key(expr: &Expr, level: usize, symbol_context_hash: u64) -> u64 {
        Self::combine(
            Self::node_hash(expr, symbol_context_hash),
            level,
            symbol_context_hash,
        )
    }

    /// Looks up a subtree at a rule-group level.
    fn get(&self, subtree: &Expr, level: usize, symbol_context_hash: u64) -> CacheResult {
        let node_hash = Self::node_hash(subtree, symbol_context_hash);
        let clean_key = Self::combine(node_hash, usize::MAX, symbol_context_hash);
        if let Some(&max_clean) = self.clean_levels.get(&clean_key)
            && max_clean >= level
        {
            return CacheResult::Terminal(max_clean);
        }

        match self
            .map
            .get(&Self::combine(node_hash, level, symbol_context_hash))
        {
            None => CacheResult::Unknown,
            Some(CacheEntry::Rewrite(rewrite)) => CacheResult::Rewrite(rewrite.clone()),
            Some(CacheEntry::Terminal) => CacheResult::Terminal(level),
        }
    }

    /// Inserts either a terminal result or a rewrite result for `from`.
    fn insert(&mut self, from: &Expr, to: Option<Expr>, level: usize, symbol_context_hash: u64) {
        self.insert_from_hash(
            Self::node_hash(from, symbol_context_hash),
            to,
            level,
            symbol_context_hash,
        );
    }

    /// Inserts using a pre-replacement source hash.
    ///
    /// This is used for ancestor mappings, where the old expression no longer exists after the
    /// zipper has rebuilt an ancestor with the replacement child.
    fn insert_from_hash(
        &mut self,
        from_hash: u64,
        to: Option<Expr>,
        level: usize,
        symbol_context_hash: u64,
    ) {
        let from_key = Self::combine(from_hash, level, symbol_context_hash);

        let Some(to_expr) = to else {
            self.map.insert(from_key, CacheEntry::Terminal);
            let clean_key = Self::combine(from_hash, usize::MAX, symbol_context_hash);
            self.clean_levels
                .entry(clean_key)
                .and_modify(|l| *l = (*l).max(level))
                .or_insert(level);
            return;
        };

        let to_key = Self::key(&to_expr, level, symbol_context_hash);
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
            },
            Some(CacheEntry::Terminal) => {
                self.map.insert(from_key, CacheEntry::Terminal);
                return;
            }
            None => CachedRewrite { expr: to_expr },
        };

        let resolved_key = Self::key(&resolved.expr, level, symbol_context_hash);
        self.map
            .insert(from_key, CacheEntry::Rewrite(resolved.clone()));

        if let Some(mut predecessors) = self.predecessors.remove(&from_key) {
            for &predecessor in &predecessors {
                self.map.insert(
                    predecessor,
                    CacheEntry::Rewrite(CachedRewrite {
                        expr: resolved.expr.clone(),
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
    /// Indexed by discriminant id for O(1) lookup (ids are small and dense; see `Rule::applicable_to`).
    rules_by_discriminant: Vec<Option<Vec<RuleData<'a>>>>,
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

        let mut rules_by_discriminant = Vec::new();
        if let Some(max_discriminant) = discriminants.iter().copied().max() {
            rules_by_discriminant.resize_with(max_discriminant + 1, || None);
        }

        for discriminant in discriminants.into_iter().unique() {
            rules_by_discriminant[discriminant] = Some(
                rules
                    .iter()
                    .filter(|rd| rule_applies_to_discriminant(rd, discriminant))
                    .cloned()
                    .collect(),
            );
        }

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
            .get(discriminant)
            .and_then(Option::as_deref)
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
    symbol_context_hash: Option<u64>,
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
            symbol_context_hash: None,
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

// Tries to rewrite the model until a full scan finds no applicable rules.
//
// Returns None if no change was made.
fn try_rewrite_model<'ctx, 'rules>(
    submodel: &mut Model,
    ctx: &mut RewritePassContext<'ctx, 'rules>,
) -> Option<()> {
    ctx.dirty_trace.passes += 1;
    if let Some(letting_name) = try_rewrite_value_letting_once(
        submodel,
        ctx.rules_grouped,
        ctx.prop_multiple_equally_applicable,
    ) {
        ctx.dirty_trace.value_letting_rewrites += 1;
        invalidate_symbol_context_caches(submodel, ctx);
        if ctx.config.dirty {
            ctx.dirty_trace.whole_model_clears_after_value_letting += 1;
            clear_clean_rule_metadata_for_name(submodel, &letting_name);
        }
        return Some(());
    }

    let mut did_rewrite = false;

    'rewrite_loop: loop {
        let mut results: Vec<ApplicableRule<'_, ExpressionZipper>> = vec![];
        let mut root_expr = Some(take_model_root(submodel));

        // Iterate over rules by priority in descending order.
        'top: for (level, rule_group) in ctx.bucketed_rules.iter().enumerate() {
            ctx.dirty_trace.priority_scans += 1;
            let scan_symbol_context_hash = ctx
                .cache
                .is_some()
                .then(|| current_symbol_context_hash(submodel, ctx));
            let mut zipper = ExpressionZipper::new(
                root_expr
                    .take()
                    .expect("rewrite scan should own the current expression root"),
            );
            loop {
                ctx.dirty_trace.expression_visits += 1;
                let expr = zipper.focus();
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

                if let Some(symbol_context_hash) = scan_symbol_context_hash {
                    let cache = ctx.cache.as_mut().expect("checked above");
                    match cache.get(expr, level, symbol_context_hash) {
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
                            apply_cache_rewrite_hit(
                                submodel,
                                ctx,
                                &zipper,
                                cached,
                                level,
                                symbol_context_hash,
                            );
                            did_rewrite = true;
                            continue 'rewrite_loop;
                        }
                        CacheResult::Unknown => {
                            ctx.dirty_trace.cache_misses += 1;
                        }
                    }
                }

                let mut attempted_rule = false;
                let results_before_expr = results.len();
                for rd in rule_group.candidates(ctx.config, expr) {
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

                    match (rd.rule.application)(expr, &submodel.symbols()) {
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
                                    expr,
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
                                    expr,
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
                if ctx.config.cache
                    && results.len() == results_before_expr
                    && let Some(symbol_context_hash) = scan_symbol_context_hash
                    && let Some(cache) = ctx.cache.as_mut()
                {
                    cache.insert(expr, None, level, symbol_context_hash);
                    ctx.dirty_trace.cache_inserts += 1;
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

            root_expr = Some(zipper.rebuild_root());
        }

        match results.as_slice() {
            [] => {
                submodel.replace_root(
                    root_expr
                        .take()
                        .expect("rewrite scan should retain the expression root"),
                );
                break;
            }
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

                let (invalidation_names, has_model_side_effects) = {
                    let symbols = submodel.symbols();
                    (
                        side_effect_invalidation_names(&result.effect, &symbols),
                        effect_has_model_side_effects(&result.effect, &symbols),
                    )
                };
                let has_new_top = !result.effect.new_top.is_empty();
                let rule_name = result.rule_data.rule.name;
                let RuleResult { effect, .. } = result;
                let crate::rule_engine::rule::RuleEffect {
                    new_expression,
                    new_top,
                    symbols,
                    new_clauses,
                    ..
                } = effect;
                let replacement = clear_expr_clean_rule_metadata(new_expression);
                let pre_effect_symbol_context_hash = ctx
                    .cache
                    .is_some()
                    .then(|| current_symbol_context_hash(submodel, ctx));

                // Replace expr with new_expression
                let cache_mapping_context = ctx
                    .config
                    .cache
                    .then_some(pre_effect_symbol_context_hash)
                    .flatten();
                let (new_root, mappings) = replace_focus_and_dirty_ancestors(
                    zipper,
                    replacement.clone(),
                    ctx.dirty_trace,
                    cache_mapping_context,
                );
                submodel.replace_root(new_root);

                // Apply new symbols and top level
                ctx.dirty_trace
                    .record_rewrite(rule_name, has_model_side_effects);
                submodel.symbols_mut().extend(symbols);
                submodel.add_constraints(new_top);
                submodel.add_clauses(new_clauses);
                if has_model_side_effects {
                    invalidate_symbol_context_caches(submodel, ctx);
                }
                if let Some(pre_effect_symbol_context_hash) = pre_effect_symbol_context_hash {
                    let cache_symbol_context_hash = if has_model_side_effects {
                        current_symbol_context_hash(submodel, ctx)
                    } else {
                        pre_effect_symbol_context_hash
                    };
                    let expr_hash = RewriteCache::node_hash(expr, cache_symbol_context_hash);
                    if let Some(cache) = ctx.cache.as_mut() {
                        cache.insert_from_hash(
                            expr_hash,
                            Some(replacement),
                            *level,
                            cache_symbol_context_hash,
                        );
                        ctx.dirty_trace.cache_inserts += 1;
                        let mapping_count = mappings.len();
                        insert_ancestor_mappings(
                            cache,
                            mappings,
                            *level,
                            cache_symbol_context_hash,
                        );
                        ctx.dirty_trace.cache_ancestor_mappings += mapping_count;
                    }
                }
                if has_model_side_effects && (ctx.config.dirty || ctx.config.cache) {
                    let mut targeted = false;
                    if !invalidation_names.is_empty() {
                        clear_clean_rule_metadata_for_names(submodel, &invalidation_names);
                        targeted = true;
                    }
                    if has_new_top {
                        clear_root_clean_rule_metadata(submodel);
                        targeted = true;
                    }
                    if !targeted {
                        ctx.dirty_trace.record_whole_model_clear(rule_name);
                        clear_model_clean_rule_metadata(submodel);
                    }
                }

                #[cfg(debug_assertions)]
                {
                    let assertion_context =
                        format!("naive rewriter after applying rule '{rule_name}'");
                    debug_assert_model_well_formed(submodel, &assertion_context);
                }

                did_rewrite = true;
                continue 'rewrite_loop;
            }
        }
    }

    did_rewrite.then_some(())
}

fn apply_cache_rewrite_hit<'ctx, 'rules>(
    submodel: &mut Model,
    ctx: &mut RewritePassContext<'ctx, 'rules>,
    zipper: &ExpressionZipper,
    cached: CachedRewrite,
    level: usize,
    symbol_context_hash: u64,
) {
    let cache = ctx.cache.as_mut().expect("cache enabled");
    let (new_root, mappings) = replace_focus_and_dirty_ancestors(
        zipper,
        cached.expr,
        ctx.dirty_trace,
        Some(symbol_context_hash),
    );
    let mapping_count = mappings.len();
    insert_ancestor_mappings(cache, mappings, level, symbol_context_hash);
    ctx.dirty_trace.cache_ancestor_mappings += mapping_count;
    submodel.replace_root(new_root);
}

/// Returns a cached hash of the symbol values visible to rule applications.
fn current_symbol_context_hash<'ctx, 'rules>(
    submodel: &Model,
    ctx: &mut RewritePassContext<'ctx, 'rules>,
) -> u64 {
    if let Some(hash) = ctx.symbol_context_hash {
        return hash;
    }

    let hash = submodel.symbols().context_hash();
    ctx.symbol_context_hash = Some(hash);
    hash
}

fn invalidate_symbol_context_caches<'ctx, 'rules>(
    submodel: &mut Model,
    ctx: &mut RewritePassContext<'ctx, 'rules>,
) {
    ctx.symbol_context_hash = None;
    submodel.symbols_mut().invalidate_context_hash_cache();
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

type AncestorCacheMappings = Vec<(u64, Expr)>;

/// Replaces the focused expression and clears rewrite metadata on the changed path to root.
///
/// When `cache_mapping_context` is `Some`, this also returns each old ancestor hash with its
/// rebuilt ancestor so future duplicate enclosing subtrees can jump directly to the rewritten form.
fn replace_focus_and_dirty_ancestors(
    zipper: &ExpressionZipper,
    new_focus: Expr,
    dirty_trace: &mut DirtyTrace,
    cache_mapping_context: Option<u64>,
) -> (Expr, AncestorCacheMappings) {
    let mut zipper = zipper.clone();
    let old_ancestor_hashes = cache_mapping_context
        .map(|symbol_context_hash| ancestor_hashes_to_root(&zipper, symbol_context_hash));
    let mut ancestor_mappings = Vec::new();
    dirty_trace.replacement_subtree_clears += 1;
    zipper.replace_focus(new_focus);

    let mut ancestor_index = 0;
    while zipper.go_up().is_some() {
        dirty_trace.ancestor_clears += 1;
        zipper.focus().meta_ref().clear_clean_rule_priority();
        zipper.focus().invalidate_cache();
        if let Some(hashes) = old_ancestor_hashes.as_ref()
            && let Some(&old_hash) = hashes.get(ancestor_index)
        {
            ancestor_mappings.push((old_hash, zipper.focus().clone()));
        }
        ancestor_index += 1;
    }

    (zipper.rebuild_root(), ancestor_mappings)
}

/// Captures ancestor content hashes before replacing the focused subtree.
fn ancestor_hashes_to_root(zipper: &ExpressionZipper, symbol_context_hash: u64) -> Vec<u64> {
    let mut zipper = zipper.clone();
    let mut hashes = Vec::new();
    while zipper.go_up().is_some() {
        hashes.push(RewriteCache::node_hash(zipper.focus(), symbol_context_hash));
    }
    hashes
}

/// Inserts old-ancestor-hash to rebuilt-ancestor mappings under one symbol context.
fn insert_ancestor_mappings(
    cache: &mut RewriteCache,
    mappings: AncestorCacheMappings,
    level: usize,
    symbol_context_hash: u64,
) {
    for (old_hash, new_ancestor) in mappings {
        cache.insert_from_hash(old_hash, Some(new_ancestor), level, symbol_context_hash);
    }
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
    let root = take_model_root(model);
    model.replace_root(clear_expr_clean_rule_metadata(root));
}

/// Clears clean-rule metadata only in subtrees that reference a changed letting.
fn clear_clean_rule_metadata_for_name(model: &mut Model, name: &Name) {
    let root = take_model_root(model);
    model.replace_root(clear_expr_clean_rule_metadata_for_name(root, name));
}

/// Clears clean-rule metadata only in subtrees that reference one of the given symbols.
fn clear_clean_rule_metadata_for_names(model: &mut Model, names: &[Name]) {
    if names.is_empty() {
        return;
    }
    let root = take_model_root(model);
    model.replace_root(clear_expr_clean_rule_metadata_for_names(root, names));
}

fn clear_root_clean_rule_metadata(model: &mut Model) {
    let root = take_model_root(model);
    let cleared = match root {
        Expr::Root(metadata, constraints) => {
            metadata.clear_clean_rule_priority();
            let root = Expr::Root(metadata, constraints);
            root.invalidate_cache();
            root
        }
        other => other,
    };
    model.replace_root(cleared);
}

fn take_model_root(model: &mut Model) -> Expr {
    model.replace_root(Expr::Root(Metadata::new(), Vec::new()))
}

fn side_effect_invalidation_names(
    effect: &crate::rule_engine::rule::RuleEffect,
    symbols: &crate::ast::SymbolTable,
) -> Vec<Name> {
    let mut names: BTreeSet<Name> = effect.added_symbols(symbols);
    names.extend(
        effect
            .changed_symbols(symbols)
            .into_iter()
            .map(|(name, _, _)| name),
    );
    names.into_iter().collect()
}

fn clear_expr_clean_rule_metadata_for_name(expr: Expr, name: &Name) -> Expr {
    clear_expr_clean_rule_metadata_for_names(expr, std::slice::from_ref(name))
}

fn clear_expr_clean_rule_metadata_for_names(expr: Expr, names: &[Name]) -> Expr {
    if !subtree_references_any(&expr, names) {
        return expr;
    }

    match expr {
        Expr::Root(metadata, constraints) => {
            metadata.clear_clean_rule_priority();
            let constraints = constraints
                .into_iter()
                .map(|child| clear_expr_clean_rule_metadata_for_names(child, names))
                .collect();
            let root = Expr::Root(metadata, constraints);
            root.invalidate_cache();
            root
        }
        Expr::Eq(metadata, left, right) => {
            metadata.clear_clean_rule_priority();
            let left = clear_expr_clean_rule_metadata_for_names(left.as_ref().clone(), names);
            let right = clear_expr_clean_rule_metadata_for_names(right.as_ref().clone(), names);
            let eq = Expr::Eq(metadata, Moo::new(left), Moo::new(right));
            eq.invalidate_cache();
            eq
        }
        Expr::Sum(metadata, matrix) => {
            metadata.clear_clean_rule_priority();
            let matrix = clear_expr_clean_rule_metadata_for_names(matrix.as_ref().clone(), names);
            let sum = Expr::Sum(metadata, Moo::new(matrix));
            sum.invalidate_cache();
            sum
        }
        other => clear_expr_clean_rule_metadata(other),
    }
}

fn subtree_references_any(expr: &Expr, names: &[Name]) -> bool {
    names.iter().any(|name| subtree_references_name(expr, name))
}

fn subtree_references_name(expr: &Expr, name: &Name) -> bool {
    expr.universe().into_iter().any(|subexpr| {
        matches!(
            subexpr,
            Expr::Atomic(_, Atom::Reference(reference)) if &*reference.name() == name
        )
    })
}

fn effect_has_model_side_effects(
    effect: &crate::rule_engine::rule::RuleEffect,
    model_symbols: &crate::ast::SymbolTable,
) -> bool {
    !effect.new_top.is_empty()
        || !effect.new_clauses.is_empty()
        || !effect.added_symbols(model_symbols).is_empty()
        || !effect.changed_symbols(model_symbols).is_empty()
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
    use crate::ast::{Atom, DeclarationPtr, Literal, Moo};
    use crate::matrix_expr;
    use crate::rule_engine::expression_zipper::ExpressionZipper;

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
        let context = 10;

        cache.insert(&a, Some(b.clone()), 0, context);
        cache.insert(&b, Some(c.clone()), 0, context);
        cache.insert(&c, Some(d.clone()), 0, context);

        for expr in [&a, &b, &c] {
            match cache.get(expr, 0, context) {
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
        let context = 10;

        cache.insert(&a, None, 0, context);
        cache.insert(&a, None, 1, context);

        match cache.get(&a, 0, context) {
            CacheResult::Terminal(level) => assert_eq!(level, 1),
            CacheResult::Unknown | CacheResult::Rewrite(_) => panic!("expected terminal hit"),
        }
        match cache.get(&a, 1, context) {
            CacheResult::Terminal(level) => assert_eq!(level, 1),
            CacheResult::Unknown | CacheResult::Rewrite(_) => panic!("expected terminal hit"),
        }
        assert!(matches!(cache.get(&a, 2, context), CacheResult::Unknown));
    }

    #[test]
    fn rewrite_cache_resolves_ancestor_mappings_transitively() {
        let old_parent = root(vec![int_lit(1)]);
        let mid_parent = root(vec![int_lit(2)]);
        let final_parent = root(vec![int_lit(3)]);
        let mut cache = RewriteCache::default();
        let context = 10;
        let old_parent_hash = RewriteCache::node_hash(&old_parent, context);

        cache.insert_from_hash(old_parent_hash, Some(mid_parent.clone()), 0, context);
        cache.insert(&mid_parent, Some(final_parent.clone()), 0, context);

        match cache.get(&old_parent, 0, context) {
            CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, final_parent),
            CacheResult::Unknown | CacheResult::Terminal(_) => {
                panic!("expected ancestor rewrite cache hit")
            }
        }
    }

    #[test]
    fn rewrite_cache_separates_symbol_contexts() {
        let from = int_lit(1);
        let to = int_lit(2);
        let terminal = int_lit(3);
        let mut cache = RewriteCache::default();
        let old_context = 10;
        let new_context = 20;

        cache.insert(&from, Some(to.clone()), 0, old_context);
        cache.insert(&terminal, None, 0, old_context);

        match cache.get(&from, 0, old_context) {
            CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, to),
            CacheResult::Unknown | CacheResult::Terminal(_) => {
                panic!("expected rewrite hit in original context")
            }
        }
        assert!(matches!(
            cache.get(&from, 0, new_context),
            CacheResult::Unknown
        ));
        assert!(matches!(
            cache.get(&terminal, 0, new_context),
            CacheResult::Unknown
        ));
    }

    #[test]
    fn rewrite_cache_resolves_chains_within_one_symbol_context() {
        let a = int_lit(1);
        let b = int_lit(2);
        let c = int_lit(3);
        let mut cache = RewriteCache::default();
        let old_context = 10;
        let new_context = 20;

        cache.insert(&a, Some(b.clone()), 0, old_context);
        cache.insert(&b, Some(c.clone()), 0, old_context);

        match cache.get(&a, 0, old_context) {
            CacheResult::Rewrite(rewritten) => assert_eq!(rewritten.expr, c),
            CacheResult::Unknown | CacheResult::Terminal(_) => {
                panic!("expected rewrite hit in original context")
            }
        }
        assert!(matches!(
            cache.get(&a, 0, new_context),
            CacheResult::Unknown
        ));
    }

    fn assert_clean_at(expr: &Expr, priority: u16) {
        assert!(
            expr.meta_ref().is_clean_for_rule_priority(priority),
            "expected expression {expr} to remain clean at priority {priority}"
        );
    }

    fn assert_not_clean_at(expr: &Expr, priority: u16) {
        assert!(
            !expr.meta_ref().is_clean_for_rule_priority(priority),
            "expected expression {expr} to require re-check from the top at priority {priority}"
        );
    }

    /// After a rewrite, only the replaced node and its ancestors are invalidated; siblings keep
    /// their clean marks and must not be re-scanned from the top.
    #[test]
    fn rewrite_dirty_invalidation_preserves_root_sibling_clean_marks() {
        let priority = 5u16;

        let sib2 = int_lit(2);
        let sib3 = int_lit(3);
        sib2.meta_ref().mark_clean_for_rule_priority(priority);
        sib3.meta_ref().mark_clean_for_rule_priority(priority);

        let tree = root(vec![int_lit(1), sib2, sib3]);
        let mut zipper = ExpressionZipper::new(tree);
        assert!(zipper.go_down().is_some());

        let mut dirty_trace = DirtyTrace::default();
        let (new_root, _) = replace_focus_and_dirty_ancestors(
            &zipper,
            clear_expr_clean_rule_metadata(int_lit(10)),
            &mut dirty_trace,
            None,
        );

        let Expr::Root(_, constraints) = &new_root else {
            panic!("expected root expression");
        };

        assert_not_clean_at(&new_root, priority);
        assert_not_clean_at(&constraints[0], priority);
        assert_clean_at(&constraints[1], priority);
        assert_clean_at(&constraints[2], priority);
    }

    /// Invalidation walks up the parent chain only; the other side of a binary node is a sibling.
    #[test]
    fn rewrite_dirty_invalidation_preserves_binary_sibling_clean_marks() {
        let priority = 5u16;

        let right = int_lit(2);
        right.meta_ref().mark_clean_for_rule_priority(priority);
        let eq = Expr::Eq(Metadata::new(), Moo::new(int_lit(1)), Moo::new(right));
        let tree = root(vec![eq]);

        let mut zipper = ExpressionZipper::new(tree);
        assert!(zipper.go_down().is_some());
        assert!(zipper.go_down().is_some());

        let mut dirty_trace = DirtyTrace::default();
        let (new_root, _) = replace_focus_and_dirty_ancestors(
            &zipper,
            clear_expr_clean_rule_metadata(int_lit(10)),
            &mut dirty_trace,
            None,
        );

        let Expr::Root(_, constraints) = &new_root else {
            panic!("expected root expression");
        };
        let Expr::Eq(_, _, right) = &constraints[0] else {
            panic!("expected equality at root child");
        };

        assert_not_clean_at(&new_root, priority);
        assert_not_clean_at(&constraints[0], priority);
        assert_clean_at(right.as_ref(), priority);
    }

    /// Cousins inside a shared parent container must also keep their clean marks.
    #[test]
    fn rewrite_dirty_invalidation_preserves_matrix_sibling_clean_marks() {
        let priority = 5u16;

        let sibling = int_lit(2);
        sibling.meta_ref().mark_clean_for_rule_priority(priority);
        let sum = Expr::Sum(Metadata::new(), Moo::new(matrix_expr![int_lit(1), sibling]));
        let tree = root(vec![sum]);

        let mut zipper = ExpressionZipper::new(tree);
        assert!(zipper.go_down().is_some());
        assert!(zipper.go_down().is_some());
        assert!(zipper.go_down().is_some());

        let mut dirty_trace = DirtyTrace::default();
        let (new_root, _) = replace_focus_and_dirty_ancestors(
            &zipper,
            clear_expr_clean_rule_metadata(int_lit(10)),
            &mut dirty_trace,
            None,
        );

        let Expr::Root(_, constraints) = &new_root else {
            panic!("expected root expression");
        };
        let Expr::Sum(_, matrix) = &constraints[0] else {
            panic!("expected sum at root child");
        };
        let Expr::AbstractLiteral(_, matrix_lit) = matrix.as_ref() else {
            panic!("expected matrix literal in sum");
        };
        let crate::ast::AbstractLiteral::Matrix(elements, _) = matrix_lit else {
            panic!("expected matrix literal");
        };

        assert_not_clean_at(&new_root, priority);
        assert_not_clean_at(&constraints[0], priority);
        assert_not_clean_at(&elements[0], priority);
        assert_clean_at(&elements[1], priority);
    }

    #[test]
    fn targeted_symbol_invalidation_preserves_unrelated_sibling_clean_marks() {
        use crate::ast::{Domain, Range, Reference};

        let priority = 5u16;
        let unrelated = int_lit(2);
        unrelated.meta_ref().mark_clean_for_rule_priority(priority);

        let x = Name::user("x");
        let ref_x = Expr::Atomic(
            Metadata::new(),
            Atom::Reference(Reference::new(DeclarationPtr::new_find(
                x.clone(),
                Domain::int(vec![Range::Bounded(1, 3)]),
            ))),
        );
        ref_x.meta_ref().mark_clean_for_rule_priority(priority);

        let tree = root(vec![ref_x, unrelated]);
        let cleared = clear_expr_clean_rule_metadata_for_names(tree, std::slice::from_ref(&x));

        let Expr::Root(_, constraints) = cleared else {
            panic!("expected root expression");
        };

        assert_not_clean_at(&constraints[0], priority);
        assert_clean_at(&constraints[1], priority);
    }
}
