//! Various selector functions for different use cases.
//!
//! A selector function accepts an iterator over ([`Rule`], [`Update`]) pairs and returns the
//! selected [`Update`], or `None`.
//!
//! [`morph`](crate::morph) will call the given selector function when there is an ambiguity in
//! which rule to apply. That is, when more than one rule from the same group returns [`Some(...)`]
//! for a given sub-tree.

use std::{collections::VecDeque, fmt::Display, io::Write};

use crate::prelude::{Rule, Update};
use multipeek::multipeek;
use uniplate::Uniplate;

/// Returns the first [`Update`] if the iterator only yields one, otherwise calls `select`.
///
/// See the module-level documentation for more information.
pub(crate) fn one_or_select<T, M, R>(
    select: impl Fn(&T, &mut dyn Iterator<Item = (&R, Update<T, M>)>) -> Option<Update<T, M>>,
    t: &T,
    rs: &mut dyn Iterator<Item = (&R, Update<T, M>)>,
) -> Option<Update<T, M>>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    let mut rs = multipeek(rs);
    if rs.peek_nth(1).is_none() {
        return rs.next().map(|(_, u)| u);
    }
    select(t, &mut rs)
}

/// Returns the first available [`Update`] if there is one, otherwise returns `None`.
///
/// This is a good default selection strategy, especially when you expect only one possible
/// rule to apply to any one term.
///
/// See the module-level documentation for more information.
pub fn select_first<T, M, R>(
    _: &T,
    rs: &mut dyn Iterator<Item = (&R, Update<T, M>)>,
) -> Option<Update<T, M>>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    rs.next().map(|(_, u)| u)
}

/// Panics when called by the engine, printing the original subtree and all applicable rules
/// and their results.
///
/// This is useful when you always expect no more than one rule to be applicable, as the
/// engine will only call the selector function when there is an ambiguity in which to apply.
///
/// See the module-level documentation for more information.
pub fn select_panic<T, M, R>(
    t: &T,
    rs: &mut dyn Iterator<Item = (&R, Update<T, M>)>,
) -> Option<Update<T, M>>
where
    T: Uniplate + std::fmt::Debug,
    R: Rule<T, M> + std::fmt::Debug,
{
    let rules = rs.map(|(r, _)| r).collect::<Vec<_>>();
    // Since `one_or_select` only calls the selector if there is more than one rule,
    // at this point there is guaranteed to be more than one rule.
    panic!("Multiple rules applicable to expression {t:?}\n{rules:?}",);
}

/// Selects an [`Update`] based on user input through stdin.
///
/// See the module-level documentation for more information.
pub fn select_user_input<T, M, R>(
    t: &T,
    rs: &mut dyn Iterator<Item = (&R, Update<T, M>)>,
) -> Option<Update<T, M>>
where
    T: Uniplate + Display,
    R: Rule<T, M> + Display,
{
    let mut choices: Vec<_> = rs.collect();

    let rules = choices
        .iter()
        .enumerate()
        .map(
            |(
                i,
                (
                    r,
                    Update {
                        new_subtree: new_tree,
                        ..
                    },
                ),
            )| {
                format!(
                    "{}. {}
   ~> {}",
                    i + 1,
                    r,
                    new_tree
                )
            },
        )
        .collect::<Vec<_>>()
        .join("\n\n");

    loop {
        print!(
            "--- Current Expression ---
{t}

--- Rules ---
{rules}

---
q   No change
<n> Apply rule n

:",
        );
        std::io::stdout().flush().unwrap(); // Print the : on same line

        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();

        match line.trim() {
            "q" => return None,
            n => {
                if let Ok(n) = n.parse::<usize>() {
                    if n > 0 && n <= choices.len() {
                        let ret = choices.swap_remove(n - 1).1;
                        return Some(ret);
                    }
                }
            }
        }
    }
}

/// Selects a random [`Update`] from the iterator.
///
/// See the module-level documentation for more information.
pub fn select_random<T, M, R>(
    _: &T,
    rs: &mut dyn Iterator<Item = (&R, Update<T, M>)>,
) -> Option<Update<T, M>>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    use rand::seq::IteratorRandom;
    let mut rng = rand::rng();
    rs.choose(&mut rng).map(|(_, u)| u)
}

/// Selects the [`Update`] which results in the smallest subtree.
///
/// Subtree size is determined by maximum depth.
/// Among trees with the same depth, the first in the iterator order is selected.
///
/// See the module-level documentation for more information.
pub fn select_smallest_subtree<T, M, R>(
    _: &T,
    rs: &mut dyn Iterator<Item = (&R, Update<T, M>)>,
) -> Option<Update<T, M>>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    rs.min_by_key(|(_, u)| {
        u.new_subtree.cata(&|_, cs: VecDeque<i32>| {
            // Max subtree height + 1
            cs.iter().max().unwrap_or(&0) + 1
        })
    })
    .map(|(_, u)| u)
}
