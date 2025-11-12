use serde::{Deserialize, Serialize};
use super::name::Name;
use uniplate::{Uniplate};
use crate::ast::{Domain, Expression};

#[derive(Clone, PartialEq, Eq, Uniplate, Serialize, Deserialize, Debug)]
pub struct AbstractComprehension {
    pub return_expr: Expression,
    pub qualifiers: Vec<Qualifier>,
}

#[derive(Clone, PartialEq, Eq, Uniplate, Serialize, Deserialize, Debug)]
pub enum Qualifier {
    Generator(Generator),
    Condition(Expression),
    ComprehensionLetting(Name, Expression),
}

#[derive(Clone, PartialEq, Eq, Uniplate, Serialize, Deserialize, Debug)]
pub enum Generator {
    DomainGenerator(Name, Domain, Expression),
    ExpressionGenerator(Name, Expression),
}

impl AbstractComprehension {
    pub fn new(return_expr: Expression, qualifiers: Vec<Qualifier>) -> Self {
        Self {
            return_expr,
            qualifiers,
        }
    }
}