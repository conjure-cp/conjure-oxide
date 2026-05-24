//! Define Caching behaviour for TreeMorph
//! This should help out with repetetive and expensive tree operations as well as long chains of
//! rules on duplicate subtrees

use std::{
    hash::{DefaultHasher, Hash, Hasher},
    marker::PhantomData,
};

use fxhash::FxHashMap;

/// Return type for RewriteCache
/// Due to the nature of Rewriting, there may be repeated subtrees where no rule can be applied.
/// In that case, we can compute it once and store it in cache stating no rules applicable.
pub enum CacheResult<T> {
    /// The Subtree does not exist in cache.
    Unknown,

    /// The Subtree exists in cache but no rule is applicable.
    Terminal(usize),

    /// The Subtree exists in cache and there is a pre computed value.
    Rewrite(T),
}

/// Caching for Treemorph.
///
/// Outward facing API is simple. Given a tree and the rule application level, check the cache
/// before attempting rule checks.
///
/// If successful, insert it into the cache. The next time we see the exact same subtree, we can
/// immediately obtain the result without redoing all the hard work of recomputing.
pub trait RewriteCache<T> {
    /// Get the cached result
    fn get(&self, subtree: &T, level: usize) -> CacheResult<T>;

    /// Insert the results into the cache.
    /// Note: Any powerful side effects such as changing other parts of the tree or replacing the
    /// root should NOT be inserted into the cache.
    fn insert(&mut self, from: &T, to: Option<T>, level: usize);

    /// Invalidate any internally cached hash for the given node.
    /// This is called on ancestors when a subtree is replaced.
    /// The default implementation is a no-op for caches that don't use node-level hash caching.
    fn invalidate_node(&self, _node: &T) {}

    /// Invalidate cached hashes for the given node and all its descendants.
    /// Called on replacement subtrees after rule application.
    fn invalidate_subtree(&self, _node: &T) {}

    /// Returns `false` if this cache never stores anything (e.g. [`NoCache`]).
    /// The engine uses this to skip clones that would only feed into a no-op insert.
    fn is_active(&self) -> bool {
        true
    }

    /// Record the hash of an ancestor node before descending into a child.
    /// Called by the zipper on every successful `go_down`.
    fn push_ancestor(&mut self, _node: &T) {}

    /// Discard the top ancestor hash after ascending back to a parent.
    /// Called by the zipper on every `go_up` during normal traversal.
    fn pop_ancestor(&mut self) {}

    /// Pop the top ancestor hash and insert a mapping from the old ancestor
    /// to the new (rebuilt) ancestor at the given level.
    /// Called by `mark_dirty_to_root` as it walks up after a replacement.
    fn pop_and_map_ancestor(&mut self, _new_ancestor: &T, _level: usize) {}
}

impl<T> RewriteCache<T> for Box<dyn RewriteCache<T>> {
    fn get(&self, subtree: &T, level: usize) -> CacheResult<T> {
        (**self).get(subtree, level)
    }

    fn insert(&mut self, from: &T, to: Option<T>, level: usize) {
        (**self).insert(from, to, level)
    }

    fn invalidate_node(&self, node: &T) {
        (**self).invalidate_node(node)
    }

    fn invalidate_subtree(&self, node: &T) {
        (**self).invalidate_subtree(node)
    }

    fn is_active(&self) -> bool {
        (**self).is_active()
    }

    fn push_ancestor(&mut self, node: &T) {
        (**self).push_ancestor(node)
    }

    fn pop_ancestor(&mut self) {
        (**self).pop_ancestor()
    }

    fn pop_and_map_ancestor(&mut self, new_ancestor: &T, level: usize) {
        (**self).pop_and_map_ancestor(new_ancestor, level)
    }
}

/// Disable Caching.
///
/// This should compile out if statically selected.
pub struct NoCache;
impl<T> RewriteCache<T> for NoCache {
    fn get(&self, _: &T, _: usize) -> CacheResult<T> {
        CacheResult::Unknown
    }

    fn insert(&mut self, _: &T, _: Option<T>, _: usize) {}

    fn is_active(&self) -> bool {
        false
    }
}

