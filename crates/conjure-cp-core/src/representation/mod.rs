pub mod registry;
mod serde;
mod store;
pub mod types;
pub mod util;

pub use conjure_cp_rule_macros::register_representation;
pub use store::ReprStore;
pub use types::ReprRule;

/// Re-exports for use by the `define_repr!` proc macro. Not part of the public API.
#[doc(hidden)]
pub mod _dependencies {
    pub use super::registry::{REPR_RULES_DISTRIBUTED_SLICE, ReprRegistryEntry};
    pub use super::serde::ReprStateSerde;
    pub use super::types::{
        LookupFn, ReprAssignment, ReprDeclLevel, ReprDomainLevel, ReprError, ReprInitResult,
        ReprRule,
    };
    pub use super::util::{ReprStateStored, instantiate_default_impl, try_up_via};
    pub use crate::ast::eval_constant;
    pub use crate::rule_engine::_dependencies::*;
    pub use funcmap;
    pub use funcmap::{FuncMap, TryFuncMap};
    pub use serde;
    pub use serde::{Deserialize, Serialize};
    pub use serde_json;
    pub use serde_with;
    pub use serde_with::{DeserializeAs, SerializeAs, serde_as};
}
