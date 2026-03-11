use crate::ast::ReturnType;
use polyquine::Quine;
use proc_macro2::TokenStream;
use quote::quote;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::sync::atomic::AtomicU64;
use uniplate::derive_unplateable;

derive_unplateable!(Metadata);

pub const NO_HASH: u64 = 0;

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Metadata {
    pub clean: bool,
    pub etype: Option<ReturnType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_id: Option<u32>,
    #[serde(skip)]
    pub stored_hash: AtomicU64
}

impl Metadata {
    pub fn new() -> Metadata {
        Metadata {
            clean: false,
            etype: None,
            span_id: None,
            stored_hash: AtomicU64::new(NO_HASH)
        }
    }

    pub fn with_span_id(span_id: u32) -> Metadata {
        Metadata {
            clean: false,
            etype: None,
            span_id: Some(span_id),
            stored_hash: AtomicU64::new(NO_HASH)
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

impl Clone for Metadata {
    fn clone(&self) -> Self {
        Metadata {
            clean: self.clean,
            etype: self.etype.clone(),
            span_id: self.span_id,
            stored_hash: AtomicU64::new(NO_HASH),
        }
    }
}

impl PartialEq for Metadata {
    fn eq(&self, other: &Self) -> bool {
        self.etype == other.etype
    }
}

impl Eq for Metadata {}

impl Quine for Metadata {
    fn ctor_tokens(&self) -> TokenStream {
        let clean = self.clean.ctor_tokens();
        let etype = self.etype.ctor_tokens();
        let span_id = self.span_id.ctor_tokens();
        quote! {
            conjure_cp::ast::Metadata {
                clean: #clean,
                etype: #etype,
                span_id: #span_id,
                stored_hash: std::sync::atomic::AtomicU64::new(0),
            }
        }
    }
}
