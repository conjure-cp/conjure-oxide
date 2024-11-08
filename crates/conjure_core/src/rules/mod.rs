//! This module contains the rewrite rules for Conjure Oxides and it's solvers.
//!
//! # Rule Semantics
//!
#![doc = include_str!("./rule_semantics.md")]

pub use constant::eval_constant;

mod base;
mod bubble;
pub mod checks;
mod cnf;
mod constant;
mod minion;
mod partial_eval;

/// Denotes a block of code as extra, optional checks for a rule. Primarily, these are checks that
/// are too expensive to do normally, or are implicit in the rule priorities and application order.
///
/// The latter is necessary as, at the time of writing, rules that cover more of the tree are
/// applied over more local rules of higher priority. In the future, rules will be applied strictly
/// by priority not size; however, for now, if we want a given global rule G to only run after a
/// local rule R, we must make it explicit by making G check that R is not applicable to any child
/// expressions.
///
/// These only run when the extra-rule-checks feature flag is enabled. At time of writing, this is
/// on by default.
macro_rules! extra_check {
    {$($stmt:stmt)*} => {
        if cfg!(feature ="extra-rule-checks") {
            ($($stmt)*)
        }
    };
}
