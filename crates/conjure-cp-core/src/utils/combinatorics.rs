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

/// Count *surjections* from an `n`-element set onto a `k`-element set: the Stirling number of the
/// second kind `S(n, k)`, i.e. the number of ways to partition `n` labelled elements into exactly
/// `k` non-empty unlabelled blocks (a surjection onto `k` elements is exactly a choice of which
/// block maps to which target element, and blocks are otherwise interchangeable until that
/// assignment -- so partitioning first and multiplying by `k!` elsewhere gives the surjection
/// count; this function returns the partition count alone).
///
/// # Formula
/// `S(n, k) = k * S(n-1, k) + S(n-1, k-1)`, with `S(0, 0) = 1`, `S(n, 0) = 0` for `n > 0`, and
/// `S(n, k) = 0` for `k > n`.
pub fn stirling_second_kind(n: u64, k: u64) -> Result<u64, CombinatoricsError> {
    if k > n {
        return Ok(0);
    }
    // table[i][j] = S(i, j), for i in 0..=n, j in 0..=k.
    let mut table = vec![vec![0u64; (k + 1) as usize]; (n + 1) as usize];
    table[0][0] = 1;
    for i in 1..=n {
        for j in 1..=k.min(i) {
            let term1 = j
                .checked_mul(table[(i - 1) as usize][j as usize])
                .ok_or(CombinatoricsError::Overflow)?;
            let term2 = table[(i - 1) as usize][(j - 1) as usize];
            table[i as usize][j as usize] = term1
                .checked_add(term2)
                .ok_or(CombinatoricsError::Overflow)?;
        }
    }
    Ok(table[n as usize][k as usize])
}
