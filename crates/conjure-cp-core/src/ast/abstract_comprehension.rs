use super::SymbolTable;
use super::declaration::{DeclarationPtr, serde::DeclarationPtrFull};
use super::serde::RcRefCellAsInner;
use crate::ast::{DomainPtr, Expression, Name, ReturnType, Typeable};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use uniplate::{Biplate, Uniplate, Tree};
use std::collections::VecDeque;
use std::fmt::{Display, Formatter};
use std::{cell::RefCell, hash::Hash, hash::Hasher, rc::Rc};

#[serde_as]
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug, Uniplate)]
#[biplate(to=Expression)]
pub struct AbstractComprehension {
    pub return_expr: Expression,
    pub qualifiers: Vec<Qualifier>,

    #[serde_as(as = "RcRefCellAsInner")]
    pub symbols: Rc<RefCell<SymbolTable>>,
}

// FIXME: remove this: https://github.com/conjure-cp/conjure-oxide/issues/1428
impl Biplate<SymbolTable> for AbstractComprehension {
    fn biplate(
        &self,
    ) -> (
        uniplate::Tree<SymbolTable>,
        Box<dyn Fn(uniplate::Tree<SymbolTable>) -> Self>,
    ) {
        let symbols: SymbolTable = (*self.symbols).borrow().clone();

        let (tables_in_exprs_tree, tables_in_exprs_ctx) =
            Biplate::<SymbolTable>::biplate(&Biplate::<Expression>::children_bi(self));

        let tree = Tree::Many(VecDeque::from([
            Tree::One(symbols),
            tables_in_exprs_tree,
        ]));

        let self2 = self.clone();
        let ctx = Box::new(move |tree: Tree<SymbolTable>| {
            let Tree::Many(vs) = tree else {
                panic!();
            };

            let Tree::One(symbols) = vs[0].clone() else {
                panic!();
            };

            let self3 = self2.with_children_bi(tables_in_exprs_ctx(vs[1].clone()));

            // WARN: I can't remember if i should change inside the refcell here, or make an new
            // one (resulting in this symbol table being detached).

            *(self3.symbols.borrow_mut()) = symbols;

            self3
        });

        (tree, ctx)
    }
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
        (*self.symbols).borrow().hash(state);
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
    pub fn new(symbols: &Rc<RefCell<SymbolTable>>) -> Self {
        Self {
            qualifiers: vec![],
            symbols: Rc::new(RefCell::new(SymbolTable::with_parent(symbols.clone()))),
        }
    }

    pub fn symbols(&self) -> Rc<RefCell<SymbolTable>> {
        self.symbols.clone()
    }

    pub fn add_domain_generator(mut self, domain: DomainPtr, name: Name) {
        let generator_decl = DeclarationPtr::new_var_quantified(name, domain);
        self.symbols.borrow_mut().update_insert(generator_decl.clone());

        self.qualifiers
            .push(Qualifier::Generator(Generator::DomainGenerator(
                DomainGenerator {
                    decl: generator_decl.clone(),
                },
            )));
    }

    pub fn new_domain_generator(self, domain: DomainPtr) -> DeclarationPtr {
        let generator_decl = self.symbols.borrow_mut().gensym(&domain);
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
        let generator_decl = DeclarationPtr::new_var_quantified(name, domain.clone());
        self.symbols.borrow_mut().update_insert(generator_decl.clone());

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

        let decl_ptr = self.symbols.borrow_mut().gensym(&domain);
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
        let letting_decl = self.symbols.borrow_mut().gensym(
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
