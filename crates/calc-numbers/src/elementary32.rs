use crate::double_word::{DoubleWord, two_product, two_sum};
use crate::integer::Integer;
use crate::number::Number;
use crate::word_conversion::{f32_from_small_i32, i32_from_small_integral_f32};

const DEFAULT_NAN_BITS: u32 = 0x7fc0_0000;
const QUIET_BIT: u32 = 0x0040_0000;
const EXPONENT_MASK: u32 = 0xff;
const FRACTION_BITS: u32 = 23;
const EXPONENT_BIAS: i32 = 127;
const SMALLEST_SUBNORMAL_EXPONENT: i32 = -149;
const SMALLEST_NORMAL_EXPONENT: i32 = -126;
const SUBNORMAL_SCALE_EXPONENT: i32 = 149;
const TINY_ARGUMENT: f32 = 6.0e-5;
const EXPONENTIAL_TINY_ARGUMENT: f32 = 2.0e-8;
const EXPONENTIAL_OVERFLOW_ARGUMENT: f32 = 89.0;
const EXPONENTIAL_UNDERFLOW_ARGUMENT: f32 = -104.0;
const REDUCTION_THRESHOLD: f32 = std::f32::consts::FRAC_PI_4;
const EXPONENTIAL_TAYLOR_TERMS: u16 = 14;
const SINE_TAYLOR_TERMS: u16 = 11;
const LOGARITHM_SERIES_TERMS: u16 = 10;
const ARCTANGENT_SERIES_TERMS: u16 = 5;
const ARCTANGENT_HALVINGS: u8 = 4;
const ARCTANGENT_HALVING_FACTOR: f32 = 16.0;
const REDUCTION_CHUNK_BITS: i32 = 24;
const LARGE_RECIPROCAL_ARGUMENT: f32 = 16_777_216.0;
const LARGE_OPERAND: f32 = 1.0e30;
const SMALL_OPERAND: f32 = 1.0e-30;
const OPERAND_SCALE_EXPONENT: i32 = 60;
const REDUCTION_EXTRA_CHUNKS: usize = 5;

const LN2: DoubleWord = DoubleWord::from_bits(0x3f31_7218, 0xb102_e308);
const PI: DoubleWord = DoubleWord::from_bits(0x4049_0fdb, 0xb3bb_bd2e);
const HALF_PI: DoubleWord = DoubleWord::from_bits(0x3fc9_0fdb, 0xb33b_bd2e);
const QUARTER_PI: DoubleWord = DoubleWord::from_bits(0x3f49_0fdb, 0xb2bb_bd2e);
const THREE_QUARTER_PI: DoubleWord = DoubleWord::from_bits(0x4016_cbe4, 0xb1cc_de2e);
const TWO_OVER_PI_CHUNKS: [f32; 12] = [
    10_680_707.0,
    7_228_996.0,
    1_387_004.0,
    2_578_385.0,
    16_069_853.0,
    12_639_074.0,
    9_804_092.0,
    4_427_841.0,
    16_666_979.0,
    11_263_675.0,
    12_935_607.0,
    2_387_514.0,
];

fn default_nan() -> f32 {
    f32::from_bits(DEFAULT_NAN_BITS)
}

fn quieted(value: f32) -> f32 {
    f32::from_bits(value.to_bits() | QUIET_BIT)
}

fn power_of_two_step(exponent: i32) -> f32 {
    let field = u32::try_from(EXPONENT_BIAS + exponent).unwrap_or_default();
    f32::from_bits(field << FRACTION_BITS)
}

fn power_of_two(exponent: i32) -> f32 {
    scaled(1.0, exponent)
}

fn scaled(value: f32, exponent: i32) -> f32 {
    let mut result = value;
    let mut remaining = exponent;
    while remaining > 0 {
        let step = remaining.min(REDUCTION_CHUNK_BITS);
        result *= power_of_two_step(step);
        remaining -= step;
    }
    while remaining < 0 {
        let step = remaining.max(-REDUCTION_CHUNK_BITS);
        result *= power_of_two_step(step);
        remaining -= step;
    }
    result
}

fn exponent_of_normal(value: f32) -> i32 {
    let field = (value.to_bits() >> FRACTION_BITS) & EXPONENT_MASK;
    i32::try_from(field).unwrap_or_default() - EXPONENT_BIAS
}

fn round_to_integer(value: f32) -> f32 {
    value.round_ties_even()
}

fn result_times_power_of_two(value: DoubleWord, exponent: i32) -> f32 {
    let leading = value.rounded();
    if leading == 0.0 || !leading.is_finite() {
        return leading;
    }
    if exponent_of_normal(leading) + exponent >= SMALLEST_NORMAL_EXPONENT {
        return scaled(leading, exponent);
    }
    let shift = exponent + SUBNORMAL_SCALE_EXPONENT;
    let high = scaled(value.high, shift);
    let low = scaled(value.low, shift);
    let mut units = round_to_integer(high);
    let excess = (high - units) + low;
    let is_odd = units % 2.0 != 0.0;
    if excess > 0.5 || (excess == 0.5 && is_odd) {
        units += 1.0;
    } else if excess < -0.5 || (excess == -0.5 && is_odd) {
        units -= 1.0;
    }
    if units == 0.0 {
        return 0.0f32.copysign(value.high);
    }
    scaled(units, SMALLEST_SUBNORMAL_EXPONENT)
}

pub fn elementary_rounding_error_bound_f32(result: f32) -> Option<Number> {
    if !result.is_finite() {
        return None;
    }
    let field = (result.to_bits() >> FRACTION_BITS) & EXPONENT_MASK;
    let unit_exponent = if field == 0 {
        SMALLEST_SUBNORMAL_EXPONENT
    } else {
        i32::try_from(field).ok()? - EXPONENT_BIAS - 23
    };
    let magnitude = Integer::from(2i64).pow(unit_exponent.unsigned_abs());
    if unit_exponent >= 0 {
        Some(Number::Integer(magnitude))
    } else {
        Number::fraction(&Integer::one(), &magnitude).ok()
    }
}

