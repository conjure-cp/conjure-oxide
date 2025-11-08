use itertools::Itertools;
use tree_morph::{helpers::select_panic, prelude::*};

use crate::{Model, bug};

use super::{RuleSet, get_rules_grouped};

/// Call the "optimized", tree-morph rewriter.
pub fn rewrite_morph<'a>(
    mut model: Model,
    rule_sets: &Vec<&'a RuleSet<'a>>,
    prop_multiple_equally_applicable: bool,
) -> Model {
    let submodel = model.as_submodel_mut();
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

    let engine = EngineBuilder::new()
        .set_selector(selector)
        .append_rule_groups(rules_grouped)
        .build();
    let (expr, symbol_table) = engine.morph(submodel.root().clone(), submodel.symbols().clone());

    *submodel.symbols_mut() = symbol_table;
    submodel.replace_root(expr);
    model
}
