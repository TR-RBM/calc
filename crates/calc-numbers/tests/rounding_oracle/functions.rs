use crate::arithmetic::Arithmetic;
use crate::bits::mantissa_and_exponent;

const EXPONENTIAL_HALVINGS: i32 = 4;
const ARCTANGENT_HALVINGS: u32 = 4;
const SERIES_TERM_LIMIT: i64 = 50_000;
const DOUBLED_REMAINDER_EXPONENT: i32 = 1;
const LARGEST_SERIES_MAGNITUDE_EXPONENT: i32 = 0;

fn remainder_target<A: Arithmetic>(arithmetic: &A, argument: &A::Value) -> i32 {
    arithmetic.truncation_exponent() + arithmetic.magnitude_exponent(argument).min(0)
}

pub fn exponential_series<A: Arithmetic>(arithmetic: &A, argument: &A::Value) -> Option<A::Value> {
    let target = arithmetic.truncation_exponent() - DOUBLED_REMAINDER_EXPONENT;
    let mut sum = arithmetic.small_integer(1);
    let mut term = arithmetic.small_integer(1);
    for index in 1..=SERIES_TERM_LIMIT {
        term = arithmetic.div(
            &arithmetic.mul(&term, argument),
            &arithmetic.small_integer(index),
        )?;
        if arithmetic.magnitude_exponent(&term) < target {
            break;
        }
        sum = arithmetic.add(&sum, &term);
    }
    let remainder = arithmetic.magnitude_exponent(&term) + DOUBLED_REMAINDER_EXPONENT;
    Some(arithmetic.widen(&sum, remainder))
}

pub fn sine_series<A: Arithmetic>(arithmetic: &A, argument: &A::Value) -> Option<A::Value> {
    let target = remainder_target(arithmetic, argument);
    let square = arithmetic.mul(argument, argument);
    let mut sum = argument.clone();
    let mut term = argument.clone();
    for index in 1..=SERIES_TERM_LIMIT {
        let divisor = arithmetic.small_integer((2 * index) * (2 * index + 1));
        term = arithmetic.div(&arithmetic.mul(&term, &square), &divisor)?;
        if arithmetic.magnitude_exponent(&term) < target {
            break;
        }
        sum = if index % 2 == 1 {
            arithmetic.sub(&sum, &term)
        } else {
            arithmetic.add(&sum, &term)
        };
    }
    Some(arithmetic.widen(&sum, arithmetic.magnitude_exponent(&term)))
}

pub fn cosine_series<A: Arithmetic>(arithmetic: &A, argument: &A::Value) -> Option<A::Value> {
    let target = arithmetic.truncation_exponent();
    let square = arithmetic.mul(argument, argument);
    let mut sum = arithmetic.small_integer(1);
    let mut term = arithmetic.small_integer(1);
    for index in 1..=SERIES_TERM_LIMIT {
        let divisor = arithmetic.small_integer((2 * index - 1) * (2 * index));
        term = arithmetic.div(&arithmetic.mul(&term, &square), &divisor)?;
        if arithmetic.magnitude_exponent(&term) < target {
            break;
        }
        sum = if index % 2 == 1 {
            arithmetic.sub(&sum, &term)
        } else {
            arithmetic.add(&sum, &term)
        };
    }
    Some(arithmetic.widen(&sum, arithmetic.magnitude_exponent(&term)))
}

pub fn arctangent_series<A: Arithmetic>(arithmetic: &A, argument: &A::Value) -> A::Value {
    alternating_odd_power_series(arithmetic, argument, true)
}

pub fn inverse_hyperbolic_tangent_series<A: Arithmetic>(
    arithmetic: &A,
    argument: &A::Value,
) -> A::Value {
    alternating_odd_power_series(arithmetic, argument, false)
}

