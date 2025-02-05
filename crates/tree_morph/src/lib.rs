#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

mod commands;
mod engine;
pub mod helpers;
mod rule;
mod update;

/// Re-exported functions and types for convenience.
pub mod prelude {
    use super::*;

    pub use crate::rule_fns;
    pub use commands::Commands;
    pub use engine::morph;
    pub use helpers::select_first;
    pub use rule::{Rule, RuleFn};
    pub use update::Update;
}

pub use commands::Commands;
pub use engine::morph;
pub use rule::{Rule, RuleFn};
pub use update::Update;