pub fn sqrt_f32(value: f32) -> f32 {
    if value.is_nan() {
        return quieted(value);
    }
    if value < 0.0 {
        return default_nan();
    }
    value.sqrt()
}

fn exponential_of_reduced(argument: DoubleWord) -> (DoubleWord, f32) {
    let multiple = round_to_integer(argument.high / LN2.high);
    let reduced = argument.sub(LN2.mul_f32(multiple));
    let mut sum = DoubleWord::from_f32(1.0);
    for index in (1..=EXPONENTIAL_TAYLOR_TERMS).rev() {
        sum = sum.mul(reduced).div_f32(f32::from(index)).add_f32(1.0);
    }
    (sum, multiple)
}

fn exponential(argument: DoubleWord) -> f32 {
    let (sum, multiple) = exponential_of_reduced(argument);
    result_times_power_of_two(sum, i32_from_small_integral_f32(multiple))
}

pub fn exp_f32(value: f32) -> f32 {
    if value.is_nan() {
        return quieted(value);
    }
    if value.abs() < EXPONENTIAL_TINY_ARGUMENT {
        return 1.0 + value;
    }
    if value > EXPONENTIAL_OVERFLOW_ARGUMENT {
        return f32::INFINITY;
    }
    if value < EXPONENTIAL_UNDERFLOW_ARGUMENT {
        return 0.0;
    }
    exponential(DoubleWord::from_f32(value))
}

fn logarithm(value: f32) -> DoubleWord {
    let bits = value.to_bits();
    let field = (bits >> FRACTION_BITS) & EXPONENT_MASK;
    let (normal, extra_exponent) = if field == 0 {
        (
            value * power_of_two(REDUCTION_CHUNK_BITS),
            -REDUCTION_CHUNK_BITS,
        )
    } else {
        (value, 0)
    };
    let mut exponent = exponent_of_normal(normal) + extra_exponent;
    let mut mantissa = scaled(normal, -exponent_of_normal(normal));
    if mantissa > std::f32::consts::SQRT_2 {
        mantissa *= 0.5;
        exponent += 1;
    }
    let numerator = DoubleWord::from_f32(mantissa - 1.0);
    let denominator = DoubleWord::from_sum(mantissa, 1.0);
    let ratio = numerator.div(denominator);
    let ratio_square = ratio.square();
    let mut sum = DoubleWord::from_f32(0.0);
    for index in (1..=LOGARITHM_SERIES_TERMS).rev() {
        let coefficient = DoubleWord::from_quotient(1.0, f32::from(2 * index + 1));
        sum = sum.add(coefficient).mul(ratio_square);
    }
    let mantissa_logarithm = sum.add_f32(1.0).mul(ratio).mul_f32(2.0);
    mantissa_logarithm.add(LN2.mul_f32(f32_from_small_i32(exponent)))
}

pub fn ln_f32(value: f32) -> f32 {
    if value.is_nan() {
        return quieted(value);
    }
    if value == 0.0 {
        return f32::NEG_INFINITY;
    }
    if value < 0.0 {
        return default_nan();
    }
    if value == f32::INFINITY {
        return f32::INFINITY;
    }
    if value == 1.0 {
        return 0.0;
    }
    logarithm(value).rounded()
}

struct Reduction {
    remainder: DoubleWord,
    quadrant: u8,
}

fn accumulate(expansion: &mut Vec<f32>, value: f32) {
    let mut carry = value;
    for component in expansion.iter_mut() {
        let (sum, error) = two_sum(*component, carry);
        *component = error;
        carry = sum;
    }
    expansion.push(carry);
}

fn quadrant_index(value: f32) -> u8 {
    if value == 0.0 {
        0
    } else if value == 1.0 {
        1
    } else if value == 2.0 {
        2
    } else {
        3
    }
}

fn integer_modulo_four(value: f32) -> f32 {
    value - 4.0 * (value * 0.25).floor()
}

fn without_multiples_of_four(value: f32) -> f32 {
    if value.abs() < 4.0 {
        return value;
    }
    integer_modulo_four(value)
}

fn reduce_by_half_pi(value: f32) -> Reduction {
    let magnitude = value.abs();
    let exponent = exponent_of_normal(magnitude);
    let integer_exponent = exponent - 23;
    let integer_mantissa = scaled(magnitude, -integer_exponent);

    let skipped =
        usize::try_from((integer_exponent - 2).max(0) / REDUCTION_CHUNK_BITS).unwrap_or_default();
    let mut expansion = Vec::new();
    for (index, chunk) in TWO_OVER_PI_CHUNKS
        .iter()
        .enumerate()
        .skip(skipped)
        .take(REDUCTION_EXTRA_CHUNKS + 2)
    {
        let position = i32::try_from(index).unwrap_or_default() + 1;
        let chunk_exponent = integer_exponent - REDUCTION_CHUNK_BITS * position;
        let (product, error) = two_product(integer_mantissa, *chunk);
        accumulate(
            &mut expansion,
            without_multiples_of_four(scaled(product, chunk_exponent)),
        );
        accumulate(
            &mut expansion,
            without_multiples_of_four(scaled(error, chunk_exponent)),
        );
    }

    let mut leading = expansion
        .iter()
        .rev()
        .fold(0.0f32, |total, part| total + part);
    let mut quadrant_value = round_to_integer(leading);
    accumulate(&mut expansion, -quadrant_value);
    leading = expansion
        .iter()
        .rev()
        .fold(0.0f32, |total, part| total + part);
    if leading > 0.5 {
        accumulate(&mut expansion, -1.0);
        quadrant_value += 1.0;
    } else if leading < -0.5 {
        accumulate(&mut expansion, 1.0);
        quadrant_value -= 1.0;
    }

    let mut fraction = DoubleWord::from_f32(0.0);
    for part in expansion.iter().rev() {
        fraction = fraction.add_f32(*part);
    }
    let mut remainder = fraction.mul(HALF_PI);
    let mut quadrant = quadrant_index(integer_modulo_four(quadrant_value));
    if value < 0.0 {
        remainder = remainder.negated();
        quadrant = (4 - quadrant) % 4;
    }
    Reduction {
        remainder,
        quadrant,
    }
}

