// #![feature(doc_auto_cfg)]

pub use conjure_core::ast;
pub use conjure_core::error::Error;
pub use conjure_core::metadata::Metadata;
pub use conjure_core::model::Model;
pub use conjure_core::parse::{get_example_model, get_example_model_by_path, model_from_json};
pub use conjure_core::rule_engine;
pub use conjure_core::rule_engine::{
    get_rule_by_name, get_rule_set_by_name, get_rule_sets, get_rule_sets_for_solver_family,
    get_rules, register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
    Rule, RuleSet,
};
pub use conjure_core::rules;
pub use conjure_core::solver;
pub use conjure_core::solver::SolverFamily;

pub mod find_conjure;
pub mod generate_custom;
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
