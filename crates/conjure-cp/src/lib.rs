pub use conjure_cp_core::*;
pub use conjure_cp_essence_macros::*;

/// Essence parsers.
pub mod parse {
    /// Parse Essence using conjure's ast-json output.
    pub use conjure_cp_core::parse as conjure_json;
    /// Parse Essence using the `tree-sitter-essence` tree-sitter grammar.
    #[doc(inline)]
    pub use conjure_cp_essence_parser as tree_sitter;
}

pub mod defaults;
