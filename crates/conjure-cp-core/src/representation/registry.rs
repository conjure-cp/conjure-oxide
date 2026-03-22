use super::stored::ReprRuleStored;
use crate::bug;
use crate::rule_engine::distributed_slice;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub type ReprRulePtr = &'static dyn ReprRuleStored;

#[doc(hidden)]
#[distributed_slice]
pub static REPR_RULES_DISTRIBUTED_SLICE: [ReprRulePtr];

pub fn get_repr_by_name(name: &str) -> Option<ReprRulePtr> {
    let mut idx = -1;
    for i in 0..REPR_RULES_DISTRIBUTED_SLICE.len() {
        if REPR_RULES_DISTRIBUTED_SLICE[i].name() == name {
            idx = i as i32;
            break;
        }
    }
    if idx >= 0 {
        Some(REPR_RULES_DISTRIBUTED_SLICE[idx as usize])
    } else {
        None
    }
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
