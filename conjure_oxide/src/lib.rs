// #![feature(doc_auto_cfg)]

pub mod error;
pub mod find_conjure;
pub mod parse;
pub mod rewrite;
mod rules;
pub mod solvers;
mod utils;

pub use conjure_core::ast; // re-export core::ast as conjure_oxide::ast
pub use conjure_core::ast::Model; // rexport core::ast::Model as conjure_oxide::Model
pub use conjure_core::solvers::Solver;
pub use rules::eval_constant;

pub use error::Error;

pub mod unstable;
