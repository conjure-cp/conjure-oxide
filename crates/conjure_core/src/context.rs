use std::sync::{Arc, RwLock};

use conjure_core::solver::model_modifier::ModelModifier;
use conjure_core::solver::SolverAdaptor;
use minion_rs::ast::Model;

use crate::rule_engine::{Rule, RuleSet};
use crate::solver::SolverFamily;

type AdaptorInstance =
    dyn SolverAdaptor<Model = Model, Modifier = dyn ModelModifier, Solution = ()>;

#[derive(Clone)]
#[non_exhaustive]
pub struct Context<'a> {
    pub target_solver_adaptor: Arc<RwLock<Option<&'a AdaptorInstance>>>,
    pub target_solver_family: Arc<RwLock<Option<SolverFamily>>>,
    pub extra_rule_set_names: Arc<RwLock<Vec<String>>>,
    pub rules: Arc<RwLock<Vec<&'a Rule<'a>>>>,
    pub rule_sets: Arc<RwLock<Vec<&'a RuleSet<'a>>>>,
}

impl<'a> Context<'a> {
    pub fn new(
        target_solver_adaptor: &'a AdaptorInstance,
        target_solver_family: SolverFamily,
        extra_rule_set_names: Vec<String>,
    ) -> Self {
        Context {
            target_solver_adaptor: Arc::new(RwLock::new(Some(target_solver_adaptor))),
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
            target_solver_adaptor: Arc::new(RwLock::new(None)),
            target_solver_family: Arc::new(RwLock::new(None)),
            extra_rule_set_names: Arc::new(RwLock::new(Vec::new())),
            rules: Arc::new(RwLock::new(Vec::new())),
            rule_sets: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl PartialEq for Context<'_> {
    #[allow(clippy::unwrap_used)] // A poisoned RWLock is probably panic worthy
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
