use conjure_core::rule::Rule;
use conjure_rules::get_rules;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub struct RuleSet<'a> {
    pub name: &'a str,
    pub priority: u8,
    pub rules: OnceLock<HashMap<&'a Rule<'a>, u8>>,
    pub dependencies: &'a [&'a str],
}

impl<'a> RuleSet<'a> {
    pub const fn new(name: &'a str, priority: u8, dependencies: &'a [&'a str]) -> Self {
        Self {
            name,
            priority,
            dependencies,
            rules: OnceLock::new(),
        }
    }

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
