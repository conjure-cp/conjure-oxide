/// Count *combinations* - the number of ways to pick `n_choose` items from `n_total`,
/// where order does not matter.
///
/// # Formula
/// C(n, r) = n! / (r! * (n-r)!)
///
/// # Returns
/// - 0 for invalid inputs (n_choose > n_total)
/// - None if the result overflows u64
pub fn count_combinations(n_total: u64, n_choose: u64) -> Option<u64> {
    if n_choose > n_total {
        Some(0u64)
    } else {
        // Use symmetry C(n, k) == C(n, n-k) to make the loop smaller
        let k = n_choose.min(n_total - n_choose);

        // Repeatedly multiply / divide as factors get big fast;
        // return None if we overflow anyway
        (1u64..=k).try_fold(1u64, |acc, val| {
            n_total
                .checked_sub(val)? // n_total - val
                .checked_add(1u64)? // + 1
                .checked_mul(acc)? // * acc
                .checked_div(val) // / val
        })
    }
}

/// Count *permutations* - the number of ways to pick `n_choose` items from `n_total`,
/// where order matters.
///
/// # Formula
/// P(n, r) = n! / (n-r)!
///
/// # Returns
/// - 0 for invalid inputs (n_choose > n_total)
/// - None if the result overflows u64
#[allow(dead_code)]
pub fn count_permutations(n_total: u64, n_choose: u64) -> Option<u64> {
    let start = n_total.checked_sub(n_choose)?.checked_add(1u64)?;
    (start..=n_total).try_fold(1u64, |acc, val| acc.checked_mul(val))
}
