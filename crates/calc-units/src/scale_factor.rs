use calc_numbers::{Integer, Number};

const DECIMAL_BASE: u64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScaleFactorError {
    PiExponentOutOfRange,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ScaleFactor {
    numerator: Integer,
    denominator: Integer,
    pi_exponent: i32,
}

impl ScaleFactor {
    pub fn one() -> Self {
        Self {
            numerator: Integer::one(),
            denominator: Integer::one(),
            pi_exponent: 0,
        }
    }

    pub(crate) fn power_of_ten(exponent: i8) -> Self {
        let magnitude = Integer::from(DECIMAL_BASE).pow(u32::from(exponent.unsigned_abs()));
        if exponent < 0 {
            Self::reduced(&Integer::one(), &magnitude, 0)
        } else {
            Self::reduced(&magnitude, &Integer::one(), 0)
        }
    }

    pub(crate) fn scaled_by_power_of_ten(&self, exponent: i8) -> Self {
        let power = Self::power_of_ten(exponent);
        Self::reduced(
            &(&self.numerator * &power.numerator),
            &(&self.denominator * &power.denominator),
            self.pi_exponent,
        )
    }

    pub(crate) fn scaled_by_power_of_two(&self, exponent: u32) -> Self {
        let power = Integer::from(2_u64).pow(exponent);
        Self::reduced(
            &(&self.numerator * &power),
            &self.denominator,
            self.pi_exponent,
        )
    }

    pub(crate) fn from_positive_ratio(numerator: u64, denominator: u64) -> Self {
        Self::reduced(&Integer::from(numerator), &Integer::from(denominator), 0)
    }

    fn reduced(numerator: &Integer, denominator: &Integer, pi_exponent: i32) -> Self {
        match Number::fraction(numerator, denominator) {
            Ok(Number::Rational(rational)) => Self {
                numerator: rational.numerator().clone(),
                denominator: rational.denominator().clone(),
                pi_exponent,
            },
            Ok(Number::Integer(integer)) => Self {
                numerator: integer,
                denominator: Integer::one(),
                pi_exponent,
            },
            Ok(Number::F32(_) | Number::F64(_)) | Err(_) => {
                unreachable!()
            }
        }
    }

    pub fn numerator(&self) -> &Integer {
        &self.numerator
    }

    pub fn denominator(&self) -> &Integer {
        &self.denominator
    }

    pub fn pi_exponent(&self) -> i32 {
        self.pi_exponent
    }

    pub fn rational_part(&self) -> Number {
        match Number::fraction(&self.numerator, &self.denominator) {
            Ok(number) => number,
            Err(_) => unreachable!(),
        }
    }

    pub fn is_one(&self) -> bool {
        self.numerator.is_one() && self.denominator.is_one() && self.pi_exponent == 0
    }

    pub fn multiply(&self, other: &Self) -> Result<Self, ScaleFactorError> {
        let pi_exponent = self
            .pi_exponent
            .checked_add(other.pi_exponent)
            .ok_or(ScaleFactorError::PiExponentOutOfRange)?;
        Ok(Self::reduced(
            &(&self.numerator * &other.numerator),
            &(&self.denominator * &other.denominator),
            pi_exponent,
        ))
    }

    pub fn reciprocal(&self) -> Result<Self, ScaleFactorError> {
        let pi_exponent = self
            .pi_exponent
            .checked_neg()
            .ok_or(ScaleFactorError::PiExponentOutOfRange)?;
        Ok(Self {
            numerator: self.denominator.clone(),
            denominator: self.numerator.clone(),
            pi_exponent,
        })
    }

    pub fn divide(&self, other: &Self) -> Result<Self, ScaleFactorError> {
        self.multiply(&other.reciprocal()?)
    }

    pub fn power(&self, exponent: i8) -> Result<Self, ScaleFactorError> {
        let pi_exponent = self
            .pi_exponent
            .checked_mul(i32::from(exponent))
            .ok_or(ScaleFactorError::PiExponentOutOfRange)?;
        let magnitude = u32::from(exponent.unsigned_abs());
        let numerator = self.numerator.pow(magnitude);
        let denominator = self.denominator.pow(magnitude);
        if exponent < 0 {
            Ok(Self::reduced(&denominator, &numerator, pi_exponent))
        } else {
            Ok(Self::reduced(&numerator, &denominator, pi_exponent))
        }
    }

    pub(crate) fn with_pi_exponent(numerator: u64, denominator: u64, pi_exponent: i32) -> Self {
        Self::reduced(
            &Integer::from(numerator),
            &Integer::from(denominator),
            pi_exponent,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ratio(numerator: u64, denominator: u64) -> ScaleFactor {
        ScaleFactor::from_positive_ratio(numerator, denominator)
    }

    #[test]
    fn one_is_one() {
        let factor = ScaleFactor::one();

        assert!(factor.is_one());
    }

    #[test]
    fn ratio_is_stored_reduced() {
        let factor = ratio(6, 4);

        assert_eq!(
            (factor.numerator(), factor.denominator()),
            (&Integer::from(3_u64), &Integer::from(2_u64))
        );
    }

    #[test]
    fn integer_ratio_has_denominator_one() {
        let factor = ratio(1000, 1);

        assert!(factor.denominator().is_one());
    }

    #[test]
    fn rational_part_of_integer_factor_is_integer_number() {
        let factor = ratio(1000, 1);

        assert_eq!(factor.rational_part(), Number::from(1000_i64));
    }

    #[test]
    fn rational_part_ignores_pi_exponent() {
        let factor = ScaleFactor::with_pi_exponent(1, 180, 1);

        let expected = Number::fraction(&Integer::from(1_u64), &Integer::from(180_u64)).unwrap();
        assert_eq!(factor.rational_part(), expected);
    }

    #[test]
    fn positive_power_of_ten_is_integer() {
        let factor = ScaleFactor::power_of_ten(3);

        assert_eq!(factor, ratio(1000, 1));
    }

    #[test]
    fn negative_power_of_ten_is_reciprocal() {
        let factor = ScaleFactor::power_of_ten(-6);

        assert_eq!(factor, ratio(1, 1_000_000));
    }

    #[test]
    fn largest_prefix_power_is_exact() {
        let factor = ScaleFactor::power_of_ten(30);

        assert_eq!(
            factor.numerator(),
            &Integer::from(1_000_000_000_000_000_i128 * 1_000_000_000_000_000_i128)
        );
    }

    #[test]
    fn multiply_multiplies_ratios_and_reduces() {
        let left = ratio(2, 3);
        let right = ratio(3, 4);

        let product = left.multiply(&right).unwrap();

        assert_eq!(product, ratio(1, 2));
    }

    #[test]
    fn multiply_adds_pi_exponents() {
        let left = ScaleFactor::with_pi_exponent(1, 180, 1);
        let right = ScaleFactor::with_pi_exponent(1, 1, 2);

        let product = left.multiply(&right).unwrap();

        assert_eq!(product.pi_exponent(), 3);
    }

    #[test]
    fn divide_by_itself_is_one() {
        let factor = ScaleFactor::with_pi_exponent(7, 180, -2);

        let quotient = factor.divide(&factor).unwrap();

        assert!(quotient.is_one());
    }

    #[test]
    fn reciprocal_swaps_ratio_and_negates_pi_exponent() {
        let factor = ScaleFactor::with_pi_exponent(1, 180, 1);

        let reciprocal = factor.reciprocal().unwrap();

        assert_eq!(reciprocal, ScaleFactor::with_pi_exponent(180, 1, -1));
    }

    #[test]
    fn negative_power_inverts_ratio() {
        let factor = ratio(1, 1000);

        let result = factor.power(-2).unwrap();

        assert_eq!(result, ratio(1_000_000, 1));
    }

    #[test]
    fn power_zero_is_one() {
        let factor = ScaleFactor::with_pi_exponent(5, 3, 4);

        let result = factor.power(0).unwrap();

        assert!(result.is_one());
    }

    #[test]
    fn power_multiplies_pi_exponent() {
        let factor = ScaleFactor::with_pi_exponent(1, 180, 1);

        let result = factor.power(-3).unwrap();

        assert_eq!(result, ScaleFactor::with_pi_exponent(5_832_000, 1, -3));
    }

    #[test]
    fn multiply_past_pi_exponent_range_is_an_error() {
        let left = ScaleFactor::with_pi_exponent(1, 1, i32::MAX);
        let right = ScaleFactor::with_pi_exponent(1, 1, 1);

        let result = left.multiply(&right);

        assert_eq!(result, Err(ScaleFactorError::PiExponentOutOfRange));
    }

    #[test]
    fn reciprocal_of_minimum_pi_exponent_is_an_error() {
        let factor = ScaleFactor::with_pi_exponent(1, 1, i32::MIN);

        let result = factor.reciprocal();

        assert_eq!(result, Err(ScaleFactorError::PiExponentOutOfRange));
    }

    #[test]
    fn power_past_pi_exponent_range_is_an_error() {
        let factor = ScaleFactor::with_pi_exponent(1, 1, i32::MAX);

        let result = factor.power(2);

        assert_eq!(result, Err(ScaleFactorError::PiExponentOutOfRange));
    }
}
