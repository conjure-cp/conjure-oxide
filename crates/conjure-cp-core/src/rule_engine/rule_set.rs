use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::sync::OnceLock;

use log::warn;

use crate::rule_engine::{Rule, get_all_rules, get_rule_set_by_name};
use crate::solver::SolverFamily;

/// A structure representing a set of rules with a name, priority, and dependencies.
///
/// `RuleSet` is a way to group related rules together under a single name.
/// You can think of it like a list of rules that belong to the same category.
/// Each `RuleSet` can also have a number that tells it what order it should run in compared to other `RuleSet` instances.
/// Additionally, a `RuleSet` can depend on other `RuleSet` instances, meaning it needs them to run first.
///
/// To make things efficient, `RuleSet` only figures out its rules and dependencies the first time they're needed,
/// and then it remembers them so it doesn't have to do the work again.
///
/// # Fields
/// - `name`: The name of the rule set.
/// - `order`: A number that decides the order in which this `RuleSet` should be applied.
///   If two `RuleSet` instances have the same rule but with different priorities,
///   the one with the higher `order` number will be the one that is used.
/// - `rules`: A lazily initialized map of rules to their priorities.
/// - `dependency_rs_names`: The names of the rule sets that this rule set depends on.
/// - `dependencies`: A lazily initialized set of `RuleSet` dependencies.
/// - `solver_families`: The solver families that this rule set applies to.
#[derive(Clone, Debug)]
pub struct RuleSet<'a> {
    /// The name of the rule set.
    pub name: &'a str,
    /// A map of rules to their priorities. This will be lazily initialized at runtime.
    rules: OnceLock<HashMap<&'a Rule<'a>, u16>>,
    /// The names of the rule sets that this rule set depends on.
    dependency_rs_names: &'a [&'a str],
    dependencies: OnceLock<HashSet<&'a RuleSet<'a>>>,
    /// The solver families that this rule set applies to.
    pub solver_families: &'a [SolverFamily],
}

impl<'a> RuleSet<'a> {
    pub const fn new(
        name: &'a str,
        dependencies: &'a [&'a str],
        solver_families: &'a [SolverFamily],
    ) -> Self {
        Self {
            name,
            dependency_rs_names: dependencies,
            solver_families,
            rules: OnceLock::new(),
            dependencies: OnceLock::new(),
        }
    }

    /// Get the rules of this rule set, evaluating them lazily if necessary
    /// Returns a `&HashMap<&Rule, u16>` where the key is the rule and the value is the priority of the rule.
    pub fn get_rules(&self) -> &HashMap<&'a Rule<'a>, u16> {
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
    fn resolve_rules(&self) -> HashMap<&'a Rule<'a>, u16> {
        let mut rules = HashMap::new();

        for rule in get_all_rules() {
            let mut found = false;
            let mut priority: u16 = 0;

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
                    warn!(
                        "Rule set {} depends on non-existent rule set {}",
                        &self.name, dep
                    );
                }
                Some(rule_set) => {
                    if !dependencies.contains(rule_set) {
                        // Prevent cycles
                        dependencies.insert(rule_set);
                        dependencies.extend(rule_set.resolve_dependencies());
                    }
                }
            }
        }

        dependencies
    }
}

impl PartialEq for RuleSet<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for RuleSet<'_> {}

impl Hash for RuleSet<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Display for RuleSet<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let n_rules = self.get_rules().len();
        let solver_families = self
            .solver_families
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<String>>();

        write!(
            f,
            "RuleSet {{\n\
            \tname: {}\n\
            \trules: {}\n\
            \tsolver_families: {:?}\n\
        }}",
            self.name, n_rules, solver_families
        )
    }
}
