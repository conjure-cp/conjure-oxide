use super::registry::get_repr_by_name;
use super::stored::{ReprRuleStored, ReprStateStored};
use super::types::ReprRule;
use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;

pub struct ReprStore {
    inner: HashMap<&'static str, Box<dyn ReprStateStored>>,
}

impl Debug for ReprStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ReprStore")
    }
}

impl Clone for ReprStore {
    fn clone(&self) -> Self {
        Self {
            inner: self
                .inner
                .iter()
                .map(|(&k, v)| (k, v.clone_box()))
                .collect(),
        }
    }
}

impl PartialEq for ReprStore {
    fn eq(&self, other: &Self) -> bool {
        self.inner
            .keys()
            .zip(other.inner.keys())
            .all(|(k1, k2)| k1 == k2)
    }
}

impl Eq for ReprStore {}

impl Default for ReprStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ReprStore {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn has<T: ReprRule + ?Sized>(&self) -> bool {
        self.get::<T>().is_some()
    }

    pub fn get<T: ReprRule + ?Sized>(&self) -> Option<&T::DeclLevel> {
        self.inner
            .get(T::NAME)
            .and_then(|x| x.as_any().downcast_ref())
    }

    pub fn get_mut<T: ReprRule + ?Sized>(&mut self) -> Option<&mut T::DeclLevel> {
        self.inner
            .get_mut(T::NAME)
            .and_then(|x| x.as_any_mut().downcast_mut())
    }

    pub fn get_by_rule(&self, rule: &dyn ReprRuleStored) -> Option<&dyn ReprStateStored> {
        self.inner.get(rule.name()).map(AsRef::as_ref)
    }

    pub fn has_repr(&self, rule: &dyn ReprRuleStored) -> bool {
        self.get_by_rule(rule).is_some()
    }

    pub fn put<T: ReprRule + ?Sized>(&mut self, value: T::DeclLevel) {
        self.inner.insert(T::NAME, Box::new(value));
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &dyn ReprStateStored)> {
        self.inner.iter().map(|(k, v)| (*k, v.as_ref()))
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Serialize for ReprStore {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.inner.len()))?;
        for (&name, state) in &self.inner {
            let value = state.serialise().map_err(serde::ser::Error::custom)?;
            map.serialize_entry(name, &value)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for ReprStore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ReprStoreVisitor;

        impl<'de> Visitor<'de> for ReprStoreVisitor {
            type Value = ReprStore;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map of repr rule names to their serialized states")
            }

            fn visit_map<M>(self, mut access: M) -> Result<ReprStore, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut inner = HashMap::new();

                while let Some((key, value)) = access.next_entry::<String, serde_json::Value>()? {
                    let repr = get_repr_by_name(&key).ok_or_else(|| {
                        serde::de::Error::custom(format!(
                            "unknown repr rule '{}'; was it registered?",
                            key
                        ))
                    })?;
                    let state = repr
                        .deserialize_state(value)
                        .map_err(serde::de::Error::custom)?;
                    inner.insert(repr.name(), state);
                }

                Ok(ReprStore { inner })
            }
        }

        deserializer.deserialize_map(ReprStoreVisitor)
    }
}