fn alternating_odd_power_series<A: Arithmetic>(
    arithmetic: &A,
    argument: &A::Value,
    is_alternating: bool,
) -> A::Value {
    let target = remainder_target(arithmetic, argument);
    let square = arithmetic.mul(argument, argument);
    let mut sum = argument.clone();
    let mut power = argument.clone();
    let mut term = argument.clone();
    for index in 1..=SERIES_TERM_LIMIT {
        power = arithmetic.mul(&power, &square);
        match arithmetic.div(&power, &arithmetic.small_integer(2 * index + 1)) {
            Some(next) => term = next,
            None => break,
        }
        if arithmetic.magnitude_exponent(&term) < target {
            break;
        }
        sum = if is_alternating && index % 2 == 1 {
            arithmetic.sub(&sum, &term)
        } else {
            arithmetic.add(&sum, &term)
        };
    }
    let remainder = if is_alternating {
        arithmetic.magnitude_exponent(&term)
    } else {
        arithmetic.magnitude_exponent(&term) + DOUBLED_REMAINDER_EXPONENT
    };
    arithmetic.widen(&sum, remainder)
}

pub fn exponential<A: Arithmetic>(arithmetic: &A, argument: &A::Value) -> Option<A::Value> {
    let ln2 = arithmetic.ln2();
    let count = arithmetic.nearest_small_integer(&arithmetic.div(argument, &ln2)?)?;
    let multiple = arithmetic.mul(&arithmetic.small_integer(count), &ln2);
    let reduced = arithmetic.sub(argument, &multiple);
    let scaled = arithmetic.times_power_of_two(&reduced, -EXPONENTIAL_HALVINGS);
    if arithmetic.magnitude_exponent(&scaled) > LARGEST_SERIES_MAGNITUDE_EXPONENT - 1 {
        return None;
    }
    let mut value = exponential_series(arithmetic, &scaled)?;
    for _ in 0..EXPONENTIAL_HALVINGS {
        value = arithmetic.mul(&value, &value);
    }
    Some(arithmetic.times_power_of_two(&value, i32::try_from(count).ok()?))
}

pub fn logarithm<A: Arithmetic>(arithmetic: &A, argument: f64) -> Option<A::Value> {
    if argument.is_nan() || argument <= 0.0 {
        return None;
    }
    let (mantissa, exponent) = mantissa_and_exponent(argument)?;
    let one = arithmetic.small_integer(1);
    let mantissa = arithmetic.exact(mantissa);
    let ratio = arithmetic.div(
        &arithmetic.sub(&mantissa, &one),
        &arithmetic.add(&mantissa, &one),
    )?;
    let half_logarithm = inverse_hyperbolic_tangent_series(arithmetic, &ratio);
    let scaled_ln2 = arithmetic.mul(
        &arithmetic.small_integer(i64::from(exponent)),
        &arithmetic.ln2(),
    );
    Some(arithmetic.add(
        &scaled_ln2,
        &arithmetic.times_power_of_two(&half_logarithm, 1),
    ))
}

pub fn square_root<A: Arithmetic>(arithmetic: &A, argument: f64) -> Option<A::Value> {
    arithmetic.square_root(&arithmetic.exact(argument))
}

pub fn sine_and_cosine<A: Arithmetic>(
    arithmetic: &A,
    argument: f64,
) -> Option<(A::Value, A::Value)> {
    let (reduced, quadrant) = arithmetic.quarter_turns(argument)?;
    let sine = sine_series(arithmetic, &reduced)?;
    let cosine = cosine_series(arithmetic, &reduced)?;
    Some(match quadrant {
        0 => (sine, cosine),
        1 => (cosine, arithmetic.negate(&sine)),
        2 => (arithmetic.negate(&sine), arithmetic.negate(&cosine)),
        _ => (arithmetic.negate(&cosine), sine),
    })
}

pub fn arctangent<A: Arithmetic>(arithmetic: &A, argument: &A::Value) -> Option<A::Value> {
    let one = arithmetic.small_integer(1);
    let mut halved = argument.clone();
    for _ in 0..ARCTANGENT_HALVINGS {
        let hypotenuse =
            arithmetic.square_root(&arithmetic.add(&one, &arithmetic.mul(&halved, &halved)))?;
        halved = arithmetic.div(&halved, &arithmetic.add(&one, &hypotenuse))?;
    }
    if arithmetic.magnitude_exponent(&halved) > LARGEST_SERIES_MAGNITUDE_EXPONENT {
        return None;
    }
    let series = arctangent_series(arithmetic, &halved);
    Some(arithmetic.times_power_of_two(&series, i32::try_from(ARCTANGENT_HALVINGS).ok()?))
}

