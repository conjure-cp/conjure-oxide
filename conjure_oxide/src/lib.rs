// #![feature(doc_auto_cfg)]

pub use conjure_core::ast;
pub use conjure_core::metadata::Metadata;
pub use conjure_core::model::Model;
pub use conjure_core::rules::{
    get_rule_by_name, get_rule_set_by_name, get_rule_sets, get_rule_sets_for_solver_family,
    get_rules, register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
    Rule, RuleSet,
};
pub use conjure_core::solvers::SolverFamily;
pub use error::Error;
pub use rules::eval_constant;

pub mod error;
pub mod find_conjure;
pub mod generate_custom;
pub mod parse;
pub mod rule_engine;
pub mod rules;
pub mod utils;

pub mod solver;

#[doc(hidden)]
pub mod unstable;
