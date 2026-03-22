use super::types::{LookupFn, ReprAssignment, ReprDeclLevel, ReprDomainLevel};
use crate::ast::{
    DeclarationKind, DomainPtr, Expression, Literal, Name, SymbolTable, eval_constant,
};
use conjure_cp_core::ast::DeclarationPtr;
use funcmap::FuncMap;
use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;

pub trait ReprStateStored: Any + Send + Sync + Debug {
    fn up(&self, lu: &LookupFn<'_>) -> Result<Literal, String>;

    fn as_any(&self) -> &dyn Any;

    fn clone_box(&self) -> Box<dyn ReprStateStored>;

    fn serialise(&self) -> Result<serde_json::Value, serde_json::Error>;
}

impl<D: ReprDeclLevel> ReprStateStored for D {
    fn up(&self, lu: &LookupFn<'_>) -> Result<Literal, String> {
        Ok(self.lookup_via(lu)?.up())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn ReprStateStored> {
        Box::new(self.clone())
    }

    fn serialise(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

pub fn try_up_via(decl: DeclarationPtr, lu: &LookupFn<'_>) -> Result<Literal, String> {
    // Look up the variable directly
    if let Some(res) = lu(&decl) {
        return Ok(res);
    }

    // No value for this variable, try to go up via representations
    for (repr_name, repr) in decl.reprs().iter() {
        if let Ok(res) = repr.up(lu) {
            return Ok(res);
        }
    }

    Err(format!(
        "None of the representations for '{}' could go up!",
        decl.name()
    ))
}

pub fn try_up(
    decl: DeclarationPtr,
    raw_assignment: &HashMap<Name, Literal>,
) -> Result<Literal, String> {
    let lu: LookupFn<'_> =
        Box::new(|decl: &DeclarationPtr| raw_assignment.get(&decl.name()).cloned());
    try_up_via(decl, &lu)
}