struct SineCosine {
    sine: DoubleWord,
    cosine: DoubleWord,
}

fn sine_cosine_of_small(argument: DoubleWord) -> SineCosine {
    let square = argument.square();
    let mut sine = DoubleWord::from_f32(1.0);
    let mut cosine = DoubleWord::from_f32(1.0);
    for index in (1..=SINE_TAYLOR_TERMS).rev() {
        let even = f32::from(2 * index);
        sine = DoubleWord::from_f32(1.0).sub(sine.mul(square).div_f32(even * (even + 1.0)));
        cosine = DoubleWord::from_f32(1.0).sub(cosine.mul(square).div_f32((even - 1.0) * even));
    }
    SineCosine {
        sine: sine.mul(argument),
        cosine,
    }
}

fn sine_cosine(value: f32) -> SineCosine {
    if value.abs() <= REDUCTION_THRESHOLD {
        return sine_cosine_of_small(DoubleWord::from_f32(value));
    }
    let reduction = reduce_by_half_pi(value);
    let pair = sine_cosine_of_small(reduction.remainder);
    match reduction.quadrant {
        0 => pair,
        1 => SineCosine {
            sine: pair.cosine,
            cosine: pair.sine.negated(),
        },
        2 => SineCosine {
            sine: pair.sine.negated(),
            cosine: pair.cosine.negated(),
        },
        _ => SineCosine {
            sine: pair.cosine.negated(),
            cosine: pair.sine,
        },
    }
}

pub fn sin_f32(value: f32) -> f32 {
    if value.is_nan() {
        return quieted(value);
    }
    if value.is_infinite() {
        return default_nan();
    }
    if value.abs() < TINY_ARGUMENT {
        return value;
    }
    sine_cosine(value).sine.rounded()
}

pub fn cos_f32(value: f32) -> f32 {
    if value.is_nan() {
        return quieted(value);
    }
    if value.is_infinite() {
        return default_nan();
    }
    if value.abs() < EXPONENTIAL_TINY_ARGUMENT {
        return 1.0;
    }
    sine_cosine(value).cosine.rounded()
}

pub fn tan_f32(value: f32) -> f32 {
    if value.is_nan() {
        return quieted(value);
    }
    if value.is_infinite() {
        return default_nan();
    }
    if value.abs() < TINY_ARGUMENT {
        return value;
    }
    let pair = sine_cosine(value);
    pair.sine.div(pair.cosine).rounded()
}

fn arctangent_of_bounded(argument: DoubleWord) -> DoubleWord {
    let mut halved = argument;
    for _ in 0..ARCTANGENT_HALVINGS {
        let root = halved.square().add_f32(1.0).sqrt();
        halved = halved.div(root.add_f32(1.0));
    }
    let square = halved.square();
    let mut sum = DoubleWord::from_f32(0.0);
    for index in (1..=ARCTANGENT_SERIES_TERMS).rev() {
        let coefficient = DoubleWord::from_quotient(1.0, f32::from(2 * index + 1));
        let signed = if index % 2 == 1 {
            coefficient.negated()
        } else {
            coefficient
        };
        sum = sum.add(signed).mul(square);
    }
    sum.add_f32(1.0)
        .mul(halved)
        .mul_f32(ARCTANGENT_HALVING_FACTOR)
}

fn arctangent(argument: DoubleWord) -> DoubleWord {
    if argument.high.abs() <= 1.0 {
        return arctangent_of_bounded(argument);
    }
    let reciprocal = if argument.high.abs() >= LARGE_RECIPROCAL_ARGUMENT {
        DoubleWord::from_f32(1.0 / argument.high)
    } else {
        DoubleWord::from_f32(1.0).div(argument)
    };
    let complement = arctangent_of_bounded(reciprocal);
    if argument.high > 0.0 {
        HALF_PI.sub(complement)
    } else {
        HALF_PI.negated().sub(complement)
    }
}

pub fn atan_f32(value: f32) -> f32 {
    if value.is_nan() {
        return quieted(value);
    }
    if value.abs() < TINY_ARGUMENT {
        return value;
    }
    if value.is_infinite() {
        return HALF_PI.rounded().copysign(value);
    }
    arctangent(DoubleWord::from_f32(value)).rounded()
}

pub fn asin_f32(value: f32) -> f32 {
    if value.is_nan() {
        return quieted(value);
    }
    if value.abs() > 1.0 {
        return default_nan();
    }
    if value.abs() < TINY_ARGUMENT {
        return value;
    }
    if value.abs() == 1.0 {
        return HALF_PI.rounded().copysign(value);
    }
    let complement = DoubleWord::from_product(value, value)
        .negated()
        .add_f32(1.0);
    let tangent = DoubleWord::from_f32(value).div(complement.sqrt());
    arctangent(tangent).rounded()
}

pub fn acos_f32(value: f32) -> f32 {
    if value.is_nan() {
        return quieted(value);
    }
    if value.abs() > 1.0 {
        return default_nan();
    }
    if value == 1.0 {
        return 0.0;
    }
    if value == -1.0 {
        return PI.rounded();
    }
    let ratio = DoubleWord::from_sum(1.0, -value).div(DoubleWord::from_sum(1.0, value));
    arctangent(ratio.sqrt()).mul_f32(2.0).rounded()
}

