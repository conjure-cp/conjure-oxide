use crate::ast::ReturnType;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use uniplate::derive_unplateable;

derive_unplateable!(Metadata);

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct Metadata {
    pub clean: bool,
    pub etype: Option<ReturnType>,
}

impl Metadata {
    pub fn new() -> Metadata {
        Metadata {
            clean: false,
            etype: None,
        }
    }

    pub fn clone_dirty(&self) -> Metadata {
        Metadata {
            clean: false,
            ..self.clone()
        }
    }
}

impl Display for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Metadata")
    }
}

// impl<T> Display for Metadata<T> where T: for<'a> MetadataKind<'a> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "Metadata")
//     }
// }

//
// impl<T> Metadata<T> where T: for<'a> MetadataKind<'a> {
//     fn new(a: T) -> Metadata<T> {
//         Metadata { a }
//     }
// }