pub fn arcsine<A: Arithmetic>(arithmetic: &A, argument: f64) -> Option<A::Value> {
    let one = arithmetic.small_integer(1);
    let value = arithmetic.exact(argument);
    let complement = arithmetic.mul(&arithmetic.sub(&one, &value), &arithmetic.add(&one, &value));
    let ratio = arithmetic.div(&value, &arithmetic.square_root(&complement)?)?;
    arctangent(arithmetic, &ratio)
}

pub fn arccosine<A: Arithmetic>(arithmetic: &A, argument: f64) -> Option<A::Value> {
    let one = arithmetic.small_integer(1);
    let value = arithmetic.exact(argument);
    let ratio = arithmetic.div(&arithmetic.sub(&one, &value), &arithmetic.add(&one, &value))?;
    let half_angle = arctangent(arithmetic, &arithmetic.square_root(&ratio)?)?;
    Some(arithmetic.times_power_of_two(&half_angle, 1))
}

pub fn half_pi<A: Arithmetic>(arithmetic: &A) -> A::Value {
    arithmetic.times_power_of_two(&arithmetic.pi(), -1)
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use calc_numbers::{Integer, Number};

    use super::*;
    use crate::arithmetic::Point;
    use crate::dyadic::DyadicArithmetic;
    use crate::machine::MachineArithmetic;

    const E_DIGITS: &str = "27182818284590452353602874713526624977572470936999";
    const SQRT2_DIGITS: &str = "14142135623730950488016887242096980785696718753769";
    const PI_DIGITS: &str = "31415926535897932384626433832795028841971693993751";

    fn decimal_point(digits: &str, offset: i64) -> Point {
        let numerator = digits.bytes().fold(Integer::zero(), |accumulated, digit| {
            &(&accumulated * &Integer::from(10i64)) + &Integer::from(i64::from(digit - b'0'))
        });
        let exponent = u32::try_from(digits.len() - 1).expect("short digits");
        let shifted = &numerator + &Integer::from(offset);
        Point::exact(
            Number::fraction(&shifted, &Integer::from(10i64).pow(exponent)).expect("fraction"),
        )
    }

    fn encloses_digits(
        arithmetic: &DyadicArithmetic,
        value: &crate::dyadic::DyadicInterval,
        digits: &str,
    ) -> bool {
        arithmetic.compare_lower(value, &decimal_point(digits, -1)) == Some(Ordering::Greater)
            && arithmetic.compare_upper(value, &decimal_point(digits, 1)) == Some(Ordering::Less)
    }

    fn encloses_machine(
        arithmetic: &DyadicArithmetic,
        value: &crate::dyadic::DyadicInterval,
        point: f64,
    ) -> bool {
        arithmetic.compare_lower(value, &Point::machine(point)) != Some(Ordering::Greater)
            && arithmetic.compare_upper(value, &Point::machine(point)) != Some(Ordering::Less)
    }

    #[test]
    fn exponential_of_one_encloses_fifty_digits_of_e() {
        let arithmetic = DyadicArithmetic::new(200);

        let value = exponential(&arithmetic, &arithmetic.small_integer(1)).expect("enclosure");

        assert!(encloses_digits(&arithmetic, &value, E_DIGITS));
    }

    #[test]
    fn logarithm_of_exponential_of_one_encloses_one() {
        let arithmetic = DyadicArithmetic::new(200);
        let e = exponential(&arithmetic, &arithmetic.small_integer(1)).expect("enclosure");
        let (lower, _) = arithmetic.bounds(&e);

        let value = logarithm(&arithmetic, lower.round_to_f64_ties_even()).expect("enclosure");
        let widened = arithmetic.widen(&value, -50);

        assert!(encloses_machine(&arithmetic, &widened, 1.0));
    }

    #[test]
    fn square_root_of_two_encloses_fifty_digits() {
        let arithmetic = DyadicArithmetic::new(200);

        let value = square_root(&arithmetic, 2.0).expect("enclosure");

        assert!(encloses_digits(&arithmetic, &value, SQRT2_DIGITS));
    }

    #[test]
    fn arctangent_of_one_is_a_quarter_of_pi() {
        let arithmetic = DyadicArithmetic::new(200);

        let value = arctangent(&arithmetic, &arithmetic.small_integer(1)).expect("enclosure");
        let whole = arithmetic.times_power_of_two(&value, 2);

        assert!(encloses_digits(&arithmetic, &whole, PI_DIGITS));
    }

    #[test]
    fn arctangent_of_huge_argument_approaches_half_pi() {
        let arithmetic = DyadicArithmetic::new(200);

        let value = arctangent(&arithmetic, &arithmetic.exact(1.0e300)).expect("enclosure");
        let doubled = arithmetic.times_power_of_two(&value, 1);

        assert!(encloses_digits(&arithmetic, &doubled, PI_DIGITS));
    }

    #[test]
    fn sine_squared_plus_cosine_squared_encloses_one() {
        let arithmetic = DyadicArithmetic::new(200);

        let (sine, cosine) = sine_and_cosine(&arithmetic, 1.0e22).expect("enclosure");
        let sum = arithmetic.add(
            &arithmetic.mul(&sine, &sine),
            &arithmetic.mul(&cosine, &cosine),
        );

        assert!(
            encloses_machine(&arithmetic, &sum, 1.0)
                && arithmetic.magnitude_exponent(&arithmetic.sub(&sum, &sum)) < -100
        );
    }

    #[test]
    fn sine_of_pi_approximation_is_its_distance_to_pi() {
        let arithmetic = DyadicArithmetic::new(200);
        let pi_approximation = std::f64::consts::PI;
        let distance = arithmetic.sub(&arithmetic.pi(), &arithmetic.exact(pi_approximation));

        let (sine, _) = sine_and_cosine(&arithmetic, pi_approximation).expect("enclosure");
        let difference = arithmetic.sub(&sine, &distance);

        assert!(arithmetic.magnitude_exponent(&difference) < -150);
    }

    #[test]
    fn arcsine_of_sine_returns_the_angle() {
        let arithmetic = DyadicArithmetic::new(200);
        let (sine, _) = sine_and_cosine(&arithmetic, 0.5).expect("enclosure");
        let (lower, _) = arithmetic.bounds(&sine);

        let angle = arcsine(&arithmetic, lower.round_to_f64_ties_even()).expect("enclosure");
        let widened = arithmetic.widen(&angle, -50);

        assert!(encloses_machine(&arithmetic, &widened, 0.5));
    }

    #[test]
    fn arccosine_of_minus_one_half_is_two_thirds_of_pi() {
        let arithmetic = DyadicArithmetic::new(200);

        let angle = arccosine(&arithmetic, -0.5).expect("enclosure");
        let whole = arithmetic
            .div(
                &arithmetic.times_power_of_two(&angle, 0),
                &arithmetic.small_integer(2),
            )
            .expect("division");
        let tripled = arithmetic.mul(&whole, &arithmetic.small_integer(3));

        assert!(encloses_digits(&arithmetic, &tripled, PI_DIGITS));
    }

    #[test]
    fn machine_exponential_of_one_contains_standard_e() {
        let arithmetic = MachineArithmetic::new();

        let value = exponential(&arithmetic, &arithmetic.small_integer(1)).expect("enclosure");

        assert!(
            value.lower <= std::f64::consts::E
                && std::f64::consts::E <= value.upper
                && value.upper - value.lower < 1.0e-11
        );
    }

    #[test]
    fn machine_logarithm_near_one_keeps_relative_precision() {
        let arithmetic = MachineArithmetic::new();
        let argument = 1.0 + f64::EPSILON;

        let value = logarithm(&arithmetic, argument).expect("enclosure");

        assert!(value.lower > 0.0 && (value.upper - value.lower) < f64::EPSILON * 1.0e-6);
    }

    #[test]
    fn machine_sine_of_largest_f32_is_narrow() {
        let arithmetic = MachineArithmetic::new();

        let (sine, _) = sine_and_cosine(&arithmetic, f64::from(f32::MAX)).expect("enclosure");

        assert!(sine.upper - sine.lower < 1.0e-12);
    }
}
