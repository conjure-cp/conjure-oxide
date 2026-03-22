use super::errors::ReprUpError;
use super::types::LookupFn;
use crate::ast::{Literal, Name};
use conjure_cp_core::ast::DeclarationPtr;
use std::collections::HashMap;

pub fn try_up_via(decl: DeclarationPtr, lu: &LookupFn<'_>) -> Result<Literal, ReprUpError> {
    // Look up the variable directly
    if let Some(res) = lu(&decl) {
        return Ok(res);
    }

    // Variable not mapped to a value and has no representations
    let reprs = decl.reprs();
    if reprs.is_empty() {
        return Err(ReprUpError::NotFound(decl.clone()));
    }

    // Go up via the first representation
    let mut itr = reprs.iter();
    let (fst_name, fst) = itr.next().expect("checked that reprs is non-empty");
    let fst_res = fst.up_via(lu)?;

    // In debug mode, check that all other representations agree
    #[cfg(debug_assertions)]
    for (repr_name, repr) in itr {
        let res = repr.up_via(lu)?;
        assert_eq!(
            res,
            fst_res,
            "representations `{}` and `{}` disagree for variable `{}`",
            fst_name,
            repr_name,
            decl.name()
        );
    }

    Ok(fst_res)
}

pub fn try_up(
    decl: DeclarationPtr,
    raw_assignment: &HashMap<Name, Literal>,
) -> Result<Literal, ReprUpError> {
    let lu: LookupFn<'_> =
        Box::new(|decl: &DeclarationPtr| raw_assignment.get(&decl.name()).cloned());
    try_up_via(decl, &lu)
}
