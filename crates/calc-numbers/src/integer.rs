use std::cmp::Ordering;
use std::ops::{Add, Mul, Neg, Sub};

use crate::natural::Natural;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Integer {
    is_negative: bool,
    magnitude: Natural,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DivisionError {
    DivisionByZero,
}

impl Integer {
    pub fn zero() -> Self {
        Self::from_sign_and_magnitude(false, Natural::zero())
    }

    pub fn one() -> Self {
        Self::from_sign_and_magnitude(false, Natural::one())
    }

    pub(crate) fn from_sign_and_magnitude(is_negative: bool, magnitude: Natural) -> Self {
        Self {
            is_negative: is_negative && !magnitude.is_zero(),
            magnitude,
        }
    }

    pub(crate) fn magnitude(&self) -> &Natural {
        &self.magnitude
    }

    pub fn is_zero(&self) -> bool {
        self.magnitude.is_zero()
    }

    pub fn is_one(&self) -> bool {
        !self.is_negative && self.magnitude.is_one()
    }

    pub fn is_negative(&self) -> bool {
        self.is_negative
    }

    pub fn bit_length(&self) -> u64 {
        u64::try_from(self.magnitude.bit_length()).unwrap_or(u64::MAX)
    }

    pub fn bit_and(&self, other: &Self) -> Option<Self> {
        self.natural_pair(other, |left, right| left & right)
    }

    pub fn bit_or(&self, other: &Self) -> Option<Self> {
        self.natural_pair(other, |left, right| left | right)
    }

    pub fn bit_xor(&self, other: &Self) -> Option<Self> {
        self.natural_pair(other, |left, right| left ^ right)
    }

    fn natural_pair(&self, other: &Self, combine: fn(u32, u32) -> u32) -> Option<Self> {
        (!self.is_negative && !other.is_negative).then(|| {
            Self::from_sign_and_magnitude(false, self.magnitude.bitwise(&other.magnitude, combine))
        })
    }

    pub fn shifted_left(&self, bits: usize) -> Self {
        Self::from_sign_and_magnitude(self.is_negative, self.magnitude.shifted_left(bits))
    }

    pub fn shifted_right(&self, bits: usize) -> Option<Self> {
        (!self.is_negative)
            .then(|| Self::from_sign_and_magnitude(false, self.magnitude.shifted_right(bits)))
    }

    pub fn absolute(&self) -> Self {
        Self::from_sign_and_magnitude(false, self.magnitude.clone())
    }

    pub fn negated(&self) -> Self {
        Self::from_sign_and_magnitude(!self.is_negative, self.magnitude.clone())
    }

    pub fn pow(&self, exponent: u32) -> Self {
        let is_negative = self.is_negative && exponent % 2 == 1;
        Self::from_sign_and_magnitude(is_negative, self.magnitude.pow(exponent))
    }

    pub fn div_rem_euclid(&self, divisor: &Self) -> Result<(Self, Self), DivisionError> {
        let (magnitude_quotient, magnitude_remainder) = self
            .magnitude
            .div_rem(&divisor.magnitude)
            .ok_or(DivisionError::DivisionByZero)?;
        let quotient_is_negative = self.is_negative != divisor.is_negative;
        let quotient = Self::from_sign_and_magnitude(quotient_is_negative, magnitude_quotient);
        let remainder = Self::from_sign_and_magnitude(self.is_negative, magnitude_remainder);
        if !remainder.is_negative() {
            return Ok((quotient, remainder));
        }
        let unit = if divisor.is_negative {
            Self::one()
        } else {
            Self::one().negated()
        };
        Ok((&quotient + &unit, &remainder + &divisor.absolute()))
    }

    pub fn gcd(&self, other: &Self) -> Self {
        Self::from_sign_and_magnitude(false, self.magnitude.gcd(&other.magnitude))
    }

    pub fn to_i64(&self) -> Option<i64> {
        let magnitude = self.magnitude.to_u64()?;
        if self.is_negative {
            0i64.checked_sub_unsigned(magnitude)
        } else {
            i64::try_from(magnitude).ok()
        }
    }

    pub fn to_i128(&self) -> Option<i128> {
        let magnitude = self.magnitude.to_u128()?;
        if self.is_negative {
            0i128.checked_sub_unsigned(magnitude)
        } else {
            i128::try_from(magnitude).ok()
        }
    }
}

impl From<i64> for Integer {
    fn from(value: i64) -> Self {
        Self::from_sign_and_magnitude(value < 0, Natural::from_u64(value.unsigned_abs()))
    }
}

impl From<u64> for Integer {
    fn from(value: u64) -> Self {
        Self::from_sign_and_magnitude(false, Natural::from_u64(value))
    }
}

impl From<i128> for Integer {
    fn from(value: i128) -> Self {
        Self::from_sign_and_magnitude(value < 0, Natural::from_u128(value.unsigned_abs()))
    }
}

impl Ord for Integer {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.is_negative, other.is_negative) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => self.magnitude.cmp(&other.magnitude),
            (true, true) => other.magnitude.cmp(&self.magnitude),
        }
    }
}

impl PartialOrd for Integer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn signed_sum(left: &Integer, right_is_negative: bool, right_magnitude: &Natural) -> Integer {
    if left.is_negative == right_is_negative {
        return Integer::from_sign_and_magnitude(
            left.is_negative,
            left.magnitude.add(right_magnitude),
        );
    }
    match left.magnitude.checked_sub(right_magnitude) {
        Some(difference) => Integer::from_sign_and_magnitude(left.is_negative, difference),
        None => Integer::from_sign_and_magnitude(
            right_is_negative,
            right_magnitude
                .checked_sub(&left.magnitude)
                .unwrap_or_default(),
        ),
    }
}

