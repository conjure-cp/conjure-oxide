use std::fmt::{self, Display, Formatter};

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
}

impl<'a> Rule<'a> {
    pub fn new(
        name: &'a str,
        application: fn(&Expression) -> Result<Expression, RuleApplicationError>,
    ) -> Self {
        Self { name, application }
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
