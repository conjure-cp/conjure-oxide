use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;

pub fn to_set<T: Eq + Hash + Debug + Clone>(a: &Vec<T>) -> HashSet<T> {
    let mut a_set: HashSet<T> = HashSet::new();
    for el in a {
        a_set.insert(el.clone());
    }
    a_set
}

pub fn if_ok<T, E: Debug>(result: Result<T, E>) -> T {
    assert!(result.is_ok());
    result.unwrap()
}
