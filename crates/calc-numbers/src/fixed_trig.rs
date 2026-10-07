use crate::binary_format::BINARY64;
use crate::integer::Integer;
use crate::word_conversion::{f64_from_small_u64, i64_from_small_integral_f64};

const FRACTION_BITS: u32 = 124;
const GUARD_BITS: usize = 64;
const HALF_PI_WORDS: [u64; 3] = [
    0x2520_49c1_114c_f98e,
    0x9898_cc51_701b_839a,
    0x1921_fb54_442d_1846,
];
const ERROR_UNITS: i128 = 64;
const SMALLEST_MAGNITUDE_EXPONENT: i64 = -30;
const LARGEST_MAGNITUDE_EXPONENT: i64 = 20;
const REDUCED_LIMIT: i128 = 0x0CCC_CCCC_CCCC_CCCC_CCCC_CCCC_CCCC_CCCC;
const SINE_DEGREE: usize = 31;
const COSINE_DEGREE: usize = 32;
const EXPONENT_BIAS: i64 = 1023;
const SIGNIFICAND_BITS: u32 = 53;

const fn inverse_factorials() -> [i128; 34] {
    let mut table = [0_i128; 34];
    let mut factorial: u128 = 1;
    let mut factor: u128 = 0;
    let mut index = 0;
    while index < 34 {
        if factor > 0 {
            factorial *= factor;
        }
        table[index] = ((1_u128 << FRACTION_BITS) / factorial).cast_signed();
        index += 1;
        factor += 1;
    }
    table
}

const INVERSE_FACTORIALS: [i128; 34] = inverse_factorials();

fn wide_product(left: u128, right: u128) -> (u128, u128) {
    let mask = u128::from(u64::MAX);
    let (left_high, left_low) = (left >> 64, left & mask);
    let (right_high, right_low) = (right >> 64, right & mask);
    let low_low = left_low * right_low;
    let high_low = left_high * right_low;
    let low_high = left_low * right_high;
    let high_high = left_high * right_high;
    let middle = (low_low >> 64) + (high_low & mask) + (low_high & mask);
    let low = (low_low & mask) | ((middle & mask) << 64);
    let high = high_high + (high_low >> 64) + (low_high >> 64) + (middle >> 64);
    (high, low)
}

fn product(left: i128, right: i128) -> i128 {
    let (high, low) = wide_product(left.unsigned_abs(), right.unsigned_abs());
    let magnitude = (high << (128 - FRACTION_BITS)) | (low >> FRACTION_BITS);
    let magnitude = i128::try_from(magnitude).unwrap_or(i128::MAX);
    if (left < 0) != (right < 0) {
        -magnitude
    } else {
        magnitude
    }
}

fn half_pi() -> Integer {
    HALF_PI_WORDS
        .iter()
        .rev()
        .fold(Integer::zero(), |total, word| {
            &total.shifted_left(64) + &Integer::from(*word)
        })
}

fn reduced(magnitude: f64) -> Option<(i128, i64)> {
    let parts = BINARY64.finite_parts(magnitude.to_bits())?;
    let quarter_turns =
        i64_from_small_integral_f64((magnitude * std::f64::consts::FRAC_2_PI).round());
    let shift = usize::try_from(
        parts.exponent + i64::from(FRACTION_BITS) + i64::try_from(GUARD_BITS).ok()?,
    )
    .ok()?;
    let scaled = Integer::from(parts.significand).shifted_left(shift);
    let difference = &scaled - &(&Integer::from(quarter_turns) * &half_pi());
    let truncated = difference.absolute().shifted_right(GUARD_BITS)?.to_i128()?;
    let reduced = if difference.is_negative() {
        -truncated
    } else {
        truncated
    };
    (reduced.abs() <= REDUCED_LIMIT).then_some((reduced, quarter_turns))
}

fn sine_of_reduced(reduced: i128) -> i128 {
    let square = product(reduced, reduced);
    let mut sum = INVERSE_FACTORIALS[SINE_DEGREE];
    let mut degree = SINE_DEGREE;
    while degree > 1 {
        degree -= 2;
        sum = INVERSE_FACTORIALS[degree] - product(square, sum);
    }
    product(reduced, sum)
}

