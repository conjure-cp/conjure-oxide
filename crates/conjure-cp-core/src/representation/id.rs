use std::fmt::{Display, Formatter};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::registry::get_repr_by_name;

/// The identity of a representation rule.
///
/// Identity is the rule's [`NAME`](super::ReprRule::NAME) -- its Rust type name, which is unique
/// across the registry. A representation's *short* name is not unique: nine representations are
/// called `packed`, four `components`, four `occurrence`. Wrapping the identity in its own type
/// keeps a short name from being used as one by accident, which has silently broken rules before.
///
/// The short name travels along for display, so rendering a represented name costs no lookup.
#[derive(Clone, Copy, Debug)]
pub struct ReprId {
    name: &'static str,
    short_name: &'static str,
}

impl ReprId {
    /// Builds an id from a rule's name and short name.
    ///
    /// Prefer [`ReprRule::id`](super::ReprRule::id) or [`ReprRuleStored::id`](super::ReprRuleStored::id)
    /// over calling this directly; those cannot disagree with the registry.
    pub const fn new(name: &'static str, short_name: &'static str) -> Self {
        ReprId { name, short_name }
    }

    /// The unique name identifying this representation.
    pub fn name(self) -> &'static str {
        self.name
    }

    /// The Essence-facing name, which several representations may share.
    pub fn short_name(self) -> &'static str {
        self.short_name
    }
}

impl PartialEq for ReprId {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for ReprId {}

impl std::hash::Hash for ReprId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl PartialOrd for ReprId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ReprId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(other.name)
    }
}

/// Displays the short name, which is what Essence and represented variable names use.
impl Display for ReprId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short_name)
    }
}

/// Serialises as the unique name, so stored models survive a short-name change.
impl Serialize for ReprId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name)
    }
}

impl<'de> Deserialize<'de> for ReprId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        get_repr_by_name(&name)
            .map(|rule| rule.id())
            .ok_or_else(|| serde::de::Error::custom(format!("unknown representation `{name}`")))
    }
}

impl polyquine::Quine for ReprId {
    fn ctor_tokens(&self) -> proc_macro2::TokenStream {
        let name = self.name;
        quote::quote! {
            conjure_cp::representation::get_repr_by_name(#name)
                .expect("representation should be registered")
                .id()
        }
    }
}
