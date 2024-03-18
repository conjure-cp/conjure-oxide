use std::sync::{Arc, RwLock};
use crate::rules::{Rule, RuleSet};
use crate::solvers::SolverFamily;

#[non_exhaustive]
pub struct Context<'a> {
    pub target_solver_family: Arc<RwLock<SolverFamily>>,
    pub extra_rule_set_names: Arc<RwLock<Vec<String>>>,
    pub rules: Arc<RwLock<Vec<&'a Rule<'a>>>>,
    pub rule_sets: Arc<RwLock<Vec<&'a RuleSet<'a>>>>,
}

impl<'a> Context<'a> {
    pub fn new(target_solver_family: SolverFamily, extra_rule_set_names: Vec<String>) -> Self {
        Context {
            target_solver_family: Arc::new(RwLock::new(target_solver_family)),
            extra_rule_set_names: Arc::new(RwLock::new(extra_rule_set_names)),
            rules: Arc::new(RwLock::new(Vec::new())),
            rule_sets: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl<'a> Default for Context<'a> {
    fn default() -> Self {
        Context {
            target_solver_family: Arc::new(RwLock::new(SolverFamily::default())),
            extra_rule_set_names: Arc::new(RwLock::new(Vec::new())),
            rules: Arc::new(RwLock::new(Vec::new())),
            rule_sets: Arc::new(RwLock::new(Vec::new())),
        }
    }
}