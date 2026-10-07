use crate::ast::versioned_cache::VersionedCache;
use crate::ast::{DomainPtr, ReturnType, declaration::declaration_content_generation};
use polyquine::Quine;
use proc_macro2::TokenStream;
use quote::quote;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use uniplate::derive_unplateable;

derive_unplateable!(Metadata);

pub const NO_HASH: u64 = 0;
/// Per-expression metadata used for typing, source mapping, and runtime caches.
///
/// Metadata is ignored by expression equality and hashing.
#[derive(Debug, Deserialize, Serialize)]
pub struct Metadata {
    /// Cached or inferred return type for this expression.
    pub etype: Option<ReturnType>,
    /// Optional source span identifier for diagnostics and reporting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span_id: Option<u32>,
    /// Cached expression content hash used by semantic hashing.
    #[serde(default, skip_serializing)]
    pub cached_content_hash: AtomicU64,
    /// Cached result of `Expression::domain_of()` for this node, stamped internally with the
    /// declaration generation used to compute it.
    ///
    /// The generation records the declaration state used to compute the domain. `domain_of()`
    /// recomputes recursively from scratch on every cache miss, so repeatedly calling it on the
    /// same subtree (as rules that check "is this operand scalar/abstract" tend to, once per rule
    /// attempt) is quadratic in the worst case. Cleared when this expression or a descendant
    /// changes, and ignored after a referenced declaration changes.
    #[serde(skip)]
    domain: VersionedCache<Option<DomainPtr>>,
}

impl Metadata {
    /// Creates empty metadata with no type, source span, or cached rewrite state.
    pub fn new() -> Metadata {
        Metadata {
            etype: None,
            span_id: None,
            cached_content_hash: AtomicU64::new(NO_HASH),
            domain: VersionedCache::new(),
        }
    }

    /// Creates empty metadata associated with a source span identifier.
    pub fn with_span_id(span_id: u32) -> Metadata {
        Metadata {
            etype: None,
            span_id: Some(span_id),
            cached_content_hash: AtomicU64::new(NO_HASH),
            domain: VersionedCache::new(),
        }
    }

    /// Clears the cached domain after this expression or a descendant changes.
    pub fn clear_cached_domain(&self) {
        self.domain.clear();
    }

    /// Returns the cached domain for this expression, computing and caching it via `compute` on
    /// first access.
    pub fn domain_or_init(
        &self,
        mut compute: impl FnMut() -> Option<DomainPtr>,
    ) -> Option<DomainPtr> {
        self.domain
            .get_or_init(declaration_content_generation, |_| compute())
    }
}

impl Default for Metadata {
    fn default() -> Self {
        Metadata::new()
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
            etype: self.etype.clone(),
            span_id: self.span_id,
            cached_content_hash: AtomicU64::new(self.cached_content_hash.load(Ordering::Relaxed)),
            // Deliberately *not* carried over, unlike the other runtime caches above: some
            // callers clone an expression template and then mutate its children directly (e.g.
            // comprehension-body substitution), bypassing the rewriter's zipper-based
            // cached-domain invalidation entirely. A clone always starts uncached so
            // that path can never observe a stale domain.
            domain: VersionedCache::new(),
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
        quote! {{
            let mut metadata = conjure_cp::ast::Metadata::new();
            metadata.etype = #etype;
            metadata.span_id = #span_id;
            metadata
        }}
    }
}
