use std::cmp::Ordering;

use calc_numbers::{Integer, Number};

use crate::arithmetic::{Arithmetic, Point};
use crate::bits::{is_integer, is_odd_integer, odd_significand_and_exponent};
use crate::functions::{
    arccosine, arcsine, arctangent, exponential, half_pi, logarithm, sine_and_cosine, square_root,
};

const EXPONENTIAL_OVERFLOW_ARGUMENT: f64 = 710.0;
const EXPONENTIAL_UNDERFLOW_ARGUMENT: f64 = -746.0;
const UNDERFLOW_RADIUS_EXPONENT: i32 = -1076;
const LARGEST_EXACT_POWER: i128 = 64;
const LARGEST_EXACT_POWER_SHIFT: i32 = 6;
const LARGEST_POWER_OF_TWO_SHIFT: i32 = 62;
const LARGEST_POWER_OF_TWO_ROOT_DEPTH: i32 = 11;
const LARGEST_ROOT_DEPTH: i32 = 5;
const POWER_OF_TWO_RANGE: i128 = 1100;
const LARGEST_ROOT_CANDIDATE_ERROR: u64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ElementaryFunction {
    Exponential,
    Logarithm,
    SquareRoot,
    Sine,
    Cosine,
    Tangent,
    Arcsine,
    Arccosine,
    Arctangent,
    Power,
    Arctangent2,
}

pub const UNIVARIATE_FUNCTIONS: [ElementaryFunction; 9] = [
    ElementaryFunction::Exponential,
    ElementaryFunction::Logarithm,
    ElementaryFunction::SquareRoot,
    ElementaryFunction::Sine,
    ElementaryFunction::Cosine,
    ElementaryFunction::Tangent,
    ElementaryFunction::Arcsine,
    ElementaryFunction::Arccosine,
    ElementaryFunction::Arctangent,
];

#[derive(Clone, Debug)]
pub enum ExactValue {
    Finite(Number),
    PositiveInfinity,
    NegativeInfinity,
}

#[derive(Clone, Debug)]
pub enum Knowledge<V> {
    NotANumber,
    Exact(ExactValue),
    Overflow { is_negative: bool },
    Enclosed(V),
    Unknown,
}

fn enclosed<V>(value: Option<V>) -> Knowledge<V> {
    value.map_or(Knowledge::Unknown, Knowledge::Enclosed)
}

fn exact_integer<V>(value: i64) -> Knowledge<V> {
    Knowledge::Exact(ExactValue::Finite(Number::from(value)))
}

fn signed<A: Arithmetic>(arithmetic: &A, value: A::Value, is_negative: bool) -> A::Value {
    if is_negative {
        arithmetic.negate(&value)
    } else {
        value
    }
}

fn underflow_enclosure<A: Arithmetic>(arithmetic: &A) -> A::Value {
    arithmetic.widen(&arithmetic.small_integer(0), UNDERFLOW_RADIUS_EXPONENT)
}

pub fn knowledge<A: Arithmetic>(
    arithmetic: &A,
    function: ElementaryFunction,
    arguments: &[f64],
) -> Knowledge<A::Value> {
    let first = arguments.first().copied().unwrap_or(f64::NAN);
    let second = arguments.get(1).copied().unwrap_or(f64::NAN);
    match function {
        ElementaryFunction::Exponential => exponential_knowledge(arithmetic, first),
        ElementaryFunction::Logarithm => logarithm_knowledge(arithmetic, first),
        ElementaryFunction::SquareRoot => square_root_knowledge(arithmetic, first),
        ElementaryFunction::Sine => trigonometric_knowledge(arithmetic, first, function),
        ElementaryFunction::Cosine => trigonometric_knowledge(arithmetic, first, function),
        ElementaryFunction::Tangent => trigonometric_knowledge(arithmetic, first, function),
        ElementaryFunction::Arcsine => arcsine_knowledge(arithmetic, first),
        ElementaryFunction::Arccosine => arccosine_knowledge(arithmetic, first),
        ElementaryFunction::Arctangent => arctangent_knowledge(arithmetic, first),
        ElementaryFunction::Power => power_knowledge(arithmetic, first, second),
        ElementaryFunction::Arctangent2 => arctangent2_knowledge(arithmetic, first, second),
    }
}

