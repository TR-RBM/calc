use std::cmp::Ordering;

use calc_numbers::{Integer, Number};

use crate::arithmetic::{Arithmetic, Point, ZERO_MAGNITUDE_EXPONENT};
use crate::functions::{arctangent_series, inverse_hyperbolic_tangent_series};

const CONSTANT_GUARD_BITS: u32 = 64;
const MACHIN_LARGE_DENOMINATOR: i64 = 5;
const MACHIN_SMALL_DENOMINATOR: i64 = 239;
const MACHIN_LARGE_FACTOR_EXPONENT: i32 = 4;
const MACHIN_SMALL_FACTOR_EXPONENT: i32 = 2;
const LN2_ARGUMENT_DENOMINATOR: i64 = 3;
const SMALL_ARGUMENT: f64 = 0.75;
const REDUCTION_GUARD_BITS: i32 = 64;
const MACHINE_CHUNK_EXPONENT: u32 = 1000;
const QUADRANT_COUNT: i64 = 4;
const TURN_WINDOW_BITS: u32 = 126;
const TURN_WINDOW_OFFSET: i32 = 125;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DyadicInterval {
    lower: Integer,
    upper: Integer,
}

pub struct DyadicArithmetic {
    precision: u32,
    scale: Integer,
    half_scale: Integer,
    pi: DyadicInterval,
    ln2: DyadicInterval,
}

fn power_of_two_integer(exponent: u32) -> Integer {
    Integer::from(2i64).pow(exponent)
}

fn floor_divide(numerator: &Integer, denominator: &Integer) -> Integer {
    numerator
        .div_rem_euclid(denominator)
        .map(|(quotient, _)| quotient)
        .expect("dyadic divisors are positive")
}

fn ceil_divide(numerator: &Integer, denominator: &Integer) -> Integer {
    floor_divide(&numerator.negated(), denominator).negated()
}

fn larger_magnitude(interval: &DyadicInterval) -> Integer {
    let lower = interval.lower.absolute();
    let upper = interval.upper.absolute();
    if lower > upper { lower } else { upper }
}

pub fn bit_length_upper(value: &Integer) -> i64 {
    let chunk = power_of_two_integer(MACHINE_CHUNK_EXPONENT);
    let mut remaining = value.absolute();
    let mut shifted = 0i64;
    loop {
        let approximation = Number::Integer(remaining.clone()).round_to_f64_ties_even();
        if approximation.is_finite() {
            let exponent = crate::bits::binary_exponent(approximation).map_or(0, i64::from);
            return shifted + exponent + 2;
        }
        remaining = floor_divide(&remaining, &chunk);
        shifted += i64::from(MACHINE_CHUNK_EXPONENT);
    }
}

pub fn integer_square_root_floor(value: &Integer) -> Integer {
    if value.is_zero() || value.is_negative() {
        return Integer::zero();
    }
    let half_length = u32::try_from((bit_length_upper(value) + 1) / 2).expect("length fits");
    let two = Integer::from(2i64);
    let mut current = power_of_two_integer(half_length);
    loop {
        let next = floor_divide(&(&current + &floor_divide(value, &current)), &two);
        if next >= current {
            return current;
        }
        current = next;
    }
}

fn exact_fraction(value: &Number) -> Option<(Integer, Integer)> {
    match value.to_exact() {
        Ok(Number::Integer(integer)) => Some((integer, Integer::one())),
        Ok(Number::Rational(rational)) => {
            Some((rational.numerator().clone(), rational.denominator().clone()))
        }
        _ => None,
    }
}

impl DyadicInterval {
    fn point(units: Integer) -> Self {
        Self {
            lower: units.clone(),
            upper: units,
        }
    }
}

impl DyadicArithmetic {
    pub fn new(precision: u32) -> Self {
        let guarded = Self::without_constants(precision + CONSTANT_GUARD_BITS);
        let pi = guarded.machin_pi();
        let ln2 = guarded.series_ln2();
        let mut arithmetic = Self::without_constants(precision);
        arithmetic.pi = arithmetic.rescaled(&pi, CONSTANT_GUARD_BITS);
        arithmetic.ln2 = arithmetic.rescaled(&ln2, CONSTANT_GUARD_BITS);
        arithmetic
    }

    pub fn precision(&self) -> u32 {
        self.precision
    }

