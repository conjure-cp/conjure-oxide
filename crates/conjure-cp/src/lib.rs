#![doc = include_str!("../README.md")]

pub use conjure_cp_core::*;
pub use conjure_cp_essence_macros::*;

/// Essence parsers.
pub mod parse {
    /// Parse Essence using the `tree-sitter-essence` tree-sitter grammar.
    #[doc(inline)]
    pub use conjure_cp_essence_parser as tree_sitter;
}

pub mod defaults;
/// Validation of the optional external Conjure executable.
pub mod find_conjure;

/// Configured parsing, rewriting and solving for library callers.
pub mod pipeline;
/// Solution collection and reconstruction.
pub mod solutions;
