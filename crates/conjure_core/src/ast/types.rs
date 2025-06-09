use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uniplate::{derive::Uniplate, Uniplate};

use super::{Name, SymbolTable};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Uniplate)]
pub enum ReturnType {
    Int,
    Bool,
    Matrix(Box<ReturnType>),
    Set(Box<ReturnType>),
    ElementTypeOf(Box<ReturnType>),
    TypeOf(Name),
}

impl ReturnType {
    /// Resolves a type containing variable bindings (`ElementTypeOf(Name)`, `TypeOf(Name)`) using the
    /// symbol table.
    pub fn resolve(self, symtab: &SymbolTable) -> ReturnType {
        let symtab = symtab.clone();
        #[allow(clippy::arc_with_non_send_sync)]
        self.transform(Arc::new(move |x| {
            match x {
                ReturnType::ElementTypeOf(typ) => {
                    match typ.resolve(&symtab){
                        ReturnType::Matrix(elem_type) => *elem_type,
                        ReturnType::Set(elem_type) => *elem_type,
                        ReturnType::Int | ReturnType::Bool => {panic!("name inside ElementTypeOf should have a type that has an element domain (e.g. set, matrix)")},
                        ReturnType::ElementTypeOf(_) | ReturnType::TypeOf(_) => {unreachable!("TypeOf and ElementTypeOf should not be in type, as it has been resolved")}
                    }
                },
                ReturnType::TypeOf(mut name) => {

                    // TODO: looking up a Name::WithRepresentation is a failure
                    if let Name::WithRepresentation(inner_name,_) = name {
                        name = *inner_name;
                    }

                    symtab.lookup(&name).expect("name inside TypeOf should exist").return_type().expect("name inside TypeOf should have a type").resolve(&symtab)
                }
                t => t,
            }
        }))
    }
}

/// Something with a return type
pub trait Typeable {
    fn return_type(&self) -> Option<ReturnType>;
}