fn exponential_knowledge<A: Arithmetic>(arithmetic: &A, argument: f64) -> Knowledge<A::Value> {
    if argument.is_nan() {
        return Knowledge::NotANumber;
    }
    if argument == f64::INFINITY {
        return Knowledge::Exact(ExactValue::PositiveInfinity);
    }
    if argument == f64::NEG_INFINITY {
        return exact_integer(0);
    }
    if argument == 0.0 {
        return exact_integer(1);
    }
    if argument > EXPONENTIAL_OVERFLOW_ARGUMENT {
        return Knowledge::Overflow { is_negative: false };
    }
    if argument < EXPONENTIAL_UNDERFLOW_ARGUMENT {
        return Knowledge::Enclosed(underflow_enclosure(arithmetic));
    }
    enclosed(exponential(arithmetic, &arithmetic.exact(argument)))
}

fn logarithm_knowledge<A: Arithmetic>(arithmetic: &A, argument: f64) -> Knowledge<A::Value> {
    if argument.is_nan() || argument < 0.0 {
        return Knowledge::NotANumber;
    }
    if argument == 0.0 {
        return Knowledge::Exact(ExactValue::NegativeInfinity);
    }
    if argument == f64::INFINITY {
        return Knowledge::Exact(ExactValue::PositiveInfinity);
    }
    if argument == 1.0 {
        return exact_integer(0);
    }
    enclosed(logarithm(arithmetic, argument))
}

fn square_root_knowledge<A: Arithmetic>(arithmetic: &A, argument: f64) -> Knowledge<A::Value> {
    if argument.is_nan() || argument < 0.0 {
        return Knowledge::NotANumber;
    }
    if argument == 0.0 {
        return exact_integer(0);
    }
    if argument == f64::INFINITY {
        return Knowledge::Exact(ExactValue::PositiveInfinity);
    }
    enclosed(square_root(arithmetic, argument))
}

fn trigonometric_knowledge<A: Arithmetic>(
    arithmetic: &A,
    argument: f64,
    function: ElementaryFunction,
) -> Knowledge<A::Value> {
    if !argument.is_finite() {
        return Knowledge::NotANumber;
    }
    if argument == 0.0 {
        return exact_integer(if function == ElementaryFunction::Cosine {
            1
        } else {
            0
        });
    }
    let Some((sine, cosine)) = sine_and_cosine(arithmetic, argument) else {
        return Knowledge::Unknown;
    };
    match function {
        ElementaryFunction::Sine => Knowledge::Enclosed(sine),
        ElementaryFunction::Cosine => Knowledge::Enclosed(cosine),
        _ => enclosed(arithmetic.div(&sine, &cosine)),
    }
}

fn arcsine_knowledge<A: Arithmetic>(arithmetic: &A, argument: f64) -> Knowledge<A::Value> {
    if argument.is_nan() || argument.abs() > 1.0 {
        return Knowledge::NotANumber;
    }
    if argument == 0.0 {
        return exact_integer(0);
    }
    if argument.abs() == 1.0 {
        return Knowledge::Enclosed(signed(arithmetic, half_pi(arithmetic), argument < 0.0));
    }
    enclosed(arcsine(arithmetic, argument))
}

fn arccosine_knowledge<A: Arithmetic>(arithmetic: &A, argument: f64) -> Knowledge<A::Value> {
    if argument.is_nan() || argument.abs() > 1.0 {
        return Knowledge::NotANumber;
    }
    if argument == 1.0 {
        return exact_integer(0);
    }
    if argument == -1.0 {
        return Knowledge::Enclosed(arithmetic.pi());
    }
    enclosed(arccosine(arithmetic, argument))
}

fn arctangent_knowledge<A: Arithmetic>(arithmetic: &A, argument: f64) -> Knowledge<A::Value> {
    if argument.is_nan() {
        return Knowledge::NotANumber;
    }
    if argument == 0.0 {
        return exact_integer(0);
    }
    if argument.is_infinite() {
        return Knowledge::Enclosed(signed(arithmetic, half_pi(arithmetic), argument < 0.0));
    }
    enclosed(arctangent(arithmetic, &arithmetic.exact(argument)))
}

