//! Rule registry for conjure_oxide.
//! TODO: doc comment.
//!

// Why all the re-exports and wierdness?
// ============================
//
// Procedural macros are unhygenic - they directly subsitute into source code, and do not have
// their own scope, imports, and so on.
//
// See [https://doc.rust-lang.org/reference/procedural-macros.html#procedural-macro-hygiene].
//
// Therefore, we cannot assume the user has any dependencies apart from the one they imported the
// macro from. (Also, note Rust does not bring transitive dependencies into scope, so we cannot
// assume the presence of a dependency of the crate.)
//
// To solve this, the crate the macro is in must re-export everything the macro needs to run.
//
// However, proc-macro crates can only export proc-macros. Therefore, we must use a "front end
// crate" (i.e. this one) to re-export both the macro and all the things it may need.

use conjure_core::rule::Rule;
use linkme::distributed_slice;

#[doc(hidden)]
pub mod _dependencies {
    pub use conjure_core::rule::Rule;
    pub use linkme::distributed_slice;
}

#[doc(hidden)]
#[distributed_slice]
pub static RULES_DISTRIBUTED_SLICE: [Rule<'static>];

pub fn get_rules() -> Vec<Rule<'static>> {
    RULES_DISTRIBUTED_SLICE.to_vec()
}

/// TODO: docs
pub use conjure_rules_proc_macro::register_rule;
