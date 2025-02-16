use std::fmt::{Debug, Display};
use std::sync::{Arc, RwLock};

use derivative::Derivative;
use serde::{Deserialize, Serialize};
use uniplate::{Biplate, Tree, Uniplate};

use crate::ast::Expression;
use crate::context::Context;

use super::types::Typeable;
use super::ReturnType;
use super::SubModel;

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

    #[derivative(PartialEq = "ignore")]
    pub context: Arc<RwLock<Context<'static>>>,
}

impl Model {
    /// Creates a new model.
    pub fn new(context: Arc<RwLock<Context<'static>>>) -> Model {
        Model {
            submodel: SubModel::new_top_level(),
            context,
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

        let self2 = self.clone();
        let ctx = Box::new(move |x| {
            let submodel = expr_ctx(x);
            let mut self3 = self2.clone();
            self3.replace_submodel(submodel);
            self3
        });

        (expr_tree, ctx)
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
}

impl SerdeModel {
    /// Initialises the model for rewriting.
    pub fn initialise(self, context: Arc<RwLock<Context<'static>>>) -> Option<Model> {
        // TODO: Once we have submodels and multiple symbol tables, de-duplicate deserialized
        // Rc<RefCell<>> symbol tables and declarations using their stored ids.
        //
        // See ast::serde::RcRefCellAsId.
        Some(Model {
            submodel: self.submodel,
            context,
        })
    }
}

impl From<Model> for SerdeModel {
    fn from(val: Model) -> Self {
        SerdeModel {
            submodel: val.submodel,
        }
    }
}

impl Display for SerdeModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.submodel, f)
    }
}