fn arctangent2_knowledge<A: Arithmetic>(
    arithmetic: &A,
    numerator: f64,
    denominator: f64,
) -> Knowledge<A::Value> {
    if numerator.is_nan() || denominator.is_nan() {
        return Knowledge::NotANumber;
    }
    let is_negative = numerator.is_sign_negative();
    let pi = arithmetic.pi();
    let quarter_pi = arithmetic.times_power_of_two(&pi, -2);
    if numerator == 0.0 {
        return if denominator.is_sign_negative() {
            Knowledge::Enclosed(signed(arithmetic, pi, is_negative))
        } else {
            exact_integer(0)
        };
    }
    if numerator.is_infinite() {
        let angle = if denominator == f64::INFINITY {
            quarter_pi
        } else if denominator == f64::NEG_INFINITY {
            arithmetic.mul(&quarter_pi, &arithmetic.small_integer(3))
        } else {
            half_pi(arithmetic)
        };
        return Knowledge::Enclosed(signed(arithmetic, angle, is_negative));
    }
    if denominator == 0.0 {
        return Knowledge::Enclosed(signed(arithmetic, half_pi(arithmetic), is_negative));
    }
    if denominator == f64::INFINITY {
        return exact_integer(0);
    }
    if denominator == f64::NEG_INFINITY {
        return Knowledge::Enclosed(signed(arithmetic, pi, is_negative));
    }
    let Some(ratio) = arithmetic.div(&arithmetic.exact(numerator), &arithmetic.exact(denominator))
    else {
        return Knowledge::Unknown;
    };
    let Some(angle) = arctangent(arithmetic, &ratio) else {
        return Knowledge::Unknown;
    };
    if denominator > 0.0 {
        Knowledge::Enclosed(angle)
    } else if is_negative {
        Knowledge::Enclosed(arithmetic.sub(&angle, &pi))
    } else {
        Knowledge::Enclosed(arithmetic.add(&angle, &pi))
    }
}

fn power_knowledge<A: Arithmetic>(arithmetic: &A, base: f64, exponent: f64) -> Knowledge<A::Value> {
    if exponent == 0.0 || base == 1.0 {
        return exact_integer(1);
    }
    if base.is_nan() || exponent.is_nan() {
        return Knowledge::NotANumber;
    }
    let is_odd_exponent = is_odd_integer(exponent);
    if base == 0.0 {
        return if exponent > 0.0 {
            exact_integer(0)
        } else if base.is_sign_negative() && is_odd_exponent {
            Knowledge::Exact(ExactValue::NegativeInfinity)
        } else {
            Knowledge::Exact(ExactValue::PositiveInfinity)
        };
    }
    if exponent.is_infinite() {
        if base == -1.0 {
            return exact_integer(1);
        }
        let grows = (base.abs() > 1.0) == (exponent > 0.0);
        return if grows {
            Knowledge::Exact(ExactValue::PositiveInfinity)
        } else {
            exact_integer(0)
        };
    }
    if base.is_infinite() {
        return if exponent < 0.0 {
            exact_integer(0)
        } else if base < 0.0 && is_odd_exponent {
            Knowledge::Exact(ExactValue::NegativeInfinity)
        } else {
            Knowledge::Exact(ExactValue::PositiveInfinity)
        };
    }
    if base < 0.0 && !is_integer(exponent) {
        return Knowledge::NotANumber;
    }
    let is_negative = base < 0.0 && is_odd_exponent;
    if let Some(known) = rational_power(arithmetic, base.abs(), exponent, is_negative) {
        return known;
    }
    let Some(logarithm) = logarithm(arithmetic, base.abs()) else {
        return Knowledge::Unknown;
    };
    let product = arithmetic.mul(&arithmetic.exact(exponent), &logarithm);
    let overflow = Point {
        machine: Some(EXPONENTIAL_OVERFLOW_ARGUMENT),
        exact: Number::F64(EXPONENTIAL_OVERFLOW_ARGUMENT).to_exact().ok(),
    };
    let underflow = Point {
        machine: Some(EXPONENTIAL_UNDERFLOW_ARGUMENT),
        exact: Number::F64(EXPONENTIAL_UNDERFLOW_ARGUMENT).to_exact().ok(),
    };
    if arithmetic.compare_lower(&product, &overflow) == Some(Ordering::Greater) {
        return Knowledge::Overflow { is_negative };
    }
    if arithmetic.compare_upper(&product, &underflow) == Some(Ordering::Less) {
        return Knowledge::Enclosed(underflow_enclosure(arithmetic));
    }
    match exponential(arithmetic, &product) {
        Some(value) => Knowledge::Enclosed(signed(arithmetic, value, is_negative)),
        None => Knowledge::Unknown,
    }
}