    fn without_constants(precision: u32) -> Self {
        let scale = power_of_two_integer(precision);
        let half_scale = power_of_two_integer(precision - 1);
        Self {
            precision,
            scale,
            half_scale,
            pi: DyadicInterval::point(Integer::zero()),
            ln2: DyadicInterval::point(Integer::zero()),
        }
    }

    fn rescaled(&self, value: &DyadicInterval, extra_bits: u32) -> DyadicInterval {
        let divisor = power_of_two_integer(extra_bits);
        DyadicInterval {
            lower: floor_divide(&value.lower, &divisor),
            upper: ceil_divide(&value.upper, &divisor),
        }
    }

    fn reciprocal(&self, denominator: i64) -> DyadicInterval {
        self.div(&self.small_integer(1), &self.small_integer(denominator))
            .expect("constant denominators are positive")
    }

    fn machin_pi(&self) -> DyadicInterval {
        let large = arctangent_series(self, &self.reciprocal(MACHIN_LARGE_DENOMINATOR));
        let small = arctangent_series(self, &self.reciprocal(MACHIN_SMALL_DENOMINATOR));
        self.sub(
            &self.times_power_of_two(&large, MACHIN_LARGE_FACTOR_EXPONENT),
            &self.times_power_of_two(&small, MACHIN_SMALL_FACTOR_EXPONENT),
        )
    }

    fn series_ln2(&self) -> DyadicInterval {
        let half_log =
            inverse_hyperbolic_tangent_series(self, &self.reciprocal(LN2_ARGUMENT_DENOMINATOR));
        self.times_power_of_two(&half_log, 1)
    }

    pub fn bounds(&self, value: &DyadicInterval) -> (Number, Number) {
        let lower = Number::fraction(&value.lower, &self.scale).expect("scale is positive");
        let upper = Number::fraction(&value.upper, &self.scale).expect("scale is positive");
        (lower, upper)
    }

    pub fn quarter_turn_window(&self, exponent: i32) -> u128 {
        let shift = u32::try_from(exponent + TURN_WINDOW_OFFSET).expect("window exponent");
        let dividend = power_of_two_integer(shift + self.precision);
        let smallest = floor_divide(&dividend, &self.pi.upper);
        let largest = floor_divide(&dividend, &self.pi.lower);
        assert_eq!(
            smallest, largest,
            "pi precision too low for exponent {exponent}"
        );
        let window = power_of_two_integer(TURN_WINDOW_BITS);
        let (_, remainder) = smallest
            .div_rem_euclid(&window)
            .expect("window is positive");
        remainder
            .to_i128()
            .expect("window fits in 128 bits")
            .cast_unsigned()
    }

    fn compare_units(&self, units: &Integer, point: &Point) -> Option<Ordering> {
        let exact = match (&point.exact, point.machine) {
            (Some(exact), _) => exact.clone(),
            (None, Some(machine)) => Number::F64(machine),
            (None, None) => return None,
        };
        let (numerator, denominator) = exact_fraction(&exact)?;
        Some((units * &denominator).cmp(&(&numerator * &self.scale)))
    }
}

impl Arithmetic for DyadicArithmetic {
    type Value = DyadicInterval;

    fn exact(&self, value: f64) -> DyadicInterval {
        let (numerator, denominator) =
            exact_fraction(&Number::F64(value)).expect("exact arguments are finite");
        let scaled = &numerator * &self.scale;
        DyadicInterval {
            lower: floor_divide(&scaled, &denominator),
            upper: ceil_divide(&scaled, &denominator),
        }
    }

    fn integer(&self, value: &Integer) -> DyadicInterval {
        DyadicInterval::point(value * &self.scale)
    }

    fn add(&self, left: &DyadicInterval, right: &DyadicInterval) -> DyadicInterval {
        DyadicInterval {
            lower: &left.lower + &right.lower,
            upper: &left.upper + &right.upper,
        }
    }

    fn sub(&self, left: &DyadicInterval, right: &DyadicInterval) -> DyadicInterval {
        DyadicInterval {
            lower: &left.lower - &right.upper,
            upper: &left.upper - &right.lower,
        }
    }

    fn mul(&self, left: &DyadicInterval, right: &DyadicInterval) -> DyadicInterval {
        let products = [
            &left.lower * &right.lower,
            &left.lower * &right.upper,
            &left.upper * &right.lower,
            &left.upper * &right.upper,
        ];
        let smallest = products.iter().min().expect("four products");
        let largest = products.iter().max().expect("four products");
        DyadicInterval {
            lower: floor_divide(smallest, &self.scale),
            upper: ceil_divide(largest, &self.scale),
        }
    }

