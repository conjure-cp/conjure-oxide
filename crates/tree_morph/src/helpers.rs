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

// TODO: Add more selection strategies:
// - random
// - smallest subtree
// - ask the user for input via blocking I/O
// - panic if there is more than 1 result, otherwise return the first
