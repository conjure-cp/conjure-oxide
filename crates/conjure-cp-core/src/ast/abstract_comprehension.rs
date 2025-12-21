use super::SymbolTable;
use super::declaration::{DeclarationPtr, serde::DeclarationPtrFull};
use super::serde::RcRefCellAsInner;
use crate::ast::{DomainPtr, Expression, Name, ReturnType, Typeable};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::fmt::{Display, Formatter};
use std::{cell::RefCell, hash::Hash, hash::Hasher, rc::Rc};

#[serde_as]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub struct AbstractComprehension {
    pub return_expr: Expression,
    pub qualifiers: Vec<Qualifier>,

    /// The symbol table used in the return expression.
    ///
    /// Variables from generator expressions are "given" in the context of the return expression.
    /// That is, they are constants which are different for each expansion of the comprehension.
    #[serde_as(as = "RcRefCellAsInner")]
    pub return_expr_symbols: Rc<RefCell<SymbolTable>>,

    /// The scope for variables in generator expressions.
    ///
    /// Variables declared in generator expressions are decision variables, since they do not
    /// have a constant value.
    #[serde_as(as = "RcRefCellAsInner")]
    pub generator_symbols: Rc<RefCell<SymbolTable>>,
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
    pub decl: DeclarationPtr,
    pub expression: Expression,
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
    pub decl: DeclarationPtr,
}

#[serde_as]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
pub struct ExpressionGenerator {
    #[serde_as(as = "DeclarationPtrFull")]
    pub decl: DeclarationPtr,
    pub expression: Expression,
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
        self.return_expr_symbols.borrow().hash(state);
        self.return_expr.hash(state);
        self.qualifiers.hash(state);
    }
}

pub struct AbstractComprehensionBuilder {
    pub qualifiers: Vec<Qualifier>,

    /// The symbol table used in the return expression.
    ///
    /// Variables from generator expressions are "given" in the context of the return expression.
    /// That is, they are constants which are different for each expansion of the comprehension.
    pub return_expr_symbols: Rc<RefCell<SymbolTable>>,

    /// The scope for variables in generator expressions.
    ///
    /// Variables declared in generator expressions are decision variables in their original
    /// context, since they do not have a constant value.
    pub generator_symbols: Rc<RefCell<SymbolTable>>,
}

impl AbstractComprehensionBuilder {
    /// Creates an [AbstractComprehensionBuilder] with:
    /// - An inner scope which inherits from the given symbol table
    /// - An empty list of qualifiers
    ///
    /// Changes to the inner scope do not affect the given symbol table.
    ///
    /// The return expression is passed when finalizing the comprehension, in [with_return_value].
    pub fn new(symbols: &Rc<RefCell<SymbolTable>>) -> Self {
        Self {
            qualifiers: vec![],
            return_expr_symbols: Rc::new(RefCell::new(SymbolTable::with_parent(symbols.clone()))),
            generator_symbols: Rc::new(RefCell::new(SymbolTable::with_parent(symbols.clone()))),
        }
    }

    pub fn return_expr_symbols(&self) -> Rc<RefCell<SymbolTable>> {
        self.return_expr_symbols.clone()
    }

    pub fn generator_symbols(&self) -> Rc<RefCell<SymbolTable>> {
        self.generator_symbols.clone()
    }

    pub fn new_domain_generator(&mut self, domain: DomainPtr) -> DeclarationPtr {
        let generator_decl = self.return_expr_symbols.borrow_mut().gensym(&domain);

        self.qualifiers
            .push(Qualifier::Generator(Generator::DomainGenerator(
                DomainGenerator {
                    decl: generator_decl.clone(),
                },
            )));

        generator_decl
    }

    /// Creates a new expression generator with the given expression and variable name.
    ///
    /// The variable "takes from" the expression, that is, it can be any element in the expression.
    ///
    /// E.g. in `[ x | x <- some_set ]`, `x` can be any element of `some_set`.
    pub fn new_expression_generator(mut self, expr: Expression, name: Name) -> Self {
        let domain = expr
            .domain_of()
            .expect("Expression must have a domain")
            .element_domain()
            .expect("Expression must contain elements with uniform domain");

        // The variable is given (a constant) in the return expression, and a decision var
        // in the generator expression
        let generator_ptr = DeclarationPtr::new_var(name, domain);
        let return_expr_ptr = DeclarationPtr::new_given_quantified(&generator_ptr)
            .expect("Return expression declaration must not be None");

        self.return_expr_symbols
            .borrow_mut()
            .insert(return_expr_ptr);
        self.generator_symbols
            .borrow_mut()
            .insert(generator_ptr.clone());

        self.qualifiers
            .push(Qualifier::Generator(Generator::ExpressionGenerator(
                ExpressionGenerator {
                    decl: generator_ptr,
                    expression: expr,
                },
            )));

        self
    }

    /// See [crate::ast::comprehension::ComprehensionBuilder::guard]
    pub fn add_condition(&mut self, condition: Expression) {
        if condition.return_type() != ReturnType::Bool {
            panic!("Condition expression must have boolean return type");
        }

        self.qualifiers.push(Qualifier::Condition(condition));
    }

    pub fn new_letting(&mut self, expression: Expression) -> DeclarationPtr {
        let letting_decl = self.return_expr_symbols.borrow_mut().gensym(
            &expression
                .domain_of()
                .expect("Expression must have a domain"),
        );

        self.qualifiers
            .push(Qualifier::ComprehensionLetting(ComprehensionLetting {
                decl: letting_decl.clone(),
                expression,
            }));

        letting_decl
    }

    // The lack of the generator_symboltable and return_expr_symboltable
    // are explained bc 1. we dont have separate symboltables for each part
    // 2. it is unclear why there would be a need to access each one uniquely

    pub fn with_return_value(self, expression: Expression) -> AbstractComprehension {
        AbstractComprehension {
            return_expr: expression,
            qualifiers: self.qualifiers,
            return_expr_symbols: self.return_expr_symbols,
            generator_symbols: self.generator_symbols,
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
