use crate::rewrite::rewrite;
use conjure_core::ast::Model;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;

pub fn assert_eq_any_order<T: Eq + Hash + Debug + Clone>(a: &Vec<Vec<T>>, b: &Vec<Vec<T>>) {
    assert_eq!(a.len(), b.len());

    let mut a_rows: Vec<HashSet<T>> = Vec::new();
    for row in a {
        let hash_row = to_set(row);
        a_rows.push(hash_row);
    }

    let mut b_rows: Vec<HashSet<T>> = Vec::new();
    for row in b {
        let hash_row = to_set(row);
        b_rows.push(hash_row);
    }

    println!("{:?},{:?}", a_rows, b_rows);
    for row in a_rows {
        assert!(b_rows.contains(&row));
    }
}

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
