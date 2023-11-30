pub mod error;
pub mod find_conjure;
pub mod parse;
pub mod rule_engine;

mod solvers;

pub use error::Error;

/************************************************/
/*        RE-EXPORT CONJURE_CORE MEMBERS        */
/************************************************/

pub use conjure_core::{ast, ast::Model, rule, solvers::Solver};

/******************************************/
/*        RE-EXPORT CONJURE_MACROS        */
/******************************************/

pub mod macros {
    /// Register a rewriting rule.
    ///
    /// Rules are functions of type: `fn(Expression) -> RuleApplicationResult`.
    ///
    /// * The rule name as it appears in Conjure-Oxide will be equal to the name of the function.  
    ///
    /// * The argument to this macro must be a valid [RuleKind](super::rule::RuleKind) variant.
    ///
    /// ```rust
    /// use conjure_oxide::macros::rule;
    /// use conjure_oxide::rule::RuleApplicationResult;
    /// use conjure_oxide::ast::Expression;
    ///
    /// #[rule(Horizontal)]
    /// fn do_nothing_rule(e: Expression) -> RuleApplicationResult {
    ///     Ok(e)
    /// }
    /// ```
    pub use conjure_macros::rule;
}
