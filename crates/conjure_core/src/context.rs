use std::sync::{Arc, RwLock};

use crate::rule_engine::{Rule, RuleSet};
use crate::solvers::SolverFamily;

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Context<'a> {
    pub target_solver_family: Arc<RwLock<Option<SolverFamily>>>,
    pub extra_rule_set_names: Arc<RwLock<Vec<String>>>,
    pub rules: Arc<RwLock<Vec<&'a Rule<'a>>>>,
    pub rule_sets: Arc<RwLock<Vec<&'a RuleSet<'a>>>>,
}

impl<'a> Context<'a> {
    pub fn new(target_solver_family: SolverFamily, extra_rule_set_names: Vec<String>) -> Self {
        Context {
            target_solver_family: Arc::new(RwLock::new(Some(target_solver_family))),
            extra_rule_set_names: Arc::new(RwLock::new(extra_rule_set_names)),
            rules: Arc::new(RwLock::new(Vec::new())),
            rule_sets: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl<'a> Default for Context<'a> {
    fn default() -> Self {
        Context {
            target_solver_family: Arc::new(RwLock::new(None)),
            extra_rule_set_names: Arc::new(RwLock::new(Vec::new())),
            rules: Arc::new(RwLock::new(Vec::new())),
            rule_sets: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl PartialEq for Context<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.target_solver_family
            .read()
            .unwrap()
            .eq(&*other.target_solver_family.read().unwrap())
            && self
                .extra_rule_set_names
                .read()
                .unwrap()
                .eq(&*other.extra_rule_set_names.read().unwrap())
            && self.rules.read().unwrap().eq(&*other.rules.read().unwrap())
            && self
                .rule_sets
                .read()
                .unwrap()
                .eq(&*other.rule_sets.read().unwrap())
    }
}

impl Eq for Context<'_> {}
