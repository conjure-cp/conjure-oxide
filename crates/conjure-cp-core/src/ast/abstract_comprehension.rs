use super::SymbolTable;
use super::declaration::{DeclarationPtr, serde::DeclarationPtrFull};
use super::serde::RcRefCellAsInner;
use crate::ast::{DomainPtr, Expression, ReturnType, Typeable};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::fmt::{Display, Formatter};
use std::{cell::RefCell, hash::Hash, hash::Hasher, rc::Rc};

#[serde_as]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct AbstractComprehension {
    pub return_expr: Expression,
    pub qualifiers: Vec<Qualifier>,
    #[serde_as(as = "RcRefCellAsInner")]
    pub symbols: Rc<RefCell<SymbolTable>>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
pub enum Qualifier {
    Generator(Generator),
    Condition(Expression),
    ComprehensionLetting(ComprehensionLetting),
}

#[serde_as]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
pub struct ComprehensionLetting {
    #[serde_as(as = "DeclarationPtrFull")]
    decl: DeclarationPtr,
    expression: Expression,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
pub enum Generator {
    DomainGenerator(DomainGenerator),
    ExpressionGenerator(ExpressionGenerator),
}

#[serde_as]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
pub struct DomainGenerator {
    #[serde_as(as = "DeclarationPtrFull")]
    decl: DeclarationPtr,
}

#[serde_as]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
pub struct ExpressionGenerator {
    #[serde_as(as = "DeclarationPtrFull")]
    decl: DeclarationPtr,
    expression: Expression,
}

impl AbstractComprehension {
    pub fn domain_of(&self) -> Option<DomainPtr> {
        self.return_expr.domain_of()
    }
}

impl Typeable for AbstractComprehension {
    fn return_type(&self) -> ReturnType {
        self.return_expr.return_type()
    }
}

impl Hash for AbstractComprehension {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.symbols.borrow().hash(state);
        self.return_expr.hash(state);
        self.qualifiers.hash(state);
    }
}

pub struct AbstractComprehensionBuilder {
    pub qualifiers: Vec<Qualifier>,
    pub symbols: Rc<RefCell<SymbolTable>>,
}

//this is the method that allows you to build an abstract comprehension
impl AbstractComprehensionBuilder {
    // this method creates an abstract comprehension builder with:
    // a symbol table
    // empty list of qualifiers
    // and no return exp yet -- that will be added in the with_return_value method
    pub fn new(symbols: Rc<RefCell<SymbolTable>>) -> Self {
        Self {
            qualifiers: vec![],
            symbols: symbols,
        }
    }

    pub fn new_domain_generator(&mut self, domain: DomainPtr) -> DeclarationPtr {
        let generator_decl = self.symbols.borrow_mut().gensym(&domain);

        self.qualifiers
            .push(Qualifier::Generator(Generator::DomainGenerator(
                DomainGenerator {
                    decl: generator_decl.clone(),
                },
            )));

        generator_decl
    }

    pub fn new_expression_generator(&mut self, expr: Expression) -> DeclarationPtr {
        let generator_decl = self
            .symbols
            .borrow_mut()
            .gensym(&expr.domain_of().expect("Expression must have a domain"));

        self.qualifiers
            .push(Qualifier::Generator(Generator::ExpressionGenerator(
                ExpressionGenerator {
                    decl: generator_decl.clone(),
                    expression: expr,
                },
            )));

        generator_decl
    }

    //this is the same as the add guard method
    pub fn add_condition(&mut self, condition: Expression) {
        if condition.return_type() != ReturnType::Bool {
            panic!("Condition expression must have boolean return type");
        }

        self.qualifiers.push(Qualifier::Condition(condition));
    }

    pub fn new_letting(&mut self, expression: Expression) -> DeclarationPtr {
        let letting_decl = self.symbols.borrow_mut().gensym(
            &expression
                .domain_of()
                .expect("Expression must have a domain"),
        );

        self.qualifiers
            .push(Qualifier::ComprehensionLetting(ComprehensionLetting {
                decl: letting_decl.clone(),
                expression: expression,
            }));

        letting_decl
    }

    //the lack of the generator_symboltable and return_expr_symboltable
    // are explained bc 1. we dont have separate symboltables for each part
    // 2. it is unclear why there would be a need to access each one uniquely

    // TODO: make a pub fn with_return_value :)
    pub fn with_return_value(self, expression: Expression) -> AbstractComprehension {
        AbstractComprehension {
            return_expr: expression,
            qualifiers: self.qualifiers,
            symbols: self.symbols,
        }
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
            Qualifier::ComprehensionLetting(comp_letting) => {
                let name = comp_letting.decl.name();
                let expr = &comp_letting.expression;
                write!(f, "letting {} = {}", name, expr)
            }
        }
    }
}

impl Display for Generator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Generator::DomainGenerator(DomainGenerator { decl }) => {
                let name = decl.name();
                let domain = decl.domain().unwrap();
                write!(f, "{} : {}", name, domain)
            }
            Generator::ExpressionGenerator(ExpressionGenerator { decl, expression }) => {
                let name = decl.name();
                write!(f, "{} <- {}", name, expression)
            }
        }
    }
}