    fn div(
        &self,
        numerator: &DyadicInterval,
        denominator: &DyadicInterval,
    ) -> Option<DyadicInterval> {
        if denominator.upper.is_negative() {
            return self.div(&self.negate(numerator), &self.negate(denominator));
        }
        if denominator.lower.is_negative() || denominator.lower.is_zero() {
            return None;
        }
        let scaled_lower = &numerator.lower * &self.scale;
        let scaled_upper = &numerator.upper * &self.scale;
        let lower = floor_divide(&scaled_lower, &denominator.lower)
            .min(floor_divide(&scaled_lower, &denominator.upper));
        let upper = ceil_divide(&scaled_upper, &denominator.lower)
            .max(ceil_divide(&scaled_upper, &denominator.upper));
        Some(DyadicInterval { lower, upper })
    }

    fn negate(&self, value: &DyadicInterval) -> DyadicInterval {
        DyadicInterval {
            lower: value.upper.negated(),
            upper: value.lower.negated(),
        }
    }

    fn square_root(&self, value: &DyadicInterval) -> Option<DyadicInterval> {
        if value.lower.is_negative() {
            return None;
        }
        let lower = integer_square_root_floor(&(&value.lower * &self.scale));
        let upper_square = &value.upper * &self.scale;
        let upper_floor = integer_square_root_floor(&upper_square);
        let upper = if &upper_floor * &upper_floor == upper_square {
            upper_floor
        } else {
            &upper_floor + &Integer::one()
        };
        Some(DyadicInterval { lower, upper })
    }

    fn times_power_of_two(&self, value: &DyadicInterval, exponent: i32) -> DyadicInterval {
        let factor = power_of_two_integer(exponent.unsigned_abs());
        if exponent >= 0 {
            DyadicInterval {
                lower: &value.lower * &factor,
                upper: &value.upper * &factor,
            }
        } else {
            DyadicInterval {
                lower: floor_divide(&value.lower, &factor),
                upper: ceil_divide(&value.upper, &factor),
            }
        }
    }

    fn widen(&self, value: &DyadicInterval, radius_exponent: i32) -> DyadicInterval {
        let precision = i32::try_from(self.precision).expect("precision fits");
        let radius = match u32::try_from(radius_exponent.saturating_add(precision)) {
            Ok(shift) => power_of_two_integer(shift),
            Err(_) => Integer::one(),
        };
        DyadicInterval {
            lower: &value.lower - &radius,
            upper: &value.upper + &radius,
        }
    }

    fn magnitude_exponent(&self, value: &DyadicInterval) -> i32 {
        let magnitude = larger_magnitude(value);
        if magnitude.is_zero() {
            return ZERO_MAGNITUDE_EXPONENT;
        }
        let length = bit_length_upper(&magnitude) - i64::from(self.precision);
        i32::try_from(length).expect("magnitude exponent fits")
    }

    fn nearest_small_integer(&self, value: &DyadicInterval) -> Option<i64> {
        floor_divide(&(&value.lower + &self.half_scale), &self.scale).to_i64()
    }

    fn quarter_turns(&self, value: f64) -> Option<(DyadicInterval, u8)> {
        if value.abs() < SMALL_ARGUMENT {
            return Some((self.exact(value), 0));
        }
        let exponent = crate::bits::binary_exponent(value)?;
        let precision = i32::try_from(self.precision).ok()?;
        if precision < exponent + REDUCTION_GUARD_BITS {
            return None;
        }
        let argument = self.exact(value);
        let turns = self.div(&self.times_power_of_two(&argument, 1), &self.pi)?;
        let count = floor_divide(&(&turns.lower + &self.half_scale), &self.scale);
        let half_pi = self.times_power_of_two(&self.pi, -1);
        let reduced = self.sub(&argument, &self.mul(&self.integer(&count), &half_pi));
        let (_, quadrant) = count.div_rem_euclid(&Integer::from(QUADRANT_COUNT)).ok()?;
        Some((reduced, u8::try_from(quadrant.to_i64()?).ok()?))
    }

    fn pi(&self) -> DyadicInterval {
        self.pi.clone()
    }

    fn ln2(&self) -> DyadicInterval {
        self.ln2.clone()
    }

    fn truncation_exponent(&self) -> i32 {
        -i32::try_from(self.precision).expect("precision fits")
    }

    fn compare_lower(&self, value: &DyadicInterval, point: &Point) -> Option<Ordering> {
        self.compare_units(&value.lower, point)
    }

