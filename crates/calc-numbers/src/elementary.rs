use crate::ball::Ball;
use crate::binary_format::{BINARY64, FiniteParts};
use crate::integer::Integer;
use crate::natural::Natural;
use crate::number::Number;
use crate::series;
use crate::word_conversion::usize_from_u64;

const BASE_PRECISION: usize = 128;
const OVERFLOW_PROBE_PRECISION: usize = 64;
const DEFAULT_NAN_BITS: u64 = 0x7ff8_0000_0000_0000;
const EXPONENTIAL_OVERFLOW_ARGUMENT: f64 = 710.0;
const EXPONENTIAL_UNDERFLOW_ARGUMENT: f64 = -746.0;
const EXPONENTIAL_OVERFLOW_ARGUMENT_INTEGER: i64 = 710;
const EXPONENTIAL_UNDERFLOW_ARGUMENT_INTEGER: i64 = -746;
const LARGEST_EXPONENT_OF_TWO: i64 = 1100;
const SMALLEST_EXPONENT_OF_TWO: i64 = -1200;
const EXACT_POWER_LIMIT: i64 = 64;
const PERFECT_ROOT_LIMIT: u32 = 5;
const PRECISION_LIMIT_BITS: usize = 1 << 16;

fn default_nan() -> f64 {
    f64::from_bits(DEFAULT_NAN_BITS)
}

fn quieted(value: f64) -> f64 {
    f64::from_bits(BINARY64.quieted(value.to_bits()))
}

fn finite_parts(value: f64) -> Option<FiniteParts> {
    BINARY64.finite_parts(value.to_bits())
}

fn magnitude_exponent(value: f64) -> i64 {
    finite_parts(value).map_or(0, |parts| {
        let bits = 64 - i64::from(parts.significand.leading_zeros());
        parts.exponent + bits
    })
}

fn truncated_magnitude(value: f64) -> usize {
    let integer_part = match Number::F64(value.abs().trunc()).to_exact() {
        Ok(Number::Integer(integer)) => integer.to_i64().unwrap_or(i64::MAX),
        _ => 0,
    };
    usize_from_u64(integer_part.unsigned_abs())
}

