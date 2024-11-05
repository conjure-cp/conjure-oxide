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