/// Abstracts how a cache computes hash keys and invalidates nodes.
///
/// Implement this trait to plug different hashing strategies into [`HashMapCache`].
pub trait CacheKey<T> {
    /// Compute a level-independent hash for `term`.
    fn node_hash(term: &T) -> u64;

    /// Combine a node hash with a rule-group level to produce a cache key.
    fn combine(node_hash: u64, level: usize) -> u64 {
        let mut hasher = DefaultHasher::new();
        node_hash.hash(&mut hasher);
        level.hash(&mut hasher);
        hasher.finish()
    }

    /// Compute a cache key for `term` at the given rule application `level`.
    fn hash(term: &T, level: usize) -> u64 {
        Self::combine(Self::node_hash(term), level)
    }

    /// Invalidate any internally cached hash for the given node.
    /// The default is a no-op (used by [`StdHashKey`]).
    fn invalidate(_node: &T) {}

    /// Invalidate cached hashes for the given node and all its descendants.
    /// The default is a no-op (used by [`StdHashKey`]).
    fn invalidate_subtree(_node: &T) {}
}

/// Hashing strategy that delegates to the standard [`Hash`] trait.
pub struct StdHashKey;

impl<T: Hash> CacheKey<T> for StdHashKey {
    fn node_hash(term: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        term.hash(&mut hasher);
        hasher.finish()
    }
}

/// Types with an internally cached hash value.
///
/// Implementors store a precomputed hash (e.g. in metadata) to avoid rehashing
/// entire subtrees on every cache lookup. The cached hash must be invalidated
/// whenever the node's content changes.
///
/// Use [`invalidate_cache`](CacheHashable::invalidate_cache) for single-node
/// invalidation (e.g. when walking up ancestors after a replacement), and
/// [`invalidate_cache_recursive`](CacheHashable::invalidate_cache_recursive)
/// for full-subtree invalidation (e.g. on rule replacement subtrees that may
/// contain cloned nodes with stale hashes from `with_children` reassembly).
pub trait CacheHashable {
    /// Invalidate the cached hash for this node only.
    /// Used by `mark_dirty_to_root` when walking up ancestors after a child replacement.
    fn invalidate_cache(&self);

    /// Invalidate the cached hash for this node and all descendants.
    /// Used on replacement subtrees after rule application to clear stale hashes
    /// from cloned-and-reassembled nodes.
    fn invalidate_cache_recursive(&self);

    /// Return the cached hash, computing and storing it if not yet cached.
    fn get_cached_hash(&self) -> u64;

    /// Compute the hash from scratch, store it, and return it.
    fn calculate_hash(&self) -> u64;
}

/// Hashing strategy that delegates to [`CacheHashable::get_cached_hash`],
/// allowing types with internally cached hashes to avoid rehashing entire subtrees.
pub struct CachedHashKey;

impl<T: CacheHashable> CacheKey<T> for CachedHashKey {
    fn node_hash(term: &T) -> u64 {
        term.get_cached_hash()
    }

    fn invalidate(node: &T) {
        node.invalidate_cache();
    }

    fn invalidate_subtree(node: &T) {
        node.invalidate_cache_recursive();
    }
}

/// RewriteCache implemented with a HashMap, generic over a [`CacheKey`] hashing strategy.
///
/// Use `HashMapCache<T>` (defaults to [`StdHashKey`]) for standard `Hash` types,
/// or `HashMapCache<T, CachedHashKey>` for types implementing [`CacheHashable`].
pub struct HashMapCache<T, K = StdHashKey>
where
    K: CacheKey<T>,
    T: Clone,
{
    map: FxHashMap<u64, Option<T>>,
    predecessors: FxHashMap<u64, Vec<u64>>,
    ancestor_stack: Vec<u64>,
    clean_levels: FxHashMap<u64, usize>,
    _key: PhantomData<K>,
}

/// Convenience alias for a [`HashMapCache`] using [`CachedHashKey`].
pub type CachedHashMapCache<T> = HashMapCache<T, CachedHashKey>;

impl<T, K> HashMapCache<T, K>
where
    K: CacheKey<T>,
    T: Clone,
{
    /// Creates a new HashMapCache that can be used as a RewriteCache
    pub fn new() -> Self {
        Self {
            map: FxHashMap::default(),
            predecessors: FxHashMap::default(),
            ancestor_stack: Vec::new(),
            clean_levels: FxHashMap::default(),
            _key: PhantomData,
        }
    }
}

