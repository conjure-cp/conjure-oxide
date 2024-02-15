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

mod rule_set;

use conjure_core::rule::Rule;

#[doc(hidden)]
pub mod _dependencies {
    pub use conjure_core::rule::Rule;
    pub use inventory;
    pub use linkme::distributed_slice;
}
//
// #[doc(hidden)]
// #[distributed_slice]
// pub static RULES_DISTRIBUTED_SLICE: [Rule<'static>];

/// Returns a copied `Vec` of all rules registered with the `register_rule` macro.
///
/// Rules are not guaranteed to be in any particular order.
///
/// # Example
/// ```rust
/// # use conjure_rules::register_rule;
/// # use conjure_core::rule::{Rule, RuleApplicationError};
/// # use conjure_core::ast::Expression;
/// #
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
pub fn get_rules() -> Vec<&'static Rule<'static>> {
    //RULES_DISTRIBUTED_SLICE.to_vec()
    let mut rules = Vec::new();
    for rule in inventory::iter::<Rule> {
        rules.push(rule);
    }
    rules
}

pub fn get_rule_by_name(name: &str) -> Option<&'static Rule<'static>> {
    get_rules().iter().find(|rule| rule.name == name).cloned()
}

/// This procedural macro registers a decorated function with `conjure_rules`' global registry.
/// It may be used in any downstream crate. For more information on linker magic, see the [`linkme`](https://docs.rs/linkme/latest/linkme/) crate.
///
/// **IMPORTANT**: Since the resulting rule may not be explicitly referenced, it may be removed by the compiler's dead code elimination.
/// To prevent this, you must ensure that either:
/// 1. codegen-units is set to 1, i.e. in Cargo.toml:
/// ```toml
/// [profile.release]
/// codegen-units = 1
/// ```
/// 2. The function is included somewhere else in the code
///
/// <hr>
///
/// Functions must have the signature `fn(&Expr) -> Result<Expr, RuleApplicationError>`.
/// The created rule will have the same name as the function.
///
/// Intermediary static variables are created to allow for the decentralized registry, with the prefix `CONJURE_GEN_`.
/// Please ensure that other variable names in the same scope do not conflict with these.
///
/// <hr>
///
/// For example:
/// ```rust
/// # use conjure_core::ast::Expression;
/// # use conjure_core::rule::RuleApplicationError;
/// # use conjure_rules::register_rule;
/// #
/// #[register_rule]
/// fn identity(expr: &Expression) -> Result<Expression, RuleApplicationError> {
///   Ok(expr.clone())
/// }
/// ```
#[doc(inline)]
pub use conjure_rules_proc_macro::register_rule;
