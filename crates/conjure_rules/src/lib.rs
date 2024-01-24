//! ### A decentralised rule registry for Conjure Oxide
//!
//! This crate allows registering valid functions as expression-reduction rules.
//! Functions can be decorated with the `register_rule` macro in any downstream crate and be used by Conjure Oxide's rule engine.
//! To achieve compile-time linking, we make use of the [`linkme`](https://docs.rs/linkme/latest/linkme/) crate.
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

/// Returns a copied `Vec` of all rules registered with the `register_rule` macro.
///
/// Rules defined in the same file will remain contiguous and in order, but order may not be maintained between files.
///
/// # Example
/// ```rust
/// use conjure_rules::register_rule;
/// use conjure_oxide::rule::{Rule, RuleApplicationError};
///
/// #[register_rule]
/// fn identity(expr: &Expression) -> Result<Expression, RuleApplicationError> {
///   Ok(expr.clone())
/// }
///
/// fn main() {
///   println!("Rules: {:?}", conjure_rules::get_rules());
/// }
/// ```
///
/// This will print (if no other rules are registered):
/// ```text
///   Rules: [Rule { name: "identity", application: MEM }]
/// ```
/// Where `MEM` is the memory address of the `identity` function.
pub fn get_rules() -> Vec<Rule<'static>> {
    RULES_DISTRIBUTED_SLICE.to_vec()
}

pub use conjure_rules_proc_macro::register_rule;
