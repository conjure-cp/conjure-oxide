// #![feature(doc_auto_cfg)]

pub mod error;
pub mod find_conjure;
pub mod generate_custom;
pub mod parse;
pub mod rule_engine;
pub mod rules;
pub mod utils;

pub use conjure_core::ast; // re-export core::ast as conjure_oxide::ast
pub use conjure_core::metadata;
pub use conjure_core::model::Model; // rexport core::ast::Model as conjure_oxide::Model
pub use conjure_core::rule;
pub use conjure_core::solvers::SolverFamily;
pub use conjure_core::solvers::SolverName;
pub use conjure_rules::rule_set;
pub use rules::eval_constant;

pub use error::Error;

pub mod solver;

#[doc(hidden)]
pub mod unstable;
