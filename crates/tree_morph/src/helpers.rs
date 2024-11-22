use std::{fmt::Display, io::Write};

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

// TODO: Add more selection strategies:
// - random
// - smallest subtree
