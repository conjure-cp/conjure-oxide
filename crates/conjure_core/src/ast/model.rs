use std::cell::{Ref, RefCell, RefMut};
use std::collections::VecDeque;
use std::fmt::{Debug, Display};
use std::rc::Rc;
use std::sync::{Arc, RwLock};

use derivative::Derivative;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use uniplate::{Biplate, Tree, Uniplate};

use crate::ast::serde::RcRefCellAsInner;
use crate::ast::{Expression, SymbolTable};
use crate::bug;
use crate::context::Context;

use crate::ast::pretty::{
    pretty_domain_letting_declaration, pretty_expressions_as_top_level,
    pretty_value_letting_declaration, pretty_variable_declaration,
};
use crate::metadata::Metadata;

use super::declaration::DeclarationKind;
use super::types::Typeable;
use super::ReturnType;

/// Represents a computational model containing variables, constraints, and a shared context.
///
/// To de/serialise a model using serde, see [`SerdeModel`].
#[derive(Derivative, Clone, Debug)]
#[derivative(PartialEq, Eq)]
pub struct Model {
    /// Top level constraints. This should be a `Expression::Root`.
    constraints: Box<Expression>,

    symbols: Rc<RefCell<SymbolTable>>,

    #[derivative(PartialEq = "ignore")]
    pub context: Arc<RwLock<Context<'static>>>,
}

impl Model {
    /// Creates a new model.
    pub fn new(
        symbols: Rc<RefCell<SymbolTable>>,
        constraints: Vec<Expression>,
        context: Arc<RwLock<Context<'static>>>,
    ) -> Model {
        Model {
            symbols,
            constraints: Box::new(Expression::Root(Metadata::new(), constraints)),
            context,
        }
    }

    pub fn new_empty(context: Arc<RwLock<Context<'static>>>) -> Model {
        Model::new(Default::default(), Vec::new(), context)
    }

    /// The symbol table for this model as a pointer.
    ///
    /// The caller should only mutate the returned symbol table if this method was called on a
    /// mutable model.
    pub fn symbols_ptr_unchecked(&self) -> &Rc<RefCell<SymbolTable>> {
        &self.symbols
    }

    /// The global symbol table for this model as a reference.
    pub fn symbols(&self) -> Ref<SymbolTable> {
        (*self.symbols).borrow()
    }

    /// The global symbol table for this model as a mutable reference.
    pub fn symbols_mut(&self) -> RefMut<SymbolTable> {
        (*self.symbols).borrow_mut()
    }

    pub fn get_constraints_vec(&self) -> Vec<Expression> {
        match *self.constraints {
            Expression::Root(_, ref exprs) => exprs.clone(),
            ref e => {
                bug!(
                    "get_constraints_vec: unexpected top level expression, {} ",
                    e
                );
            }
        }
    }

    pub fn set_constraints(&mut self, constraints: Vec<Expression>) {
        self.constraints = Box::new(Expression::Root(Metadata::new(), constraints));
    }

    pub fn set_context(&mut self, context: Arc<RwLock<Context<'static>>>) {
        self.context = context;
    }

    pub fn add_constraint(&mut self, expression: Expression) {
        // TODO (gs248): there is no checking whatsoever
        // We need to properly validate the expression but this is just for testing
        let mut constraints = self.get_constraints_vec();
        constraints.push(expression);
        self.set_constraints(constraints);
    }

    pub fn add_constraints(&mut self, expressions: Vec<Expression>) {
        let mut constraints = self.get_constraints_vec();
        constraints.extend(expressions);
        self.set_constraints(constraints);
    }
}

impl Typeable for Model {
    fn return_type(&self) -> Option<ReturnType> {
        Some(ReturnType::Bool)
    }
}

// At time of writing (03/02/2025), the Uniplate derive macro doesn't like the lifetimes inside
// context, and we do not yet have a way of ignoring this field.
impl Uniplate for Model {
    fn uniplate(&self) -> (Tree<Self>, Box<dyn Fn(Tree<Self>) -> Self>) {
        // Model contains no sub-models.
        let self2 = self.clone();
        (Tree::Zero, Box::new(move |_| self2.clone()))
    }
}

// TODO: replace with derive macro when possible.
impl Biplate<Expression> for Model {
    fn biplate(&self) -> (Tree<Expression>, Box<dyn Fn(Tree<Expression>) -> Self>) {
        let (symtab_tree, symtab_ctx) = (*self.symbols).borrow().biplate();
        let (constraints_tree, constraints_ctx) = self.constraints.biplate();

        let tree = Tree::Many(VecDeque::from([symtab_tree, constraints_tree]));

        let self2 = self.clone();
        let ctx = Box::new(move |tree| {
            let Tree::Many(fields) = tree else {
                panic!("number of children changed!");
            };

            let mut self3 = self2.clone();
            {
                let mut symbols = (*self3.symbols).borrow_mut();
                *symbols = (symtab_ctx)(fields[0].clone());
            }
            self3.constraints = Box::new((constraints_ctx)(fields[1].clone()));
            self3
        });

        (tree, ctx)
    }
}

impl Display for Model {
    #[allow(clippy::unwrap_used)] // [rustdocs]: should only fail iff the formatter fails
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (name, decl) in self.symbols().clone().into_iter_local() {
            match decl.kind() {
                DeclarationKind::DecisionVariable(_) => {
                    writeln!(
                        f,
                        "{}",
                        pretty_variable_declaration(&self.symbols(), &name).unwrap()
                    )?;
                }
                DeclarationKind::ValueLetting(_) => {
                    writeln!(
                        f,
                        "{}",
                        pretty_value_letting_declaration(&self.symbols(), &name).unwrap()
                    )?;
                }
                DeclarationKind::DomainLetting(_) => {
                    writeln!(
                        f,
                        "{}",
                        pretty_domain_letting_declaration(&self.symbols(), &name).unwrap()
                    )?;
                }
            }
        }

        writeln!(f, "\nsuch that\n")?;

        writeln!(
            f,
            "{}",
            pretty_expressions_as_top_level(&self.get_constraints_vec())
        )?;

        Ok(())
    }
}

/// A model that is de/serializable using `serde`.
///
/// To turn this into a rewritable model, it needs to be initialised using [`initialise`](SerdeModel::initialise).
///
/// To deserialise a [`Model`], use `.into()` to convert it into a `SerdeModel` first.
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerdeModel {
    constraints: Box<Expression>,

    #[serde_as(as = "RcRefCellAsInner")]
    symbols: Rc<RefCell<SymbolTable>>,
}

impl SerdeModel {
    /// Initialises the model for rewriting.
    pub fn initialise(self, context: Arc<RwLock<Context<'static>>>) -> Option<Model> {
        // TODO: Once we have submodels and multiple symbol tables, de-duplicate deserialized
        // Rc<RefCell<>> symbol tables and declarations using their stored ids.
        //
        // See ast::serde::RcRefCellAsId.
        Some(Model {
            constraints: self.constraints,
            symbols: self.symbols,
            context,
        })
    }
}

impl From<Model> for SerdeModel {
    fn from(val: Model) -> Self {
        SerdeModel {
            constraints: val.constraints,
            symbols: val.symbols,
        }
    }
}
