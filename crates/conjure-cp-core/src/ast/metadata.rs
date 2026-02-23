use crate::ast::ReturnType;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use uniplate::derive_unplateable;

derive_unplateable!(Metadata);

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default, Quine)]
#[path_prefix(conjure_cp::ast)]
pub struct Metadata {
    pub etype: Option<ReturnType>,
}

impl Metadata {
    pub fn new() -> Metadata {
        Metadata {
            etype: None,
        }
    }
}

impl Display for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Metadata")
    }
}

impl Hash for Metadata {
    fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {
        // Dummy method - Metadata is ignored when hashing an Expression
    }
}
