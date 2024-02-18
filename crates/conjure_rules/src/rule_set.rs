use crate::get_rules;
use conjure_core::rule::Rule;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::OnceLock;

/**
 * A set of rules with a name, priority, and dependencies.
 *
 * # Fields
 * - `name` The name of the rule set.
 * - `order` The order of the rule set.
 * - `rules` A map of rules to their priorities. This is evaluated lazily at runtime.
 * - `dependencies` A list of rule set names that this rule set depends on.
 */
#[derive(Clone, Debug)]
pub struct RuleSet<'a> {
    pub name: &'a str,
    pub order: u8,
    pub rules: OnceLock<HashMap<&'a Rule<'a>, u8>>,
    pub dependencies: &'a [&'a str],
}

impl<'a> RuleSet<'a> {
    pub const fn new(name: &'a str, priority: u8, dependencies: &'a [&'a str]) -> Self {
        Self {
            name,
            order: priority,
            dependencies,
            rules: OnceLock::new(),
        }
    }

    /**
     * Get the rules of this rule set, evaluating them lazily if necessary.
     * @returns A map of rules to their priorities.
     */
    pub fn get_rules(&self) -> &HashMap<&'a Rule<'a>, u8> {
        match self.rules.get() {
            None => {
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
                match self.rules.set(rules) {
                    Ok(_) => {
                        match self.rules.get() {
                            None => {
                                panic!("RuleSet::rules was set, but RuleSet::rules.get() returned None!");
                                // This should never happen
                            }
                            Some(rules) => rules,
                        }
                    }
                    Err(e) => {
                        panic!("Could not set RuleSet::rules! Error: {:?}", e); // This should also never happen :)
                    }
                }
            }
            Some(rules) => rules,
        }
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
