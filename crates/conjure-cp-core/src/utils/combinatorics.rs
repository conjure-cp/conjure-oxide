use thiserror::Error;
use ustr::Ustr;

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum CombinatoricsError {
    #[error("The operation is not defined for the given input: {0}")]
    NotDefined(Ustr),
    #[error("The result is too large to fit into the return type")]
    Overflow,
}

impl CombinatoricsError {
    pub fn not_defined(input: impl Into<Ustr>) -> Self {
        Self::NotDefined(input.into())
    }
}

/// Count *combinations* - the number of ways to pick `n_choose` items from `n_total`,
/// where order does not matter.
///
/// # Formula
/// C(n, r) = n! / (r! * (n-r)!)
///
/// Not defined for r > n.
pub fn count_combinations(n_total: u64, n_choose: u64) -> Result<u64, CombinatoricsError> {
    if n_choose > n_total {
        return Err(CombinatoricsError::not_defined(
            "n_choose must be <= n_total",
        ));
    }

    // Use symmetry C(n, k) == C(n, n-k) to make the loop smaller
    let k = n_choose.min(n_total - n_choose);

    // Repeatedly multiply / divide as factors get big fast;
    // return None if we overflow anyway
    (1u64..=k).try_fold(1u64, |acc, val| {
        n_total
            .checked_sub(val)
            .ok_or(CombinatoricsError::Overflow)? // n_total - val
            .checked_add(1u64)
            .ok_or(CombinatoricsError::Overflow)? // + 1
            .checked_mul(acc)
            .ok_or(CombinatoricsError::Overflow)? // * acc
            .checked_div(val)
            .ok_or(CombinatoricsError::Overflow) // / val
    })
}

/// Count *permutations* - the number of ways to pick `n_choose` items from `n_total`,
/// where order matters.
///
/// # Formula
/// P(n, r) = n! / (n-r)!
///
/// Not defined for r > n.
#[allow(dead_code)]
pub fn count_permutations(n_total: u64, n_choose: u64) -> Result<u64, CombinatoricsError> {
    if n_choose > n_total {
        return Err(CombinatoricsError::not_defined(
            "n_choose must be <= n_total",
        ));
    }

    let start = n_total
        .checked_sub(n_choose)
        .ok_or(CombinatoricsError::Overflow)?
        .checked_add(1u64)
        .ok_or(CombinatoricsError::Overflow)?;
    (start..=n_total).try_fold(1u64, |acc, val| {
        acc.checked_mul(val).ok_or(CombinatoricsError::Overflow)
    })
}
