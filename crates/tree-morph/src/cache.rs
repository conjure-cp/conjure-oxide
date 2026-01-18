use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

pub trait RewriteCache<T> {
    fn get(&self, subtree: &T, level: usize) -> Option<T>;

    fn insert(&mut self, from: T, to: T, level: usize);
}

pub struct NoCache;
impl<T> RewriteCache<T> for NoCache {
    fn get(&self, _: &T, _: usize) -> Option<T> {
        None
    }

    fn insert(&mut self, _: T, _: T, _: usize) {}
}

pub struct HashMapCache<T>
where
    T: Hash + Clone + Eq,
{
    map: HashMap<u64, T>,

    // Adjacency list to enable transitive updates across.
    // If we see A -> B and B -> C, we use this field to store B's dependants
    // Then update A -> C accordingly. If it is a longer chain, there can be
    // more than one dependants.
    dependencies: HashMap<u64, Vec<u64>>,

    // To avoid repeated cloning and save some memory,
    // we can store the actual subtree once.
    // This is better if the cost of hashing twice is less than the cost of cloning.
    // NOT IMPLEMENTED: Maybe unneeded? Would just be pushing the cloning logic elsewhere
    subtree_map: HashMap<u64, T>,
}

impl<T> HashMapCache<T>
where
    T: Hash + Clone + Eq,
{
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            dependencies: HashMap::new(),
            subtree_map: HashMap::new(),
        }
    }

    fn hash(&self, term: &T, level: usize) -> u64 {
        let mut hasher = DefaultHasher::new();
        term.hash(&mut hasher);
        level.hash(&mut hasher);
        hasher.finish()
    }
}

impl<T> RewriteCache<T> for HashMapCache<T>
where
    T: Hash + Clone + Eq,
{
    fn get(&self, subtree: &T, level: usize) -> Option<T> {
        self.map.get(&self.hash(subtree, level)).cloned()
    }

    fn insert(&mut self, from: T, to: T, level: usize) {
        let from_hash = self.hash(&from, level);
        let to_hash = self.hash(&to, level);

        if self.map.contains_key(&from_hash) {
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
