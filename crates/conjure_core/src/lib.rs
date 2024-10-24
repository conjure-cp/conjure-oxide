#![cfg_attr(feature = "nightly", feature(coverage_attribute))]
#![cfg_attr(feature = "nightly", feature(stmt_expr_attributes))]

pub extern crate self as conjure_core;

pub use model::Model;

pub mod ast;
pub mod bug;
pub mod context;
pub mod error;
pub mod metadata;
pub mod model;
pub mod parse;
pub mod rule_engine;
pub mod rules;
pub mod solver;
pub mod stats;
