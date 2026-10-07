use crate::ball::Ball;
use crate::binary_format::BINARY64;
use crate::integer::Integer;
use crate::natural::Natural;
use crate::number::Number;
use crate::word_conversion::usize_from_u64;
use std::sync::OnceLock;

const EXPONENTIAL_SCALING_BITS: i64 = 16;
const FIRST_CACHED_EXPONENTIAL_PRECISION: usize = 128;
const SECOND_CACHED_EXPONENTIAL_PRECISION: usize = 130;
const ARCTANGENT_HALVINGS: u32 = 4;
const ARCTANGENT_HALVING_FACTOR: i64 = 16;
static FIRST_EXPONENTIAL_LN2: OnceLock<Ball> = OnceLock::new();
static SECOND_EXPONENTIAL_LN2: OnceLock<Ball> = OnceLock::new();

fn terms_for_precision(precision: usize, bits_per_term: usize, spare: usize) -> u64 {
    u64::try_from(precision / bits_per_term + spare).unwrap_or(u64::MAX)
}

fn alternating_sum(partial: &Ball, term: &Ball, index: u64) -> Ball {
    if index % 2 == 1 {
        partial.sub(term)
    } else {
        partial.add(term)
    }
}

fn reciprocal(denominator: u64, precision: usize) -> Ball {
    let value = Number::fraction(&Integer::one(), &Integer::from(denominator))
        .unwrap_or(Number::Integer(Integer::zero()));
    Ball::from_exact(&value, precision).unwrap_or_else(|| Ball::one(precision))
}

fn never_cancelled() -> bool {
    false
}

fn arctangent_of_reciprocal(
    denominator: u64,
    precision: usize,
    terms: u64,
    is_cancelled: &dyn Fn() -> bool,
) -> Option<Ball> {
    let square = denominator * denominator;
    let mut power = reciprocal(denominator, precision);
    let mut sum = power.clone();
    for index in 1..=terms {
        if is_cancelled() {
            return None;
        }
        power = power.div_small(square);
        let term = power.div_small(2 * index + 1);
        sum = alternating_sum(&sum, &term, index);
    }
    Some(sum.widened(1))
}

pub(crate) fn pi_until(precision: usize, is_cancelled: &dyn Fn() -> bool) -> Option<Ball> {
    let fifth = arctangent_of_reciprocal(
        5,
        precision,
        terms_for_precision(precision + 6, 4, 1),
        is_cancelled,
    )?;
    let small = arctangent_of_reciprocal(
        239,
        precision,
        terms_for_precision(precision + 6, 14, 1),
        is_cancelled,
    )?;
    Some(
        fifth
            .mul_integer(&Integer::from(16i64))
            .sub(&small.mul_integer(&Integer::from(4i64))),
    )
}

pub(crate) fn pi(precision: usize) -> Ball {
    match pi_until(precision, &never_cancelled) {
        Some(ball) => ball,
        None => unreachable!(),
    }
}

pub(crate) fn ln2_until(precision: usize, is_cancelled: &dyn Fn() -> bool) -> Option<Ball> {
    let terms = terms_for_precision(precision + 2, 3, 1);
    let mut power = reciprocal(3, precision);
    let mut sum = power.clone();
    for index in 1..=terms {
        if is_cancelled() {
            return None;
        }
        power = power.div_small(9);
        sum = sum.add(&power.div_small(2 * index + 1));
    }
    Some(sum.widened(1).mul_integer(&Integer::from(2i64)))
}

pub(crate) fn ln2(precision: usize) -> Ball {
    match ln2_until(precision, &never_cancelled) {
        Some(ball) => ball,
        None => unreachable!(),
    }
}

pub(crate) fn exp(argument: &Ball) -> Option<Ball> {
    match cached_exponential_ln2(argument.precision()) {
        Some(logarithm_of_two) => {
            exp_with_logarithm_of_two(argument, logarithm_of_two, &never_cancelled)
        }
        None => exp_until(argument, &never_cancelled),
    }
}

fn cached_exponential_ln2(precision: usize) -> Option<&'static Ball> {
    match precision {
        FIRST_CACHED_EXPONENTIAL_PRECISION => {
            Some(FIRST_EXPONENTIAL_LN2.get_or_init(|| ln2(FIRST_CACHED_EXPONENTIAL_PRECISION)))
        }
        SECOND_CACHED_EXPONENTIAL_PRECISION => {
            Some(SECOND_EXPONENTIAL_LN2.get_or_init(|| ln2(SECOND_CACHED_EXPONENTIAL_PRECISION)))
        }
        _ => None,
    }
}

