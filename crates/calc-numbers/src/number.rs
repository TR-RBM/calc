use std::hash::{Hash, Hasher};

use crate::binary_format::{BINARY32, BINARY64, BinaryFormat};
use crate::integer::Integer;
use crate::natural::Natural;
use crate::rational::{Rational, ReducedFraction};
use crate::word_conversion::low_half;

#[derive(Clone, Debug)]
pub enum Number {
    Integer(Integer),
    Rational(Rational),
    F32(f32),
    F64(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExactArithmeticError {
    MachineOperand,
    DivisionByZero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToExactError {
    NotFinite,
}

struct Fraction {
    numerator: Integer,
    denominator: Integer,
}

impl Number {
    pub fn fraction(
        numerator: &Integer,
        denominator: &Integer,
    ) -> Result<Self, ExactArithmeticError> {
        match Rational::reduce(numerator, denominator) {
            Some(ReducedFraction::Integer(integer)) => Ok(Self::Integer(integer)),
            Some(ReducedFraction::Rational(rational)) => Ok(Self::Rational(rational)),
            None => Err(ExactArithmeticError::DivisionByZero),
        }
    }

    pub fn is_exact(&self) -> bool {
        matches!(self, Self::Integer(_) | Self::Rational(_))
    }

    fn as_fraction(&self) -> Result<Fraction, ExactArithmeticError> {
        match self {
            Self::Integer(integer) => Ok(Fraction {
                numerator: integer.clone(),
                denominator: Integer::one(),
            }),
            Self::Rational(rational) => Ok(Fraction {
                numerator: rational.numerator().clone(),
                denominator: rational.denominator().clone(),
            }),
            Self::F32(_) | Self::F64(_) => Err(ExactArithmeticError::MachineOperand),
        }
    }

    #[cfg(test)]
    pub(crate) fn is_negative_exact(&self) -> bool {
        match self {
            Self::Integer(integer) => integer.is_negative(),
            Self::Rational(rational) => rational.numerator().is_negative(),
            Self::F32(_) | Self::F64(_) => false,
        }
    }

    pub fn add_exact(&self, other: &Self) -> Result<Self, ExactArithmeticError> {
        let left = self.as_fraction()?;
        let right = other.as_fraction()?;
        let numerator =
            &(&left.numerator * &right.denominator) + &(&right.numerator * &left.denominator);
        Self::fraction(&numerator, &(&left.denominator * &right.denominator))
    }

    pub fn sub_exact(&self, other: &Self) -> Result<Self, ExactArithmeticError> {
        self.add_exact(&other.negate_exact()?)
    }

    pub fn mul_exact(&self, other: &Self) -> Result<Self, ExactArithmeticError> {
        let left = self.as_fraction()?;
        let right = other.as_fraction()?;
        Self::fraction(
            &(&left.numerator * &right.numerator),
            &(&left.denominator * &right.denominator),
        )
    }

    pub fn div_exact(&self, other: &Self) -> Result<Self, ExactArithmeticError> {
        let left = self.as_fraction()?;
        let right = other.as_fraction()?;
        Self::fraction(
            &(&left.numerator * &right.denominator),
            &(&left.denominator * &right.numerator),
        )
    }

    pub fn negate_exact(&self) -> Result<Self, ExactArithmeticError> {
        let value = self.as_fraction()?;
        Self::fraction(&value.numerator.negated(), &value.denominator)
    }

    pub fn round_to_f64_ties_even(&self) -> f64 {
        f64::from_bits(self.round_to_format(&BINARY64))
    }

    pub fn round_to_f32_ties_even(&self) -> f32 {
        f32::from_bits(low_half(self.round_to_format(&BINARY32)))
    }

    fn round_to_format(&self, target: &BinaryFormat) -> u64 {
        match self {
            Self::Integer(integer) => round_integer_fraction(target, integer, &Natural::one()),
            Self::Rational(rational) => round_integer_fraction(
                target,
                rational.numerator(),
                rational.denominator().magnitude(),
            ),
            Self::F32(value) => round_machine(&BINARY32, target, u64::from(value.to_bits())),
            Self::F64(value) => round_machine(&BINARY64, target, value.to_bits()),
        }
    }

    pub fn to_exact(&self) -> Result<Self, ToExactError> {
        let converted = match self {
            Self::Integer(_) | Self::Rational(_) => Ok(self.clone()),
            Self::F32(value) => BINARY32.to_exact(u64::from(value.to_bits())),
            Self::F64(value) => BINARY64.to_exact(value.to_bits()),
        };
        converted.map_err(|_| ToExactError::NotFinite)
    }
}

fn round_integer_fraction(
    target: &BinaryFormat,
    numerator: &Integer,
    denominator: &Natural,
) -> u64 {
    target.round_fraction(numerator.is_negative(), numerator.magnitude(), denominator)
}

fn round_machine(source: &BinaryFormat, target: &BinaryFormat, bits: u64) -> u64 {
    let is_negative = source.is_negative(bits);
    if source.is_nan(bits) {
        return source.convert_nan_payload(target, bits);
    }
    match source.to_exact(bits) {
        Ok(Number::Integer(integer)) => {
            target.round_fraction(is_negative, integer.magnitude(), &Natural::one())
        }
        Ok(Number::Rational(rational)) => target.round_fraction(
            is_negative,
            rational.numerator().magnitude(),
            rational.denominator().magnitude(),
        ),
        Ok(Number::F32(_) | Number::F64(_)) | Err(_) => target.infinity_bits(is_negative),
    }
}

impl From<Integer> for Number {
    fn from(value: Integer) -> Self {
        Self::Integer(value)
    }
}

impl From<Rational> for Number {
    fn from(value: Rational) -> Self {
        Self::Rational(value)
    }
}

impl From<i64> for Number {
    fn from(value: i64) -> Self {
        Self::Integer(Integer::from(value))
    }
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Integer(left), Self::Integer(right)) => left == right,
            (Self::Rational(left), Self::Rational(right)) => left == right,
            (Self::F32(left), Self::F32(right)) => left.to_bits() == right.to_bits(),
            (Self::F64(left), Self::F64(right)) => left.to_bits() == right.to_bits(),
            _ => false,
        }
    }
}

