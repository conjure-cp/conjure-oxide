mod channelling;
mod direct;
mod log;
mod order;

use conjure_cp::ast::floor_div;

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
