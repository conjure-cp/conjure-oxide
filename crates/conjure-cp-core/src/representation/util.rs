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

pub fn instantiate_default_impl<DomL, DecL, A, SF>(
    dom_level: DomL,
    decl: DeclarationPtr,
    repr_name: &str,
    structural: SF,
) -> (DecL, SymbolTable, Vec<Expression>)
where
    DomL: ReprDomainLevel<Assignment = A, DeclLevel = DecL>
        + FuncMap<DomainPtr, DeclarationPtr, Output = DecL>,
    A: FuncMap<Literal, DeclarationPtr, Output = DecL>,
    SF: Fn(&DecL) -> Vec<Expression>,
{
    let src_name = decl.name();
    let mut symtab = SymbolTable::new();
    let mut counter = 1;

    match &decl.kind() as &DeclarationKind {
        DeclarationKind::Find(_) => {
            let declare_field_var = |dom: DomainPtr| {
                let name = Name::repr(src_name.clone(), repr_name, &counter.to_string());
                let mut field_decl = DeclarationPtr::new_find(name, dom);
                *(field_decl.source_mut()) = Some(decl.clone());
                symtab
                    .insert(field_decl.clone())
                    .expect("declaration already exists");
                counter += 1;
                field_decl
            };

            let decl_level = dom_level.func_map(declare_field_var);
            let constraints = structural(&decl_level);
            (decl_level, symtab, constraints)
        }
        DeclarationKind::ValueLetting(expr, _) | DeclarationKind::TemporaryValueLetting(expr) => {
            let declare_field_value_letting = |lit: Literal| {
                let name = Name::repr(src_name.clone(), repr_name, &counter.to_string());
                let mut field_decl = DeclarationPtr::new_value_letting(name, lit.into());
                *(field_decl.source_mut()) = Some(decl.clone());
                symtab
                    .insert(field_decl.clone())
                    .expect("declaration already exists");
                counter += 1;
                field_decl
            };

            let val = eval_constant(&expr).expect("expression to be constant");
            let val_down = dom_level.down(val).unwrap();
            let decl_level = val_down.func_map(declare_field_value_letting);
            // no constraints for value letting reprs
            let constraints = vec![];
            (decl_level, symtab, constraints)
        }
        _ => todo!(),
    }
}