impl Eq for Number {}

impl Hash for Number {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Integer(integer) => integer.hash(state),
            Self::Rational(rational) => rational.hash(state),
            Self::F32(value) => value.to_bits().hash(state),
            Self::F64(value) => value.to_bits().hash(state),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALLEST_SUBNORMAL_F64_BITS: u64 = 1;
    const LARGEST_SUBNORMAL_F64_BITS: u64 = 0x000f_ffff_ffff_ffff;
    const SMALLEST_SUBNORMAL_F32_BITS: u32 = 1;
    const EXACT_DECIMAL_DIGITS: usize = 1100;
    const DECIMAL_CASES: usize = 4000;

    struct SplitMix64 {
        state: u64,
    }

    impl SplitMix64 {
        fn next(&mut self) -> u64 {
            self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut mixed = self.state;
            mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            mixed ^ (mixed >> 31)
        }

        fn next_below(&mut self, bound: u64) -> u64 {
            self.next() % bound
        }
    }

    fn integer(value: i64) -> Number {
        Number::from(value)
    }

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    fn power_of_two(exponent: u32) -> Integer {
        Integer::from(2i64).pow(exponent)
    }

    fn decimal_number(digits: u64, decimal_exponent: i64) -> Number {
        let ten = Integer::from(10i64);
        let power = ten.pow(u32::try_from(decimal_exponent.unsigned_abs()).unwrap());
        let significand = Integer::from(digits);
        if decimal_exponent >= 0 {
            Number::Integer(&significand * &power)
        } else {
            Number::fraction(&significand, &power).unwrap()
        }
    }

    fn exact_decimal_text_f64(value: f64) -> String {
        format!("{value:.EXACT_DECIMAL_DIGITS$e}")
    }

    fn special_f64_values() -> Vec<f64> {
        vec![
            0.0,
            -0.0,
            f64::from_bits(SMALLEST_SUBNORMAL_F64_BITS),
            f64::from_bits(LARGEST_SUBNORMAL_F64_BITS),
            f64::MIN_POSITIVE,
            f64::MAX,
            f64::MIN,
            f64::from_bits(1.0f64.to_bits() - 1),
            f64::from_bits(1.0f64.to_bits() + 1),
            -1.5,
        ]
    }

    #[test]
    fn fraction_with_denominator_one_is_stored_as_integer() {
        assert_eq!(fraction(6, 3), integer(2));
    }

