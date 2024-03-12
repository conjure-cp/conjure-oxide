use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Metadata {
    pub dirtyclean: bool,
}

impl Default for Metadata {
    fn default() -> Self {
        Self::new()
    }
}

impl Metadata {
    pub fn new() -> Metadata {
        Metadata { dirtyclean: false }
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
