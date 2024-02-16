use crate::_dependencies::distributed_slice;
pub use crate::rule_set::RuleSet;

pub mod rule_set;

#[doc(hidden)]
pub mod _dependencies {
    pub use conjure_core::rule::Rule;
    pub use linkme::distributed_slice;
}

#[doc(hidden)]
#[distributed_slice]
pub static RULE_SETS_DISTRIBUTED_SLICE: [RuleSet<'static>];

pub fn get_rule_sets() -> Vec<&'static RuleSet<'static>> {
    RULE_SETS_DISTRIBUTED_SLICE.iter().collect()
}

pub fn get_rule_set_by_name(name: &str) -> Option<&'static RuleSet<'static>> {
    get_rule_sets()
        .iter()
        .find(|rule_set| rule_set.name == name)
        .cloned()
}

pub use conjure_rules_proc_macro::register_rule_set;
