use std::{collections::VecDeque, fmt::Display, io::Write, sync::Arc};

use crate::{Reduction, Rule};
use multipeek::multipeek;
use uniplate::Uniplate;

/// Returns the first result if the iterator has only one, otherwise calls `select`.
pub(crate) fn one_or_select<T, M, R>(
    select: impl Fn(&T, &mut dyn Iterator<Item = (&R, Reduction<T, M>)>) -> Option<Reduction<T, M>>,
    t: &T,
    rs: &mut dyn Iterator<Item = (&R, Reduction<T, M>)>,
) -> Option<Reduction<T, M>>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    let mut rs = multipeek(rs);
    if rs.peek_nth(1).is_none() {
        return rs.next().map(|(_, r)| r);
    }
    select(t, &mut rs)
}

/// Returns the first available `Reduction` if there is one, otherwise returns `None`.
///
/// This is a good default selection strategy, especially when you expect only one possible result.
pub fn select_first<T, M, R>(
    _: &T,
    rs: &mut dyn Iterator<Item = (&R, Reduction<T, M>)>,
) -> Option<Reduction<T, M>>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    rs.next().map(|(_, r)| r)
}

/// Select the first result or panic if there is more than one.
///
/// This is useful when you expect exactly one rule to be applicable in all cases.
pub fn select_first_or_panic<T, M, R>(
    t: &T,
    rs: &mut dyn Iterator<Item = (&R, Reduction<T, M>)>,
) -> Option<Reduction<T, M>>
where
    T: Uniplate + Display,
    R: Rule<T, M>,
{
    let mut rs = multipeek(rs);
    if rs.peek_nth(1).is_some() {
        // TODO (Felix) Log list of rules
        panic!("Multiple rules applicable to expression \"{}\"", t);
    }
    rs.next().map(|(_, r)| r)
}

macro_rules! select_prompt {
    () => {
        "--- Current Expression ---
{}

--- Rules ---
{}

---
q   No change
<n> Apply rule n

:"
    };
}

pub fn select_user_input<T, M, R>(
    t: &T,
    rs: &mut dyn Iterator<Item = (&R, Reduction<T, M>)>,
) -> Option<Reduction<T, M>>
where
    T: Uniplate + Display,
    R: Rule<T, M> + Display,
{
    let mut choices: Vec<_> = rs.collect();

    let rules = choices
        .iter()
        .enumerate()
        .map(|(i, (r, Reduction { new_tree, .. }))| {
            format!(
                "{}. {}
   ~> {}",
                i + 1,
                r,
                new_tree
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    loop {
        print!(select_prompt!(), t, rules);
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

/// Selects a random `Reduction` from the iterator.
pub fn select_random<T, M, R>(
    _: &T,
    rs: &mut dyn Iterator<Item = (&R, Reduction<T, M>)>,
) -> Option<Reduction<T, M>>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    use rand::seq::IteratorRandom;
    let mut rng = rand::rng();
    rs.choose(&mut rng).map(|(_, r)| r)
}

/// Selects the `Reduction` which results in the smallest subtree.
///
/// Subtree size is determined by maximum depth.
/// Among trees with the same depth, the first in the iterator order is selected.
pub fn select_smallest_subtree<T, M, R>(
    _: &T,
    rs: &mut dyn Iterator<Item = (&R, Reduction<T, M>)>,
) -> Option<Reduction<T, M>>
where
    T: Uniplate,
    R: Rule<T, M>,
{
    rs.min_by_key(|(_, r)| {
        r.new_tree.cata(Arc::new(|_, cs: VecDeque<i32>| {
            // Max subtree height + 1
            cs.iter().max().unwrap_or(&0) + 1
        }))
    })
    .map(|(_, r)| r)
}