fn precision_for_magnitude(exponent: i64) -> usize {
    BASE_PRECISION + usize_from_u64(exponent.min(0).unsigned_abs())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementaryError {
    PrecisionLimitReached { limit_bits: usize },
}

fn correctly_rounded_up_to(
    initial_precision: usize,
    limit_bits: usize,
    evaluate: impl Fn(usize) -> Option<Ball>,
) -> Result<f64, ElementaryError> {
    let mut precision = initial_precision.min(limit_bits);
    loop {
        if let Some(rounded) = evaluate(precision).and_then(|ball| ball.rounded_f64()) {
            return Ok(rounded);
        }
        if precision >= limit_bits {
            return Err(ElementaryError::PrecisionLimitReached { limit_bits });
        }
        precision = precision.saturating_mul(2).min(limit_bits);
    }
}

fn correctly_rounded(
    initial_precision: usize,
    evaluate: impl Fn(usize) -> Option<Ball>,
) -> Result<f64, ElementaryError> {
    correctly_rounded_up_to(initial_precision, PRECISION_LIMIT_BITS, evaluate)
}

fn value_or_nan(result: Result<f64, ElementaryError>) -> f64 {
    result.unwrap_or_else(|_| default_nan())
}

pub fn elementary_rounding_error_bound_f64(result: f64) -> Option<Number> {
    let parts = finite_parts(result)?;
    let unit_exponent = if parts.significand >> 52 == 0 {
        -1074
    } else {
        parts.exponent
    };
    let half_unit_exponent = unit_exponent - 1;
    let magnitude = Integer::from(2i64).pow(u32::try_from(half_unit_exponent.unsigned_abs()).ok()?);
    if half_unit_exponent >= 0 {
        Some(Number::Integer(magnitude))
    } else {
        Number::fraction(&Integer::one(), &magnitude).ok()
    }
}

pub fn sqrt_f64(value: f64) -> f64 {
    if value.is_nan() {
        return quieted(value);
    }
    if value < 0.0 {
        return default_nan();
    }
    value.sqrt()
}

pub fn checked_exp_f64(value: f64) -> Result<f64, ElementaryError> {
    if value.is_nan() {
        return Ok(quieted(value));
    }
    if value == 0.0 {
        return Ok(1.0);
    }
    if value >= EXPONENTIAL_OVERFLOW_ARGUMENT {
        return Ok(f64::INFINITY);
    }
    if value <= EXPONENTIAL_UNDERFLOW_ARGUMENT {
        return Ok(0.0);
    }
    let initial_precision = if value < 0.0 {
        BASE_PRECISION + 2 * truncated_magnitude(value)
    } else {
        BASE_PRECISION
    };
    correctly_rounded(initial_precision, |precision| {
        series::exp(&Ball::from_f64(value, precision)?)
    })
}

pub fn checked_ln_f64(value: f64) -> Result<f64, ElementaryError> {
    if value.is_nan() {
        return Ok(quieted(value));
    }
    if value == 0.0 {
        return Ok(f64::NEG_INFINITY);
    }
    if value < 0.0 {
        return Ok(default_nan());
    }
    if value == f64::INFINITY {
        return Ok(f64::INFINITY);
    }
    if value == 1.0 {
        return Ok(0.0);
    }
    correctly_rounded(BASE_PRECISION, |precision| series::ln(value, precision))
}

pub fn checked_sin_f64(value: f64) -> Result<f64, ElementaryError> {
    if value.is_nan() {
        return Ok(quieted(value));
    }
    if value.is_infinite() {
        return Ok(default_nan());
    }
    if value == 0.0 {
        return Ok(value);
    }
    if let Some(sine) = crate::fast_sine_cosine::sine(value) {
        return Ok(sine);
    }
    accurate_sin(value)
}

fn accurate_sin(value: f64) -> Result<f64, ElementaryError> {
    if let Some(sine) = crate::fixed_trig::sine(value) {
        return Ok(sine);
    }
    correctly_rounded(
        precision_for_magnitude(magnitude_exponent(value)),
        |precision| Some(series::sine_cosine(value, precision)?.sine),
    )
}

#[cfg(test)]
pub(crate) fn accurate_sin_f64(value: f64) -> f64 {
    value_or_nan(accurate_sin(value))
}

pub fn checked_cos_f64(value: f64) -> Result<f64, ElementaryError> {
    if value.is_nan() {
        return Ok(quieted(value));
    }
    if value.is_infinite() {
        return Ok(default_nan());
    }
    if value == 0.0 {
        return Ok(1.0);
    }
    if let Some(cosine) = crate::fast_sine_cosine::cosine(value) {
        return Ok(cosine);
    }
    accurate_cos(value)
}

fn accurate_cos(value: f64) -> Result<f64, ElementaryError> {
    if let Some(cosine) = crate::fixed_trig::cosine(value) {
        return Ok(cosine);
    }
    correctly_rounded(BASE_PRECISION, |precision| {
        Some(series::sine_cosine(value, precision)?.cosine)
    })
}

#[cfg(test)]
pub(crate) fn accurate_cos_f64(value: f64) -> f64 {
    value_or_nan(accurate_cos(value))
}

pub fn checked_tan_f64(value: f64) -> Result<f64, ElementaryError> {
    if value.is_nan() {
        return Ok(quieted(value));
    }
    if value.is_infinite() {
        return Ok(default_nan());
    }
    if value == 0.0 {
        return Ok(value);
    }
    correctly_rounded(
        precision_for_magnitude(magnitude_exponent(value)),
        |precision| {
            let pair = series::sine_cosine(value, precision)?;
            pair.sine.div(&pair.cosine)
        },
    )
}

fn half_pi_ball(precision: usize) -> Option<Ball> {
    Some(series::pi(precision).mul_power_of_two(-1))
}

fn pi_ball(precision: usize) -> Option<Ball> {
    Some(series::pi(precision))
}

fn quarter_pi_ball(precision: usize) -> Option<Ball> {
    Some(series::pi(precision).mul_power_of_two(-2))
}

fn three_quarter_pi_ball(precision: usize) -> Option<Ball> {
    Some(
        series::pi(precision)
            .mul_integer(&Integer::from(3i64))
            .mul_power_of_two(-2),
    )
}

pub fn checked_atan_f64(value: f64) -> Result<f64, ElementaryError> {
    if value.is_nan() {
        return Ok(quieted(value));
    }
    if value == 0.0 {
        return Ok(value);
    }
    if value.is_infinite() {
        return Ok(correctly_rounded(BASE_PRECISION, half_pi_ball)?.copysign(value));
    }
    if let Some(angle) = crate::fast_arctangent::arctangent(value) {
        return Ok(angle);
    }
    accurate_atan(value)
}

fn accurate_atan(value: f64) -> Result<f64, ElementaryError> {
    correctly_rounded(
        precision_for_magnitude(magnitude_exponent(value)),
        |precision| series::arctangent(&Ball::from_f64(value, precision)?),
    )
}

#[cfg(test)]
pub(crate) fn accurate_atan_f64(value: f64) -> f64 {
    value_or_nan(accurate_atan(value))
}

#[cfg(test)]
pub(crate) fn accurate_atan2_f64(numerator: f64, denominator: f64) -> f64 {
    value_or_nan(accurate_atan2(numerator, denominator))
}

pub fn checked_asin_f64(value: f64) -> Result<f64, ElementaryError> {
    if value.is_nan() {
        return Ok(quieted(value));
    }
    if value.abs() > 1.0 {
        return Ok(default_nan());
    }
    if value == 0.0 {
        return Ok(value);
    }
    if value.abs() == 1.0 {
        return Ok(correctly_rounded(BASE_PRECISION, half_pi_ball)?.copysign(value));
    }
    let Ok(exact) = Number::F64(value).to_exact() else {
        return Ok(default_nan());
    };
    let complement = exact
        .mul_exact(&exact)
        .and_then(|square| Number::from(1i64).sub_exact(&square));
    let Ok(complement) = complement else {
        return Ok(default_nan());
    };
    correctly_rounded(
        precision_for_magnitude(magnitude_exponent(value)),
        |precision| {
            let root = Ball::from_exact(&complement, precision)?.sqrt()?;
            let tangent = Ball::from_exact(&exact, precision)?.div(&root)?;
            series::arctangent(&tangent)
        },
    )
}

pub fn checked_acos_f64(value: f64) -> Result<f64, ElementaryError> {
    if value.is_nan() {
        return Ok(quieted(value));
    }
    if value.abs() > 1.0 {
        return Ok(default_nan());
    }
    if value == 1.0 {
        return Ok(0.0);
    }
    if value == -1.0 {
        return correctly_rounded(BASE_PRECISION, pi_ball);
    }
    let Ok(exact) = Number::F64(value).to_exact() else {
        return Ok(default_nan());
    };
    let one = Number::from(1i64);
    let quotient = one
        .sub_exact(&exact)
        .and_then(|numerator| numerator.div_exact(&one.add_exact(&exact)?));
    let Ok(quotient) = quotient else {
        return Ok(default_nan());
    };
    correctly_rounded(BASE_PRECISION, |precision| {
        let root = Ball::from_exact(&quotient, precision)?.sqrt()?;
        Some(series::arctangent(&root)?.mul_integer(&Integer::from(2i64)))
    })
}

pub fn checked_atan2_f64(numerator: f64, denominator: f64) -> Result<f64, ElementaryError> {
    if numerator.is_nan() {
        return Ok(quieted(numerator));
    }
    if denominator.is_nan() {
        return Ok(quieted(denominator));
    }
    let denominator_is_negative = denominator.is_sign_negative();

    if numerator == 0.0 {
        return if denominator_is_negative {
            Ok(correctly_rounded(BASE_PRECISION, pi_ball)?.copysign(numerator))
        } else {
            Ok(numerator)
        };
    }
    if numerator.is_infinite() {
        let angle = match (denominator.is_infinite(), denominator_is_negative) {
            (true, true) => correctly_rounded(BASE_PRECISION, three_quarter_pi_ball)?,
            (true, false) => correctly_rounded(BASE_PRECISION, quarter_pi_ball)?,
            (false, _) => correctly_rounded(BASE_PRECISION, half_pi_ball)?,
        };
        return Ok(angle.copysign(numerator));
    }
    if denominator == 0.0 {
        return Ok(correctly_rounded(BASE_PRECISION, half_pi_ball)?.copysign(numerator));
    }
    if denominator.is_infinite() {
        return if denominator_is_negative {
            Ok(correctly_rounded(BASE_PRECISION, pi_ball)?.copysign(numerator))
        } else {
            Ok(0.0f64.copysign(numerator))
        };
    }

    if let Some(angle) = crate::fast_arctangent::arctangent_of_quotient(numerator, denominator) {
        return Ok(angle);
    }
    accurate_atan2(numerator, denominator)
}

fn accurate_atan2(numerator: f64, denominator: f64) -> Result<f64, ElementaryError> {
    let denominator_is_negative = denominator.is_sign_negative();
    let (Ok(top), Ok(bottom)) = (
        Number::F64(numerator).to_exact(),
        Number::F64(denominator).to_exact(),
    ) else {
        return Ok(default_nan());
    };
    let Ok(ratio) = top.div_exact(&bottom) else {
        return Ok(default_nan());
    };
    let ratio_exponent = magnitude_exponent(numerator) - magnitude_exponent(denominator);
    let adjustment: i64 = if !denominator_is_negative {
        0
    } else if numerator > 0.0 {
        1
    } else {
        -1
    };
    correctly_rounded(precision_for_magnitude(ratio_exponent - 1), |precision| {
        let base = series::arctangent(&Ball::from_exact(&ratio, precision)?)?;
        Some(base.add(&series::pi(precision).mul_integer(&Integer::from(adjustment))))
    })
}

fn is_odd_integer(value: f64) -> bool {
    matches!(
        Number::F64(value).to_exact(),
        Ok(Number::Integer(integer)) if integer.magnitude().is_odd()
    )
}

fn is_integer(value: f64) -> bool {
    matches!(Number::F64(value).to_exact(), Ok(Number::Integer(_)))
}

fn power_of_two_number(exponent: &Integer) -> Option<Number> {
    let clamped = exponent
        .to_i64()
        .unwrap_or(if exponent.is_negative() {
            SMALLEST_EXPONENT_OF_TWO
        } else {
            LARGEST_EXPONENT_OF_TWO
        })
        .clamp(SMALLEST_EXPONENT_OF_TWO, LARGEST_EXPONENT_OF_TWO);
    let magnitude = Integer::from(2i64).pow(u32::try_from(clamped.unsigned_abs()).ok()?);
    if clamped >= 0 {
        Some(Number::Integer(magnitude))
    } else {
        Number::fraction(&Integer::one(), &magnitude).ok()
    }
}

fn perfect_root(value: &Natural, halvings: u32) -> Option<Natural> {
    let mut root = value.clone();
    for _ in 0..halvings {
        let next = root.floor_sqrt();
        if next.mul(&next) != root {
            return None;
        }
        root = next;
    }
    Some(root)
}

fn exact_power(base: f64, exponent: f64) -> Option<Number> {
    let parts = finite_parts(base)?;
    let odd_significand_shift = parts.significand.trailing_zeros();
    let odd_significand = Natural::from_u64(parts.significand >> odd_significand_shift);
    let base_exponent = Integer::from(parts.exponent + i64::from(odd_significand_shift));

    let (numerator, halvings) = match Number::F64(exponent).to_exact().ok()? {
        Number::Integer(integer) => (integer, 0u32),
        Number::Rational(rational) => {
            let halvings =
                u32::try_from(rational.denominator().magnitude().bit_length() - 1).ok()?;
            (rational.numerator().clone(), halvings)
        }
        Number::F32(_) | Number::F64(_) => return None,
    };
    let scale = Integer::from(2i64).pow(halvings);

    if odd_significand.is_one() {
        let (quotient, remainder) = (&base_exponent * &numerator).div_rem_euclid(&scale).ok()?;
        if !remainder.is_zero() {
            return None;
        }
        return power_of_two_number(&quotient);
    }

    if halvings > PERFECT_ROOT_LIMIT {
        return None;
    }
    let (scaled_exponent, remainder) = base_exponent.div_rem_euclid(&scale).ok()?;
    if !remainder.is_zero() {
        return None;
    }
    let root = perfect_root(&odd_significand, halvings)?;
    let power = numerator
        .to_i64()
        .filter(|power| (1..=EXACT_POWER_LIMIT).contains(power))?;
    let power = u32::try_from(power).ok()?;
    let odd_part = Integer::from_sign_and_magnitude(false, root.pow(power));
    let two_exponent = &scaled_exponent * &Integer::from(i64::from(power));
    let two_power = power_of_two_number(&two_exponent)?;
    Number::Integer(odd_part).mul_exact(&two_power).ok()
}

fn pow_special_case(base: f64, exponent: f64) -> Option<f64> {
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
            (true, true) => f64::INFINITY.copysign(base),
            (true, false) => f64::INFINITY,
            (false, true) => base,
            (false, false) => 0.0,
        });
    }
    if exponent.is_infinite() {
        let magnitude = base.abs();
        return Some(if magnitude == 1.0 {
            1.0
        } else if (magnitude < 1.0) == (exponent < 0.0) {
            f64::INFINITY
        } else {
            0.0
        });
    }
    if base.is_infinite() {
        let magnitude = if exponent < 0.0 { 0.0 } else { f64::INFINITY };
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

pub fn checked_pow_f64(base: f64, exponent: f64) -> Result<f64, ElementaryError> {
    if let Some(result) = pow_special_case(base, exponent) {
        return Ok(result);
    }
    let is_negative_result = base < 0.0 && is_odd_integer(exponent);
    let magnitude_base = base.abs();
    let with_sign = |magnitude: f64| {
        if is_negative_result {
            -magnitude
        } else {
            magnitude
        }
    };

    if let Some(exact) = exact_power(magnitude_base, exponent) {
        return Ok(with_sign(exact.round_to_f64_ties_even()));
    }

    let exponent_bits = usize_from_u64(magnitude_exponent(exponent).max(0).unsigned_abs());
    let overflow_bound = Integer::from(EXPONENTIAL_OVERFLOW_ARGUMENT_INTEGER);
    let underflow_bound = Integer::from(EXPONENTIAL_UNDERFLOW_ARGUMENT_INTEGER);
    let mut probe_precision = OVERFLOW_PROBE_PRECISION + exponent_bits;
    let probe = loop {
        let Some(probe) = power_logarithm(magnitude_base, exponent, probe_precision) else {
            return Ok(default_nan());
        };
        if probe.is_surely_above(&overflow_bound) {
            return Ok(with_sign(f64::INFINITY));
        }
        if probe.is_surely_below(&underflow_bound) {
            return Ok(with_sign(0.0));
        }
        if probe.has_radius_below_one() {
            break probe;
        }
        if probe_precision >= PRECISION_LIMIT_BITS {
            return Err(ElementaryError::PrecisionLimitReached {
                limit_bits: PRECISION_LIMIT_BITS,
            });
        }
        probe_precision = probe_precision.saturating_mul(2).min(PRECISION_LIMIT_BITS);
    };
    let result_exponent_estimate = probe.floor_of_center().to_i64().unwrap_or_default();
    let initial_precision = BASE_PRECISION
        + exponent_bits
        + 2 * usize_from_u64(result_exponent_estimate.min(0).unsigned_abs());
    Ok(with_sign(correctly_rounded(
        initial_precision,
        |precision| series::exp(&power_logarithm(magnitude_base, exponent, precision)?),
    )?))
}

pub fn exp_f64(value: f64) -> f64 {
    value_or_nan(checked_exp_f64(value))
}

pub fn ln_f64(value: f64) -> f64 {
    value_or_nan(checked_ln_f64(value))
}

pub fn sin_f64(value: f64) -> f64 {
    value_or_nan(checked_sin_f64(value))
}

pub fn cos_f64(value: f64) -> f64 {
    value_or_nan(checked_cos_f64(value))
}

pub fn tan_f64(value: f64) -> f64 {
    value_or_nan(checked_tan_f64(value))
}

pub fn atan_f64(value: f64) -> f64 {
    value_or_nan(checked_atan_f64(value))
}

pub fn asin_f64(value: f64) -> f64 {
    value_or_nan(checked_asin_f64(value))
}

pub fn acos_f64(value: f64) -> f64 {
    value_or_nan(checked_acos_f64(value))
}

pub fn atan2_f64(numerator: f64, denominator: f64) -> f64 {
    value_or_nan(checked_atan2_f64(numerator, denominator))
}

pub fn pow_f64(base: f64, exponent: f64) -> f64 {
    value_or_nan(checked_pow_f64(base, exponent))
}

fn power_logarithm(base: f64, exponent: f64, precision: usize) -> Option<Ball> {
    let logarithm = series::ln(base, precision)?;
    let factor = Ball::from_f64(exponent, precision)?;
    Some(logarithm.mul(&factor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{absolute_difference, decimal_reference, is_at_most};

    const SMALLEST_SUBNORMAL: f64 = 5e-324;
    const NEGATIVE_QUIET_NAN_BITS: u64 = 0xfff8_0000_0000_0001;
    const SIGNALING_NAN_BITS: u64 = 0x7ff0_0000_0000_0001;

    fn below_one() -> f64 {
        f64::from_bits(1.0f64.to_bits() - 1)
    }

    fn above_one() -> f64 {
        f64::from_bits(1.0f64.to_bits() + 1)
    }

    fn assert_within_bound(result: f64, reference: &str) {
        let reference = decimal_reference(reference);
        let exact_result = Number::F64(result).to_exact().unwrap();
        let bound = elementary_rounding_error_bound_f64(result).unwrap();
        let allowed = bound.add_exact(&reference.last_digit_unit).unwrap();
        let error = absolute_difference(&exact_result, &reference.value);
        assert!(is_at_most(&error, &allowed), "{result:e}");
    }

    fn assert_same(actual: f64, expected: f64) {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "{actual:e} {expected:e}"
        );
    }

    fn assert_nan(value: f64) {
        assert!(value.is_nan(), "{value:e}");
    }

    #[test]
    fn precision_limit_stops_the_loop_with_a_typed_error() {
        let midpoint_above_one = Number::fraction(
            &(&Integer::from(2i64).pow(53) + &Integer::one()),
            &Integer::from(2i64).pow(53),
        )
        .unwrap();
        let result = correctly_rounded_up_to(BASE_PRECISION, 256, |precision| {
            Some(Ball::from_exact(&midpoint_above_one, precision)?.widened(1))
        });

        assert_eq!(
            result,
            Err(ElementaryError::PrecisionLimitReached { limit_bits: 256 })
        );
    }

    #[test]
    fn undecided_evaluation_at_the_limit_never_returns_a_value() {
        let result = correctly_rounded_up_to(8, 8, |precision| series::exp(&Ball::one(precision)));

        assert!(result.is_err());
    }

    #[test]
    fn evaluation_decided_below_the_limit_is_returned() {
        let result =
            correctly_rounded_up_to(8, 1024, |precision| series::exp(&Ball::one(precision)));

        assert_eq!(result.map(f64::to_bits), Ok(std::f64::consts::E.to_bits()));
    }

    #[test]
    fn precision_limit_error_becomes_nan_in_the_plain_function() {
        let value = value_or_nan(Err(ElementaryError::PrecisionLimitReached {
            limit_bits: PRECISION_LIMIT_BITS,
        }));

        assert!(value.is_nan());
    }

    #[test]
    fn checked_function_agrees_with_plain_function() {
        assert_eq!(
            checked_sin_f64(1.0).map(f64::to_bits),
            Ok(sin_f64(1.0).to_bits())
        );
    }

    #[test]
    fn cached_exponential_keeps_uncached_bits_and_precision_errors() {
        for value in [
            -700.0,
            -2.5,
            -1.6322261495516757,
            -0.010195950715390869,
            0.1,
            1.0,
        ] {
            let initial_precision = if value < 0.0 {
                BASE_PRECISION + 2 * truncated_magnitude(value)
            } else {
                BASE_PRECISION
            };
            let original = correctly_rounded(initial_precision, |precision| {
                series::exp_until(&Ball::from_f64(value, precision)?, &|| false)
            });
            assert_eq!(
                checked_exp_f64(value).map(f64::to_bits),
                original.map(f64::to_bits)
            );
        }
        let wide = Ball::from_f64(1.0, 8)
            .unwrap()
            .widened(255)
            .with_precision(128);
        let cached = correctly_rounded_up_to(128, 128, |_| series::exp(&wide));
        let original = correctly_rounded_up_to(128, 128, |_| series::exp_until(&wide, &|| false));
        assert_eq!(cached, original);
        assert_eq!(
            cached,
            Err(ElementaryError::PrecisionLimitReached { limit_bits: 128 })
        );
    }

    #[test]
    fn radius_bearing_power_logarithms_keep_uncached_exponential_enclosures() {
        for (base, exponent) in [(2.5, 0.3), (0.75, 2.5), (1.3, -1.25)] {
            for precision in [128, 130, 132] {
                let argument = power_logarithm(base, exponent, precision).unwrap();
                let (lower, upper) = argument.bounds();
                assert_ne!(lower, upper);
                let cached = series::exp(&argument).unwrap();
                let original = series::exp_until(&argument, &|| false).unwrap();
                assert_eq!(cached.precision(), original.precision());
                assert_eq!(cached.bounds(), original.bounds());
                assert_eq!(
                    cached.rounded_f64().map(f64::to_bits),
                    original.rounded_f64().map(f64::to_bits)
                );
            }
        }
    }

    #[test]
    fn bound_is_half_unit_in_last_place() {
        let expected = Number::fraction(&Integer::one(), &Integer::from(2i64).pow(53)).unwrap();

        assert_eq!(elementary_rounding_error_bound_f64(1.0), Some(expected));
    }

    #[test]
    fn bound_for_subnormal_and_zero_is_half_smallest_subnormal() {
        let expected = Number::fraction(&Integer::one(), &Integer::from(2i64).pow(1075)).unwrap();

        assert_eq!(
            elementary_rounding_error_bound_f64(0.0),
            Some(expected.clone())
        );
        assert_eq!(
            elementary_rounding_error_bound_f64(SMALLEST_SUBNORMAL),
            Some(expected)
        );
    }

    #[test]
    fn bound_for_max_is_power_of_two() {
        let expected = Number::Integer(Integer::from(2i64).pow(970));

        assert_eq!(
            elementary_rounding_error_bound_f64(f64::MAX),
            Some(expected)
        );
    }

    #[test]
    fn bound_for_non_finite_result_is_none() {
        assert_eq!(elementary_rounding_error_bound_f64(f64::INFINITY), None);
        assert_eq!(elementary_rounding_error_bound_f64(f64::NAN), None);
    }

    #[test]
    fn nan_input_returns_quiet_nan_with_payload() {
        let signaling = f64::from_bits(SIGNALING_NAN_BITS);

        assert_eq!(exp_f64(signaling).to_bits(), 0x7ff8_0000_0000_0001);
        assert_eq!(
            sin_f64(f64::from_bits(NEGATIVE_QUIET_NAN_BITS)).to_bits(),
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
                700.0,
                "1.01423205473500450945532959523126761520467957224307334878054E+304",
            ),
            (
                -700.0,
                "9.85967654375977085670537294784946510511560018140094171058647E-305",
            ),
            (
                1e-10,
                "1.00000000010000000000500000364338639858076696442308164431317E+0",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(exp_f64(argument), reference);
        }
    }

    #[test]
    fn exp_special_values() {
        assert_same(exp_f64(0.0), 1.0);
        assert_same(exp_f64(-0.0), 1.0);
        assert_same(exp_f64(f64::INFINITY), f64::INFINITY);
        assert_same(exp_f64(f64::NEG_INFINITY), 0.0);
        assert_same(exp_f64(f64::MAX), f64::INFINITY);
        assert_same(exp_f64(f64::MIN), 0.0);
        assert_same(exp_f64(SMALLEST_SUBNORMAL), 1.0);
        assert_same(exp_f64(f64::MIN_POSITIVE), 1.0);
        assert_same(exp_f64(-745.2), 0.0);
        assert_same(exp_f64(-744.5), SMALLEST_SUBNORMAL);
        assert_nan(exp_f64(f64::NAN));
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
                "-2.30258509299404562850684022342653872716347359387638277683520E+0",
            ),
            (
                1e-310,
                "-7.13801378828154165100644600662388374138233855976586993015134E+2",
            ),
            (
                f64::MAX,
                "7.09782712893383996732223389910657145503973148736664163038603E+2",
            ),
            (
                above_one(),
                "2.22044604925031283432823045461548792598233180790111470255693E-16",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(ln_f64(argument), reference);
        }
    }

    #[test]
    fn ln_special_values() {
        assert_same(ln_f64(0.0), f64::NEG_INFINITY);
        assert_same(ln_f64(-0.0), f64::NEG_INFINITY);
        assert_same(ln_f64(1.0), 0.0);
        assert_same(ln_f64(f64::INFINITY), f64::INFINITY);
        assert_nan(ln_f64(-1.0));
        assert_nan(ln_f64(f64::NEG_INFINITY));
        assert_nan(ln_f64(f64::MIN));
        assert_nan(ln_f64(f64::NAN));
        assert!(ln_f64(below_one()) < 0.0);
        assert_same(ln_f64(SMALLEST_SUBNORMAL), -744.4400719213812);
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
                1e-310,
                "9.99999999999998472466375144883431788541334256353157646847580E-156",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(sqrt_f64(argument), reference);
        }
    }

    #[test]
    fn sqrt_special_values() {
        assert_same(sqrt_f64(0.0), 0.0);
        assert_same(sqrt_f64(-0.0), -0.0);
        assert_same(sqrt_f64(f64::INFINITY), f64::INFINITY);
        assert_same(sqrt_f64(1.0), 1.0);
        assert_nan(sqrt_f64(-SMALLEST_SUBNORMAL));
        assert_nan(sqrt_f64(f64::NEG_INFINITY));
        assert_nan(sqrt_f64(f64::NAN));
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
                1e22,
                "-8.52200849767188801772705893753029368261762150410043656256509E-1",
            ),
            (
                355.0,
                "-3.01443533594884492143302800086500995902558070663246491057898E-5",
            ),
            (
                1e-310,
                "9.99999999999996944932750289769196936057731524703811503348409E-311",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(sin_f64(argument), reference);
        }
    }

    #[test]
    fn sin_special_values() {
        assert_same(sin_f64(0.0), 0.0);
        assert_same(sin_f64(-0.0), -0.0);
        assert_same(sin_f64(SMALLEST_SUBNORMAL), SMALLEST_SUBNORMAL);
        assert_same(sin_f64(-f64::MIN_POSITIVE), -f64::MIN_POSITIVE);
        assert_nan(sin_f64(f64::INFINITY));
        assert_nan(sin_f64(f64::NEG_INFINITY));
        assert_nan(sin_f64(f64::NAN));
        assert_same(sin_f64(f64::MAX), 0.004961954789184062);
    }

    #[test]
    fn cos_of_known_arguments_is_within_bound() {
        let cases = [
            (
                1.0,
                "5.40302305868139717400936607442976603732310420617922227670097E-1",
            ),
            (
                std::f64::consts::FRAC_PI_2,
                "6.12323399573676588613032966137500146464037779883628305209605E-17",
            ),
            (
                1e22,
                "5.23214785395138945497594473384709492140919972439387953527211E-1",
            ),
            (
                1e-8,
                "9.99999999999999949999999999999998324410583653819410728371501E-1",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(cos_f64(argument), reference);
        }
    }

    #[test]
    fn cos_special_values() {
        assert_same(cos_f64(0.0), 1.0);
        assert_same(cos_f64(-0.0), 1.0);
        assert_same(cos_f64(SMALLEST_SUBNORMAL), 1.0);
        assert_nan(cos_f64(f64::INFINITY));
        assert_nan(cos_f64(f64::NAN));
        assert_same(cos_f64(f64::MIN), -0.9999876894265599);
    }

    #[test]
    fn tan_of_known_arguments_is_within_bound() {
        let cases = [
            (
                1.0,
                "1.55740772465490223050697480745836017308725077238152003838395E+0",
            ),
            (
                std::f64::consts::FRAC_PI_2,
                "1.63312393531953697559677370415289165308640681049103028975845E+16",
            ),
            (
                1e-200,
                "9.99999999999999982100262399082759596054411789289747099615054E-201",
            ),
            (
                -2.5,
                "7.47022297238660279355352687825274557904116956883011279066593E-1",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(tan_f64(argument), reference);
        }
    }

    #[test]
    fn tan_special_values() {
        assert_same(tan_f64(0.0), 0.0);
        assert_same(tan_f64(-0.0), -0.0);
        assert_same(tan_f64(SMALLEST_SUBNORMAL), SMALLEST_SUBNORMAL);
        assert_nan(tan_f64(f64::NEG_INFINITY));
        assert_nan(tan_f64(f64::NAN));
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
                "1.57079631189373542538366530377631601659396877728833164082312E+0",
            ),
            (
                -1e-20,
                "-9.99999999999999945153271454209571651729520369454059113771640E-21",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(asin_f64(argument), reference);
        }
    }

    #[test]
    fn asin_special_values() {
        assert_same(asin_f64(0.0), 0.0);
        assert_same(asin_f64(-0.0), -0.0);
        assert_same(asin_f64(1.0), std::f64::consts::FRAC_PI_2);
        assert_same(asin_f64(-1.0), -std::f64::consts::FRAC_PI_2);
        assert_same(asin_f64(SMALLEST_SUBNORMAL), SMALLEST_SUBNORMAL);
        assert_nan(asin_f64(above_one()));
        assert_nan(asin_f64(f64::INFINITY));
        assert_nan(asin_f64(f64::NAN));
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
                "1.49011611938476563878634354255046159223992212696643562220725E-8",
            ),
            (
                -below_one(),
                "3.14159263868863204461498699541606745869255347697588455131059E+0",
            ),
            (
                0.0,
                "1.57079632679489661923132169163975144209858469968755291048747E+0",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(acos_f64(argument), reference);
        }
    }

    #[test]
    fn acos_special_values() {
        assert_same(acos_f64(1.0), 0.0);
        assert_same(acos_f64(-1.0), std::f64::consts::PI);
        assert_same(acos_f64(-0.0), std::f64::consts::FRAC_PI_2);
        assert_same(acos_f64(SMALLEST_SUBNORMAL), std::f64::consts::FRAC_PI_2);
        assert_nan(acos_f64(-above_one()));
        assert_nan(acos_f64(f64::NEG_INFINITY));
        assert_nan(acos_f64(f64::NAN));
    }

    #[test]
    fn atan_of_known_arguments_is_within_bound() {
        let cases = [
            (
                1.0,
                "7.85398163397448309615660845819875721049292349843776455243736E-1",
            ),
            (
                1e300,
                "1.57079632679489661923132169163975144209858469968755291048747E+0",
            ),
            (
                -0.5,
                "-4.63647609000806116214256231461214402028537054286120263810933E-1",
            ),
            (
                1e-310,
                "9.99999999999996944932750289769196936057731524703811503348409E-311",
            ),
        ];
        for (argument, reference) in cases {
            assert_within_bound(atan_f64(argument), reference);
        }
    }

    #[test]
    fn atan_special_values() {
        assert_same(atan_f64(0.0), 0.0);
        assert_same(atan_f64(-0.0), -0.0);
        assert_same(atan_f64(f64::INFINITY), std::f64::consts::FRAC_PI_2);
        assert_same(atan_f64(f64::NEG_INFINITY), -std::f64::consts::FRAC_PI_2);
        assert_same(atan_f64(f64::MAX), std::f64::consts::FRAC_PI_2);
        assert_same(atan_f64(-SMALLEST_SUBNORMAL), -SMALLEST_SUBNORMAL);
        assert_nan(atan_f64(f64::NAN));
    }

    #[test]
    fn atan2_of_known_arguments_is_within_bound() {
        let cases = [
            (
                1.0,
                -1e-300,
                "1.57079632679489661923132169163975144209858469968755291048747E+0",
            ),
            (
                -3.0,
                -4.0,
                "-2.49809154479650885165983415456218024615565880825979343810934E+0",
            ),
            (
                1e-300,
                1e300,
                "9.99999999999999972554331580004340878019918662302726799553352E-601",
            ),
            (
                2.5,
                0.5,
                "1.37340076694501586086127192644496114865099959589970080896978E+0",
            ),
        ];
        for (numerator, denominator, reference) in cases {
            assert_within_bound(atan2_f64(numerator, denominator), reference);
        }
    }

    #[test]
    fn atan2_of_maximum_over_smallest_subnormal_is_half_pi() {
        assert_same(
            atan2_f64(f64::MAX, SMALLEST_SUBNORMAL),
            std::f64::consts::FRAC_PI_2,
        );
    }

    #[test]
    fn atan2_of_subnormal_over_negative_maximum_is_pi() {
        assert_same(
            atan2_f64(SMALLEST_SUBNORMAL, -f64::MAX),
            std::f64::consts::PI,
        );
        assert_same(
            atan2_f64(-SMALLEST_SUBNORMAL, -f64::MAX),
            -std::f64::consts::PI,
        );
    }

    #[test]
    fn atan2_special_values() {
        use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};
        assert_same(atan2_f64(0.0, 0.0), 0.0);
        assert_same(atan2_f64(-0.0, 0.0), -0.0);
        assert_same(atan2_f64(0.0, -0.0), PI);
        assert_same(atan2_f64(-0.0, -1.0), -PI);
        assert_same(atan2_f64(1.0, 0.0), FRAC_PI_2);
        assert_same(atan2_f64(-1.0, -0.0), -FRAC_PI_2);
        assert_same(atan2_f64(f64::INFINITY, f64::INFINITY), FRAC_PI_4);
        assert_same(
            atan2_f64(f64::NEG_INFINITY, f64::NEG_INFINITY),
            -3.0 * FRAC_PI_4,
        );
        assert_same(atan2_f64(f64::INFINITY, 1.0), FRAC_PI_2);
        assert_same(atan2_f64(1.0, f64::NEG_INFINITY), PI);
        assert_same(atan2_f64(-1.0, f64::INFINITY), -0.0);
        assert_nan(atan2_f64(f64::NAN, 1.0));
        assert_nan(atan2_f64(1.0, f64::NAN));
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
                1e15,
                "1.24862707153908615018200935515990592741867663615223487256054E+0",
            ),
            (10.0, -3.0, "1E-3"),
            (
                0.1,
                3.5,
                "3.16227766016837994639475054535543844812088478699832728428646E-4",
            ),
            (
                1e-300,
                1.01,
                "9.99999999999993889990515392027327523666527641167989613394307E-304",
            ),
            (-2.0, -3.0, "-1.25E-1"),
        ];
        for (base, exponent, reference) in cases {
            assert_within_bound(pow_f64(base, exponent), reference);
        }
    }

    #[test]
    fn pow_exact_midpoint_rounds_to_even() {
        assert_same(pow_f64(3.0, 34.0), 16_677_181_699_666_568.0);
        assert_same(pow_f64(25.0, 11.5), 11_920_928_955_078_124.0);
        assert_same(pow_f64(-3.0, 35.0), -50_031_545_098_999_704.0);
    }

    #[test]
    fn pow_exact_power_of_two_below_subnormal_range_rounds_to_zero() {
        assert_same(pow_f64(2.0, -1075.0), 0.0);
        assert_same(pow_f64(0.5, 1074.0), SMALLEST_SUBNORMAL);
        assert_same(pow_f64(-2.0, -1075.0), -0.0);
    }

    #[test]
    fn pow_special_values_with_zero_and_one() {
        assert_same(pow_f64(f64::NAN, 0.0), 1.0);
        assert_same(pow_f64(1.0, f64::NAN), 1.0);
        assert_same(pow_f64(-0.0, 3.0), -0.0);
        assert_same(pow_f64(-0.0, -3.0), f64::NEG_INFINITY);
        assert_same(pow_f64(-0.0, -2.0), f64::INFINITY);
        assert_same(pow_f64(0.0, 0.5), 0.0);
        assert_same(pow_f64(-0.0, f64::NEG_INFINITY), f64::INFINITY);
        assert_nan(pow_f64(f64::NAN, 1.0));
        assert_nan(pow_f64(2.0, f64::NAN));
    }

    #[test]
    fn pow_of_minus_one_to_the_maximum_is_one() {
        assert_same(pow_f64(-1.0, f64::MAX), 1.0);
        assert_same(pow_f64(-1.0, -f64::MAX), 1.0);
    }

    #[test]
    fn pow_special_values_with_infinities() {
        assert_same(pow_f64(-1.0, f64::INFINITY), 1.0);
        assert_same(pow_f64(0.5, f64::NEG_INFINITY), f64::INFINITY);
        assert_same(pow_f64(2.0, f64::NEG_INFINITY), 0.0);
        assert_same(pow_f64(below_one(), f64::INFINITY), 0.0);
        assert_same(pow_f64(f64::NEG_INFINITY, -3.0), -0.0);
        assert_same(pow_f64(f64::NEG_INFINITY, 2.0), f64::INFINITY);
        assert_same(pow_f64(f64::INFINITY, -SMALLEST_SUBNORMAL), 0.0);
    }

    #[test]
    fn pow_of_negative_base_with_fractional_exponent_is_nan() {
        assert_nan(pow_f64(-8.0, 1.0 / 3.0));
    }

    #[test]
    fn pow_overflow_and_underflow_keep_sign() {
        assert_same(pow_f64(f64::MAX, 2.0), f64::INFINITY);
        assert_same(pow_f64(-f64::MAX, 3.0), f64::NEG_INFINITY);
        assert_same(pow_f64(f64::MIN_POSITIVE, 1.5), 0.0);
        assert_same(pow_f64(-SMALLEST_SUBNORMAL, 3.0), -0.0);
    }
}