fn integer_root(value: u64, depth: i32) -> Option<u64> {
    let mut candidate = f64::from(u32::try_from(value >> 32).ok()?) * 4_294_967_296.0
        + f64::from(u32::try_from(value & 0xffff_ffff).ok()?);
    for _ in 0..depth {
        candidate = candidate.sqrt();
    }
    let rounded = crate::bits::integral_to_i64(candidate.round_ties_even())?;
    let power = 1u32 << depth;
    let low = u64::try_from(rounded)
        .ok()?
        .saturating_sub(LARGEST_ROOT_CANDIDATE_ERROR);
    (low..=low + 2 * LARGEST_ROOT_CANDIDATE_ERROR)
        .find(|root| u128::from(*root).checked_pow(power) == Some(u128::from(value)))
}

fn rational_power<A: Arithmetic>(
    arithmetic: &A,
    magnitude: f64,
    exponent: f64,
    is_negative: bool,
) -> Option<Knowledge<A::Value>> {
    let (base_significand, base_exponent) = odd_significand_and_exponent(magnitude)?;
    let (exponent_significand, exponent_shift) = odd_significand_and_exponent(exponent.abs())?;
    let signed_significand = if exponent < 0.0 {
        -i128::from(exponent_significand)
    } else {
        i128::from(exponent_significand)
    };
    if base_significand == 1 {
        let power = power_of_two_exponent(base_exponent, signed_significand, exponent_shift)?;
        let grows = (base_exponent > 0) == (exponent > 0.0);
        return Some(power_of_two_knowledge(
            arithmetic,
            power,
            grows,
            is_negative,
        ));
    }
    let root_depth = (-exponent_shift).max(0);
    if root_depth > LARGEST_ROOT_DEPTH || base_exponent % (1 << root_depth) != 0 {
        return None;
    }
    let root_significand = if root_depth == 0 {
        base_significand
    } else {
        integer_root(base_significand, root_depth)?
    };
    if exponent_shift > LARGEST_EXACT_POWER_SHIFT {
        return None;
    }
    let whole_exponent = signed_significand << exponent_shift.max(0);
    if whole_exponent.abs() > LARGEST_EXACT_POWER {
        return None;
    }
    let base = exact_dyadic(root_significand, base_exponent >> root_depth)?;
    let mut value = Number::from(1i64);
    for _ in 0..whole_exponent.abs() {
        value = value.mul_exact(&base).ok()?;
    }
    if whole_exponent < 0 {
        value = Number::from(1i64).div_exact(&value).ok()?;
    }
    if is_negative {
        value = value.negate_exact().ok()?;
    }
    Some(Knowledge::Exact(ExactValue::Finite(value)))
}

fn exact_dyadic(significand: u64, exponent: i32) -> Option<Number> {
    let power = Integer::from(2i64).pow(exponent.unsigned_abs());
    let integer = Integer::from(significand);
    if exponent >= 0 {
        Some(Number::Integer(&integer * &power))
    } else {
        Number::fraction(&integer, &power).ok()
    }
}

fn power_of_two_exponent(
    base_exponent: i32,
    significand: i128,
    shift: i32,
) -> Option<Option<i128>> {
    let exponent = i128::from(base_exponent);
    if exponent == 0 {
        return Some(Some(0));
    }
    if shift >= 0 {
        if shift > LARGEST_POWER_OF_TWO_SHIFT {
            return Some(None);
        }
        return Some(
            significand
                .checked_mul(exponent)
                .and_then(|product| product.checked_mul(1i128 << shift)),
        );
    }
    let depth = -shift;
    if depth > LARGEST_POWER_OF_TWO_ROOT_DEPTH || base_exponent % (1 << depth) != 0 {
        return None;
    }
    Some(significand.checked_mul(i128::from(base_exponent >> depth)))
}

