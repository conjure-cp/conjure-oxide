use super::types::{LookupFn, ReprAssignment, ReprDeclLevel, ReprDomainLevel};
use crate::ast::{
    DeclarationKind, DomainPtr, Expression, Literal, Name, SymbolTable, eval_constant,
};
use conjure_cp_core::ast::DeclarationPtr;
use funcmap::FuncMap;
use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;

pub fn try_up_via(decl: DeclarationPtr, lu: &LookupFn<'_>) -> Result<Literal, String> {
    // Look up the variable directly
    if let Some(res) = lu(&decl) {
        return Ok(res);
    }

    // No value for this variable, try to go up via representations
    for (repr_name, repr) in decl.reprs().iter() {
        if let Ok(res) = repr.up_via(lu) {
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
