use std::collections::BTreeMap;

use core::fmt::Debug;

//TODO: write good documentation on this! ~niklasdewally

use crate::{
    ast::{DeclarationPtr, Expression, Literal, Name, SymbolTable},
    rule_engine::ApplicationError,
};

pub trait Representation: Send + Sync + Debug {
    /// Creates a representation object for the given name.
    fn init(name: &Name, symtab: &SymbolTable) -> Option<Self>
    where
        Self: Sized;

    /// The variable being represented.
    fn variable_name(&self) -> &Name;

    /// Given an assignment for `self`, creates assignments for its representation variables.
    fn value_down(&self, value: Literal) -> Result<BTreeMap<Name, Literal>, ApplicationError>;

    /// Given assignments for its representation variables, creates an assignment for `self`.
    fn value_up(&self, values: &BTreeMap<Name, Literal>) -> Result<Literal, ApplicationError>;

    /// Returns [`Expression`]s representing each representation variable.
    fn expression_down(
        &self,
        symtab: &SymbolTable,
    ) -> Result<BTreeMap<Name, Expression>, ApplicationError>;

    /// Creates declarations for the representation variables of `self`.
    fn declaration_down(&self) -> Result<Vec<DeclarationPtr>, ApplicationError>;

    /// The rule name for this representaion.
    fn repr_name(&self) -> &str;

    /// The identity of this representation.
    fn repr_id(&self) -> super::ReprId;

    /// Makes a clone of `self` into a `Representation` trait object.
    fn box_clone(&self) -> Box<dyn Representation>;
}

impl Clone for Box<dyn Representation> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}
