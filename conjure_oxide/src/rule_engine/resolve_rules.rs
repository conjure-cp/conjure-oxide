use conjure_core::rule::Rule;
use conjure_rule_sets::RuleSet;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use thiserror::Error;

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

/**
* Helper function to get a rule set by name, or return an error if it doesn't exist.
* @param `rule_set_name` The name of the rule set to get.
* @returns The rule set with the given name or RuleSetError::RuleSetNotFound if it doesn't exist.
*/
fn get_rule_set(rule_set_name: &str) -> Result<&'static RuleSet<'static>, ResolveRulesError> {
    match conjure_rule_sets::get_rule_set_by_name(rule_set_name) {
        Some(rule_set) => Ok(rule_set),
        None => Err(ResolveRulesError::RuleSetNotFound),
    }
}

/**
* Helper function to resolve the dependencies of a rule set.
* @param `rule_set_name` The name of the rule set to resolve.
* @returns A set of the given rule set and all of its dependencies.
*/
fn resolve_dependencies(
    rule_set_name: &str,
) -> Result<HashSet<&'static RuleSet<'static>>, ResolveRulesError> {
    let mut ans: HashSet<&'static RuleSet<'static>> = HashSet::new();
    let rule_set = get_rule_set(rule_set_name)?;

    ans.insert(rule_set);

    if rule_set.dependencies.is_empty() {
        return Ok(ans);
    }

    for dep in rule_set.dependencies {
        let new_dependencies = resolve_dependencies(dep)?;
        ans.extend(new_dependencies);
    }

    Ok(ans)
}

/**
* Helper function to resolve a list of rule set names into a list of rule sets.
* @param `rule_set_names` The names of the rule sets to resolve.
* @returns A list of the given rule sets and all of their dependencies, or error
*/
pub fn resolve_rule_sets<'a>(
    rule_set_names: Vec<&str>,
) -> Result<Vec<&'a RuleSet<'static>>, ResolveRulesError> {
    let mut rs_set: HashSet<&'static RuleSet<'static>> = HashSet::new();

    for rule_set_name in rule_set_names {
        let new_dependencies = resolve_dependencies(rule_set_name)?;
        rs_set.extend(new_dependencies);
    }

    Ok(rs_set.into_iter().collect())
}

/**
* Convert a list of rule sets into a final map of rules to their priorities.
* @param `rule_sets` The rule sets to get the rules from.
* @returns A map of rules to their priorities.
*/
pub fn get_rule_priorities<'a>(
    rule_sets: &Vec<&'a RuleSet<'a>>,
) -> Result<HashMap<&'a Rule<'a>, u8>, ResolveRulesError> {
    let mut rule_priorities: HashMap<&'a Rule<'a>, (&'a RuleSet<'a>, u8)> = HashMap::new();

    for rs in rule_sets {
        for (rule, priority) in rs.get_rules() {
            if let Some((old_rs, _)) = rule_priorities.get(rule) {
                if rs.priority >= old_rs.priority {
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

/**
* Compare two rules by their priorities and names.
*
* Takes the rules and a map of rules to their priorities.
* If rules are not in the map, they are assumed to have priority 0.
* If the rules have the same priority, they are compared by their names.
*
* @param `a` first rule to compare.
* @param `b` second rule to compare.
* @param `rule_priorities` The priorities of the rules.
* @returns The ordering of the two rules.
*/
pub fn rule_cmp<'a>(
    a: &Rule<'a>,
    b: &Rule<'a>,
    rule_priorities: &HashMap<&'a Rule<'a>, u8>,
) -> std::cmp::Ordering {
    let a_priority = *rule_priorities.get(a).unwrap_or(&0);
    let b_priority = *rule_priorities.get(b).unwrap_or(&0);

    if a_priority == b_priority {
        return b.name.cmp(a.name);
    }

    b_priority.cmp(&a_priority)
}

/**
* Get a final ordering of rules based on their priorities and names.
* @param `rule_priorities` The priorities of the rules.
* @returns A list of rules sorted by their priorities and names.
*/
pub fn get_rules_vec<'a>(rule_priorities: &HashMap<&'a Rule<'a>, u8>) -> Vec<&'a Rule<'a>> {
    let mut rules: Vec<&'a Rule<'a>> = rule_priorities.keys().copied().collect();
    rules.sort_by(|a, b| rule_cmp(a, b, rule_priorities));
    rules
}
