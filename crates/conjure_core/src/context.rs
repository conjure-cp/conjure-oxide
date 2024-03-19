use crate::rules::{Rule, RuleSet};
use crate::solvers::SolverFamily;
use std::sync::{Arc, RwLock};

#[non_exhaustive]
pub struct Context<'a, T> {
    pub target_solver_adaptor: Arc<RwLock<Option<&'a T>>>, // The generic is necessary to avoid importing the SolverAdaptor trait, which would create a circular dependency
    pub target_solver_family: Arc<RwLock<Option<SolverFamily>>>,
    pub extra_rule_set_names: Arc<RwLock<Vec<String>>>,
    pub rules: Arc<RwLock<Vec<&'a Rule<'a>>>>,
    pub rule_sets: Arc<RwLock<Vec<&'a RuleSet<'a>>>>,
}

impl<'a, T> Context<'a, T> {
    pub fn new(target_solver_adaptor: &'a T, target_solver_family: SolverFamily, extra_rule_set_names: Vec<String>) -> Self {
        Context {
            target_solver_adaptor: Arc::new(RwLock::new(Some(target_solver_adaptor))),
            target_solver_family: Arc::new(RwLock::new(Some(target_solver_family))),
            extra_rule_set_names: Arc::new(RwLock::new(extra_rule_set_names)),
            rules: Arc::new(RwLock::new(Vec::new())),
            rule_sets: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl<'a, T> Default for Context<'a, T> {
    fn default() -> Self {
        Context {
            target_solver_adaptor: Arc::new(RwLock::new(None)),
            target_solver_family: Arc::new(RwLock::new(None)),
            extra_rule_set_names: Arc::new(RwLock::new(Vec::new())),
            rules: Arc::new(RwLock::new(Vec::new())),
            rule_sets: Arc::new(RwLock::new(Vec::new())),
        }
    }
}
