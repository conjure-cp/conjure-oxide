use super::errors::ReprUpError;
use super::types::LookupFn;
use crate::ast::{GroundDomain, Literal, Name};
use conjure_cp_core::ast::DeclarationPtr;
use std::collections::HashMap;

pub fn try_up_via(decl: DeclarationPtr, lu: &LookupFn<'_>) -> Result<Literal, ReprUpError> {
    // Look up the variable directly
    if let Some(mut result) = lu(&decl) {
        if decl
            .domain()
            .and_then(|domain| domain.resolve().ok())
            .is_some_and(|domain| domain.as_ref() == &GroundDomain::Bool)
        {
            result = match result {
                Literal::Int(0) => Literal::Bool(false),
                Literal::Int(1) => Literal::Bool(true),
                result => result,
            };
        }
        return Ok(result);
    }

    // Variable not mapped to a value and has no representations
    let reprs = decl.reprs();
    if reprs.is_empty() {
        return Err(ReprUpError::NotFound(decl.clone()));
    }

    // Go up via the first representation
    let mut itr = reprs.iter();
    let (_fst_name, fst) = itr.next().expect("checked that reprs is non-empty");
    let fst_res = fst.up_via(lu)?;

    // In debug mode, check that all other representations agree
    #[cfg(debug_assertions)]
    for (repr_name, repr) in itr {
        let res = repr.up_via(lu)?;
        assert_eq!(
            res,
            fst_res,
            "representations `{}` and `{}` disagree for variable `{}`",
            _fst_name,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{DeclarationPtr, Domain};
    use crate::{domain_int, range};

    #[test]
    fn represented_boolean_leaf_coerces_minion_zero_one_values() {
        let declaration = DeclarationPtr::new_find(Name::user("flag"), Domain::bool());
        let lookup: LookupFn<'_> = Box::new(|_| Some(Literal::Int(1)));
        assert_eq!(
            try_up_via(declaration, &lookup).unwrap(),
            Literal::Bool(true)
        );
    }

    #[test]
    fn integer_zero_one_leaf_remains_an_integer() {
        let declaration = DeclarationPtr::new_find(Name::user("value"), domain_int!(0..1));
        let lookup: LookupFn<'_> = Box::new(|_| Some(Literal::Int(1)));
        assert_eq!(try_up_via(declaration, &lookup).unwrap(), Literal::Int(1));
    }
}
