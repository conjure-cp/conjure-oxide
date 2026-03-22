use crate::representation::ReprRule;
use crate::rule_engine::distributed_slice;
use conjure_cp_core::ast::DeclarationPtr;
use conjure_cp_core::representation::types::ReprInitResult;
use conjure_cp_core::representation::util::ReprStateStored;
use derivative::Derivative;
use parking_lot::MappedRwLockReadGuard;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;

#[doc(hidden)]
#[distributed_slice]
pub static REPR_RULES_DISTRIBUTED_SLICE: [ReprRegistryEntry];

type DeserializeFn = fn(serde_json::Value) -> Box<dyn ReprStateStored>;
type InitFn = fn(&mut DeclarationPtr) -> ReprInitResult;

#[derive(Clone, Copy, Derivative)]
#[derivative(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReprRegistryEntry {
    pub name: &'static str,
    #[derivative(
        Debug = "ignore",
        PartialEq = "ignore",
        PartialOrd = "ignore",
        Ord = "ignore",
        Hash = "ignore"
    )]
    pub(super) deserialize_state: DeserializeFn,
    #[derivative(
        Debug = "ignore",
        PartialEq = "ignore",
        PartialOrd = "ignore",
        Ord = "ignore",
        Hash = "ignore"
    )]
    init_for: InitFn,
}

impl ReprRegistryEntry {
    pub fn init_for(&self, decl: &mut DeclarationPtr) -> ReprInitResult {
        (self.init_for)(decl)
    }
    pub fn get_for<'a>(
        &self,
        decl: &'a DeclarationPtr,
    ) -> Option<MappedRwLockReadGuard<'a, dyn ReprStateStored>> {
        MappedRwLockReadGuard::try_map(decl.reprs(), |r| r.get_by_name(self.name)).ok()
    }
    pub const fn from_rule<T: ReprRule + ?Sized>() -> ReprRegistryEntry {
        let deserialize_state: DeserializeFn = |val| {
            let res = T::DeclLevel::deserialize(val).expect("deserializer to succeed");
            Box::new(res)
        };
        let init_for: InitFn = T::init_for;
        Self {
            name: T::NAME,
            deserialize_state,
            init_for,
        }
    }
}

pub fn get_repr_by_name(name: &str) -> Option<&'static ReprRegistryEntry> {
    let mut idx = -1;
    for i in 0..REPR_RULES_DISTRIBUTED_SLICE.len() {
        if REPR_RULES_DISTRIBUTED_SLICE[i].name == name {
            idx = i as i32;
            break;
        }
    }
    if idx >= 0 {
        Some(&REPR_RULES_DISTRIBUTED_SLICE[idx as usize])
    } else {
        None
    }
}

impl Serialize for ReprRegistryEntry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.name.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for &'static ReprRegistryEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let name = String::deserialize(deserializer)?;
        get_repr_by_name(&name)
            .ok_or_else(|| serde::de::Error::custom(format!("Unknown representation '{}'", name)))
    }
}

impl<'de> Deserialize<'de> for ReprRegistryEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let res: &'static ReprRegistryEntry = Deserialize::deserialize(deserializer)?;
        Ok(res.clone())
    }
}