pub(crate) fn exp_until(argument: &Ball, is_cancelled: &dyn Fn() -> bool) -> Option<Ball> {
    let precision = argument.precision();
    let logarithm_of_two = ln2_until(precision, is_cancelled)?;
    exp_with_logarithm_of_two(argument, &logarithm_of_two, is_cancelled)
}

fn exp_with_logarithm_of_two(
    argument: &Ball,
    logarithm_of_two: &Ball,
    is_cancelled: &dyn Fn() -> bool,
) -> Option<Ball> {
    let precision = argument.precision();
    let multiple = argument.rounded_quotient_of_centers(logarithm_of_two);
    let reduced = argument.sub(&logarithm_of_two.mul_integer(&multiple));
    let scaled = reduced.mul_power_of_two(-EXPONENTIAL_SCALING_BITS);

    let terms = terms_for_precision(precision + 2, 17, 2);
    let mut term = Ball::one(precision);
    let mut sum = Ball::one(precision);
    for index in 1..=terms {
        if is_cancelled() {
            return None;
        }
        term = term.mul(&scaled).div_small(index);
        sum = sum.add(&term);
    }
    let mut result = sum.widened(1);
    for _ in 0..EXPONENTIAL_SCALING_BITS {
        result = result.mul(&result);
    }
    Some(result.mul_power_of_two(multiple.to_i64()?))
}

pub(crate) fn ln(value: f64, precision: usize) -> Option<Ball> {
    let parts = BINARY64.finite_parts(value.to_bits())?;
    if parts.is_negative || parts.significand == 0 {
        return None;
    }
    let significand = Natural::from_u64(parts.significand);
    let bits = significand.bit_length();
    let mut half_scale = Natural::one().shifted_left(bits - 1);
    let mut exponent = parts.exponent + i64::try_from(bits).ok()? - 1;
    let is_above_root_two =
        significand.mul(&significand) > Natural::one().shifted_left(2 * bits - 1);
    if is_above_root_two {
        half_scale = half_scale.shifted_left(1);
        exponent += 1;
    }
    let significand_integer = Integer::from_sign_and_magnitude(false, significand);
    let half_scale_integer = Integer::from_sign_and_magnitude(false, half_scale);
    let ratio = Number::fraction(
        &(&significand_integer - &half_scale_integer),
        &(&significand_integer + &half_scale_integer),
    )
    .ok()?;

    let hyperbolic_argument = Ball::from_exact(&ratio, precision)?;
    let square = hyperbolic_argument.mul(&hyperbolic_argument);
    let terms = terms_for_precision(precision, 4, 2);
    let mut power = hyperbolic_argument.clone();
    let mut sum = hyperbolic_argument;
    for index in 1..=terms {
        power = power.mul(&square);
        sum = sum.add(&power.div_small(2 * index + 1));
    }
    let mantissa_logarithm = sum.widened(1).mul_integer(&Integer::from(2i64));
    Some(mantissa_logarithm.add(&ln2(precision).mul_integer(&Integer::from(exponent))))
}

const BALL_ARGUMENT_WIDTH_BITS: usize = 4;

const MINIMUM_HALVINGS: usize = 8;
const HALVING_GUARD_BITS: usize = 16;
const PI_INTEGER_BITS: usize = 2;

fn halvings_for_precision(precision: usize) -> usize {
    (precision.isqrt() / 2).max(MINIMUM_HALVINGS)
}

pub(crate) fn ln_of_ball(argument: &Ball, is_cancelled: &dyn Fn() -> bool) -> Option<Ball> {
    if !argument.is_surely_positive() {
        return None;
    }
    let precision = argument.precision();
    let mut exponent =
        i64::try_from(argument.center_bit_length()).ok()? - 1 - i64::try_from(precision).ok()?;
    let mut normalized = argument.mul_power_of_two(-exponent);
    if normalized.center_square_exceeds_two() {
        normalized = normalized.mul_power_of_two(-1);
        exponent += 1;
    }
    if !normalized.has_radius_below_power_of_two(BALL_ARGUMENT_WIDTH_BITS) {
        return None;
    }
    let one = Ball::one(precision);
    let hyperbolic_argument = normalized.sub(&one).div(&normalized.add(&one))?;
    let square = hyperbolic_argument.mul(&hyperbolic_argument);
    let terms = terms_for_precision(precision, 4, 2);
    let mut power = hyperbolic_argument.clone();
    let mut sum = hyperbolic_argument;
    for index in 1..=terms {
        if is_cancelled() {
            return None;
        }
        power = power.mul(&square);
        sum = sum.add(&power.div_small(2 * index + 1));
    }
    let mantissa_logarithm = sum.widened(1).mul_integer(&Integer::from(2i64));
    let logarithm_of_two = ln2_until(precision, is_cancelled)?;
    Some(mantissa_logarithm.add(&logarithm_of_two.mul_integer(&Integer::from(exponent))))
}

