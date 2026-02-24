use super::declaration::DeclarationPtr;
use super::serde::PtrAsInner;
use super::{DomainPtr, Expression, Name, ReturnType, SubModel, SymbolTablePtr, Typeable};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use uniplate::Uniplate;

#[serde_as]
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug, Uniplate)]
#[biplate(to=Expression)]
#[biplate(to=SubModel)]
pub struct AbstractComprehension {
    pub return_expr: Expression,
    pub qualifiers: Vec<Qualifier>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, Hash, Uniplate)]
#[biplate(to=Expression)]
#[biplate(to=SubModel)]
pub enum Qualifier {
    Generator(Generator),
    Condition(Expression),
    ComprehensionLetting(ComprehensionLetting),
}

#[serde_as]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
pub struct ComprehensionLetting {
    #[serde_as(as = "PtrAsInner")]
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
    #[serde_as(as = "PtrAsInner")]
    pub decl: DeclarationPtr,
}

#[serde_as]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, Hash)]
pub struct ExpressionGenerator {
    #[serde_as(as = "PtrAsInner")]
    pub decl: DeclarationPtr,
    pub expression: Expression,
}

impl AbstractComprehension {
    pub fn symbols(&self) -> Rc<RefCell<SymbolTable>> {
        Rc::clone(self.submodel.symbols_ptr_unchecked())
    }

    pub fn return_expr(&self) -> &Expression {
        self.submodel.constraints().first().unwrap()
    }

    pub fn domain_of(&self) -> Option<DomainPtr> {
        self.return_expr().domain_of()
    }
}

impl Typeable for AbstractComprehension {
    fn return_type(&self) -> ReturnType {
        self.return_expr().return_type()
    }
}

impl Hash for AbstractComprehension {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.return_expr.hash(state);
        self.qualifiers.hash(state);
    }
}

pub struct AbstractComprehensionBuilder {
    pub qualifiers: Vec<Qualifier>,
}

impl AbstractComprehensionBuilder {
    /// Creates an [AbstractComprehensionBuilder] with:
    /// - An inner scope which inherits from the given symbol table
    /// - An empty list of qualifiers
    ///
    /// Changes to the inner scope do not affect the given symbol table.
    ///
    /// The return expression is passed when finalizing the comprehension, in [with_return_value].
    pub fn new(symbols: &SymbolTablePtr) -> Self {
        Self {
            qualifiers: vec![],
        }
    }

    pub fn return_expr_symbols(&self) -> Rc<RefCell<SymbolTable>> {
        self.return_expr_symbols.clone()
    }

    pub fn generator_symbols(&self) -> Rc<RefCell<SymbolTable>> {
        self.generator_symbols.clone()
    }

    pub fn add_domain_generator(mut self, domain: DomainPtr, name: Name) {
        let generator_decl = DeclarationPtr::new_var_quantified(name, domain);
        self.symbols().borrow_mut().update_insert(generator_decl.clone());

        self.qualifiers
            .push(Qualifier::Generator(Generator::DomainGenerator(
                DomainGenerator {
                    decl: generator_decl.clone(),
                },
            )));
    }

    pub fn new_domain_generator(self, domain: DomainPtr) -> DeclarationPtr {
        let generator_decl = self.symbols().borrow_mut().gensym(&domain);
        let name = generator_decl.name().clone();

        self.add_domain_generator(domain, name);

        generator_decl
    }

    /// Creates a new expression generator with the given expression and variable name.
    ///
    /// The variable "takes from" the expression, that is, it can be any element in the expression.
    ///
    /// E.g. in `[ x | x <- some_set ]`, `x` can be any element of `some_set`.
    pub fn add_expression_generator(mut self, expr: Expression, name: Name) -> Self {
        let domain = expr
            .domain_of()
            .expect("Expression must have a domain")
            .element_domain()
            .expect("Expression must contain elements with uniform domain");

        // The variable is quantified in both scopes.
        let generator_ptr = DeclarationPtr::new_quantified(name, domain);
        let return_expr_ptr = DeclarationPtr::new_quantified_from_generator(&generator_ptr)
            .expect("Return expression declaration must not be None");

        self.return_expr_symbols.write().insert(return_expr_ptr);
        self.generator_symbols.write().insert(generator_ptr.clone());

        self.qualifiers
            .push(Qualifier::Generator(Generator::ExpressionGenerator(
                ExpressionGenerator {
                    decl: generator_decl,
                    expression: expr,
                },
            )));

        self
    }

    pub fn new_expression_generator(self, expr: Expression) -> DeclarationPtr {
        let domain = expr
            .domain_of()
            .expect("Expression must have a domain")
            .element_domain()
            .expect("Expression must contain elements with uniform domain");

        let decl_ptr = self.symbols().borrow_mut().gensym(&domain);
        let name = decl_ptr.name().clone();

        self.add_expression_generator(expr, name);

        decl_ptr
    }

    //this is the same as the add guard method
    pub fn add_condition(&mut self, condition: Expression) {
        if condition.return_type() != ReturnType::Bool {
            panic!("Condition expression must have boolean return type");
        }

        self.qualifiers.push(Qualifier::Condition(condition));
    }

    pub fn new_letting(&mut self, expression: Expression) -> DeclarationPtr {
        let letting_decl = self.return_expr_symbols.write().gensym(
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

    pub fn with_return_value(mut self, expression: Expression) -> AbstractComprehension {
        self.submodel.add_constraint(expression);
        AbstractComprehension {
            submodel: self.submodel,
            qualifiers: self.qualifiers,
        }
    }
}

impl Display for AbstractComprehension {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[ {} | ", self.return_expr())?;
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
