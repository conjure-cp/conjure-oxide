use crate::get_rules;
use conjure_core::rule::Rule;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::OnceLock;

pub struct RuleSet<'a> {
    pub name: &'a str,
    pub rules: OnceLock<HashMap<Rule<'a>, u8>>,
    pub dependencies: &'a [&'a RuleSet<'a>],
}

impl<'a> RuleSet<'a> {
    pub fn new(name: &'a str, dependencies: &'a [&'a RuleSet<'a>]) -> Self {
        Self {
            name,
            rules: OnceLock::new(),
            dependencies,
        }
    }

    pub fn get_rules(&self) -> &HashMap<Rule<'a>, u8> {
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
                            Some(rules) => {
                                return rules;
                            }
                        }
                    }
                    Err(e) => {
                        panic!("Could not set RuleSet::rules! Error: {:?}", e); // This should also never happen :)
                    }
                }
            }
            Some(rules) => {
                return rules;
            }
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