    #[test]
    fn fraction_with_zero_denominator_is_an_error() {
        assert_eq!(
            Number::fraction(&Integer::one(), &Integer::zero()),
            Err(ExactArithmeticError::DivisionByZero)
        );
    }

    #[test]
    fn exact_kinds_are_exact_and_machine_kinds_are_not() {
        assert!(integer(1).is_exact());
        assert!(fraction(1, 2).is_exact());
        assert!(!Number::F32(1.0).is_exact());
        assert!(!Number::F64(1.0).is_exact());
    }

    #[test]
    fn add_exact_reduces_the_sum() {
        assert_eq!(
            fraction(1, 6).add_exact(&fraction(1, 3)),
            Ok(fraction(1, 2))
        );
    }

    #[test]
    fn add_exact_of_integers_stays_integer() {
        assert_eq!(integer(-7).add_exact(&integer(3)), Ok(integer(-4)));
    }

    #[test]
    fn sub_exact_to_zero_gives_integer_zero() {
        assert_eq!(fraction(2, 7).sub_exact(&fraction(2, 7)), Ok(integer(0)));
    }

    #[test]
    fn mul_exact_of_reciprocals_gives_integer_one() {
        assert_eq!(fraction(2, 3).mul_exact(&fraction(3, 2)), Ok(integer(1)));
    }

    #[test]
    fn div_exact_of_integers_gives_rational() {
        assert_eq!(integer(1).div_exact(&integer(-10)), Ok(fraction(-1, 10)));
    }

    #[test]
    fn div_exact_by_zero_is_an_error() {
        assert_eq!(
            fraction(1, 2).div_exact(&integer(0)),
            Err(ExactArithmeticError::DivisionByZero)
        );
    }

    #[test]
    fn negate_exact_flips_the_sign_of_a_rational() {
        assert_eq!(fraction(3, 4).negate_exact(), Ok(fraction(-3, 4)));
    }

    #[test]
    fn exact_arithmetic_rejects_machine_operand() {
        assert_eq!(
            integer(1).add_exact(&Number::F64(1.0)),
            Err(ExactArithmeticError::MachineOperand)
        );
    }

    #[test]
    fn machine_numbers_are_identical_only_with_equal_bits() {
        assert_ne!(Number::F64(0.0), Number::F64(-0.0));
        assert_eq!(Number::F64(f64::NAN), Number::F64(f64::NAN));
        assert_ne!(Number::F32(1.0), Number::F64(1.0));
    }

    #[test]
    fn one_tenth_rounds_to_nearest_f64() {
        assert_eq!(
            fraction(1, 10).round_to_f64_ties_even().to_bits(),
            0.1f64.to_bits()
        );
    }

    #[test]
    fn decimal_rationals_round_like_correctly_rounded_f64_parsing() {
        let mut generator = SplitMix64 { state: 11 };
        for _ in 0..DECIMAL_CASES {
            let digits = generator.next() >> generator.next_below(64);
            let decimal_exponent = i64::try_from(generator.next_below(700)).unwrap() - 350;

            let rounded = decimal_number(digits, decimal_exponent).round_to_f64_ties_even();

            let expected: f64 = format!("{digits}e{decimal_exponent}").parse().unwrap();
            assert_eq!(
                rounded.to_bits(),
                expected.to_bits(),
                "{digits}e{decimal_exponent}"
            );
        }
    }

    #[test]
    fn decimal_rationals_near_subnormal_range_round_like_f64_parsing() {
        let mut generator = SplitMix64 { state: 12 };
        for _ in 0..DECIMAL_CASES {
            let digits = generator.next() >> generator.next_below(64);
            let decimal_exponent = i64::try_from(generator.next_below(60)).unwrap() - 360;

            let rounded = decimal_number(digits, decimal_exponent).round_to_f64_ties_even();

            let expected: f64 = format!("{digits}e{decimal_exponent}").parse().unwrap();
            assert_eq!(
                rounded.to_bits(),
                expected.to_bits(),
                "{digits}e{decimal_exponent}"
            );
        }
    }

    #[test]
    fn decimal_rationals_round_like_correctly_rounded_f32_parsing() {
        let mut generator = SplitMix64 { state: 13 };
        for _ in 0..DECIMAL_CASES {
            let digits = generator.next() >> generator.next_below(64);
            let decimal_exponent = i64::try_from(generator.next_below(120)).unwrap() - 70;

            let rounded = decimal_number(digits, decimal_exponent).round_to_f32_ties_even();

            let expected: f32 = format!("{digits}e{decimal_exponent}").parse().unwrap();
            assert_eq!(
                rounded.to_bits(),
                expected.to_bits(),
                "{digits}e{decimal_exponent}"
            );
        }
    }

