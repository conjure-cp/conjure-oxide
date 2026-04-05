use crate::{
    Model,
    ast::{Expression, SymbolTable, discriminant_from_value},
    bug,
    settings::{MorphCachingStrategy, MorphConfig, Rewriter, set_current_rewriter},
};
use itertools::Itertools;
use tracing::trace;
use tree_morph::{
    cache::{CachedHashMapCache, HashMapCache, NoCache, RewriteCache, StdHashKey},
    helpers::select_panic,
    prelude::*,
};

use super::{
    RuleData, RuleSet, get_rules_grouped, rewriter_common::try_rewrite_value_letting_once,
};

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
        target: "rule_engine_rule_trace",
        "Model before rewriting:\n\n{}\n--\n",
        model
    );

    let rules_grouped = get_rules_grouped(rule_sets)
        .unwrap_or_else(|_| bug!("get_rule_priorities() failed!"))
        .into_iter()
        .collect_vec();

    let mut engine = build_engine(&rules_grouped, prop_multiple_equally_applicable, config);
    let model_ref = &mut model;

    loop {
        if try_rewrite_value_letting_once(
            model_ref,
            &rules_grouped,
            prop_multiple_equally_applicable,
        )
        .is_some()
        {
            continue;
        }

        let (expr, symbol_table) = if config.naive {
            engine.morph_naive(model_ref.root().clone(), model_ref.symbols().clone())
        } else {
            engine.morph(model_ref.root().clone(), model_ref.symbols().clone())
        };

        *model_ref.symbols_mut() = symbol_table;
        model_ref.replace_root(expr);

        if try_rewrite_value_letting_once(
            model_ref,
            &rules_grouped,
            prop_multiple_equally_applicable,
        )
        .is_none()
        {
            break;
        }
    }

    trace!(
        target: "rule_engine_rule_trace",
        "Final model:\n\n{}",
        model
    );

    model
}

fn build_engine<'a>(
    rules_grouped: &Vec<(u16, Vec<RuleData<'a>>)>,
    prop_multiple_equally_applicable: bool,
    config: MorphConfig,
) -> Engine<Expression, SymbolTable, RuleData<'a>, Box<dyn RewriteCache<Expression>>> {
    let morph_rule_groups = rules_grouped
        .iter()
        .map(|(_, rules)| rules.clone())
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
        .append_rule_groups(morph_rule_groups)
        .add_cacher(cache)
        .set_discriminant_fn(if config.prefilter {
            Some(discriminant_from_value)
        } else {
            None
        })
        .set_parallel(false)
        .set_fixedpoint(config.fixedpoint)
        // .add_down_predicate(|node| ! matches!(node, Expression::Comprehension(_, _)))
        .build()
}
