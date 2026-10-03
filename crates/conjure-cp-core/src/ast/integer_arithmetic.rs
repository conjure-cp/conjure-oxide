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

#[cfg(test)]
mod tests {
    use super::{floor_div, floor_mod};

    #[test]
    fn floor_arithmetic_preserves_identity_and_divisor_sign() {
        for dividend in -20..=20 {
            for divisor in -20..=20 {
                if divisor == 0 {
                    assert_eq!(floor_div(dividend, divisor), None);
                    assert_eq!(floor_mod(dividend, divisor), None);
                    continue;
                }
                let quotient = floor_div(dividend, divisor).unwrap();
                let remainder = floor_mod(dividend, divisor).unwrap();
                assert_eq!(dividend, quotient * divisor + remainder);
                assert!(if divisor > 0 {
                    0 <= remainder && remainder < divisor
                } else {
                    divisor < remainder && remainder <= 0
                });
            }
        }
    }

    #[test]
    fn floor_arithmetic_is_exact_at_machine_boundaries() {
        assert_eq!(floor_div(i32::MIN, -1), None);
        assert_eq!(floor_mod(i32::MIN, -1), Some(0));
        assert_eq!(floor_mod(i32::MIN, 3), Some(1));
        assert_eq!(floor_mod(i32::MAX, -3), Some(-2));
        assert_eq!(floor_mod(i32::MAX, i32::MIN), Some(-1));
        assert_eq!(floor_div(i32::MIN, 3), Some(-715827883));
        assert_eq!(floor_div(i32::MAX, 1), Some(i32::MAX));
        assert_eq!(floor_mod(i32::MAX, 1), Some(0));
    }
}
