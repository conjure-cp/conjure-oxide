//! Default implementations of some Repr trait methods using functors;
//! These methods are used by the register_repr! macro

use super::types::{LookupFn, ReprDomainLevel, ReprError};
use super::util::try_up_via;
use crate::ast::{
    DeclarationKind, DeclarationPtr, DomainPtr, Expression, Literal, Name, SymbolTable,
    eval_constant,
};
use funcmap::{FuncMap, TryFuncMap};

/// Implement [ReprDeclLevel::lookup_via] as a functor `S<DeclarationPtr> -> S<Literal>`.
pub fn lookup_via_default_impl<DeclL, A>(
    decl_level: &DeclL,
    lookup: &LookupFn<'_>,
) -> Result<A, ReprError>
where
    DeclL: Clone + TryFuncMap<DeclarationPtr, Literal, Output = A>,
{
    decl_level
        .clone()
        .try_func_map(|decl: DeclarationPtr| try_up_via(decl, lookup))
}

/// Implement [ReprDeclLevel::to_domain_level] as a functor `S<DeclarationPtr> -> S<DomainPtr>`.
pub fn to_domain_level_default_impl<DomL, DecL>(decl_level: DecL) -> DomL
where
    DecL: FuncMap<DeclarationPtr, DomainPtr, Output = DomL>,
{
    let field_dom = |decl: DeclarationPtr| decl.domain().expect("variable must have a domain");
    decl_level.func_map(field_dom)
}

/// Implement [ReprDomainLevel::instantiate] as a functor `S<DomainPtr> -> S<DeclarationPtr>`.
/// Needs the following additional arguments:
/// - `structural` - function which takes `&S<DeclarationPtr>` and generates its structural constraints
pub fn instantiate_default_impl<DomL, DecL, A, SF>(
    dom_level: DomL,
    decl: DeclarationPtr,
    structural: SF,
) -> (DecL, SymbolTable, Vec<Expression>)
where
    DomL: ReprDomainLevel<Assignment = A, DeclLevel = DecL>
        + FuncMap<DomainPtr, DeclarationPtr, Output = DecL>,
    A: FuncMap<Literal, DeclarationPtr, Output = DecL>,
    SF: Fn(&DecL) -> Vec<Expression>,
{
    let src_name = decl.name();
    let repr_name = <DomL as ReprDomainLevel>::RULE.name();
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

            let val = eval_constant(expr).expect("expression to be constant");
            let val_down = dom_level.down(val).unwrap();
            let decl_level = val_down.func_map(declare_field_value_letting);
            // no constraints for value letting reprs
            let constraints = vec![];
            (decl_level, symtab, constraints)
        }
        _ => todo!(),
    }
}