    #[test]
    fn integer_tie_rounds_to_even_f64() {
        let base = power_of_two(53);

        let tie_below_even = Number::Integer(&base + &Integer::one());
        let tie_above_odd = Number::Integer(&base + &Integer::from(3i64));

        assert_eq!(
            tie_below_even.round_to_f64_ties_even(),
            9_007_199_254_740_992.0
        );
        assert_eq!(
            tie_above_odd.round_to_f64_ties_even(),
            9_007_199_254_740_996.0
        );
    }

    #[test]
    fn overflow_threshold_rounds_to_infinity() {
        let threshold = &(&power_of_two(54) - &Integer::one()) * &power_of_two(970);

        let rounded = Number::Integer(threshold).round_to_f64_ties_even();

        assert_eq!(rounded.to_bits(), f64::INFINITY.to_bits());
    }

    #[test]
    fn value_just_below_overflow_threshold_rounds_to_max() {
        let threshold = &(&power_of_two(54) - &Integer::one()) * &power_of_two(970);

        let rounded = Number::Integer(&threshold - &Integer::one()).round_to_f64_ties_even();

        assert_eq!(rounded.to_bits(), f64::MAX.to_bits());
    }

    #[test]
    fn negative_overflow_rounds_to_negative_infinity() {
        let huge = power_of_two(1024).negated();

        assert_eq!(
            Number::Integer(huge).round_to_f64_ties_even().to_bits(),
            f64::NEG_INFINITY.to_bits()
        );
    }

    #[test]
    fn half_of_smallest_subnormal_rounds_to_zero() {
        let half = Number::fraction(&Integer::one(), &power_of_two(1075)).unwrap();

        assert_eq!(half.round_to_f64_ties_even().to_bits(), 0);
    }

    #[test]
    fn three_quarters_of_smallest_subnormal_rounds_to_smallest_subnormal() {
        let three_quarters = Number::fraction(&Integer::from(3i64), &power_of_two(1076)).unwrap();

        assert_eq!(
            three_quarters.round_to_f64_ties_even().to_bits(),
            SMALLEST_SUBNORMAL_F64_BITS
        );
    }

    #[test]
    fn negative_value_below_half_subnormal_rounds_to_negative_zero() {
        let tiny = Number::fraction(&Integer::from(-1i64), &power_of_two(1080)).unwrap();

        assert_eq!(tiny.round_to_f64_ties_even().to_bits(), (-0.0f64).to_bits());
    }

    #[test]
    fn tie_above_largest_subnormal_rounds_to_smallest_normal() {
        let numerator = &power_of_two(53) - &Integer::one();
        let tie = Number::fraction(&numerator, &power_of_two(1075)).unwrap();

        assert_eq!(
            tie.round_to_f64_ties_even().to_bits(),
            f64::MIN_POSITIVE.to_bits()
        );
    }

    #[test]
    fn exact_zero_rounds_to_positive_zero() {
        assert_eq!(integer(0).round_to_f64_ties_even().to_bits(), 0);
        assert_eq!(integer(0).round_to_f32_ties_even().to_bits(), 0);
    }

    #[test]
    fn f32_rounds_to_f64_without_change_of_value() {
        let values = [
            0.0f32,
            -0.0,
            f32::from_bits(SMALLEST_SUBNORMAL_F32_BITS),
            f32::MIN_POSITIVE,
            f32::MAX,
            f32::MIN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::from_bits(1.0f32.to_bits() - 1),
            f32::from_bits(1.0f32.to_bits() + 1),
        ];
        for value in values {
            let widened = Number::F32(value).round_to_f64_ties_even();

            assert_eq!(widened.to_bits(), f64::from(value).to_bits());
        }
    }

    #[test]
    fn f32_nan_payload_moves_to_top_of_f64_payload() {
        let nan = f32::from_bits(0xffc0_0001);

        let widened = Number::F32(nan).round_to_f64_ties_even();

        assert_eq!(widened.to_bits(), 0xfff8_0000_2000_0000);
    }

