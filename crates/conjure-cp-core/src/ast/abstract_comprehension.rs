use super::name::Name;
use crate::ast::{Domain, Expression};
use minion_sys::ast::SymbolTable;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, fmt::{Display, Formatter}, rc::Rc};
use uniplate::Uniplate;

#[derive(Clone, PartialEq, Eq, Uniplate, Serialize, Deserialize, Debug)]
pub struct AbstractComprehension {
    pub return_expr: Expression,
    pub qualifiers: Vec<Qualifier>,
    pub symbols: Rc<RefCell<SymbolTable>>,
}

#[derive(Clone, PartialEq, Eq, Uniplate, Serialize, Deserialize, Debug)]
pub enum Qualifier {
    Generator(Generator),
    Condition(Expression),
    ComprehensionLetting(Name, Expression),
}

#[derive(Clone, PartialEq, Eq, Uniplate, Serialize, Deserialize, Debug)]
pub enum Generator {
    DomainGenerator(Name, Domain),
    ExpressionGenerator(Name, Expression),
}

impl AbstractComprehension {
    pub fn new(return_expr: Expression, qualifiers: Vec<Qualifier>, symbols: Rc<RefCell<SymbolTable>>) -> Self {
        Self {
            return_expr,
            qualifiers,
            symbols,
        }
    }

    pub fn domain_of(&self) -> Option<Domain> {
        self.return_expr.domain_of()
    }
}

impl Display for AbstractComprehension {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[ {} | ", self.return_expr)?;
        let mut first = true;
        for qualifier in &self.qualifiers {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            qualifier.fmt(f)?;
        }
        write!(f, " ]")
    }
}

impl Display for Qualifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Qualifier::Generator(generator) => generator.fmt(f),
            Qualifier::Condition(condition) => condition.fmt(f),
            Qualifier::ComprehensionLetting(name, expr) => {
                write!(f, "letting {} = {}", name, expr)
            }
        }
    }
}

impl Display for Generator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Generator::DomainGenerator(name, domain) => {
                write!(f, "{} : {}", name, domain)
            }
            Generator::ExpressionGenerator(name, expr) => {
                write!(f, "{} <- {}", name, expr)
            }
        }
    }
}
