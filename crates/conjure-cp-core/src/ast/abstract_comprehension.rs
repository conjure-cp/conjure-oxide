use super::{Name, SymbolTable};
use crate::ast::{DeclarationPtr, Domain, Expression, ReturnType, Typeable};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    fmt::{Display, Formatter},
    rc::Rc,
};
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
    pub fn new(
        return_expr: Expression,
        qualifiers: Vec<Qualifier>,
        symbols: Rc<RefCell<SymbolTable>>,
    ) -> Self {
        Self {
            return_expr,
            qualifiers,
            symbols,
        }
    }

    pub fn domain_of(&self) -> Option<Domain> {
        self.return_expr.domain_of()
    }

    pub fn add_domain_generator(&mut self, name: Name, domain: Domain) {
        self.symbols.borrow_mut().insert(DeclarationPtr::new_var(name.clone(), domain.clone()));
        self.qualifiers
            .push(Qualifier::Generator(Generator::DomainGenerator(
                name, domain,
            )));
    }

    pub fn add_expression_generator(&mut self, name: Name, expr: Expression) {
        self.symbols.borrow_mut().insert(DeclarationPtr::new_var(name.clone(), expr.domain_of().unwrap()));
        self.qualifiers
            .push(Qualifier::Generator(Generator::ExpressionGenerator(
                name, expr,
            )));
    }

    pub fn add_condition(&mut self, condition: Expression) {
        if condition.return_type() != Some(ReturnType::Bool) {
            panic!("Condition expression must have boolean return type");
        }

        self.qualifiers.push(Qualifier::Condition(condition));
    }

    pub fn add_letting(&mut self, name: Name, expr: Expression) {
        self.qualifiers
            .push(Qualifier::ComprehensionLetting(name, expr));
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
