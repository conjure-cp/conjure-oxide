use super::{Name, SymbolTable};
use crate::ast::{DeclarationPtr, Domain, Expression, ReturnType, Typeable, ac_operators::ACOperatorKind};
use serde::{Deserialize, Serialize, RcRefCellAsInner};
use serde_with::serde_as;
use std::{
    cell::RefCell,
    fmt::{Display, Formatter},
    ops::Deref,
    rc::Rc,
};
use uniplate::Uniplate;

#[serde_as]
#[derive(Clone, PartialEq, Eq, Uniplate, Serialize, Deserialize, Debug)]
pub struct AbstractComprehension {
    pub return_expr: Expression,
    pub qualifiers: Vec<Qualifier>,
    #[serde_as(as = "RcRefCellAsInner")]
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

pub struct AbstractComprehensionBuilder{
    pub qualifiers: Vec<Qualifier>,
    pub symbols: Rc<RefCell<SymbolTable>>,
}

//this is the method that allows you to build an abstract comprehension
impl AbstractComprehensionBuilder {
    // this method creates an abstract comprehension builder with:
        // a symbol table
        // empty list of qualifiers
        // and no return exp yet -- that will be added in the with_return_value method
    pub fn new(
        symbols: Rc<RefCell<SymbolTable>>,
    ) -> Self {
        Self {
            qualifiers: vec![],
            symbols: Rc::new(RefCell::new(SymbolTable::with_parent(
                symbol_table_ptr,)))
        }
    }

    // TODO: figure out how to return a domain when this is dependent on the final result of the comprehension
    // Potentially unresolved domain?? tbd
    pub fn domain_of(&self) -> Option<Domain> {
        self.return_expr.domain_of()
    }

    pub fn new_domain_generator(&mut self, domain: Domain) -> Name {
        let name = self
            .symbols
            .borrow_mut()
            .gensym(&domain)
            .name()
            .deref()
            .to_owned();
        self.add_domain_generator(name.clone(), domain);
        name
    }

    pub fn add_domain_generator(&mut self, name: Name, domain: Domain) {
        self.symbols
            .borrow_mut()
            .insert(DeclarationPtr::new_var(name.clone(), domain.clone()));
        self.qualifiers
            .push(Qualifier::Generator(Generator::DomainGenerator(
                name, domain,
            )));
    }

    pub fn new_expression_generator(&mut self, expr: Expression) -> Name {
        let name = self
            .symbols
            .borrow_mut()
            .gensym(&expr.domain_of().expect("Expression must have a domain"))
            .name()
            .deref()
            .to_owned();
        self.add_expression_generator(name.clone(), expr);
        name
    }

    pub fn add_expression_generator(&mut self, name: Name, expr: Expression) {
        self.symbols.borrow_mut().insert(DeclarationPtr::new_var(
            name.clone(),
            expr.domain_of().unwrap(),
        ));
        self.qualifiers
            .push(Qualifier::Generator(Generator::ExpressionGenerator(
                name, expr,
            )));
    }

    //this is the same as the add guard method
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

    //the lack of the generator_symboltable and return_expr_symboltable
    // are explained bc 1. we dont have separate symboltables for each part
    // 2. it is unclear why there would be a need to access each one uniquely

    pub fn with_return_value(
        self, 
        mut expression: Expression,
    ) -> AbstractComprehension {
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
