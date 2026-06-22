use crate::ast::ReturnType;
use polyquine::Quine;
use proc_macro2::TokenStream;
use quote::quote;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use uniplate::derive_unplateable;

derive_unplateable!(Metadata);

pub const NO_HASH: u64 = 0;
/// Sentinel for expressions with no clean-rule marker.
const NO_CLEAN_RULE_PRIORITY: u16 = u16::MAX;

/// Per-expression metadata used for typing, source mapping, and rewrite-time caches.
///
/// Metadata is ignored by expression equality and hashing.
#[derive(Debug, Deserialize, Serialize)]
pub struct Metadata {
    /// Cached or inferred return type for this expression.
    pub etype: Option<ReturnType>,
    /// Optional source span identifier for diagnostics and reporting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_id: Option<u32>,
    /// Cached structural hash used by tree rewriting infrastructure.
    #[serde(default, skip_serializing)]
    pub stored_hash: AtomicU64,
    /// Highest-priority rule group known not to rewrite this unchanged expression.
    ///
    /// This is runtime-only dirty/clean state and is cleared when the expression or one of its
    /// children changes.
    #[serde(default = "default_clean_rule_priority", skip_serializing)]
    #[doc(hidden)]
    pub clean_rule_priority: AtomicU16,
}

impl Metadata {
    /// Creates empty metadata with no type, source span, or cached rewrite state.
    pub fn new() -> Metadata {
        Metadata {
            etype: None,
            span_id: None,
            stored_hash: AtomicU64::new(NO_HASH),
            clean_rule_priority: AtomicU16::new(NO_CLEAN_RULE_PRIORITY),
        }
    }

    /// Creates empty metadata associated with a source span identifier.
    pub fn with_span_id(span_id: u32) -> Metadata {
        Metadata {
            etype: None,
            span_id: Some(span_id),
            stored_hash: AtomicU64::new(NO_HASH),
            clean_rule_priority: AtomicU16::new(NO_CLEAN_RULE_PRIORITY),
        }
    }

    /// Records that rules at `priority` have been attempted and failed for this expression.
    ///
    /// When a rewrite changes a child expression, the rewriter clears this mark on each ancestor
    /// while rebuilding the root from the zipper.
    pub fn mark_clean_for_rule_priority(&self, priority: u16) {
        self.clean_rule_priority.store(priority, Ordering::Relaxed);
    }

    /// Returns whether this expression is known clean for the given rule priority.
    pub fn is_clean_for_rule_priority(&self, priority: u16) -> bool {
        self.clean_rule_priority.load(Ordering::Relaxed) == priority
    }

    /// Clears any clean-rule marker on this expression.
    pub fn clear_clean_rule_priority(&self) {
        self.clean_rule_priority
            .store(NO_CLEAN_RULE_PRIORITY, Ordering::Relaxed);
    }
}

impl Default for Metadata {
    fn default() -> Self {
        Metadata::new()
    }
}

/// Serde default for runtime-only clean-rule metadata.
fn default_clean_rule_priority() -> AtomicU16 {
    AtomicU16::new(NO_CLEAN_RULE_PRIORITY)
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
            etype: self.etype.clone(),
            span_id: self.span_id,
            stored_hash: AtomicU64::new(self.stored_hash.load(Ordering::Relaxed)),
            clean_rule_priority: AtomicU16::new(self.clean_rule_priority.load(Ordering::Relaxed)),
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
        let etype = self.etype.ctor_tokens();
        let span_id = self.span_id.ctor_tokens();
        quote! {
            conjure_cp::ast::Metadata {
                etype: #etype,
                span_id: #span_id,
                stored_hash: std::sync::atomic::AtomicU64::new(0),
                clean_rule_priority: std::sync::atomic::AtomicU16::new(u16::MAX),
            }
        }
    }
}
