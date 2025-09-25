#![doc = include_str!("docs/lib.md")]
#![warn(missing_docs)]

pub mod commands;
pub mod engine;
pub mod engine_builder;
mod events;
pub mod helpers;
pub mod rule;
mod update;

/// Re-exported functions and types for convenience.
pub mod prelude {
    pub use crate::commands::Commands;
    pub use crate::engine::Engine;
    pub use crate::engine_builder::EngineBuilder;
    pub use crate::helpers::select_first;
    pub use crate::rule::{Rule, RuleFn};
    pub use crate::rule_fns;
    pub use crate::update::Update;
}
