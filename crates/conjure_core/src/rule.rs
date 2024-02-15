use std::fmt::{self, Display, Formatter};
use std::hash::Hash;

use thiserror::Error;

use crate::ast::Expression;

#[derive(Debug, Error)]
pub enum RuleApplicationError {
    #[error("Rule is not applicable")]
    RuleNotApplicable,
}

#[derive(Clone, Debug)]
pub struct Rule<'a> {
    pub name: &'a str,
    pub application: fn(&Expression) -> Result<Expression, RuleApplicationError>,
    pub rule_sets: &'a [(&'a str, u8)], // (name, priority). At runtime, we add the rule to rulesets
}

impl<'a> Rule<'a> {
    pub const fn new(
        name: &'a str,
        application: fn(&Expression) -> Result<Expression, RuleApplicationError>,
        rule_sets: &'a [(&'a str, u8)],
    ) -> Self {
        Self {
            name,
            application,
            rule_sets,
        }
    }

    pub fn apply(&self, expr: &Expression) -> Result<Expression, RuleApplicationError> {
        (self.application)(expr)
    }
}

impl<'a> Display for Rule<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl<'a> PartialEq for Rule<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl<'a> Eq for Rule<'a> {}

impl<'a> Hash for Rule<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

inventory::collect!(Rule<'static>);
