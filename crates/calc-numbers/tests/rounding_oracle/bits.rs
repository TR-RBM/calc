const FRACTION_BITS: u32 = 52;
const FRACTION_MASK: u64 = (1 << FRACTION_BITS) - 1;
const EXPONENT_MASK: u64 = 0x7ff;
const EXPONENT_BIAS: i32 = 1023;
const SUBNORMAL_EXPONENT: i32 = -1074;
const SIGNIFICAND_EXPONENT_OFFSET: i32 = 1075;
const LOWEST_POWER_EXPONENT: i32 = -1074;
const HIGHEST_NORMAL_EXPONENT: i32 = 1023;
const LOWEST_NORMAL_EXPONENT: i32 = -1022;
const MANTISSA_SPLIT: f64 = 1.5;

pub fn odd_significand_and_exponent(value: f64) -> Option<(u64, i32)> {
    if value == 0.0 || !value.is_finite() {
        return None;
    }
    let bits = value.to_bits();
    let field = i32::try_from((bits >> FRACTION_BITS) & EXPONENT_MASK).ok()?;
    let fraction = bits & FRACTION_MASK;
    let (significand, exponent) = if field == 0 {
        (fraction, SUBNORMAL_EXPONENT)
    } else {
        (
            fraction | (1 << FRACTION_BITS),
            field - SIGNIFICAND_EXPONENT_OFFSET,
        )
    };
    let trailing = significand.trailing_zeros();
    Some((
        significand >> trailing,
        exponent + i32::try_from(trailing).ok()?,
    ))
}

pub fn binary_exponent(value: f64) -> Option<i32> {
    let (significand, exponent) = odd_significand_and_exponent(value)?;
    let width = i32::try_from(u64::BITS - 1 - significand.leading_zeros()).ok()?;
    Some(exponent + width)
}

pub fn power_of_two(exponent: i32) -> f64 {
    if exponent > HIGHEST_NORMAL_EXPONENT {
        return f64::INFINITY;
    }
    if exponent < LOWEST_POWER_EXPONENT {
        return 0.0;
    }
    if exponent < LOWEST_NORMAL_EXPONENT {
        let shift = u32::try_from(exponent - LOWEST_POWER_EXPONENT).unwrap_or_default();
        return f64::from_bits(1u64 << shift);
    }
    let field = u64::try_from(exponent + EXPONENT_BIAS).unwrap_or_default();
    f64::from_bits(field << FRACTION_BITS)
}

pub fn mantissa_and_exponent(value: f64) -> Option<(f64, i32)> {
    let exponent = binary_exponent(value.abs())?;
    let (significand, odd_exponent) = odd_significand_and_exponent(value.abs())?;
    let shift = u32::try_from(exponent - odd_exponent).ok()?;
    let fraction = (significand << (FRACTION_BITS - shift)) & FRACTION_MASK;
    let field = u64::try_from(EXPONENT_BIAS).ok()?;
    let mantissa = f64::from_bits((field << FRACTION_BITS) | fraction);
    if mantissa >= MANTISSA_SPLIT {
        Some((mantissa * 0.5, exponent + 1))
    } else {
        Some((mantissa, exponent))
    }
}

pub fn integral_to_i64(value: f64) -> Option<i64> {
    if value == 0.0 {
        return Some(0);
    }
    let (significand, exponent) = odd_significand_and_exponent(value.abs())?;
    let shift = u32::try_from(exponent).ok()?;
    let magnitude = i64::try_from(significand).ok()?.checked_shl(shift)?;
    if magnitude.checked_shr(shift)? != i64::try_from(significand).ok()? {
        return None;
    }
    if value < 0.0 {
        Some(-magnitude)
    } else {
        Some(magnitude)
    }
}

pub fn is_odd_integer(value: f64) -> bool {
    matches!(odd_significand_and_exponent(value), Some((_, 0)))
}

pub fn is_integer(value: f64) -> bool {
    value == 0.0
        || matches!(odd_significand_and_exponent(value), Some((_, exponent)) if exponent >= 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odd_significand_of_twelve_is_three_times_four() {
        let parts = odd_significand_and_exponent(12.0);

        assert_eq!(parts, Some((3, 2)));
    }

    #[test]
    fn odd_significand_of_smallest_subnormal_is_one() {
        let parts = odd_significand_and_exponent(f64::from_bits(1));

        assert_eq!(parts, Some((1, -1074)));
    }

    #[test]
    fn binary_exponent_of_value_below_one_is_negative_one() {
        let exponent = binary_exponent(0.75);

        assert_eq!(exponent, Some(-1));
    }

    #[test]
    fn power_of_two_reaches_subnormal_range() {
        let value = power_of_two(-1074);

        assert_eq!(value.to_bits(), 1);
    }

    #[test]
    fn mantissa_of_three_is_three_quarters_times_four() {
        let parts = mantissa_and_exponent(3.0);

        assert_eq!(parts, Some((0.75, 2)));
    }

    #[test]
    fn mantissa_of_subnormal_lies_below_one_and_a_half() {
        let value = f64::from_bits(3);

        let parts = mantissa_and_exponent(value);

        assert_eq!(parts, Some((0.75, -1072)));
    }

    #[test]
    fn integral_to_i64_rejects_fraction() {
        let converted = integral_to_i64(2.5);

        assert_eq!(converted, None);
    }

    #[test]
    fn integral_to_i64_keeps_sign() {
        let converted = integral_to_i64(-1024.0);

        assert_eq!(converted, Some(-1024));
    }

    #[test]
    fn two_to_the_fifty_three_is_even_integer() {
        let value = power_of_two(53);

        assert!(is_integer(value) && !is_odd_integer(value));
    }
}
