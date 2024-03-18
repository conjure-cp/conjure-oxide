use crate::{get_rule_set_by_name, get_rules};
use conjure_core::rule::Rule;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::sync::OnceLock;
use log::warn;
use conjure_core::SolverName;
use conjure_core::solvers::SolverFamily;

/// A set of rules with a name, priority, and dependencies.
#[derive(Clone, Debug)]
pub struct RuleSet<'a> {
    /// The name of the rule set.
    pub name: &'a str,
    /// Order of the RuleSet. Used to establish a consistent order of operations when resolving rules.
    /// If two RuleSets overlap (contain the same rule but with different priorities), the RuleSet with the higher order will be used as the source of truth.
    pub order: u8,
    /// A map of rules to their priorities. This will be lazily initialized at runtime.
    rules: OnceLock<HashMap<&'a Rule<'a>, u8>>,
    /// The names of the rule sets that this rule set depends on.
    dependency_rs_names: &'a [&'a str],
    dependencies: OnceLock<HashSet<&'a RuleSet<'a>>>,
    /// The solver families that this rule set applies to.
    pub solver_families: &'a [SolverFamily],
    /// The solvers that this rule set applies to.
    pub solvers: &'a [SolverName],
}

impl<'a> RuleSet<'a> {
    pub const fn new(name: &'a str, order: u8, dependencies: &'a [&'a str], solver_families: &'a [SolverFamily], solvers: &'a [SolverName]) -> Self {
        Self {
            name,
            order,
            dependency_rs_names: dependencies,
            solvers,
            solver_families,
            rules: OnceLock::new(),
            dependencies: OnceLock::new(),
        }
    }

    /// Get the rules of this rule set, evaluating them lazily if necessary
    /// Returns a `&HashMap<&Rule, u8>` where the key is the rule and the value is the priority of the rule.
    pub fn get_rules(&self) -> &HashMap<&'a Rule<'a>, u8> {
        match self.rules.get() {
            None => {
                let rules = self.resolve_rules();
                let _ = self.rules.set(rules); // Try to set the rules, but ignore if it fails.

                // At this point, the rules cell is guaranteed to be set, so we can unwrap safely.
                // see: https://doc.rust-lang.org/stable/std/sync/struct.OnceLock.html#method.set
                #[allow(clippy::unwrap_used)]
                self.rules.get().unwrap()
            }
            Some(rules) => rules,
        }
    }

    /// Get the dependencies of this rule set, evaluating them lazily if necessary
    /// Returns a `&HashSet<&RuleSet>` of the rule sets that this rule set depends on.
    #[allow(clippy::mutable_key_type)] // RuleSet is 'static so it's fine
    pub fn get_dependencies(&self) -> &HashSet<&'static RuleSet> {
        match self.dependencies.get() {
            None => {
                let dependencies = self.resolve_dependencies();
                let _ = self.dependencies.set(dependencies); // Try to set the dependencies, but ignore if it fails.

                // At this point, the dependencies cell is guaranteed to be set, so we can unwrap safely.
                // see: https://doc.rust-lang.org/stable/std/sync/struct.OnceLock.html#method.set
                #[allow(clippy::unwrap_used)]
                self.dependencies.get().unwrap()
            }
            Some(dependencies) => dependencies,
        }
    }

    /// Get the dependencies of this rule set, including itself
    #[allow(clippy::mutable_key_type)] // RuleSet is 'static so it's fine
    pub fn with_dependencies(&self) -> HashSet<&'static RuleSet> {
        let mut deps = self.get_dependencies().clone();
        deps.insert(self);
        deps
    }

    /// Resolve the rules of this rule set ("reverse the arrows")
    fn resolve_rules(&self) -> HashMap<&'a Rule<'a>, u8> {
        let mut rules = HashMap::new();

        for rule in get_rules() {
            let mut found = false;
            let mut priority: u8 = 0;

            for (name, p) in rule.rule_sets {
                if *name == self.name {
                    found = true;
                    priority = *p;
                    break;
                }
            }

            if found {
                rules.insert(rule, priority);
            }
        }

        rules
    }

    /// Recursively resolve the dependencies of this rule set.
    #[allow(clippy::mutable_key_type)] // RuleSet is 'static so it's fine
    fn resolve_dependencies(&self) -> HashSet<&'static RuleSet> {
        let mut dependencies = HashSet::new();

        for dep in self.dependency_rs_names {
            match get_rule_set_by_name(dep) {
                None => {
                    warn!("Rule set {} depends on non-existent rule set {}", &self.name, dep);
                }
                Some(rule_set) => {
                    if !dependencies.contains(rule_set) { // Prevent cycles
                        dependencies.insert(rule_set);
                        dependencies.extend(rule_set.resolve_dependencies());
                    }
                }
            }
        }

        dependencies
    }
}

impl<'a> PartialEq for RuleSet<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl<'a> Eq for RuleSet<'a> {}

impl<'a> Hash for RuleSet<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl<'a> Display for RuleSet<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let n_rules = self.get_rules().len();
        let solver_families = self.solver_families.iter().map(|f| f.to_string()).collect::<Vec<String>>();
        let solvers = self.solvers.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        
        write!(f, "RuleSet {{\n\
            \tname: {}\n\
            \torder: {}\n\
            \trules: {}\n\
            \tsolver_families: {:?}\n\
            \tsolvers: {:?}\n\
        }}", self.name, self.order, n_rules, solver_families, solvers)
    }
}