pub fn atan2_f32(numerator: f32, denominator: f32) -> f32 {
    if numerator.is_nan() {
        return quieted(numerator);
    }
    if denominator.is_nan() {
        return quieted(denominator);
    }
    let denominator_is_negative = denominator.is_sign_negative();
    if numerator == 0.0 {
        return if denominator_is_negative {
            PI.rounded().copysign(numerator)
        } else {
            numerator
        };
    }
    if numerator.is_infinite() {
        let angle = match (denominator.is_infinite(), denominator_is_negative) {
            (true, true) => THREE_QUARTER_PI,
            (true, false) => QUARTER_PI,
            (false, _) => HALF_PI,
        };
        return angle.rounded().copysign(numerator);
    }
    if denominator == 0.0 {
        return HALF_PI.rounded().copysign(numerator);
    }
    if denominator.is_infinite() {
        return if denominator_is_negative {
            PI.rounded().copysign(numerator)
        } else {
            0.0f32.copysign(numerator)
        };
    }

    let numerator_is_positive = numerator > 0.0;
    let denominator_is_positive = denominator > 0.0;
    let larger_operand = numerator.abs().max(denominator.abs());
    let (numerator, denominator) = if larger_operand > LARGE_OPERAND {
        (
            scaled(numerator, -OPERAND_SCALE_EXPONENT),
            scaled(denominator, -OPERAND_SCALE_EXPONENT),
        )
    } else if larger_operand < SMALL_OPERAND {
        (
            scaled(numerator, OPERAND_SCALE_EXPONENT),
            scaled(denominator, OPERAND_SCALE_EXPONENT),
        )
    } else {
        (numerator, denominator)
    };
    let base = if numerator.abs() <= denominator.abs() {
        let quotient = numerator / denominator;
        if quotient.abs() < TINY_ARGUMENT {
            DoubleWord::from_f32(quotient)
        } else {
            arctangent_of_bounded(DoubleWord::from_quotient(numerator, denominator))
        }
    } else {
        let inverse = arctangent_of_bounded(DoubleWord::from_quotient(denominator, numerator));
        if numerator_is_positive == denominator_is_positive {
            HALF_PI.sub(inverse)
        } else {
            HALF_PI.negated().sub(inverse)
        }
    };
    let angle = if !denominator_is_negative {
        base
    } else if numerator_is_positive {
        base.add(PI)
    } else {
        base.sub(PI)
    };
    angle.rounded()
}

fn is_integer(value: f32) -> bool {
    value.trunc() == value
}

fn is_odd_integer(value: f32) -> bool {
    is_integer(value) && value.abs() < 16_777_216.0 && (value * 0.5).trunc() != value * 0.5
}

fn pow_special_case(base: f32, exponent: f32) -> Option<f32> {
    if exponent == 0.0 || base == 1.0 {
        return Some(1.0);
    }
    if base.is_nan() {
        return Some(quieted(base));
    }
    if exponent.is_nan() {
        return Some(quieted(exponent));
    }
    let exponent_is_odd_integer = is_odd_integer(exponent);
    if base == 0.0 {
        return Some(match (exponent < 0.0, exponent_is_odd_integer) {
            (true, true) => f32::INFINITY.copysign(base),
            (true, false) => f32::INFINITY,
            (false, true) => base,
            (false, false) => 0.0,
        });
    }
    if exponent.is_infinite() {
        let magnitude = base.abs();
        return Some(if magnitude == 1.0 {
            1.0
        } else if (magnitude < 1.0) == (exponent < 0.0) {
            f32::INFINITY
        } else {
            0.0
        });
    }
    if base.is_infinite() {
        let magnitude = if exponent < 0.0 { 0.0 } else { f32::INFINITY };
        return Some(if base < 0.0 && exponent_is_odd_integer {
            -magnitude
        } else {
            magnitude
        });
    }
    if base < 0.0 && !is_integer(exponent) {
        return Some(default_nan());
    }
    None
}

