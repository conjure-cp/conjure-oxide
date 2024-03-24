// #![feature(doc_auto_cfg)]

pub use conjure_core::ast;
pub use conjure_core::metadata::Metadata;
pub use conjure_core::model::Model;
pub use conjure_core::rules::{
    ApplicationError, ApplicationResult, get_rule_by_name, get_rule_set_by_name,
    get_rule_sets, get_rule_sets_for_solver_family, get_rules, Reduction, register_rule, register_rule_set,
    Rule, RuleSet,
};
pub use conjure_core::solvers::SolverFamily;
pub use error::Error;
pub use rules::eval_constant;

pub mod find_conjure;
pub mod rules;
pub mod utils;

#[doc(hidden)]
pub mod unstable;
