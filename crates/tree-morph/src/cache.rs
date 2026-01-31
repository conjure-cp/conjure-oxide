//! Define Caching behaviour for TreeMorph
//! This should help out with repetetive and expensive tree operations as well as long chains of
//! rules on duplicate subtrees 

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

/// Return type for RewriteCache
/// Due to the nature of Rewriting, there may be repeated subtrees where no rule can be applied. 
/// In that case, we can compute it once and store it in cache stating no rules applicable. 
pub enum CacheResult<T> {
    /// The Subtree does not exist in cache.
    Unknown,

    /// The Subtree exists in cache but no rule is applicable. 
    Terminal,

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
    fn insert(&mut self, from: T, to: Option<T>, level: usize);
}

/// Disable Caching.
///
/// This should compile out if statically selected.
pub struct NoCache;
impl<T> RewriteCache<T> for NoCache {
    fn get(&self, _: &T, _: usize) -> CacheResult<T> {
        CacheResult::Unknown
    }

    fn insert(&mut self, _: T, _: Option<T>, _: usize) {}
}

/// RewriteCache implemented with a HashMap
pub struct HashMapCache<T>
where
    T: Hash + Clone + Eq,
{
    map: HashMap<u64, Option<T>>,

    // Adjacency list to enable transitive updates across.
    // If we see A -> B and B -> C, we use this field to store B's dependants
    // Then update A -> C accordingly. If it is a longer chain, there can be
    // more than one dependants.
    dependencies: HashMap<u64, Vec<u64>>,
    // To avoid repeated cloning and save some memory,
    // we can store the actual subtree once.
    // This is better if the cost of hashing twice is less than the cost of cloning.
    // NOT IMPLEMENTED: Maybe unneeded? Would just be pushing the cloning logic elsewhere
    // subtree_map: HashMap<u64, T>,
}

impl<T> HashMapCache<T>
where
    T: Hash + Clone + Eq,
{
    /// Creates a new HashMapCache that can be used as a RewriteCache
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            dependencies: HashMap::new(),
            // subtree_map: HashMap::new(),
        }
    }

    fn hash(&self, term: &T, level: usize) -> u64 {
        let mut hasher = DefaultHasher::new();
        term.hash(&mut hasher);
        level.hash(&mut hasher);
        hasher.finish()
    }
}

impl<T> Default for HashMapCache<T>
where
    T: Hash + Clone + Eq,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> RewriteCache<T> for HashMapCache<T>
where
    T: Hash + Clone + Eq,
{
    fn get(&self, subtree: &T, level: usize) -> CacheResult<T> {
        let hashed = self.hash(subtree, level);

        if !self.map.contains_key(&hashed) {
            return CacheResult::Unknown;
        }

        match self.map.get(&hashed).unwrap() {
            Some(res) => CacheResult::Rewrite(res.clone()),
            None => CacheResult::Terminal,
        }
    }

    fn insert(&mut self, from: T, to: Option<T>, level: usize) {
        let from_hash = self.hash(&from, level);

        if to.is_none() {
            self.map.insert(from_hash, None);
            return;
        }

        let to_hash = self.hash(to.as_ref().unwrap(), level);

        if self.map.contains_key(&from_hash) {
            // TODO: Change mapping from hash -> Vec<T> -- Leave it as single for now
            panic!("Overriding an existing mapping loses transitive closure.");
        }

        self.map.insert(from_hash, to.clone());

        if let Some(mut dependencies) = self.dependencies.remove(&from_hash) {
            for &dependant in &dependencies {
                self.map.insert(dependant, to.clone());
            }

            self.dependencies
                .entry(to_hash)
                .or_default()
                .append(&mut dependencies);
        }

        self.dependencies
            .entry(to_hash)
            .or_default()
            .push(from_hash);
    }
}