pub fn pow_f32(base: f32, exponent: f32) -> f32 {
    if let Some(result) = pow_special_case(base, exponent) {
        return result;
    }
    let is_negative_result = base < 0.0 && is_odd_integer(exponent);
    if base.abs() == 1.0 {
        return if is_negative_result { -1.0 } else { 1.0 };
    }
    let logarithm_of_base = logarithm(base.abs());
    let estimate = logarithm_of_base.high * exponent;
    let magnitude = if estimate > EXPONENTIAL_OVERFLOW_ARGUMENT + 1.0 {
        f32::INFINITY
    } else if estimate < EXPONENTIAL_UNDERFLOW_ARGUMENT - 1.0 {
        0.0
    } else {
        let product = logarithm_of_base.mul_f32(exponent);
        if product.high > EXPONENTIAL_OVERFLOW_ARGUMENT {
            f32::INFINITY
        } else if product.high < EXPONENTIAL_UNDERFLOW_ARGUMENT {
            0.0
        } else {
            exponential(product)
        }
    };
    if is_negative_result {
        -magnitude
    } else {
        magnitude
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ball::Ball;
    use crate::series;
    use crate::test_support::{absolute_difference, decimal_reference, is_at_most};

    const SMALLEST_SUBNORMAL: f32 = 1.0e-45;
    const NEGATIVE_QUIET_NAN_BITS: u32 = 0xffc0_0001;
    const SIGNALING_NAN_BITS: u32 = 0x7f80_0001;
    const CONSTANT_PRECISION: usize = 400;

    fn below_one() -> f32 {
        f32::from_bits(1.0f32.to_bits() - 1)
    }

    fn above_one() -> f32 {
        f32::from_bits(1.0f32.to_bits() + 1)
    }

    fn assert_within_bound(result: f32, reference: &str) {
        let reference = decimal_reference(reference);
        let exact_result = Number::F32(result).to_exact().unwrap();
        let bound = elementary_rounding_error_bound_f32(result).unwrap();
        let allowed = bound.add_exact(&reference.last_digit_unit).unwrap();
        let error = absolute_difference(&exact_result, &reference.value);
        assert!(is_at_most(&error, &allowed), "{result:e}");
    }

    fn assert_same(actual: f32, expected: f32) {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{actual:e} {expected:e}"
        );
    }

    fn assert_nan(value: f32) {
        assert!(value.is_nan(), "{value:e}");
    }

    fn assert_double_word_is_within(constant: DoubleWord, ball: &Ball) {
        let value = Number::F32(constant.high)
            .to_exact()
            .unwrap()
            .add_exact(&Number::F32(constant.low).to_exact().unwrap())
            .unwrap();
        let (lower, upper) = ball.bounds();
        let tolerance = Number::fraction(&Integer::one(), &Integer::from(2i64).pow(46)).unwrap();
        let widened_lower = lower.sub_exact(&tolerance).unwrap();
        let widened_upper = upper.add_exact(&tolerance).unwrap();
        assert!(is_at_most(&widened_lower, &value) && is_at_most(&value, &widened_upper));
    }

    fn assert_relative_error_below_2_to_minus_40(computed: DoubleWord, expected: &Ball) {
        let value = Number::F32(computed.high)
            .to_exact()
            .unwrap()
            .add_exact(&Number::F32(computed.low).to_exact().unwrap())
            .unwrap();
        let (lower, upper) = expected.bounds();
        let magnitude = absolute_difference(&lower, &Number::Integer(Integer::zero()));
        let tolerance = magnitude
            .div_exact(&Number::Integer(Integer::from(2i64).pow(40)))
            .unwrap();
        let widened_lower = lower.sub_exact(&tolerance).unwrap();
        let widened_upper = upper.add_exact(&tolerance).unwrap();
        assert!(is_at_most(&widened_lower, &value) && is_at_most(&value, &widened_upper));
    }

    fn reference_ball(value: f32) -> Ball {
        Ball::from_exact(&Number::F32(value).to_exact().unwrap(), CONSTANT_PRECISION).unwrap()
    }

    #[test]
    fn sine_and_cosine_series_are_accurate_at_quarter_pi() {
        let argument = std::f32::consts::FRAC_PI_4;
        let exact = series::sine_cosine(f64::from(argument), CONSTANT_PRECISION).unwrap();

        let computed = sine_cosine_of_small(DoubleWord::from_f32(argument));

        assert_relative_error_below_2_to_minus_40(computed.sine, &exact.sine);
        assert_relative_error_below_2_to_minus_40(computed.cosine, &exact.cosine);
    }

    #[test]
    fn logarithm_series_is_accurate_at_root_two() {
        let argument = std::f32::consts::SQRT_2;
        let exact = series::ln(f64::from(argument), CONSTANT_PRECISION).unwrap();

        assert_relative_error_below_2_to_minus_40(logarithm(argument), &exact);
    }

    #[test]
    fn arctangent_series_is_accurate_at_one() {
        let exact = series::arctangent(&Ball::one(CONSTANT_PRECISION)).unwrap();

        let computed = arctangent_of_bounded(DoubleWord::from_f32(1.0));

        assert_relative_error_below_2_to_minus_40(computed, &exact);
    }

    #[test]
    fn exponential_series_is_accurate_at_half_ln2() {
        let argument = 0.346_573_6f32;
        let exact = series::exp(&reference_ball(argument)).unwrap();

        let (computed, multiple) = exponential_of_reduced(DoubleWord::from_f32(argument));

        assert_eq!(multiple, 0.0);
        assert_relative_error_below_2_to_minus_40(computed, &exact);
    }

    #[test]
    fn ln2_constant_matches_series() {
        assert_double_word_is_within(LN2, &series::ln2(CONSTANT_PRECISION));
    }

    #[test]
    fn pi_constants_match_series() {
        let pi = series::pi(CONSTANT_PRECISION);
        assert_double_word_is_within(PI, &pi);
        assert_double_word_is_within(HALF_PI, &pi.mul_power_of_two(-1));
        assert_double_word_is_within(QUARTER_PI, &pi.mul_power_of_two(-2));
        assert_double_word_is_within(
            THREE_QUARTER_PI,
            &pi.mul_integer(&Integer::from(3i64)).mul_power_of_two(-2),
        );
    }

    #[test]
    fn two_over_pi_chunks_match_series() {
        let chunk_bits = TWO_OVER_PI_CHUNKS.len() * 24;
        let pi = series::pi(CONSTANT_PRECISION);
        let scaled_two = Ball::exact_integer(
            &Integer::from(2i64).pow(u32::try_from(chunk_bits).unwrap() + 1),
            CONSTANT_PRECISION,
        );
        let (lower, _) = scaled_two.div(&pi).unwrap().bounds();
        let expected = TWO_OVER_PI_CHUNKS
            .iter()
            .fold(Integer::zero(), |total, chunk| {
                let chunk_value = match Number::F32(*chunk).to_exact().unwrap() {
                    Number::Integer(integer) => integer,
                    _ => panic!(),
                };
                &(&total * &Integer::from(16_777_216i64)) + &chunk_value
            });
        let difference = lower.sub_exact(&Number::Integer(expected)).unwrap();
        assert!(!difference.is_negative_exact());
        assert!(is_at_most(&difference, &Number::Integer(Integer::one())));
    }

    #[test]
    fn bound_is_one_unit_in_last_place() {
        let expected = Number::fraction(&Integer::one(), &Integer::from(2i64).pow(23)).unwrap();

        assert_eq!(elementary_rounding_error_bound_f32(1.0), Some(expected));
    }

    #[test]
    fn bound_for_subnormal_and_zero_is_smallest_subnormal() {
        let expected = Number::fraction(&Integer::one(), &Integer::from(2i64).pow(149)).unwrap();

        assert_eq!(
            elementary_rounding_error_bound_f32(0.0),
            Some(expected.clone())
        );
        assert_eq!(
            elementary_rounding_error_bound_f32(SMALLEST_SUBNORMAL),
            Some(expected)
        );
    }

    #[test]
    fn bound_for_non_finite_result_is_none() {
        assert_eq!(elementary_rounding_error_bound_f32(f32::NEG_INFINITY), None);
        assert_eq!(elementary_rounding_error_bound_f32(f32::NAN), None);
    }

    #[test]
    fn nan_input_returns_quiet_nan_with_payload() {
        assert_eq!(
            exp_f32(f32::from_bits(SIGNALING_NAN_BITS)).to_bits(),
            0x7fc0_0001
        );
        assert_eq!(
            cos_f32(f32::from_bits(NEGATIVE_QUIET_NAN_BITS)).to_bits(),
            NEGATIVE_QUIET_NAN_BITS
        );
    }

    #[test]
    fn exp_of_known_arguments_is_within_bound() {
        let cases = [
            (
                1.0,
                "2.71828182845904523536028747135266249775724709369995957496697E+0",
            ),
            (
                -1.0,
                "3.67879441171442321595523770161460867445811131031767834507837E-1",
            ),
            (
                0.5,
                "1.64872127070012814684865078781416357165377610071014801157508E+0",
            ),
            (
                88.0,
                "1.65163625499400185552832979626485876706962884200004481388881E+38",
            ),
            (
                -103.0,
                "1.85211676951797546226470240473974201344887203227801964322490E-45",
            ),
            (
                1e-7,
                "1.00000010000000616861002583568708290733914731712395968562253E+0",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(exp_f32(argument), reference);
        }
    }

    #[test]
    fn exp_special_values() {
        assert_same(exp_f32(0.0), 1.0);
        assert_same(exp_f32(-0.0), 1.0);
        assert_same(exp_f32(f32::INFINITY), f32::INFINITY);
        assert_same(exp_f32(f32::NEG_INFINITY), 0.0);
        assert_same(exp_f32(f32::MAX), f32::INFINITY);
        assert_same(exp_f32(f32::MIN), 0.0);
        assert_same(exp_f32(SMALLEST_SUBNORMAL), 1.0);
        assert_same(exp_f32(f32::MIN_POSITIVE), 1.0);
        assert_same(exp_f32(89.0), f32::INFINITY);
        assert_same(exp_f32(-104.0), 0.0);
        assert_nan(exp_f32(f32::NAN));
    }

    #[test]
    fn ln_of_known_arguments_is_within_bound() {
        let cases = [
            (
                2.0,
                "6.93147180559945309417232121458176568075500134360255254120680E-1",
            ),
            (
                10.0,
                "2.30258509299404568401799145468436420760110148862877297603333E+0",
            ),
            (
                0.1,
                "-2.30258507809288460119263656429254717187886561068331284575531E+0",
            ),
            (
                1e-40,
                "-9.21034091096648769039117773968459622866452053173442507406095E+1",
            ),
            (
                f32::MAX,
                "8.87228390520683530536581765603140427337204118943246239837798E+1",
            ),
            (
                above_one(),
                "1.19209282445354457087579157062530716060862365149639064382486E-7",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(ln_f32(argument), reference);
        }
    }

    #[test]
    fn ln_special_values() {
        assert_same(ln_f32(0.0), f32::NEG_INFINITY);
        assert_same(ln_f32(-0.0), f32::NEG_INFINITY);
        assert_same(ln_f32(1.0), 0.0);
        assert_same(ln_f32(f32::INFINITY), f32::INFINITY);
        assert_nan(ln_f32(-1.0));
        assert_nan(ln_f32(f32::MIN));
        assert_nan(ln_f32(f32::NAN));
        assert!(ln_f32(below_one()) < 0.0);
        assert_same(ln_f32(SMALLEST_SUBNORMAL), -103.278_93);
    }

    #[test]
    fn sqrt_of_known_arguments_is_within_bound() {
        let cases = [
            (
                2.0,
                "1.41421356237309504880168872420969807856967187537694807317668E+0",
            ),
            (
                0.5,
                "7.07106781186547524400844362104849039284835937688474036588340E-1",
            ),
            (
                1e-40,
                "9.99997305052106607002244274415351358149057088819999096930281E-21",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(sqrt_f32(argument), reference);
        }
    }

    #[test]
    fn sqrt_special_values() {
        assert_same(sqrt_f32(0.0), 0.0);
        assert_same(sqrt_f32(-0.0), -0.0);
        assert_same(sqrt_f32(f32::INFINITY), f32::INFINITY);
        assert_nan(sqrt_f32(-SMALLEST_SUBNORMAL));
        assert_nan(sqrt_f32(f32::NAN));
    }

    #[test]
    fn sin_of_known_arguments_is_within_bound() {
        let cases = [
            (
                1.0,
                "8.41470984807896506652502321630298999622563060798371065672752E-1",
            ),
            (
                3.0,
                "1.41120008059867222100744802808110279846933264252265584151883E-1",
            ),
            (
                1e20,
                "6.56576677854590289966024878320102584287970255582014554647274E-1",
            ),
            (
                355.0,
                "-3.01443533594884492143302800086500995902558070663246491057898E-5",
            ),
            (
                6.230965e16,
                "-7.12414830923192525935250959322137520051436147762011602696499E-1",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(sin_f32(argument), reference);
        }
    }

    #[test]
    fn sin_special_values() {
        assert_same(sin_f32(0.0), 0.0);
        assert_same(sin_f32(-0.0), -0.0);
        assert_same(sin_f32(SMALLEST_SUBNORMAL), SMALLEST_SUBNORMAL);
        assert_same(sin_f32(-f32::MIN_POSITIVE), -f32::MIN_POSITIVE);
        assert_nan(sin_f32(f32::INFINITY));
        assert_nan(sin_f32(f32::NAN));
        assert!(sin_f32(f32::MAX).abs() <= 1.0);
    }

    #[test]
    fn cos_of_known_arguments_is_within_bound() {
        let cases = [
            (
                1.0,
                "5.40302305868139717400936607442976603732310420617922227670097E-1",
            ),
            (
                1.570_796_4,
                "-4.37113900018624143885728940026521523166125321751554758307294E-8",
            ),
            (
                -1.756_195_4e6,
                "4.54178735613814534143701171629927705851648907609109049404684E-1",
            ),
            (
                1e-4,
                "9.99999995000000256787911417816171104830777634690841576880974E-1",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(cos_f32(argument), reference);
        }
    }

    #[test]
    fn cos_special_values() {
        assert_same(cos_f32(0.0), 1.0);
        assert_same(cos_f32(-0.0), 1.0);
        assert_same(cos_f32(SMALLEST_SUBNORMAL), 1.0);
        assert_nan(cos_f32(f32::NEG_INFINITY));
        assert_nan(cos_f32(f32::NAN));
    }

    #[test]
    fn tan_of_known_arguments_is_within_bound() {
        let cases = [
            (
                1.0,
                "1.55740772465490223050697480745836017308725077238152003838395E+0",
            ),
            (
                1.570_796_4,
                "-2.28773324288564598739487467394576951815025309530285794103513E+7",
            ),
            (
                1.178_514_2e31,
                "4.15209889411914878931710279471040143133737952813674102014040E+0",
            ),
            (
                -2.5,
                "7.47022297238660279355352687825274557904116956883011279066593E-1",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(tan_f32(argument), reference);
        }
    }

    #[test]
    fn tan_special_values() {
        assert_same(tan_f32(0.0), 0.0);
        assert_same(tan_f32(-0.0), -0.0);
        assert_same(tan_f32(SMALLEST_SUBNORMAL), SMALLEST_SUBNORMAL);
        assert_nan(tan_f32(f32::INFINITY));
        assert_nan(tan_f32(f32::NAN));
    }

    #[test]
    fn asin_of_known_arguments_is_within_bound() {
        let cases = [
            (
                0.5,
                "5.23598775598298873077107230546583814032861566562517636829157E-1",
            ),
            (
                below_one(),
                "1.57045105981018041564371844215711270560129979141841731695490E+0",
            ),
            (
                -1e-3,
                "-1.00000021416421672084530611050124773943109014260877289994713E-3",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(asin_f32(argument), reference);
        }
    }

    #[test]
    fn asin_special_values() {
        assert_same(asin_f32(0.0), 0.0);
        assert_same(asin_f32(-0.0), -0.0);
        assert_same(asin_f32(1.0), std::f32::consts::FRAC_PI_2);
        assert_same(asin_f32(-1.0), -std::f32::consts::FRAC_PI_2);
        assert_same(asin_f32(SMALLEST_SUBNORMAL), SMALLEST_SUBNORMAL);
        assert_nan(asin_f32(above_one()));
        assert_nan(asin_f32(f32::NAN));
    }

    #[test]
    fn acos_of_known_arguments_is_within_bound() {
        let cases = [
            (
                0.5,
                "1.04719755119659774615421446109316762806572313312503527365831E+0",
            ),
            (
                below_one(),
                "3.45266984716203587603249482638736497284908269135593532568866E-4",
            ),
            (
                -below_one(),
                "3.14124738660507703487504013379686414769988449110597022744238E+0",
            ),
            (
                0.0,
                "1.57079632679489661923132169163975144209858469968755291048747E+0",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(acos_f32(argument), reference);
        }
    }

    #[test]
    fn acos_special_values() {
        assert_same(acos_f32(1.0), 0.0);
        assert_same(acos_f32(-1.0), std::f32::consts::PI);
        assert_same(acos_f32(-0.0), std::f32::consts::FRAC_PI_2);
        assert_nan(acos_f32(-above_one()));
        assert_nan(acos_f32(f32::NAN));
    }

    #[test]
    fn atan_of_known_arguments_is_within_bound() {
        let cases = [
            (
                1.0,
                "7.85398163397448309615660845819875721049292349843776455243736E-1",
            ),
            (
                1e30,
                "1.57079632679489661923132169163875144211363216568100336311132E+0",
            ),
            (
                -0.5,
                "-4.63647609000806116214256231461214402028537054286120263810933E-1",
            ),
            (
                5.769_890_7e-3,
                "5.76982670463559387047349943099045236771479102066416768352881E-3",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(atan_f32(argument), reference);
        }
    }

    #[test]
    fn atan_special_values() {
        assert_same(atan_f32(0.0), 0.0);
        assert_same(atan_f32(-0.0), -0.0);
        assert_same(atan_f32(f32::INFINITY), std::f32::consts::FRAC_PI_2);
        assert_same(atan_f32(f32::NEG_INFINITY), -std::f32::consts::FRAC_PI_2);
        assert_same(atan_f32(f32::MAX), std::f32::consts::FRAC_PI_2);
        assert_same(atan_f32(-SMALLEST_SUBNORMAL), -SMALLEST_SUBNORMAL);
        assert_nan(atan_f32(f32::NAN));
    }

    #[test]
    fn atan2_of_known_arguments_is_within_bound() {
        let cases = [
            (
                1.0,
                -1e-30,
                "1.57079632679489661923132169164075144210175577653852396183461E+0",
            ),
            (
                -3.0,
                -4.0,
                "-2.49809154479650885165983415456218024615565880825979343810934E+0",
            ),
            (
                1e-30,
                1e30,
                "9.99999988123610809803927645679327533097296580142323757401563E-61",
            ),
            (
                2.5,
                0.5,
                "1.37340076694501586086127192644496114865099959589970080896978E+0",
            ),
            (
                3e38,
                -2e38,
                "2.15879891474061049551958794594996670125378671061012826797633E+0",
            ),
        ];
        for (numerator, denominator, reference) in cases {
            assert_within_bound(atan2_f32(numerator, denominator), reference);
        }
    }

    const ATAN2_ORACLE_FINDINGS: &str = include_str!("../testdata/atan2-f32-oracle-findings.txt");

    #[test]
    fn atan2_of_maximum_over_smallest_subnormal_is_half_pi() {
        assert_same(
            atan2_f32(f32::MAX, SMALLEST_SUBNORMAL),
            std::f32::consts::FRAC_PI_2,
        );
    }

    #[test]
    fn atan2_of_subnormal_over_negative_maximum_is_pi() {
        assert_same(
            atan2_f32(SMALLEST_SUBNORMAL, -f32::MAX),
            std::f32::consts::PI,
        );
    }

    #[test]
    fn atan2_of_negative_subnormal_over_negative_maximum_is_minus_pi() {
        assert_same(
            atan2_f32(-SMALLEST_SUBNORMAL, -f32::MAX),
            -std::f32::consts::PI,
        );
    }

    #[test]
    fn atan2_oracle_findings_are_within_bound() {
        for line in ATAN2_ORACLE_FINDINGS.lines() {
            let mut fields = line.split_whitespace();
            let numerator_bits =
                u32::from_str_radix(fields.next().unwrap().trim_start_matches("0x"), 16).unwrap();
            let denominator_bits =
                u32::from_str_radix(fields.next().unwrap().trim_start_matches("0x"), 16).unwrap();
            let numerator = f32::from_bits(numerator_bits);
            let denominator = f32::from_bits(denominator_bits);

            let result = atan2_f32(numerator, denominator);

            let reference =
                crate::elementary::atan2_f64(f64::from(numerator), f64::from(denominator));
            let exact_reference = Number::F64(reference).to_exact().unwrap();
            let reference_error =
                crate::elementary::elementary_rounding_error_bound_f64(reference).unwrap();
            let allowed = elementary_rounding_error_bound_f32(result)
                .unwrap()
                .sub_exact(&reference_error)
                .unwrap();
            let error =
                absolute_difference(&Number::F32(result).to_exact().unwrap(), &exact_reference);
            assert!(is_at_most(&error, &allowed), "{line}");
        }
    }

    #[test]
    fn atan2_special_values() {
        use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};
        assert_same(atan2_f32(0.0, 0.0), 0.0);
        assert_same(atan2_f32(-0.0, 0.0), -0.0);
        assert_same(atan2_f32(0.0, -0.0), PI);
        assert_same(atan2_f32(1.0, 0.0), FRAC_PI_2);
        assert_same(atan2_f32(f32::INFINITY, f32::INFINITY), FRAC_PI_4);
        assert_same(
            atan2_f32(f32::NEG_INFINITY, f32::NEG_INFINITY),
            -3.0 * FRAC_PI_4,
        );
        assert_same(atan2_f32(1.0, f32::NEG_INFINITY), PI);
        assert_same(atan2_f32(-1.0, f32::INFINITY), -0.0);
        assert_same(atan2_f32(SMALLEST_SUBNORMAL, 1.0), SMALLEST_SUBNORMAL);
        assert_same(atan2_f32(-SMALLEST_SUBNORMAL, 1.0e30), -0.0);
        assert_nan(atan2_f32(f32::NAN, 1.0));
        assert_nan(atan2_f32(1.0, f32::NAN));
    }

    #[test]
    fn pow_of_known_arguments_is_within_bound() {
        let cases = [
            (
                2.0,
                0.5,
                "1.41421356237309504880168872420969807856967187537694807317668E+0",
            ),
            (
                above_one(),
                1e7,
                "3.29396769490594135712452767591519749177295138265370659357276E+0",
            ),
            (10.0, -3.0, "1E-3"),
            (
                0.1,
                3.5,
                "3.16227782509401444253167273404337697125216886637026586820670E-4",
            ),
            (
                1e-30,
                1.3,
                "1.00000329800223493840817329084279662282931674519337642034740E-39",
            ),
            (-2.0, -3.0, "-1.25E-1"),
        ];
        for (base, exponent, reference) in cases {
            assert_within_bound(pow_f32(base, exponent), reference);
        }
    }

    #[test]
    fn pow_special_values_with_zero_and_one() {
        assert_same(pow_f32(f32::NAN, 0.0), 1.0);
        assert_same(pow_f32(1.0, f32::NAN), 1.0);
        assert_same(pow_f32(-0.0, 3.0), -0.0);
        assert_same(pow_f32(-0.0, -3.0), f32::NEG_INFINITY);
        assert_same(pow_f32(-0.0, -2.0), f32::INFINITY);
        assert_same(pow_f32(0.0, 0.5), 0.0);
        assert_nan(pow_f32(f32::NAN, 1.0));
        assert_nan(pow_f32(2.0, f32::NAN));
    }

    #[test]
    fn pow_of_minus_one_to_the_maximum_is_one() {
        assert_same(pow_f32(-1.0, f32::MAX), 1.0);
        assert_same(pow_f32(-1.0, -f32::MAX), 1.0);
        assert_same(pow_f32(-1.0, 3.0), -1.0);
    }

    #[test]
    fn pow_special_values_with_infinities() {
        assert_same(pow_f32(-1.0, f32::INFINITY), 1.0);
        assert_same(pow_f32(0.5, f32::NEG_INFINITY), f32::INFINITY);
        assert_same(pow_f32(below_one(), f32::INFINITY), 0.0);
        assert_same(pow_f32(f32::NEG_INFINITY, -3.0), -0.0);
        assert_same(pow_f32(f32::NEG_INFINITY, 2.0), f32::INFINITY);
    }

    #[test]
    fn pow_of_negative_base_with_fractional_exponent_is_nan() {
        assert_nan(pow_f32(-8.0, 0.333_333_34));
    }

    #[test]
    fn pow_overflow_and_underflow_keep_sign() {
        assert_same(pow_f32(f32::MAX, 2.0), f32::INFINITY);
        assert_same(pow_f32(-f32::MAX, 3.0), f32::NEG_INFINITY);
        assert_same(pow_f32(f32::MIN_POSITIVE, 1.5), 0.0);
        assert_same(pow_f32(-SMALLEST_SUBNORMAL, 3.0), -0.0);
    }

    #[test]
    fn pow_with_exact_result_is_exact() {
        assert_same(pow_f32(2.0, 10.0), 1024.0);
        assert_same(pow_f32(4.0, 0.5), 2.0);
        assert_same(pow_f32(0.5, 149.0), SMALLEST_SUBNORMAL);
    }
}
