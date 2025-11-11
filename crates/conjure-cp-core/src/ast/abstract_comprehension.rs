use serde::{Deserialize, Serialize};
use uniplate::{Biplate, Uniplate};
use crate::ast::{DeclarationPtr, Domain, Expression, SubModel};

#[derive(Clone, PartialEq, Eq, Uniplate, Serialize, Deserialize, Debug)]
pub struct AbstractComprehension {
    pub return_expr: Expression,
    pub qualifiers: Vec<Qualifier>,
}

#[derive(Clone, PartialEq, Eq, Uniplate, Serialize,Deserialize, Debug)]
pub enum Qualifier {
    Generator(Generator),
    Condition(Expression),
    ComprehensionLetting(DeclarationPtr, Expression),
}

#[derive(Clone, PartialEq, Eq, Uniplate, Serialize, Deserialize, Debug)]
pub enum Generator {
    DomainGenerator(DeclarationPtr, Domain, Expression),
    ExpressionGenerator(DeclarationPtr, Expression),
}