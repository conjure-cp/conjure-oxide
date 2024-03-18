pub use linkme::distributed_slice;

/// This procedural macro registers a decorated function with `conjure_rules`' global registry, and
/// adds the rule to one or more `RuleSet`'s.
///
/// It may be used in any downstream crate.
/// For more information on linker magic, see the [`linkme`](https://docs.rs/linkme/latest/linkme/) crate.
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
/// Functions must have the signature `fn(&Expr) -> ApplicationResult`.
/// The created rule will have the same name as the function.
///
/// Intermediary static variables are created to allow for the decentralized registry, with the prefix `CONJURE_GEN_`.
/// Please ensure that other variable names in the same scope do not conflict with these.
///
/// This macro must decorate a function with the given signature.
/// As arguments, it excepts a tuple of 2-tuples in the format:
/// `((<RuleSet name>, <Priority in RuleSet>), ...)`
///
/// <hr>
///
/// For example:
/// ```rust
/// use conjure_core::ast::Expression;
/// use conjure_core::model::Model;
/// use conjure_core::rules::{ApplicationError, ApplicationResult, Reduction};
/// use conjure_core::rules::register_rule;
///
/// #[register_rule(("RuleSetName", 10))]
/// fn identity(expr: &Expression, mdl: &Model) -> ApplicationResult {
///   Ok(Reduction::pure(expr.clone()))
/// }
/// ```
#[doc(inline)]
pub use conjure_rules_proc_macro::register_rule;

/// This procedural macro registers a rule set with the global registry.
/// It may be used in any downstream crate.
///
/// For more information on linker magic, see the [`linkme`](https://docs.rs/linkme/latest/linkme/) crate.
///
/// This macro uses the following syntax:
///
/// ```text
/// register_rule_set!(<RuleSet name>, <RuleSet order>, (<DependencyRuleSet1>, <DependencyRuleSet2>, ...));
/// ```
///
/// # Example
///
/// ```rust
/// use conjure_core::rules::register_rule_set;
///
/// register_rule_set!("MyRuleSet", 10, ("DependencyRuleSet", "AnotherRuleSet"));
/// ```
pub use conjure_rules_proc_macro::register_rule_set;

pub use rule::{ApplicationError, ApplicationResult, Reduction, Rule};
pub use rule_set::RuleSet;

use crate::solvers::{SolverFamily};

pub mod rule;
pub mod rule_set;