    #[test]
    fn f64_rounds_to_f32_like_parsing_its_exact_decimal() {
        let mut values = special_f64_values();
        values.extend([0.1, 1e-40, 3.4028235677973366e38, 1e39, -7.006e-46]);
        for value in values {
            let narrowed = Number::F64(value).round_to_f32_ties_even();

            let expected: f32 = exact_decimal_text_f64(value).parse().unwrap();
            assert_eq!(narrowed.to_bits(), expected.to_bits(), "{value:e}");
        }
    }

    #[test]
    fn f64_tie_rounds_to_even_f32() {
        let tie_to_one = 1.0 + f64::from_bits(0x3e70_0000_0000_0000);
        let tie_to_upper = 1.0 + 3.0 * f64::from_bits(0x3e70_0000_0000_0000);

        assert_eq!(Number::F64(tie_to_one).round_to_f32_ties_even(), 1.0);
        assert_eq!(
            Number::F64(tie_to_upper).round_to_f32_ties_even().to_bits(),
            1.0f32.to_bits() + 2
        );
    }

    #[test]
    fn f64_infinities_round_to_f32_infinities() {
        assert_eq!(
            Number::F64(f64::INFINITY).round_to_f32_ties_even(),
            f32::INFINITY
        );
        assert_eq!(
            Number::F64(f64::NEG_INFINITY).round_to_f32_ties_even(),
            f32::NEG_INFINITY
        );
    }

    #[test]
    fn f64_nan_keeps_sign_and_top_payload_bits_in_f32() {
        let signaling_negative_nan = f64::from_bits(0xfff0_0000_2000_0000);

        let narrowed = Number::F64(signaling_negative_nan).round_to_f32_ties_even();

        assert_eq!(narrowed.to_bits(), 0xffc0_0001);
    }

    #[test]
    fn finite_f64_rounds_to_itself() {
        for value in special_f64_values() {
            assert_eq!(
                Number::F64(value).round_to_f64_ties_even().to_bits(),
                value.to_bits()
            );
        }
    }

    #[test]
    fn f64_to_exact_denotes_the_same_value() {
        for value in special_f64_values() {
            let exact = Number::F64(value).to_exact().unwrap();

            let expected = if value == 0.0 { 0.0 } else { value };
            assert_eq!(exact.round_to_f64_ties_even().to_bits(), expected.to_bits());
        }
    }

    #[test]
    fn negative_zero_to_exact_is_integer_zero() {
        assert_eq!(Number::F64(-0.0).to_exact(), Ok(integer(0)));
    }

    #[test]
    fn f64_one_half_to_exact_is_rational() {
        assert_eq!(Number::F64(0.5).to_exact(), Ok(fraction(1, 2)));
    }

    #[test]
    fn smallest_subnormal_to_exact_has_power_of_two_denominator() {
        let expected = Number::fraction(&Integer::one(), &power_of_two(1074)).unwrap();

        assert_eq!(
            Number::F64(f64::from_bits(SMALLEST_SUBNORMAL_F64_BITS)).to_exact(),
            Ok(expected)
        );
    }

    #[test]
    fn f32_max_to_exact_is_integer() {
        let expected = &(&power_of_two(24) - &Integer::one()) * &power_of_two(104);

        assert_eq!(
            Number::F32(f32::MAX).to_exact(),
            Ok(Number::Integer(expected))
        );
    }

    #[test]
    fn neighbour_above_one_to_exact_is_rational() {
        let above_one = f64::from_bits(1.0f64.to_bits() + 1);
        let expected =
            Number::fraction(&(&power_of_two(52) + &Integer::one()), &power_of_two(52)).unwrap();

        assert_eq!(Number::F64(above_one).to_exact(), Ok(expected));
    }

    #[test]
    fn non_finite_values_have_no_exact_value() {
        let values = [
            Number::F64(f64::INFINITY),
            Number::F64(f64::NEG_INFINITY),
            Number::F64(f64::NAN),
            Number::F64(-f64::NAN),
            Number::F32(f32::NAN),
            Number::F32(f32::NEG_INFINITY),
        ];
        for value in values {
            assert_eq!(value.to_exact(), Err(ToExactError::NotFinite));
        }
    }

    #[test]
    fn exact_value_to_exact_is_unchanged() {
        assert_eq!(fraction(-5, 3).to_exact(), Ok(fraction(-5, 3)));
    }
}
