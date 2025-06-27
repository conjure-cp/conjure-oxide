// #![feature(doc_auto_cfg)]

pub use conjure_core::Model;
pub use conjure_core::ast;
pub use conjure_core::error::Error;
pub use conjure_core::metadata::Metadata;
pub use conjure_core::parse::{get_example_model, get_example_model_by_path, model_from_json};
pub use conjure_core::rule_engine;
pub use conjure_core::rule_engine::{
    ApplicationError, ApplicationResult, Reduction, Rule, RuleSet, get_all_rule_sets,
    get_rule_by_name, get_rule_set_by_name, get_rule_sets_for_solver_family, get_rules,
    register_rule, register_rule_set,
};
pub use conjure_core::solver;
pub use conjure_core::solver::SolverFamily;
pub use conjure_essence_macros::essence_expr;
pub use conjure_essence_parser::{
    EssenceParseError, parse_essence_file, parse_essence_file_native,
};
pub use conjure_rules;
pub mod find_conjure;
pub mod utils;

#[doc(hidden)]
pub mod unstable;

pub mod defaults;