fn small_argument_terms(halvings: usize, precision: usize) -> u64 {
    let mut bits = 0_usize;
    let mut degree = 0_u64;
    while bits <= precision + 2 {
        degree += 1;
        let degree_bits = usize::try_from(u64::BITS - degree.leading_zeros()).unwrap_or(0);
        bits += halvings + degree_bits - 1;
    }
    degree
}

pub(crate) fn sine_cosine_of_ball(
    argument: &Ball,
    budget_bits: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Option<SineCosine> {
    if !argument.has_radius_below_power_of_two(BALL_ARGUMENT_WIDTH_BITS) {
        return None;
    }
    let precision = argument.precision();
    let halvings = halvings_for_precision(precision);
    let integer_bits = argument
        .center_bit_length()
        .saturating_sub(precision)
        .max(PI_INTEGER_BITS);
    let working = (precision + 2 * halvings + HALVING_GUARD_BITS)
        .min(budget_bits.saturating_sub(integer_bits))
        .max(precision);
    let widened_argument = argument.with_precision(working);
    let half_pi = pi_until(working, is_cancelled)?.mul_power_of_two(-1);
    let multiple = widened_argument.rounded_quotient_of_centers(&half_pi);
    let reduced = widened_argument.sub(&half_pi.mul_integer(&multiple));
    let (_, quadrant) = multiple.div_rem_euclid(&Integer::from(4i64)).ok()?;
    if !reduced.is_surely_below(&Integer::one()) || !reduced.is_surely_above(&Integer::from(-1i64))
    {
        return None;
    }
    let small = reduced.mul_power_of_two(-i64::try_from(halvings).ok()?);
    let square = small.mul(&small);
    let last_power = small_argument_terms(halvings, working);
    let mut sine_term = small.clone();
    let mut sine = small;
    let mut cosine_term = Ball::one(working);
    let mut cosine = Ball::one(working);
    let mut index = 1u64;
    while 2 * index <= last_power + 1 {
        if is_cancelled() {
            return None;
        }
        cosine_term = cosine_term
            .mul(&square)
            .div_small((2 * index - 1) * (2 * index));
        cosine = alternating_sum(&cosine, &cosine_term, index);
        sine_term = sine_term
            .mul(&square)
            .div_small((2 * index) * (2 * index + 1));
        sine = alternating_sum(&sine, &sine_term, index);
        index += 1;
    }
    let mut sine = sine.widened(1);
    let mut cosine = cosine.widened(1);
    let one = Ball::one(working);
    for _ in 0..halvings {
        if is_cancelled() {
            return None;
        }
        let doubled_sine = sine.mul(&cosine).mul_integer(&Integer::from(2i64));
        cosine = one.sub(&sine.mul(&sine).mul_integer(&Integer::from(2i64)));
        sine = doubled_sine;
    }
    let sine = sine.reduced_to_precision(precision);
    let cosine = cosine.reduced_to_precision(precision);
    Some(match quadrant.to_i64()? {
        0 => SineCosine { sine, cosine },
        1 => SineCosine {
            sine: cosine,
            cosine: sine.negated(),
        },
        2 => SineCosine {
            sine: sine.negated(),
            cosine: cosine.negated(),
        },
        _ => SineCosine {
            sine: cosine.negated(),
            cosine: sine,
        },
    })
}

fn factorial_terms(precision: usize) -> u64 {
    let mut factorial = Natural::one();
    let mut index = 0u64;
    while factorial.bit_length() <= precision + 2 {
        index += 1;
        factorial = factorial.mul(&Natural::from_u64(index));
    }
    index + 1
}

pub(crate) struct SineCosine {
    pub(crate) sine: Ball,
    pub(crate) cosine: Ball,
}

pub(crate) fn sine_cosine(value: f64, base_precision: usize) -> Option<SineCosine> {
    let integer_bits = BINARY64.finite_parts(value.to_bits()).map_or(0, |parts| {
        usize_from_u64(parts.exponent.max(0).unsigned_abs()) + 53
    });
    let precision = base_precision + integer_bits;
    let argument = Ball::from_f64(value, precision)?;
    let (reduced, quadrant) = if value.abs() <= 0.75 {
        (argument, 0)
    } else {
        let half_pi = pi(precision).mul_power_of_two(-1);
        let multiple = argument.rounded_quotient_of_centers(&half_pi);
        let reduced = argument.sub(&half_pi.mul_integer(&multiple));
        let (_, quadrant) = multiple.div_rem_euclid(&Integer::from(4i64)).ok()?;
        (reduced, quadrant.to_i64()?)
    };

    let square = reduced.mul(&reduced);
    let last_power = factorial_terms(precision);
    let mut sine_term = reduced.clone();
    let mut sine = reduced;
    let mut cosine_term = Ball::one(precision);
    let mut cosine = Ball::one(precision);
    let mut index = 1u64;
    while 2 * index <= last_power + 1 {
        cosine_term = cosine_term
            .mul(&square)
            .div_small((2 * index - 1) * (2 * index));
        cosine = alternating_sum(&cosine, &cosine_term, index);
        sine_term = sine_term
            .mul(&square)
            .div_small((2 * index) * (2 * index + 1));
        sine = alternating_sum(&sine, &sine_term, index);
        index += 1;
    }
    let sine = sine.widened(1);
    let cosine = cosine.widened(1);

    Some(match quadrant {
        0 => SineCosine { sine, cosine },
        1 => SineCosine {
            sine: cosine,
            cosine: sine.negated(),
        },
        2 => SineCosine {
            sine: sine.negated(),
            cosine: cosine.negated(),
        },
        _ => SineCosine {
            sine: cosine.negated(),
            cosine: sine,
        },
    })
}

pub(crate) fn arctangent(argument: &Ball) -> Option<Ball> {
    arctangent_until(argument, &never_cancelled)
}

pub(crate) fn arctangent_until(argument: &Ball, is_cancelled: &dyn Fn() -> bool) -> Option<Ball> {
    let precision = argument.precision();
    let one = Ball::one(precision);
    let mut halved = argument.clone();
    for _ in 0..ARCTANGENT_HALVINGS {
        let root = one.add(&halved.mul(&halved)).sqrt()?;
        halved = halved.div(&one.add(&root))?;
    }
    let square = halved.mul(&halved);
    let terms = terms_for_precision(precision, 6, 2);
    let mut power = halved.clone();
    let mut sum = halved;
    for index in 1..=terms {
        if is_cancelled() {
            return None;
        }
        power = power.mul(&square);
        let term = power.div_small(2 * index + 1);
        sum = alternating_sum(&sum, &term, index);
    }
    Some(
        sum.widened(1)
            .mul_integer(&Integer::from(ARCTANGENT_HALVING_FACTOR)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{decimal_reference, is_at_most};
    use std::hint::black_box;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    const PRECISION: usize = 256;

    fn assert_overlaps_reference(ball: &Ball, reference: &str) {
        let reference = decimal_reference(reference);
        let low = reference
            .value
            .sub_exact(&reference.last_digit_unit)
            .unwrap();
        let high = reference
            .value
            .add_exact(&reference.last_digit_unit)
            .unwrap();
        let (ball_low, ball_high) = ball.bounds();
        assert!(is_at_most(&low, &ball_high) && is_at_most(&ball_low, &high));
    }

    #[test]
    fn pi_ball_overlaps_reference_digits() {
        assert_overlaps_reference(
            &pi(PRECISION),
            "3.14159265358979323846264338327950288419716939937510582097494459E+0",
        );
    }

    #[test]
    fn ln2_ball_overlaps_reference_digits() {
        assert_overlaps_reference(
            &ln2(PRECISION),
            "6.93147180559945309417232121458176568075500134360255254120680E-1",
        );
    }

    #[test]
    fn exponential_ball_contains_e() {
        let ball = exp(&Ball::one(PRECISION)).unwrap();

        assert_overlaps_reference(
            &ball,
            "2.71828182845904523536028747135266249775724709369995957496697E+0",
        );
    }

    #[test]
    fn exponential_ball_of_negative_argument_contains_reference() {
        let ball = exp(&Ball::from_f64(-700.0, PRECISION + 1100).unwrap()).unwrap();

        assert_overlaps_reference(
            &ball,
            "9.85967654375977085670537294784946510511560018140094171058647E-305",
        );
    }

    #[test]
    fn logarithm_ball_contains_ln_ten() {
        assert_overlaps_reference(
            &ln(10.0, PRECISION).unwrap(),
            "2.30258509299404568401799145468436420760110148862877297603333E+0",
        );
    }

    #[test]
    fn sine_and_cosine_balls_contain_references_after_reduction() {
        let pair = sine_cosine(1e22, PRECISION).unwrap();

        assert_overlaps_reference(
            &pair.sine,
            "-8.52200849767188801772705893753029368261762150410043656256509E-1",
        );
        assert_overlaps_reference(
            &pair.cosine,
            "5.23214785395138945497594473384709492140919972439387953527211E-1",
        );
    }

    #[test]
    fn arctangent_ball_contains_quarter_pi() {
        assert_overlaps_reference(
            &arctangent(&Ball::one(PRECISION)).unwrap(),
            "7.85398163397448309615660845819875721049292349843776455243736E-1",
        );
    }

    #[test]
    fn pi_ball_rounds_to_nearest_f64_of_pi() {
        assert_eq!(
            pi(PRECISION).rounded_f64().map(f64::to_bits),
            Some(std::f64::consts::PI.to_bits())
        );
    }

    #[test]
    fn cached_ln2_balls_match_the_original_enclosures_exactly() {
        for precision in [
            FIRST_CACHED_EXPONENTIAL_PRECISION,
            SECOND_CACHED_EXPONENTIAL_PRECISION,
        ] {
            let cached = cached_exponential_ln2(precision).unwrap();
            let original = ln2_until(precision, &never_cancelled).unwrap();
            assert_eq!(cached.precision(), original.precision());
            assert_eq!(cached.bounds(), original.bounds());
        }
        assert!(cached_exponential_ln2(129).is_none());
    }

    #[test]
    fn cached_exponential_balls_match_the_original_enclosures_exactly() {
        for precision in [128, 129, 130, 256] {
            for value in [-700.0, -1.6322261495516757, -0.010195950715390869, 0.1, 1.0] {
                let argument = Ball::from_f64(value, precision).unwrap();
                let cached = exp(&argument).unwrap();
                let original = exp_until(&argument, &never_cancelled).unwrap();
                assert_eq!(cached.precision(), original.precision());
                assert_eq!(cached.bounds(), original.bounds());
            }
        }
    }

    #[test]
    fn cancellable_exponential_still_polls_during_ln2_after_cache_is_warm() {
        let argument = Ball::from_f64(-1.0, FIRST_CACHED_EXPONENTIAL_PRECISION).unwrap();
        exp(&argument).unwrap();
        let polls = AtomicUsize::new(0);
        let cancel = || polls.fetch_add(1, Ordering::SeqCst) + 1 >= 5;
        assert!(exp_until(&argument, &cancel).is_none());
        assert_eq!(polls.load(Ordering::SeqCst), 5);
    }

    #[test]
    #[ignore]
    fn profile_ln2_and_exponential_candidate() {
        for (precision, repeats) in [(128, 1024), (130, 1024), (256, 512), (512, 128), (1618, 16)] {
            let started = Instant::now();
            black_box(ln2_until(black_box(precision), &never_cancelled).unwrap());
            let cold = started.elapsed();
            let started = Instant::now();
            for _ in 0..repeats {
                black_box(ln2_until(black_box(precision), &never_cancelled).unwrap());
            }
            let warm = started.elapsed();
            eprintln!(
                "ln2_precision_bits={precision} cold_ns={} warm_mean_ns={} repeats={repeats}",
                cold.as_nanos(),
                warm.as_nanos() / repeats
            );
        }
        for (argument, repeats) in [(-0.010195950715390869, 1024), (-1.6322261495516757, 1024)] {
            let started = Instant::now();
            black_box(crate::elementary::checked_exp_f64(black_box(argument)).unwrap());
            let cold = started.elapsed();
            let started = Instant::now();
            for _ in 0..repeats {
                black_box(crate::elementary::checked_exp_f64(black_box(argument)).unwrap());
            }
            let warm = started.elapsed();
            eprintln!(
                "exp_argument={argument} argument_bits={:016x} cold_ns={} warm_mean_ns={} repeats={repeats}",
                argument.to_bits(),
                cold.as_nanos(),
                warm.as_nanos() / repeats
            );
        }
    }
}
