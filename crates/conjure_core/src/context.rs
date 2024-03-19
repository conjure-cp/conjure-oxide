use crate::rules::{get_rule_by_name, get_rule_set_by_name, Rule, RuleSet};
use crate::solvers::SolverFamily;
use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Formatter;
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Context<'a> {
    pub target_solver_family: Arc<RwLock<Option<SolverFamily>>>,
    pub extra_rule_set_names: Arc<RwLock<Vec<String>>>,
    pub rules: Arc<RwLock<Vec<&'a Rule<'a>>>>,
    pub rule_sets: Arc<RwLock<Vec<&'a RuleSet<'a>>>>,
}

impl<'a> Context<'a> {
    pub fn new(target_solver_family: SolverFamily, extra_rule_set_names: Vec<String>) -> Self {
        Context {
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
            target_solver_family: Arc::new(RwLock::new(None)),
            extra_rule_set_names: Arc::new(RwLock::new(Vec::new())),
            rules: Arc::new(RwLock::new(Vec::new())),
            rule_sets: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl PartialEq for Context<'_> {
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

impl<'a> Serialize for Context<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let target_solver_family = self.target_solver_family.read().unwrap().clone();
        let extra_rule_set_names = self.extra_rule_set_names.read().unwrap().clone();
        let rules: Vec<String> = self
            .rules
            .read()
            .unwrap()
            .iter()
            .map(|r| r.name.to_string())
            .collect();
        let rule_sets: Vec<String> = self
            .rule_sets
            .read()
            .unwrap()
            .iter()
            .map(|rs| rs.name.to_string())
            .collect();

        let mut state = serializer.serialize_struct("Context", 4)?;
        state.serialize_field("target_solver_family", &target_solver_family)?;
        state.serialize_field("extra_rule_set_names", &extra_rule_set_names)?;
        state.serialize_field("rules", &rules)?;
        state.serialize_field("rule_sets", &rule_sets)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Context<'static> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        pub enum Field {
            TargetSolverFamily,
            ExtraRuleSetNames,
            Rules,
            RuleSets,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                        formatter.write_str(
                            "target_solver_family, extra_rule_set_names, rules, rule_sets",
                        )
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "target_solver_family" => Ok(Field::TargetSolverFamily),
                            "extra_rule_set_names" => Ok(Field::ExtraRuleSetNames),
                            "rules" => Ok(Field::Rules),
                            "rule_sets" => Ok(Field::RuleSets),
                            _ => Err(serde::de::Error::unknown_field(
                                value,
                                &[
                                    "target_solver_family",
                                    "extra_rule_set_names",
                                    "rules",
                                    "rule_sets",
                                ],
                            )),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct ContextVisitor;

        impl<'de> Visitor<'de> for ContextVisitor {
            type Value = Context<'static>;
            // We can only "deserialize" rules / rule sets by getting them from the global registry by name, so they will always be static.
            // Also, due to lifetime shenanigans, we can't actually deserialize a Context<'a> because we can't pass through the 'a parameter to here
            // (See: E0207, E0401)
            // ToDo (gs248) - smarter people than me might find a better way to do this

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("struct Context")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut target_solver_family: Option<SolverFamily> = None;
                let mut extra_rule_set_names: Option<Vec<String>> = None;
                let mut rules: Option<Vec<&Rule>> = None;
                let mut rule_sets: Option<Vec<&RuleSet>> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::TargetSolverFamily => {
                            if target_solver_family.is_some() {
                                return Err(serde::de::Error::duplicate_field(
                                    "target_solver_family",
                                ));
                            }
                            target_solver_family = map.next_value()?;
                        }
                        Field::ExtraRuleSetNames => {
                            if extra_rule_set_names.is_some() {
                                return Err(serde::de::Error::duplicate_field(
                                    "extra_rule_set_names",
                                ));
                            }
                            extra_rule_set_names = Some(map.next_value()?);
                        }
                        Field::Rules => {
                            if rules.is_some() {
                                return Err(serde::de::Error::duplicate_field("rules"));
                            }
                            let rule_names: Vec<String> = map.next_value()?;
                            let found_rules: Vec<&Rule> = rule_names
                                .iter()
                                .filter_map(|name| get_rule_by_name(name))
                                .collect();
                            rules = Some(found_rules);
                        }
                        Field::RuleSets => {
                            if rule_sets.is_some() {
                                return Err(serde::de::Error::duplicate_field("rule_sets"));
                            }
                            let rule_set_names: Vec<String> = map.next_value()?;
                            let found_rule_sets: Vec<&RuleSet> = rule_set_names
                                .iter()
                                .filter_map(|name| get_rule_set_by_name(name))
                                .collect();
                            rule_sets = Some(found_rule_sets);
                        }
                    }
                }

                let target_solver_family = target_solver_family;
                let extra_rule_set_names = extra_rule_set_names
                    .ok_or_else(|| serde::de::Error::missing_field("extra_rule_set_names"))?;
                let rules = rules.ok_or_else(|| serde::de::Error::missing_field("rules"))?;
                let rule_sets =
                    rule_sets.ok_or_else(|| serde::de::Error::missing_field("rule_sets"))?;

                Ok(Context {
                    target_solver_family: Arc::new(RwLock::new(target_solver_family)),
                    extra_rule_set_names: Arc::new(RwLock::new(extra_rule_set_names)),
                    rules: Arc::new(RwLock::new(rules)),
                    rule_sets: Arc::new(RwLock::new(rule_sets)),
                })
            }
        }

        const FIELDS: &[&str] = &[
            "target_solver_family",
            "extra_rule_set_names",
            "rules",
            "rule_sets",
        ];
        deserializer.deserialize_struct("Context", FIELDS, ContextVisitor)
    }
}
