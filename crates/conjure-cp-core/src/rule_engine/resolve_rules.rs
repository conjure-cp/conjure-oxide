use itertools::Itertools;
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashSet};
use std::fmt::Display;
use thiserror::Error;

use crate::rule_engine::{Rule, RuleSet, get_rule_set_by_name, get_rule_sets_for_solver_family};
use crate::solver::SolverFamily;

/// Holds a rule and its priority, along with the rule set it came from.
#[derive(Debug, Clone)]
pub struct RuleData<'a> {
    pub rule: &'a Rule<'a>,
    pub priority: u16,
    pub rule_set: &'a RuleSet<'a>,
}

impl Display for RuleData<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Rule: {} (priority: {}, from rule set: {})",
            self.rule.name, self.priority, self.rule_set.name
        )
    }
}

// Equality is based on the rule itself.
// Note: this is intentional.
// If two RuleSets reference the same rule (possibly with different priorities),
// we only want to keep one copy of the rule.
impl PartialEq for RuleData<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.rule == other.rule
    }
}

impl Eq for RuleData<'_> {}

// Sort by priority (higher priority first), then by rule name (alphabetical).
impl PartialOrd for RuleData<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RuleData<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => self.rule.name.cmp(other.rule.name),
            ord => ord.reverse(),
        }
    }
}

/// Error type for rule resolution.
#[derive(Debug, Error)]
pub enum ResolveRulesError {
    RuleSetNotFound,
}

impl Display for ResolveRulesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveRulesError::RuleSetNotFound => write!(f, "Rule set not found."),
        }
    }
}

/// Helper function to get a rule set by name, or return an error if it doesn't exist.
///
/// # Arguments
/// - `rule_set_name` The name of the rule set to get.
///
/// # Returns
/// - The rule set with the given name or `RuleSetError::RuleSetNotFound` if it doesn't exist.
fn get_rule_set(rule_set_name: &str) -> Result<&'static RuleSet<'static>, ResolveRulesError> {
    match get_rule_set_by_name(rule_set_name) {
        Some(rule_set) => Ok(rule_set),
        None => Err(ResolveRulesError::RuleSetNotFound),
    }
}

/// Resolve a list of rule sets (and dependencies) by their names
///
/// # Arguments
/// - `rule_set_names` The names of the rule sets to resolve.
///
/// # Returns
/// - A list of the given rule sets and all of their dependencies, or error
///
#[allow(clippy::mutable_key_type)] // RuleSet is 'static so it's fine
fn rule_sets_by_names(
    rule_set_names: &[&str],
) -> Result<HashSet<&'static RuleSet<'static>>, ResolveRulesError> {
    let mut rs_set: HashSet<&'static RuleSet<'static>> = HashSet::new();

    for rule_set_name in rule_set_names {
        let rule_set = get_rule_set(rule_set_name)?;
        let new_dependencies = rule_set.get_dependencies();
        rs_set.insert(rule_set);
        rs_set.extend(new_dependencies);
    }

    Ok(rs_set)
}

/// Build a list of rules to apply (sorted by priority) from a list of rule sets.
///
/// # Arguments
/// - `rule_sets` The rule sets to resolve the rules from.
///
/// # Returns
/// - Rules to apply, sorted from highest to lowest priority.
pub fn get_rules<'a>(
    rule_sets: &Vec<&'a RuleSet<'a>>,
) -> Result<impl IntoIterator<Item = RuleData<'a>>, ResolveRulesError> {
    // Hashing is done by name which never changes, and the references are 'static
    #[allow(clippy::mutable_key_type)]
    let mut ans = BTreeSet::<RuleData<'a>>::new();

    for rs in rule_sets {
        for (rule, priority) in rs.get_rules() {
            ans.insert(RuleData {
                rule,
                priority: *priority,
                rule_set: rs,
            });
        }
    }

    Ok(ans)
}

/// Get rules grouped by priority from a list of rule sets.
///
/// # Arguments
/// - `rule_sets` The rule sets to resolve the rules from.
///
/// # Returns
/// - Rules to apply, grouped by priority, sorted from highest to lowest priority.
pub fn get_rules_grouped<'a>(
    rule_sets: &Vec<&'a RuleSet<'a>>,
) -> Result<impl IntoIterator<Item = (u16, Vec<RuleData<'a>>)> + 'a, ResolveRulesError> {
    let rules = get_rules(rule_sets)?;
    let grouped: Vec<(u16, Vec<RuleData<'a>>)> = rules
        .into_iter()
        .chunk_by(|rule_data| rule_data.priority)
        .into_iter()
        // Each chunk here is short-lived, so we clone/copy out the data
        .map(|(priority, chunk)| (priority, chunk.collect()))
        .collect();
    Ok(grouped)
}

/// Resolves the final set of rule sets to apply based on target solver and extra rule set names.
///
/// # Arguments
/// - `target_solver` The solver family we're targeting
/// - `extra_rs_names` Optional extra rule set names to enable
///
/// # Returns
/// - A vector of rule sets to apply.
///
pub fn resolve_rule_sets(
    target_solver: SolverFamily,
    extra_rs_names: &[&str],
) -> Result<Vec<&'static RuleSet<'static>>, ResolveRulesError> {
    #[allow(clippy::mutable_key_type)]
    // Hashing is done by name which never changes, and the references are 'static
    let mut ans = HashSet::new();

    for rs in get_rule_sets_for_solver_family(target_solver) {
        ans.extend(rs.with_dependencies());
    }

    ans.extend(rule_sets_by_names(extra_rs_names)?);
    Ok(ans.iter().copied().collect())
}
