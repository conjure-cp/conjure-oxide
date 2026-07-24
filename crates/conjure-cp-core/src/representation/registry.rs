use super::stored::ReprRuleStored;
use crate::bug;
use crate::rule_engine::distributed_slice;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub type ReprRulePtr = &'static dyn ReprRuleStored;

#[doc(hidden)]
#[distributed_slice]
pub static REPR_RULES_DISTRIBUTED_SLICE: [ReprRulePtr];

pub fn get_repr_rules() -> impl Iterator<Item = ReprRulePtr> {
    REPR_RULES_DISTRIBUTED_SLICE.iter().copied()
}

pub fn get_repr_by_name(name: &str) -> Option<ReprRulePtr> {
    REPR_RULES_DISTRIBUTED_SLICE
        .iter()
        .copied()
        .find(|rule| rule.name() == name)
}

/// Look up a representation rule by its Essence short name (e.g. `"packed"`, `"occurrence"`).
///
/// If multiple rules share the same short name (e.g. set and tuple both use `"packed"`), the first
/// registration wins. Prefer [`get_applicable_repr_by_short_name`] when a declaration is available.
pub fn get_repr_by_short_name(short_name: &str) -> Option<ReprRulePtr> {
    REPR_RULES_DISTRIBUTED_SLICE
        .iter()
        .copied()
        .find(|rule| rule.short_name() == short_name)
}

/// Look up a representation by short name that is applicable to `decl`.
///
/// Disambiguates shared short names such as `"packed"` (set vs tuple) via [`ReprRuleStored::probe_for`].
pub fn get_applicable_repr_by_short_name(
    decl: &crate::ast::DeclarationPtr,
    short_name: &str,
) -> Option<ReprRulePtr> {
    REPR_RULES_DISTRIBUTED_SLICE
        .iter()
        .copied()
        .find(|rule| rule.short_name() == short_name && rule.probe_for(decl).is_ok())
}

impl Serialize for ReprRulePtr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.name().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ReprRulePtr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let name = String::deserialize(deserializer)?;
        let res = get_repr_by_name(&name)
            .unwrap_or_else(|| bug!("Unknown representation rule: {}", name));
        Ok(res)
    }
}
