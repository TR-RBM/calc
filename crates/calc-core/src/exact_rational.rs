use std::cmp::Ordering;

use calc_numbers::{Integer, Number};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ExactRational {
    numerator: Integer,
    denominator: Integer,
}

impl ExactRational {
    pub fn zero() -> Self {
        Self::from_integer(Integer::zero())
    }

    pub fn one() -> Self {
        Self::from_integer(Integer::one())
    }

    pub fn one_half() -> Self {
        Self {
            numerator: Integer::one(),
            denominator: Integer::from(2_u64),
        }
    }

    pub fn from_integer(value: Integer) -> Self {
        Self {
            numerator: value,
            denominator: Integer::one(),
        }
    }

    pub fn from_i64(value: i64) -> Self {
        Self::from_integer(Integer::from(value))
    }

    pub fn fraction(numerator: &Integer, denominator: &Integer) -> Option<Self> {
        match Number::fraction(numerator, denominator).ok()? {
            Number::Integer(integer) => Some(Self::from_integer(integer)),
            Number::Rational(rational) => Some(Self {
                numerator: rational.numerator().clone(),
                denominator: rational.denominator().clone(),
            }),
            Number::F32(_) | Number::F64(_) => None,
        }
    }

    pub fn from_number(number: &Number) -> Option<Self> {
        match number {
            Number::Integer(integer) => Some(Self::from_integer(integer.clone())),
            Number::Rational(rational) => Some(Self {
                numerator: rational.numerator().clone(),
                denominator: rational.denominator().clone(),
            }),
            Number::F32(_) | Number::F64(_) => None,
        }
    }

    pub fn to_number(&self) -> Number {
        if self.denominator.is_one() {
            Number::Integer(self.numerator.clone())
        } else {
            match Number::fraction(&self.numerator, &self.denominator) {
                Ok(number) => number,
                Err(_) => unreachable!(),
            }
        }
    }

    pub fn numerator(&self) -> &Integer {
        &self.numerator
    }

    pub fn denominator(&self) -> &Integer {
        &self.denominator
    }

    pub fn is_zero(&self) -> bool {
        self.numerator.is_zero()
    }

    pub fn is_one(&self) -> bool {
        self.numerator.is_one() && self.denominator.is_one()
    }

    pub fn is_integer(&self) -> bool {
        self.denominator.is_one()
    }

    pub fn sign(&self) -> Ordering {
        if self.numerator.is_zero() {
            Ordering::Equal
        } else if self.numerator.is_negative() {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    }

    fn reduced(numerator: &Integer, denominator: &Integer) -> Self {
        match Self::fraction(numerator, denominator) {
            Some(value) => value,
            None => unreachable!(),
        }
    }

    pub fn plus(&self, other: &Self) -> Self {
        let numerator =
            &(&self.numerator * &other.denominator) + &(&other.numerator * &self.denominator);
        Self::reduced(&numerator, &(&self.denominator * &other.denominator))
    }

    pub fn negated(&self) -> Self {
        Self {
            numerator: self.numerator.negated(),
            denominator: self.denominator.clone(),
        }
    }

    pub fn subtract(&self, other: &Self) -> Self {
        self.plus(&other.negated())
    }

    pub fn multiply(&self, other: &Self) -> Self {
        Self::reduced(
            &(&self.numerator * &other.numerator),
            &(&self.denominator * &other.denominator),
        )
    }

    pub fn reciprocal(&self) -> Option<Self> {
        Self::fraction(&self.denominator, &self.numerator)
    }

    pub fn divide(&self, other: &Self) -> Option<Self> {
        Some(self.multiply(&other.reciprocal()?))
    }

    pub fn absolute(&self) -> Self {
        Self {
            numerator: self.numerator.absolute(),
            denominator: self.denominator.clone(),
        }
    }

    pub fn compare(&self, other: &Self) -> Ordering {
        self.subtract(other).sign()
    }

    pub fn floor(&self) -> Integer {
        match self.numerator.div_rem_euclid(&self.denominator) {
            Ok((quotient, _)) => quotient,
            Err(_) => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fraction(numerator: i64, denominator: i64) -> ExactRational {
        ExactRational::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    #[test]
    fn fraction_is_reduced_with_positive_denominator() {
        let value = fraction(4, -6);

        assert_eq!(
            (value.numerator(), value.denominator()),
            (&Integer::from(-2_i64), &Integer::from(3_i64))
        );
    }

    #[test]
    fn zero_denominator_is_rejected() {
        assert_eq!(
            ExactRational::fraction(&Integer::one(), &Integer::zero()),
            None
        );
    }

    #[test]
    fn sum_of_thirds_and_sixths_is_one_half() {
        assert_eq!(fraction(1, 3).plus(&fraction(1, 6)), fraction(1, 2));
    }

    #[test]
    fn product_reduces() {
        assert_eq!(fraction(2, 3).multiply(&fraction(3, 4)), fraction(1, 2));
    }

    #[test]
    fn reciprocal_of_zero_is_none() {
        assert_eq!(ExactRational::zero().reciprocal(), None);
    }

    #[test]
    fn floor_of_negative_fraction_rounds_toward_negative_infinity() {
        assert_eq!(fraction(-7, 2).floor(), Integer::from(-4_i64));
    }

    #[test]
    fn floor_of_positive_fraction_truncates() {
        assert_eq!(fraction(7, 2).floor(), Integer::from(3_i64));
    }

    #[test]
    fn number_round_trip_keeps_value() {
        let value = fraction(-5, 7);

        assert_eq!(ExactRational::from_number(&value.to_number()), Some(value));
    }

    #[test]
    fn integer_valued_fraction_becomes_integer_number() {
        assert_eq!(fraction(6, 3).to_number(), Number::from(2_i64));
    }
}