impl<T, K> Default for HashMapCache<T, K>
where
    K: CacheKey<T>,
    T: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, K> RewriteCache<T> for HashMapCache<T, K>
where
    K: CacheKey<T>,
    T: Clone,
{
    fn invalidate_node(&self, node: &T) {
        K::invalidate(node);
    }

    fn invalidate_subtree(&self, node: &T) {
        K::invalidate_subtree(node);
    }

    fn get(&self, subtree: &T, level: usize) -> CacheResult<T> {
        let node_hash = K::node_hash(subtree);
        if let Some(&max_clean) = self.clean_levels.get(&node_hash)
            && max_clean >= level
        {
            return CacheResult::Terminal(max_clean);
        }

        let hashed = K::combine(node_hash, level);

        match self.map.get(&hashed) {
            None => CacheResult::Unknown,
            Some(entry) => match entry {
                Some(res) => CacheResult::Rewrite(res.clone()),
                None => CacheResult::Terminal(level),
            },
        }
    }

    fn insert(&mut self, from: &T, to: Option<T>, level: usize) {
        let node_hash = K::node_hash(from);
        let from_hash = K::combine(node_hash, level);

        if to.is_none() {
            self.map.insert(from_hash, None);
            self.clean_levels
                .entry(node_hash)
                .and_modify(|l| *l = (*l).max(level))
                .or_insert(level);
            return;
        }

        let to_hash = K::hash(to.as_ref().unwrap(), level);

        if from_hash == to_hash {
            panic!("From and To have the same Hash - Cycle Detected!");
        }

        if self.map.contains_key(&from_hash) {
            panic!("Overriding an existing mapping loses transitive closure.");
        }

        // Forward Resolution
        let to = match self.map.get(&to_hash) {
            Some(stored) => stored.clone(),
            None => to,
        };

        let to_hash = match &to {
            Some(resolved) => K::hash(resolved, level),
            None => {
                self.map.insert(from_hash, None);
                return;
            }
        };

        self.map.insert(from_hash, to.clone());

        if let Some(mut predecessors) = self.predecessors.remove(&from_hash) {
            for &dependant in &predecessors {
                self.map.insert(dependant, to.clone());
            }

            self.predecessors
                .entry(to_hash)
                .or_default()
                .append(&mut predecessors);
        }

        self.predecessors
            .entry(to_hash)
            .or_default()
            .push(from_hash);
    }

    fn push_ancestor(&mut self, node: &T) {
        self.ancestor_stack.push(K::node_hash(node));
    }

    fn pop_ancestor(&mut self) {
        self.ancestor_stack.pop();
    }

    fn pop_and_map_ancestor(&mut self, new_ancestor: &T, level: usize) {
        if let Some(old_node_hash) = self.ancestor_stack.pop() {
            let old_key = K::combine(old_node_hash, level);
            let new_key = K::hash(new_ancestor, level);

            // No change at this ancestor level
            if old_key == new_key {
                return;
            }

            // If old_key has a rewrite mapping, don't override (preserves transitive closure).
            // But DO override terminal entries
            if let Some(existing) = self.map.get(&old_key) {
                if existing.is_some() {
                    return;
                }
                // Remove the stale terminal entry so we can insert the ancestor mapping
                self.map.remove(&old_key);
            }

            // Forward resolution
            let to = match self.map.get(&new_key) {
                Some(stored) => stored.clone(),
                None => Some(new_ancestor.clone()),
            };

            let to_key = match &to {
                Some(resolved) => K::hash(resolved, level),
                None => {
                    self.map.insert(old_key, None);
                    return;
                }
            };

            self.map.insert(old_key, to.clone());

            // Predecessor tracking
            if let Some(mut preds) = self.predecessors.remove(&old_key) {
                for &dep in &preds {
                    self.map.insert(dep, to.clone());
                }
                self.predecessors
                    .entry(to_key)
                    .or_default()
                    .append(&mut preds);
            }

            self.predecessors.entry(to_key).or_default().push(old_key);
        }
    }
}
