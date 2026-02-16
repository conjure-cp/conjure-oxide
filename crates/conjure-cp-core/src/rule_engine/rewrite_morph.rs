use itertools::Itertools;
use tree_morph::{
    cache::{HashMapCache, NoCache, RewriteCache},
    helpers::select_panic,
    prelude::*,
};

use crate::{
    Model,
    ast::{Expression, SymbolTablePtr},
    bug,
    rule_engine::Rule,
};

use super::{RuleSet, get_rules_grouped};

pub struct MorphConfig {
    use_cache: bool,
    use_naive: bool,
}

impl MorphConfig {
    pub fn new(use_cache: bool, use_naive: bool) -> Self {
        Self {
            use_cache,
            use_naive,
        }
    }
}

impl Default for MorphConfig {
    fn default() -> Self {
        Self {
            use_cache: true,
            use_naive: false,
        }
    }
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
    morph_config: MorphConfig,
) -> Model {
    let mut model = model.clone();
    let submodel = model.as_submodel_mut();
    let mut engine = build_engine(rule_sets, prop_multiple_equally_applicable, &morph_config);

    let root = submodel.root().clone();
    let symbols = submodel.symbols_ptr_unchecked_mut().clone();

    let (expr, symbol_tabel) = engine.morph(root, symbols);

    *submodel.symbols_ptr_unchecked_mut() = symbol_tabel;
    submodel.replace_root(expr);

    model
}

fn build_engine<'a>(
    rule_sets: &Vec<&'a RuleSet<'a>>,
    prop_multiple_equally_applicable: bool,
    morph_config: &MorphConfig,
) -> Engine<Expression, SymbolTablePtr, &'a Rule<'a>, Box<dyn RewriteCache<Expression>>> {
    let rules_grouped = get_rules_grouped(rule_sets)
        .unwrap_or_else(|_| bug!("get_rule_priorities() failed!"))
        .into_iter()
        .map(|(_, rule)| rule.into_iter().map(|f| f.rule).collect_vec())
        .collect_vec();
    let selector = if prop_multiple_equally_applicable {
        select_panic
    } else {
        select_first
    };
    let cache: Box<dyn RewriteCache<Expression>> = match morph_config.use_cache {
        true => Box::new(HashMapCache::new()),
        false => Box::new(NoCache),
    };
    EngineBuilder::new()
        .set_selector(selector)
        .append_rule_groups(rules_grouped)
        .add_cacher(cache)
        .build()
}