fn power_of_two_knowledge<A: Arithmetic>(
    arithmetic: &A,
    power: Option<i128>,
    grows: bool,
    is_negative: bool,
) -> Knowledge<A::Value> {
    match power {
        Some(value) if value.abs() <= POWER_OF_TWO_RANGE => {
            let exponent = i32::try_from(value).unwrap_or_default();
            let magnitude = exact_dyadic(1, exponent).unwrap_or_else(|| Number::from(0i64));
            let signed_exact = if is_negative {
                magnitude.negate_exact().unwrap_or(magnitude)
            } else {
                magnitude
            };
            Knowledge::Exact(ExactValue::Finite(signed_exact))
        }
        _ if grows => Knowledge::Overflow { is_negative },
        _ => Knowledge::Enclosed(underflow_enclosure(arithmetic)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dyadic::DyadicArithmetic;

    fn exact_of(knowledge: &Knowledge<crate::dyadic::DyadicInterval>) -> Option<Number> {
        match knowledge {
            Knowledge::Exact(ExactValue::Finite(value)) => Some(value.clone()),
            _ => None,
        }
    }

    #[test]
    fn power_of_three_to_thirty_four_is_exact_midpoint_integer() {
        let arithmetic = DyadicArithmetic::new(64);

        let known = knowledge(&arithmetic, ElementaryFunction::Power, &[3.0, 34.0]);

        assert_eq!(
            exact_of(&known),
            Some(Number::Integer(Integer::from(16_677_181_699_666_569i64)))
        );
    }

    #[test]
    fn power_of_twenty_five_to_eleven_and_a_half_is_exact() {
        let arithmetic = DyadicArithmetic::new(64);

        let known = knowledge(&arithmetic, ElementaryFunction::Power, &[25.0, 11.5]);

        assert_eq!(
            exact_of(&known),
            Some(Number::Integer(Integer::from(11_920_928_955_078_125i64)))
        );
    }

    #[test]
    fn power_of_two_to_minus_1075_is_exact_half_subnormal() {
        let arithmetic = DyadicArithmetic::new(64);

        let known = knowledge(&arithmetic, ElementaryFunction::Power, &[2.0, -1075.0]);
        let expected =
            Number::fraction(&Integer::one(), &Integer::from(2i64).pow(1075)).expect("fraction");

        assert_eq!(exact_of(&known), Some(expected));
    }

    #[test]
    fn power_of_negative_base_with_fraction_exponent_is_not_a_number() {
        let arithmetic = DyadicArithmetic::new(64);

        let known = knowledge(&arithmetic, ElementaryFunction::Power, &[-2.0, 0.5]);

        assert!(matches!(known, Knowledge::NotANumber));
    }

    #[test]
    fn power_of_negative_two_to_three_is_minus_eight() {
        let arithmetic = DyadicArithmetic::new(64);

        let known = knowledge(&arithmetic, ElementaryFunction::Power, &[-2.0, 3.0]);

        assert_eq!(exact_of(&known), Some(Number::from(-8i64)));
    }

    #[test]
    fn power_of_three_to_one_half_is_enclosed() {
        let arithmetic = DyadicArithmetic::new(64);

        let known = knowledge(&arithmetic, ElementaryFunction::Power, &[3.0, 0.5]);

        assert!(matches!(known, Knowledge::Enclosed(_)));
    }

    #[test]
    fn power_of_two_to_huge_exponent_overflows() {
        let arithmetic = DyadicArithmetic::new(64);

        let known = knowledge(&arithmetic, ElementaryFunction::Power, &[2.0, 1.0e300]);

        assert!(matches!(known, Knowledge::Overflow { is_negative: false }));
    }

    #[test]
    fn exponential_beyond_overflow_argument_overflows() {
        let arithmetic = DyadicArithmetic::new(64);

        let known = knowledge(&arithmetic, ElementaryFunction::Exponential, &[711.0]);

        assert!(matches!(known, Knowledge::Overflow { is_negative: false }));
    }

    #[test]
    fn logarithm_of_negative_argument_is_not_a_number() {
        let arithmetic = DyadicArithmetic::new(64);

        let known = knowledge(&arithmetic, ElementaryFunction::Logarithm, &[-1.0]);

        assert!(matches!(known, Knowledge::NotANumber));
    }

    #[test]
    fn arctangent2_of_negative_zero_over_negative_zero_is_minus_pi() {
        let arithmetic = DyadicArithmetic::new(64);

        let known = knowledge(&arithmetic, ElementaryFunction::Arctangent2, &[-0.0, -0.0]);

        assert!(
            matches!(known, Knowledge::Enclosed(ref value) if arithmetic.compare_upper(value, &Point::machine(-3.0)) == Some(Ordering::Less))
        );
    }
}
