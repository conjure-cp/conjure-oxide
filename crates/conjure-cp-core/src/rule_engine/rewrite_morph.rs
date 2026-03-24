use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    Model,
    ast::{Expression, SymbolTable, discriminant_from_value},
    bug,
    settings::{
        MorphCachingStrategy, MorphConfig, Rewriter,
        comprehension_expander, current_parser, current_rewriter,
        minion_discrete_threshold,
        set_comprehension_expander, set_current_parser, set_current_rewriter,
        set_minion_discrete_threshold,
    },
};
use itertools::Itertools;
use tracing::trace;
use tree_morph::{
    cache::{CachedHashMapCache, HashMapCache, NoCache, RewriteCache, StdHashKey},
    helpers::select_panic,
    prelude::*,
};

use super::{RuleData, RuleSet, get_rules_grouped};

/// Counts how many times each rule has been checked (attempted) during rewriting.
static RULE_CHECK_COUNTS: LazyLock<Mutex<HashMap<String, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static CACHE_MISSES: AtomicU64 = AtomicU64::new(0);

/// Returns a snapshot of how many times each rule has been checked.
pub fn get_rule_check_counts() -> HashMap<String, usize> {
    RULE_CHECK_COUNTS.lock().unwrap().clone()
}

/// Resets the rule check counts to zero.
pub fn reset_rule_check_counts() {
    RULE_CHECK_COUNTS.lock().unwrap().clear();
}

/// Returns (hits, misses) for the cache.
pub fn get_cache_stats() -> (u64, u64) {
    (CACHE_HITS.load(Ordering::Relaxed), CACHE_MISSES.load(Ordering::Relaxed))
}

/// Resets cache hit/miss counters.
pub fn reset_cache_stats() {
    CACHE_HITS.store(0, Ordering::Relaxed);
    CACHE_MISSES.store(0, Ordering::Relaxed);
}

fn count_rule_check(_: &Expression, _: &mut SymbolTable, rule: &RuleData<'_>) {
    if let Ok(mut counts) = RULE_CHECK_COUNTS.lock() {
        *counts.entry(rule.name().to_string()).or_insert(0) += 1;
    }
}

fn count_cache_hit(_: &Expression, _: &mut SymbolTable) {
    CACHE_HITS.fetch_add(1, Ordering::Relaxed);
}

fn count_cache_miss(_: &Expression, _: &mut SymbolTable) {
    CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
}

fn print_rule_check_counts() {
    let counts = RULE_CHECK_COUNTS.lock().unwrap();
    let mut entries: Vec<(&String, &usize)> = counts.iter().collect();
    entries.sort_by_key(|(name, _)| *name);
    println!("Rule check counts:");
    for (name, count) in entries {
        println!("  {name}: {count}");
    }
}

fn print_cache_stats() {
    let (hits, misses) = get_cache_stats();
    let total = hits + misses;
    let rate = if total > 0 { hits as f64 / total as f64 * 100.0 } else { 0.0 };
    println!("Cache stats: {hits} hits, {misses} misses, {rate:.1}% hit rate");
}

/// Rewrites a `Model` by applying rule sets using an optimized, tree-morphing rewriter.
///
/// This function traverses the expression tree of the model and applies the given rules
/// to transform it. It operates on the model's internal structure, replacing the root
/// expression and updating the symbol table based on the transformations performed by the
/// `morph` function.
///
/// # Parameters
///
/// - `model`: The `Model` to be rewritten. It is consumed and a new, transformed version is returned.
/// - `rule_sets`: A vector of `RuleSet` references containing the rules for transformation. These rules are grouped by priority before being applied.
/// - `prop_multiple_equally_applicable`: A boolean flag to control behavior when multiple rules of the same priority can be applied to the same expression.
///   - If `true`, the rewriter will use a selection strategy (`select_panic`) that panics.
///   - If `false`, the rewriter will use a selection strategy (`select_first`) that simply picks the first applicable rule it encounters.
///   TODO: CHANGE
/// - `variant`: The `MorphVariant` selecting cache and traversal behaviour:
///   - `NoCache` → no cache, standard traversal
///   - `Cache` → `HashMapCache`, standard traversal
///   - `Hashcache` → `CachedHashMapCache`, standard traversal
///   - `Naive` → no cache, naive traversal
///
/// # Returns
///
/// The rewritten `Model` after all applicable rules have been applied.
///
/// # Panics
///
/// This function will panic under two conditions:
/// - If the internal grouping of rules by priority fails (from `get_rules_grouped`).
/// - If `prop_multiple_equally_applicable` is set to `true` and more than one rule of the same priority can be applied to the same expression.
pub fn rewrite_morph<'a>(
    mut model: Model,
    rule_sets: &Vec<&'a RuleSet<'a>>,
    prop_multiple_equally_applicable: bool,
    config: MorphConfig,
) -> Model {
    set_current_rewriter(Rewriter::Morph(config));

    trace!(
        target: "rule_engine_human",
        "Model before rewriting:\n\n{}\n--\n",
        model
    );

    if config.parallel {
        // Propagate thread-local settings to Rayon worker threads so parallel rule
        // checking can access them.
        let tl_parser = current_parser();
        let tl_rewriter = current_rewriter();
        let tl_expander = comprehension_expander();
        let tl_threshold = minion_discrete_threshold();
        rayon::broadcast(|_| {
            set_current_parser(tl_parser);
            set_current_rewriter(tl_rewriter);
            set_comprehension_expander(tl_expander);
            set_minion_discrete_threshold(tl_threshold);
        });
    }

    let model_ref = &mut model;
    let mut engine = build_engine(rule_sets, prop_multiple_equally_applicable, config);

    let (expr, symbol_table) = if config.naive {
        engine.morph_naive(model_ref.root().clone(), model_ref.symbols().clone())
    } else {
        engine.morph(model_ref.root().clone(), model_ref.symbols().clone())
    };

    *model_ref.symbols_mut() = symbol_table;
    model_ref.replace_root(expr);

    print_rule_check_counts();
    print_cache_stats();

    trace!(
        target: "rule_engine_human",
        "Final model:\n\n{}",
        model
    );

    model
}

fn build_engine<'a>(
    rule_sets: &Vec<&'a RuleSet<'a>>,
    prop_multiple_equally_applicable: bool,
    config: MorphConfig,
) -> Engine<Expression, SymbolTable, RuleData<'a>, Box<dyn RewriteCache<Expression>>> {
    let rules_grouped = get_rules_grouped(rule_sets)
        .unwrap_or_else(|_| bug!("get_rule_priorities() failed!"))
        .into_iter()
        .map(|(_, rules)| rules)
        .collect_vec();
    let selector = if prop_multiple_equally_applicable {
        select_panic
    } else {
        select_first
    };

    let cache: Box<dyn RewriteCache<Expression>> = match config.cache {
        MorphCachingStrategy::NoCache => Box::new(NoCache),
        MorphCachingStrategy::Cache => Box::new(HashMapCache::<_, StdHashKey>::new()),
        MorphCachingStrategy::IncrementalCache => Box::new(CachedHashMapCache::new()),
    };

    EngineBuilder::new()
        .set_selector(selector)
        .append_rule_groups(rules_grouped)
        .add_cacher(cache)
        .set_discriminant_fn(if config.prefilter {
            Some(discriminant_from_value)
        } else {
            None
        })
        .add_before_rule(count_rule_check)
        .add_on_cache_hit(count_cache_hit)
        .add_on_cache_miss(count_cache_miss)
        .set_parallel(config.parallel)
        .set_faster(config.faster)
        .build()
}
