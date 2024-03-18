use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use thiserror::Error;

use crate::{get_rule_set_by_name, get_rule_sets_for_solver, Rule, RuleSet, SolverName};

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
pub fn rule_sets_by_names<'a>(
    rule_set_names: Vec<&str>,
) -> Result<HashSet<&'a RuleSet<'static>>, ResolveRulesError> {
    let mut rs_set: HashSet<&'static RuleSet<'static>> = HashSet::new();

    for rule_set_name in rule_set_names {
        let rule_set = get_rule_set(rule_set_name)?;
        let new_dependencies = rule_set.get_dependencies();
        rs_set.insert(rule_set);
        rs_set.extend(new_dependencies);
    }

    Ok(rs_set)
}

/// Resolves the final set of rule sets to apply based on target solver and extra rule set names.
///
/// # Arguments
/// - `target_solver` The solver to resolve the rule sets for.
/// - `extra_rs_names` The names of the extra rule sets to use
///
/// # Returns
/// - A vector of rule sets to apply.
///
#[allow(clippy::mutable_key_type)] // RuleSet is 'static so it's fine
pub fn resolve_rule_sets<'a>(
    target_solver: SolverName,
    extra_rs_names: Vec<&str>,
) -> Result<Vec<&'a RuleSet<'static>>, ResolveRulesError> {
    let mut ans = HashSet::new();

    for rs in get_rule_sets_for_solver(target_solver) {
        ans.extend(rs.with_dependencies());
    }

    ans.extend(rule_sets_by_names(extra_rs_names)?);
    Ok(ans.iter().cloned().collect())
}

/// Convert a list of rule sets into a final map of rules to their priorities.
///
/// # Arguments
/// - `rule_sets` The rule sets to get the rules from.
/// # Returns
/// - A map of rules to their priorities.
pub fn get_rule_priorities<'a>(
    rule_sets: &Vec<&'a RuleSet<'a>>,
) -> Result<HashMap<&'a Rule<'a>, u8>, ResolveRulesError> {
    let mut rule_priorities: HashMap<&'a Rule<'a>, (&'a RuleSet<'a>, u8)> = HashMap::new();

    for rs in rule_sets {
        for (rule, priority) in rs.get_rules() {
            if let Some((old_rs, _)) = rule_priorities.get(rule) {
                if rs.order >= old_rs.order {
                    rule_priorities.insert(rule, (&rs, *priority));
                }
            } else {
                rule_priorities.insert(rule, (&rs, *priority));
            }
        }
    }

    let mut ans: HashMap<&'a Rule<'a>, u8> = HashMap::new();
    for (rule, (_, priority)) in rule_priorities {
        ans.insert(rule, priority);
    }

    Ok(ans)
}

/// Compare two rules by their priorities and names.
///
/// Takes the rules and a map of rules to their priorities.
/// If rules are not in the map, they are assumed to have priority 0.
/// If the rules have the same priority, they are compared by their names.
///
/// # Arguments
/// - `a` first rule to compare.
/// - `b` second rule to compare.
/// - `rule_priorities` The priorities of the rules.
///
/// # Returns
/// - The ordering of the two rules.
pub fn rule_cmp<'a>(
    a: &Rule<'a>,
    b: &Rule<'a>,
    rule_priorities: &HashMap<&'a Rule<'a>, u8>,
) -> std::cmp::Ordering {
    let a_priority = *rule_priorities.get(a).unwrap_or(&0);
    let b_priority = *rule_priorities.get(b).unwrap_or(&0);

    if a_priority == b_priority {
        return a.name.cmp(b.name);
    }

    b_priority.cmp(&a_priority)
}

/// Get a final ordering of rules based on their priorities and names.
///
/// # Arguments
/// - `rule_priorities` The priorities of the rules.
///
/// # Returns
/// - A list of rules sorted by their priorities and names.
pub fn get_rules_vec<'a>(rule_priorities: &HashMap<&'a Rule<'a>, u8>) -> Vec<&'a Rule<'a>> {
    let mut rules: Vec<&'a Rule<'a>> = rule_priorities.keys().copied().collect();
    rules.sort_by(|a, b| rule_cmp(a, b, rule_priorities));
    rules
}
