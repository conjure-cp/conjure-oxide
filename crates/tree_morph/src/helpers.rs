use uniplate::Uniplate;

use crate::{Reduction, Rule};

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

// TODO: Add more selection strategies (e.g. random, smallest subtree, ask the user for input, etc.)
// The engine will also have to use a new function which only calls a selector function if there is >1 result