    fn compare_upper(&self, value: &DyadicInterval, point: &Point) -> Option<Ordering> {
        self.compare_units(&value.upper, point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PI_DIGITS: &str = "31415926535897932384626433832795028841971693993751";
    const LN2_DIGITS: &str = "69314718055994530941723212145817656807550013436025";

    fn decimal_integer(digits: &str) -> Integer {
        digits.bytes().fold(Integer::zero(), |accumulated, digit| {
            &(&accumulated * &Integer::from(10i64)) + &Integer::from(i64::from(digit - b'0'))
        })
    }

    fn contains_decimal(
        arithmetic: &DyadicArithmetic,
        value: &DyadicInterval,
        digits: &str,
        leading_digits: u32,
    ) -> bool {
        let exponent = u32::try_from(digits.len()).expect("short digits") - leading_digits;
        let denominator = Integer::from(10i64).pow(exponent);
        let numerator = decimal_integer(digits);
        let below =
            Number::fraction(&(&numerator - &Integer::one()), &denominator).expect("fraction");
        let above =
            Number::fraction(&(&numerator + &Integer::one()), &denominator).expect("fraction");
        arithmetic.compare_lower(value, &Point::exact(below)) == Some(Ordering::Greater)
            && arithmetic.compare_upper(value, &Point::exact(above)) == Some(Ordering::Less)
    }

    #[test]
    fn pi_lies_within_fifty_known_digits() {
        let arithmetic = DyadicArithmetic::new(200);

        let pi = arithmetic.pi();

        assert!(contains_decimal(&arithmetic, &pi, PI_DIGITS, 1));
    }

    #[test]
    fn ln2_lies_within_fifty_known_digits() {
        let arithmetic = DyadicArithmetic::new(200);

        let ln2 = arithmetic.ln2();

        assert!(contains_decimal(&arithmetic, &ln2, LN2_DIGITS, 0));
    }

    #[test]
    fn constants_are_narrow_at_their_precision() {
        let arithmetic = DyadicArithmetic::new(200);

        let exponent =
            arithmetic.magnitude_exponent(&arithmetic.sub(&arithmetic.pi(), &arithmetic.pi()));

        assert!(exponent < -190);
    }

    #[test]
    fn square_root_of_exact_square_is_a_point() {
        let arithmetic = DyadicArithmetic::new(64);

        let root = arithmetic
            .square_root(&arithmetic.exact(6.25))
            .expect("nonnegative");

        assert_eq!(root, arithmetic.exact(2.5));
    }

    #[test]
    fn square_root_of_two_encloses_its_square() {
        let arithmetic = DyadicArithmetic::new(128);

        let root = arithmetic
            .square_root(&arithmetic.small_integer(2))
            .expect("nonnegative");
        let square = arithmetic.mul(&root, &root);

        assert!(
            arithmetic.compare_lower(&square, &Point::machine(2.0)) != Some(Ordering::Greater)
                && arithmetic.compare_upper(&square, &Point::machine(2.0)) != Some(Ordering::Less)
        );
    }

    #[test]
    fn integer_square_root_floor_of_one_below_a_square() {
        let root = integer_square_root_floor(&Integer::from(80i64));

        assert_eq!(root, Integer::from(8i64));
    }

    #[test]
    fn bit_length_upper_covers_large_power_of_two() {
        let value = power_of_two_integer(3000);

        let length = bit_length_upper(&value);

        assert!((3001..=3004).contains(&length));
    }

    #[test]
    fn division_by_interval_containing_zero_is_refused() {
        let arithmetic = DyadicArithmetic::new(64);
        let straddling = arithmetic.widen(&arithmetic.small_integer(0), -3);

        let quotient = arithmetic.div(&arithmetic.small_integer(1), &straddling);

        assert_eq!(quotient, None);
    }

    #[test]
    fn exact_subnormal_argument_rounds_outward() {
        let arithmetic = DyadicArithmetic::new(64);

        let tiny = arithmetic.exact(f64::from_bits(1));

        assert!(tiny.lower.is_zero() && tiny.upper == Integer::one());
    }

    #[test]
    fn quarter_turns_of_three_lands_in_quadrant_two() {
        let arithmetic = DyadicArithmetic::new(128);

        let (reduced, quadrant) = arithmetic.quarter_turns(3.0).expect("reduction");

        assert!(
            quadrant == 2
                && arithmetic.compare_upper(&reduced, &Point::machine(0.0)) == Some(Ordering::Less)
        );
    }
}