impl Add for &Integer {
    type Output = Integer;

    fn add(self, other: &Integer) -> Integer {
        signed_sum(self, other.is_negative, &other.magnitude)
    }
}

impl Sub for &Integer {
    type Output = Integer;

    fn sub(self, other: &Integer) -> Integer {
        signed_sum(self, !other.is_negative, &other.magnitude)
    }
}

impl Mul for &Integer {
    type Output = Integer;

    fn mul(self, other: &Integer) -> Integer {
        Integer::from_sign_and_magnitude(
            self.is_negative != other.is_negative,
            self.magnitude.mul(&other.magnitude),
        )
    }
}

impl Neg for &Integer {
    type Output = Integer;

    fn neg(self) -> Integer {
        self.negated()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bit_and_of_twelve_and_ten_is_eight() {
        let twelve = Integer::from(12_u64);
        let ten = Integer::from(10_u64);
        assert_eq!(twelve.bit_and(&ten), Some(Integer::from(8_u64)));
        assert_eq!(twelve.bit_or(&ten), Some(Integer::from(14_u64)));
        assert_eq!(twelve.bit_xor(&ten), Some(Integer::from(6_u64)));
    }

    #[test]
    fn a_negative_integer_has_no_bitwise_and() {
        assert_eq!(Integer::from(-1_i64).bit_and(&Integer::one()), None);
    }

    #[test]
    fn shifts_cross_limb_boundaries() {
        let one = Integer::one();
        assert_eq!(
            one.shifted_left(70).shifted_right(69),
            Some(Integer::from(2_u64))
        );
    }

    use super::*;

    const SAMPLES: [i128; 13] = [
        0,
        1,
        -1,
        7,
        -7,
        9_223_372_036_854_775_807,
        -9_223_372_036_854_775_808,
        18_446_744_073_709_551_615,
        -18_446_744_073_709_551_615,
        1 << 100,
        -(1 << 100),
        (1 << 120) + 12345,
        -((1 << 119) - 999),
    ];

    fn integer(value: i128) -> Integer {
        Integer::from(value)
    }

    #[test]
    fn zero_is_never_negative() {
        assert!(!integer(0).negated().is_negative());
    }

    #[test]
    fn i128_round_trips() {
        for value in SAMPLES {
            assert_eq!(integer(value).to_i128(), Some(value));
        }
    }

    #[test]
    fn i64_extremes_round_trip() {
        assert_eq!(Integer::from(i64::MIN).to_i64(), Some(i64::MIN));
        assert_eq!(Integer::from(i64::MAX).to_i64(), Some(i64::MAX));
    }

    #[test]
    fn value_beyond_i64_has_no_i64() {
        assert_eq!(Integer::from(u64::MAX).to_i64(), None);
    }

    #[test]
    fn addition_matches_i128() {
        for left in SAMPLES {
            for right in SAMPLES {
                assert_eq!(
                    (&integer(left) + &integer(right)).to_i128(),
                    Some(left + right)
                );
            }
        }
    }

    #[test]
    fn subtraction_matches_i128() {
        for left in SAMPLES {
            for right in SAMPLES {
                assert_eq!(
                    (&integer(left) - &integer(right)).to_i128(),
                    Some(left - right)
                );
            }
        }
    }

    #[test]
    fn bit_length_is_the_bit_length_of_the_magnitude() {
        let lengths =
            [0_i64, 1, -1, 255, -256, i64::MIN].map(|value| Integer::from(value).bit_length());

        assert_eq!(lengths, [0, 1, 1, 8, 9, 64]);
    }

    #[test]
    fn multiplication_matches_i128() {
        let small = [
            0i128,
            1,
            -1,
            3,
            -3,
            9_223_372_036_854_775_807,
            -9_223_372_036_854_775_808,
        ];
        for left in small {
            for right in small {
                assert_eq!(
                    (&integer(left) * &integer(right)).to_i128(),
                    Some(left * right)
                );
            }
        }
    }

    #[test]
    fn negation_flips_sign() {
        for value in SAMPLES {
            assert_eq!((-&integer(value)).to_i128(), Some(-value));
        }
    }

    #[test]
    fn absolute_value_is_nonnegative() {
        for value in SAMPLES {
            assert_eq!(integer(value).absolute().to_i128(), Some(value.abs()));
        }
    }

    #[test]
    fn power_of_negative_base_with_odd_exponent_is_negative() {
        assert_eq!(integer(-3).pow(5).to_i128(), Some(-243));
    }

    #[test]
    fn euclidean_division_matches_i128() {
        for dividend in SAMPLES {
            for divisor in SAMPLES.into_iter().filter(|value| *value != 0) {
                let (quotient, remainder) =
                    integer(dividend).div_rem_euclid(&integer(divisor)).unwrap();

                assert_eq!(quotient.to_i128(), Some(dividend.div_euclid(divisor)));
                assert_eq!(remainder.to_i128(), Some(dividend.rem_euclid(divisor)));
            }
        }
    }

    #[test]
    fn euclidean_division_by_zero_is_an_error() {
        assert_eq!(
            integer(5).div_rem_euclid(&integer(0)),
            Err(DivisionError::DivisionByZero)
        );
    }

    #[test]
    fn gcd_is_nonnegative_for_negative_operands() {
        assert_eq!(integer(-12).gcd(&integer(18)), integer(6));
    }

    #[test]
    fn ordering_matches_i128() {
        for left in SAMPLES {
            for right in SAMPLES {
                assert_eq!(integer(left).cmp(&integer(right)), left.cmp(&right));
            }
        }
    }
}
