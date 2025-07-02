use std::fmt::{Debug, Formatter};
use std::sync::{Arc, RwLock};

use derivative::Derivative;
use schemars::JsonSchema;
use serde::Serialize;
use serde_with::skip_serializing_none;

use crate::rule_engine::{RuleData, RuleSet};
use crate::solver::SolverFamily;
use crate::stats::Stats;

#[skip_serializing_none]
#[derive(Clone, Serialize, Default, Derivative, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[derivative(Eq, PartialEq)]
#[non_exhaustive]
pub struct Context<'a> {
    pub target_solver_family: Option<SolverFamily>,

    pub file_name: Option<String>,

    pub extra_rule_set_names: Vec<String>,

    #[serde(skip)]
    pub rules: Vec<RuleData<'a>>,

    #[serde(skip)]
    pub rule_sets: Vec<&'a RuleSet<'a>>,

    #[derivative(PartialEq = "ignore")]
    pub stats: Stats,
}

impl<'a> Context<'a> {
    pub fn new(
        target_solver_family: SolverFamily,
        extra_rule_set_names: Vec<String>,
        rules: Vec<RuleData<'a>>,
        rule_sets: Vec<&'a RuleSet<'a>>,
    ) -> Self {
        Context {
            target_solver_family: Some(target_solver_family),
            extra_rule_set_names,
            rules,
            rule_sets,
            stats: Default::default(),
            ..Default::default()
        }
    }
}

impl Context<'static> {
    pub fn new_ptr(
        target_solver_family: SolverFamily,
        extra_rule_set_names: Vec<String>,
        rules: Vec<RuleData<'static>>,
        rule_sets: Vec<&'static RuleSet<'static>>,
    ) -> Arc<RwLock<Context<'static>>> {
        Arc::new(RwLock::new(Context::new(
            target_solver_family,
            extra_rule_set_names,
            rules,
            rule_sets,
        )))
    }

    pub fn new_ptr_empty(target_solver_family: SolverFamily) -> Arc<RwLock<Context<'static>>> {
        Arc::new(RwLock::new(Context::new(
            target_solver_family,
            vec![],
            vec![],
            vec![],
        )))
    }
}

impl Debug for Context<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let target_solver_family: Option<SolverFamily> = self.target_solver_family;
        let extra_rule_set_names: Vec<String> = self.extra_rule_set_names.clone();
        let rules: Vec<&str> = self.rules.iter().map(|rd| rd.rule.name).collect();
        let rule_sets: Vec<&str> = self.rule_sets.iter().map(|r| r.name).collect();

        write!(
            f,
            "Context {{\n\
            \ttarget_solver_family: {target_solver_family:?}\n\
            \textra_rule_set_names: {extra_rule_set_names:?}\n\
            \trules: {rules:?}\n\
            \trule_sets: {rule_sets:?}\n\
        }}"
        )
    }
}
