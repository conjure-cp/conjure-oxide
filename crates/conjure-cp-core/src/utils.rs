pub fn count_combinations(n_total: u64, n_choose: u64) -> u64 {
    if n_choose > n_total {
        0
    } else {
        (1..=n_choose.min(n_total - n_choose)).fold(1, |acc, val| acc * (n_total - val + 1) / val)
    }
}

#[allow(dead_code)]
pub fn count_permutations(n_total: u64, n_choose: u64) -> u64 {
    (n_total - n_choose + 1..=n_total).product()
}
