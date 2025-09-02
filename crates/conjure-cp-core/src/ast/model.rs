#![allow(clippy::arc_with_non_send_sync)] // uniplate needs this
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fmt::{Debug, Display};
use std::rc::Rc;
use std::sync::{Arc, RwLock};

use derivative::Derivative;
use serde::{Deserialize, Serialize};
use uniplate::{Biplate, Tree, Uniplate};

use crate::ast::Expression;
use crate::context::Context;

use super::serde::{HasId, ObjId};
use super::types::Typeable;
use super::{DeclarationPtr, Name, SubModel};
use super::{ReturnType, SymbolTable};

/// An Essence model.
///
/// - This type wraps a [`Submodel`] containing the top-level lexical scope. To manipulate the
///   model's constraints or symbols, first convert it to a [`Submodel`] using
///   [`as_submodel`](Model::as_submodel) / [`as_submodel_mut`](Model::as_submodel_mut).
///
/// - To de/serialise a model using `serde`, see [`SerdeModel`].
#[derive(Derivative, Clone, Debug)]
#[derivative(PartialEq, Eq)]
pub struct Model {
    submodel: SubModel,
    pub search_order: Option<Vec<Name>>,
    pub dominance: Option<Expression>,
    #[derivative(PartialEq = "ignore")]
    pub context: Arc<RwLock<Context<'static>>>,
}

impl Model {
    pub fn from_submodel(submodel: SubModel) -> Model {
        Model {
            submodel,
            ..Default::default()
        }
    }

    /// Creates a new model from the given context.
    pub fn new(context: Arc<RwLock<Context<'static>>>) -> Model {
        Model {
            submodel: SubModel::new_top_level(),
            dominance: None,
            context,
            search_order: None,
        }
    }

    /// Returns this model as a [`Submodel`].
    pub fn as_submodel(&self) -> &SubModel {
        &self.submodel
    }

    /// Returns this model as a mutable [`Submodel`].
    pub fn as_submodel_mut(&mut self) -> &mut SubModel {
        &mut self.submodel
    }

    /// Replaces the model contents with `new_submodel`, returning the old contents.
    pub fn replace_submodel(&mut self, new_submodel: SubModel) -> SubModel {
        std::mem::replace(self.as_submodel_mut(), new_submodel)
    }
}

impl Default for Model {
    fn default() -> Self {
        Model {
            submodel: SubModel::new_top_level(),
            dominance: None,
            context: Arc::new(RwLock::new(Context::default())),
            search_order: None,
        }
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

impl Biplate<Expression> for Model {
    fn biplate(&self) -> (Tree<Expression>, Box<dyn Fn(Tree<Expression>) -> Self>) {
        // walk into submodel
        let submodel = self.as_submodel();
        let (expr_tree, expr_ctx) = <SubModel as Biplate<Expression>>::biplate(submodel);

        // walk into dominance relation if it exists
        let dom_tree = match &self.dominance {
            Some(expr) => Tree::One(expr.clone()),
            None => Tree::Zero,
        };
        let tree = Tree::<Expression>::Many(VecDeque::from([expr_tree, dom_tree]));

        let self2 = self.clone();
        let ctx = Box::new(move |x| match x {
            Tree::Many(xs) => {
                if xs.len() != 2 {
                    panic!("Expected a tree with two children");
                }
                let submodel_tree = xs[0].clone();
                let dom_tree = xs[1].clone();

                // reconstruct the submodel
                let submodel = expr_ctx(submodel_tree);
                // reconstruct the dominance relation
                let dominance = match dom_tree {
                    Tree::One(expr) => Some(expr),
                    Tree::Zero => None,
                    _ => panic!("Expected a tree with two children"),
                };

                let mut self3 = self2.clone();
                self3.replace_submodel(submodel);
                self3.dominance = dominance;
                self3
            }
            _ => {
                panic!("Expected a tree with two children");
            }
        });

        (tree, ctx)
    }
}

impl Biplate<SubModel> for Model {
    fn biplate(&self) -> (Tree<SubModel>, Box<dyn Fn(Tree<SubModel>) -> Self>) {
        let submodel = self.as_submodel().clone();

        let self2 = self.clone();
        let ctx = Box::new(move |x| {
            let Tree::One(submodel) = x else {
                panic!();
            };

            let mut self3 = self2.clone();
            self3.replace_submodel(submodel);
            self3
        });

        (Tree::One(submodel), ctx)
    }
}

impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.as_submodel(), f)
    }
}

/// A model that is de/serializable using `serde`.
///
/// To turn this into a rewritable model, it needs to be initialised using [`initialise`](SerdeModel::initialise).
///
/// To deserialise a [`Model`], use `.into()` to convert it into a `SerdeModel` first.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerdeModel {
    #[serde(flatten)]
    submodel: SubModel,
    search_order: Option<Vec<Name>>, // TODO: make this a [expressions]
    dominance: Option<Expression>,
}

impl SerdeModel {
    /// Initialises the model for rewriting.
    ///
    /// This swizzles the pointers to symbol tables and declarations using the stored ids.
    pub fn initialise(mut self, context: Arc<RwLock<Context<'static>>>) -> Option<Model> {
        // The definitive versions of each symbol table are stored in the submodels. Parent
        // pointers store dummy values with the correct ids, but nothing else. We need to replace
        // these dummy values with pointers to the actual parent symbol tables, using the ids to
        // know which tables should be equal.
        //
        // See super::serde::RcRefCellToInner, super::serde::RcRefCellToId.

        // Store the definitive versions of all symbol tables by id.
        let mut tables: HashMap<ObjId, Rc<RefCell<SymbolTable>>> = HashMap::new();

        // Find the definitive versions by traversing the sub-models.
        for submodel in self.submodel.universe() {
            let id = submodel.symbols().id();

            // ids should be unique!
            assert_eq!(
                tables.insert(id, submodel.symbols_ptr_unchecked().clone()),
                None
            );
        }

        // Restore parent pointers using `tables`.
        for table in tables.clone().into_values() {
            let mut table_mut = table.borrow_mut();
            let parent_mut = table_mut.parent_mut_unchecked();

            #[allow(clippy::unwrap_used)]
            if let Some(parent) = parent_mut {
                let parent_id = parent.borrow().id();

                *parent = tables.get(&parent_id).unwrap().clone();
            }
        }

        // The definitive versions of declarations are stored in the symbol table. References store
        // dummy values with the correct ids, but nothing else.

        // Store the definitive version of all declarations by id.
        let mut all_declarations: HashMap<ObjId, DeclarationPtr> = HashMap::new();
        for table in tables.values() {
            for (_, decl) in table.as_ref().borrow().clone().into_iter_local() {
                let id = decl.id();
                all_declarations.insert(id, decl);
            }
        }

        // Swizzle declaration pointers in expressions (references, auxdecls) using their ids and `all_declarations`.
        *self.submodel.constraints_mut() = self.submodel.constraints().transform_bi(&move |decl: DeclarationPtr| {
                let id = decl.id();
                        all_declarations
                            .get(&id)
                            .unwrap_or_else(|| panic!("A declaration used in the expression tree should exist in the symbol table. The missing declaration has id {id}."))
                            .clone()
        });

        Some(Model {
            submodel: self.submodel,
            dominance: self.dominance,
            context,
            search_order: self.search_order,
        })
    }
}

impl From<Model> for SerdeModel {
    fn from(val: Model) -> Self {
        SerdeModel {
            submodel: val.submodel,
            dominance: val.dominance,
            search_order: val.search_order,
        }
    }
}

impl Display for SerdeModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.submodel, f)
    }
}
