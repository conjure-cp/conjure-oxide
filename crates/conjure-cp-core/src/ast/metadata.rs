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
    pub clean: bool,
    pub etype: Option<ReturnType>,
    pub span_id: Option<u32>,
}

impl Metadata {
    pub fn new() -> Metadata {
        Metadata {
            clean: false,
            etype: None,
            span_id: None,
        }
    }

    pub fn with_span_id(span_id: u32) -> Metadata {
        Metadata {
            clean: false,
            etype: None,
            span_id: Some(span_id),
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

impl Hash for Metadata {
    fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {
        // Dummy method - Metadata is ignored when hashing an Expression
    }
}