#[cfg(test)]
mod fast_phase_tests {
    use super::*;

    fn accurate_sine(value: f64) -> f64 {
        value_or_nan(correctly_rounded(
            precision_for_magnitude(magnitude_exponent(value)),
            |precision| Some(series::sine_cosine(value, precision)?.sine),
        ))
    }

    fn accurate_cosine(value: f64) -> f64 {
        value_or_nan(correctly_rounded(BASE_PRECISION, |precision| {
            Some(series::sine_cosine(value, precision)?.cosine)
        }))
    }

    fn arguments(count: usize) -> Vec<f64> {
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            state
        };
        let mut values = Vec::with_capacity(count);
        while values.len() < count {
            let exponent = i64::try_from(next() % 52).unwrap_or(0) - 31;
            let fraction = next() >> 12;
            let bits = (u64::try_from(exponent + 1023).unwrap_or(1023) << 52) | fraction;
            let magnitude = f64::from_bits(bits);
            values.push(if next() % 2 == 0 {
                magnitude
            } else {
                -magnitude
            });
        }
        values
    }

    fn near_quarter_turns() -> Vec<f64> {
        let mut values = Vec::new();
        for turns in (1..20_000_u32).step_by(7) {
            let centre = f64::from(turns) * std::f64::consts::FRAC_PI_2;
            let mut below = centre;
            let mut above = centre;
            values.push(centre);
            for _ in 0..3 {
                below = below.next_down();
                above = above.next_up();
                values.push(below);
                values.push(above);
            }
        }
        values
    }

    const HARD_ROUNDING: [(bool, u64); 4] = [
        (true, 0x3FDF_E767_739D_0F6D),
        (true, 0x3FF9_21FB_5444_2D18),
        (false, 0x3F99_7CCD_3D2C_438F),
        (false, 0x3FF6_B8A6_273D_7C21),
    ];

    #[test]
    fn the_hardest_known_arguments_round_as_the_accurate_path_does() {
        for (is_sine, bits) in HARD_ROUNDING {
            let value = f64::from_bits(bits);
            let (fast, accurate) = if is_sine {
                (crate::fixed_trig::sine(value), accurate_sine(value))
            } else {
                (crate::fixed_trig::cosine(value), accurate_cosine(value))
            };
            assert_eq!(
                fast.map(f64::to_bits),
                Some(accurate.to_bits()),
                "{value:e}"
            );
        }
    }

    #[test]
    fn the_fast_phase_gives_the_accurate_bits_on_generated_arguments() {
        let mut values = arguments(4000);
        values.extend(near_quarter_turns());
        for value in values {
            if let Some(fast) = crate::fixed_trig::sine(value) {
                assert_eq!(
                    fast.to_bits(),
                    accurate_sine(value).to_bits(),
                    "sin {value:e}"
                );
            }
            if let Some(fast) = crate::fixed_trig::cosine(value) {
                assert_eq!(
                    fast.to_bits(),
                    accurate_cosine(value).to_bits(),
                    "cos {value:e}"
                );
            }
        }
    }

    #[test]
    #[ignore = "a long comparison run for the evidence of the method change"]
    fn the_fast_phase_gives_the_accurate_bits_on_a_million_arguments() {
        let mut decided = 0_u64;
        let mut values = arguments(1_000_000);
        values.extend(near_quarter_turns());
        let total = values.len();
        for value in values {
            if let Some(fast) = crate::fixed_trig::sine(value) {
                decided += 1;
                assert_eq!(
                    fast.to_bits(),
                    accurate_sine(value).to_bits(),
                    "sin {value:e}"
                );
            }
            if let Some(fast) = crate::fixed_trig::cosine(value) {
                decided += 1;
                assert_eq!(
                    fast.to_bits(),
                    accurate_cosine(value).to_bits(),
                    "cos {value:e}"
                );
            }
        }
        eprintln!(
            "COMPARED {total} arguments, {decided} of {} fast results decided",
            2 * total
        );
        let sample = arguments(20_000);
        let start = std::time::Instant::now();
        let fast: f64 = sample.iter().map(|value| sin_f64(*value)).sum();
        let fast_time = start.elapsed();
        let start = std::time::Instant::now();
        let accurate: f64 = sample.iter().map(|value| accurate_sine(*value)).sum();
        let accurate_time = start.elapsed();
        eprintln!(
            "TIME sin_f64 {:?} per call, accurate path {:?} per call ({fast} {accurate})",
            fast_time / 20_000,
            accurate_time / 20_000
        );
    }
}
