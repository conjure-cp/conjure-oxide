/// Essence division rounds towards negative infinity.
/// Returns `None` for zero divisors or a quotient outside the integer range.
pub fn floor_div(dividend: i32, divisor: i32) -> Option<i32> {
    let (quotient, _) = floor_div_mod(dividend, divisor)?;
    i32::try_from(quotient).ok()
}

/// Essence modulo has the divisor's sign and is undefined for zero divisors.
/// Widened arithmetic also handles `i32::MIN % -1`, whose remainder is zero.
pub fn floor_mod(dividend: i32, divisor: i32) -> Option<i32> {
    let (_, remainder) = floor_div_mod(dividend, divisor)?;
    i32::try_from(remainder).ok()
}

fn floor_div_mod(dividend: i32, divisor: i32) -> Option<(i64, i64)> {
    let (dividend, divisor) = (i64::from(dividend), i64::from(divisor));
    let quotient = dividend.checked_div(divisor)?;
    let remainder = dividend % divisor;
    if remainder != 0 && (dividend < 0) != (divisor < 0) {
        Some((quotient - 1, remainder + divisor))
    } else {
        Some((quotient, remainder))
    }
}