fn cosine_of_reduced(reduced: i128) -> i128 {
    let square = product(reduced, reduced);
    let mut sum = INVERSE_FACTORIALS[COSINE_DEGREE];
    let mut degree = COSINE_DEGREE;
    while degree > 0 {
        degree -= 2;
        sum = INVERSE_FACTORIALS[degree] - product(square, sum);
    }
    sum
}

fn power_of_two(exponent: i64) -> Option<f64> {
    let field = u64::try_from(exponent + EXPONENT_BIAS).ok()?;
    Some(f64::from_bits(field << 52))
}

fn nearest(value: i128) -> Option<f64> {
    let magnitude = value.unsigned_abs();
    if magnitude == 0 {
        return Some(0.0);
    }
    let bits = 128 - magnitude.leading_zeros();
    let shift = bits.saturating_sub(SIGNIFICAND_BITS);
    let mut significand = magnitude >> shift;
    if shift > 0 {
        let rest = magnitude & ((1_u128 << shift) - 1);
        let half = 1_u128 << (shift - 1);
        if rest > half || (rest == half && significand & 1 == 1) {
            significand += 1;
        }
    }
    let scale = power_of_two(i64::from(shift) - i64::from(FRACTION_BITS))?;
    let rounded = f64_from_small_u64(u64::try_from(significand).ok()?) * scale;
    Some(if value < 0 { -rounded } else { rounded })
}

fn decided(value: i128) -> Option<f64> {
    let low = nearest(value - ERROR_UNITS)?;
    let high = nearest(value + ERROR_UNITS)?;
    (low.to_bits() == high.to_bits()).then_some(low)
}

fn in_range(magnitude: f64) -> bool {
    BINARY64
        .finite_parts(magnitude.to_bits())
        .is_some_and(|parts| {
            let top = parts.exponent + i64::from(SIGNIFICAND_BITS);
            (SMALLEST_MAGNITUDE_EXPONENT..=LARGEST_MAGNITUDE_EXPONENT).contains(&top)
        })
}

pub(crate) fn sine(value: f64) -> Option<f64> {
    let magnitude = value.abs();
    if !in_range(magnitude) {
        return None;
    }
    let (reduced, quarter_turns) = reduced(magnitude)?;
    let result = match quarter_turns.rem_euclid(4) {
        0 => sine_of_reduced(reduced),
        1 => cosine_of_reduced(reduced),
        2 => -sine_of_reduced(reduced),
        _ => -cosine_of_reduced(reduced),
    };
    let rounded = decided(result)?;
    Some(if value < 0.0 { -rounded } else { rounded })
}

pub(crate) fn cosine(value: f64) -> Option<f64> {
    let magnitude = value.abs();
    if !in_range(magnitude) {
        return None;
    }
    let (reduced, quarter_turns) = reduced(magnitude)?;
    let result = match quarter_turns.rem_euclid(4) {
        0 => cosine_of_reduced(reduced),
        1 => -sine_of_reduced(reduced),
        2 => -cosine_of_reduced(reduced),
        _ => sine_of_reduced(reduced),
    };
    decided(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_half_pi_constant_is_the_floor_of_half_pi_scaled() {
        let scaled = crate::series::pi(400).mul_power_of_two(187);
        let floor = half_pi();

        assert!(scaled.is_surely_above(&floor));
        assert!(scaled.is_surely_below(&(&floor + &Integer::one())));
    }

    #[test]
    fn the_inverse_factorials_are_floors_of_scaled_reciprocals() {
        let mut factorial = Integer::one();
        for (index, value) in INVERSE_FACTORIALS.iter().enumerate() {
            if index > 0 {
                factorial = &factorial * &Integer::from(u64::try_from(index).unwrap());
            }
            let (quotient, _) = Integer::one()
                .shifted_left(usize::try_from(FRACTION_BITS).unwrap())
                .div_rem_euclid(&factorial)
                .unwrap();
            assert_eq!(quotient.to_i128(), Some(*value));
        }
    }

    #[test]
    fn a_small_argument_is_left_to_the_accurate_path() {
        assert_eq!(sine(1.0e-12), None);
        assert_eq!(cosine(1.0e-12), None);
    }

    #[test]
    fn a_large_argument_is_left_to_the_accurate_path() {
        assert_eq!(sine(1.0e9), None);
    }

    #[test]
    fn the_sine_of_the_double_nearest_pi_is_decided_from_its_exact_reduction() {
        assert_eq!(
            sine(std::f64::consts::PI),
            Some(1.224_646_799_147_353_2e-16)
        );
    }
}
