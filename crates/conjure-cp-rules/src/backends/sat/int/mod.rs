mod channelling;
mod direct;
mod log;
mod order;

/// Essence division rounds towards negative infinity; widened arithmetic avoids MIN / -1.
fn floor_div(dividend: i32, divisor: i32) -> Option<i32> {
    let (dividend, divisor) = (i64::from(dividend), i64::from(divisor));
    let quotient = dividend.checked_div(divisor)?;
    let remainder = dividend % divisor;
    let quotient = if remainder != 0 && (dividend < 0) != (divisor < 0) {
        quotient - 1
    } else {
        quotient
    };
    i32::try_from(quotient).ok()
}

#[cfg(test)]
mod tests {
    use super::floor_div;
    #[test]
    fn floor_division_handles_signs_and_machine_boundaries() {
        for dividend in -20..=20 {
            for divisor in -20..=20 {
                if divisor == 0 {
                    assert_eq!(floor_div(dividend, divisor), None);
                    continue;
                }
                let quotient = floor_div(dividend, divisor).unwrap();
                let remainder = dividend - quotient * divisor;
                assert!(if divisor > 0 {
                    0 <= remainder && remainder < divisor
                } else {
                    divisor < remainder && remainder <= 0
                });
            }
        }
        assert_eq!(floor_div(i32::MIN, -1), None);
        assert_eq!(floor_div(i32::MIN, 1), Some(i32::MIN));
        assert_eq!(floor_div(i32::MIN, 3), Some(-715827883));
    }
}