#[doc(hidden)]
#[distributed_slice]
pub static RULES_DISTRIBUTED_SLICE: [Rule<'static>];

#[doc(hidden)]
#[distributed_slice]
pub static RULE_SETS_DISTRIBUTED_SLICE: [RuleSet<'static>];

/// Returns a copied `Vec` of all rules registered with the `register_rule` macro.
///
/// Rules are not guaranteed to be in any particular order.
///
/// # Example
/// ```rust
/// # use conjure_core::rules::{ApplicationResult, Reduction, get_rules};
/// # use conjure_core::ast::Expression;
/// # use conjure_core::model::Model;
/// # use conjure_core::rules::register_rule;
///
/// #[register_rule]
/// fn identity(expr: &Expression, mdl: &Model) -> ApplicationResult {
///   Ok(Reduction::pure(expr.clone()))
/// }
///
/// fn main() {
///   println!("Rules: {:?}", get_rules());
/// }
/// ```
///
/// This will print (if no other rules are registered):
/// ```text
///   Rules: [Rule { name: "identity", application: MEM }]
/// ```
/// Where `MEM` is the memory address of the `identity` function.
pub fn get_rules() -> Vec<&'static Rule<'static>> {
    RULES_DISTRIBUTED_SLICE.iter().collect()
}

/// Get a rule by name.
/// Returns the rule with the given name or None if it doesn't exist.
///
/// # Example
/// ```rust
/// use conjure_core::rules::register_rule;
/// use conjure_core::rules::{Rule, ApplicationResult, Reduction, get_rule_by_name};
/// use conjure_core::ast::Expression;
/// use conjure_core::model::Model;
///
/// #[register_rule]
/// fn identity(expr: &Expression, mdl: &Model) -> ApplicationResult {
///  Ok(Reduction::pure(expr.clone()))
/// }
///
/// fn main() {
/// println!("Rule: {:?}", get_rule_by_name("identity"));
/// }
/// ```
///
/// This will print:
/// ```text
/// Rule: Some(Rule { name: "identity", application: MEM })
/// ```
pub fn get_rule_by_name(name: &str) -> Option<&'static Rule<'static>> {
    get_rules().iter().find(|rule| rule.name == name).cloned()
}

/// Get all rule sets
/// Returns a `Vec` of static references to all rule sets registered with the `register_rule_set` macro.
/// Rule sets are not guaranteed to be in any particular order.
///
/// # Example
/// ```rust
/// use conjure_core::rules::register_rule_set;
/// use conjure_core::rules::get_rule_sets;
///
/// register_rule_set!("MyRuleSet", 10, ("AnotherRuleSet"));
/// register_rule_set!("AnotherRuleSet", 5, ());
///
/// println!("Rule sets: {:?}", get_rule_sets());
/// ```
///
/// This will print (if no other rule sets are registered):
/// ```text
/// Rule sets: [
///   RuleSet { name: "MyRuleSet", order: 10, rules: OnceLock { state: Uninitialized }, dependencies: ["AnotherRuleSet"] },
///   RuleSet { name: "AnotherRuleSet", order: 5, rules: OnceLock { state: Uninitialized }, dependencies: [] }
/// ]
/// ```
///
pub fn get_rule_sets() -> Vec<&'static RuleSet<'static>> {
    RULE_SETS_DISTRIBUTED_SLICE.iter().collect()
}

/// Get a rule set by name.
/// Returns the rule set with the given name or None if it doesn't exist.
///
/// # Example
/// ```rust
/// use conjure_core::rules::register_rule_set;
/// use conjure_core::rules::get_rule_set_by_name;
///
/// register_rule_set!("MyRuleSet", 10, ("DependencyRuleSet", "AnotherRuleSet"));
///
/// println!("Rule set: {:?}", get_rule_set_by_name("MyRuleSet"));
/// ```
///
/// This will print:
/// ```text
/// Rule set: Some(RuleSet { name: "MyRuleSet", order: 10, rules: OnceLock { state: Uninitialized }, dependencies: ["DependencyRuleSet", "AnotherRuleSet"] })
/// ```
pub fn get_rule_set_by_name(name: &str) -> Option<&'static RuleSet<'static>> {
    get_rule_sets()
        .iter()
        .find(|rule_set| rule_set.name == name)
        .cloned()
}

/// Get all rule sets for a given solver family.
/// Returns a `Vec` of static references to all rule sets that are applicable to the given solver family.
///
/// # Example
///
/// ```rust
/// use conjure_core::solvers::SolverFamily;
/// use conjure_core::rules::get_rule_sets_for_solver_family;
/// use conjure_core::rules::register_rule_set;
///
/// register_rule_set!("RS1", 10, ("DependencyRuleSet"));
/// register_rule_set!("RS2", 5, ("AnotherRuleSet"), (SolverFamily::SAT));
///
/// let rule_sets = get_rule_sets_for_solver_family(SolverFamily::SAT);
/// assert_eq!(rule_sets.len(), 1);
/// assert_eq!(rule_sets[0].name, "RS2");
/// ```
pub fn get_rule_sets_for_solver_family(
    solver_family: SolverFamily,
) -> Vec<&'static RuleSet<'static>> {
    get_rule_sets()
        .iter()
        .filter(|rule_set| {
            rule_set
                .solver_families
                .iter()
                .any(|family| family.eq(&solver_family))
        })
        .cloned()
        .collect()
}
