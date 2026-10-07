use crate::integer::Integer;
use crate::natural::Natural;
use crate::number::Number;
use crate::word_conversion::usize_from_u64;

#[derive(Clone, Debug)]
pub(crate) struct Ball {
    center: Integer,
    radius: Natural,
    precision: usize,
}

fn power_of_two(bits: usize) -> Natural {
    Natural::one().shifted_left(bits)
}

fn shifted_integer(value: &Integer, bits: usize) -> Integer {
    Integer::from_sign_and_magnitude(value.is_negative(), value.magnitude().shifted_left(bits))
}

fn divide_rounded(dividend: &Integer, divisor: &Natural) -> Integer {
    let doubled = dividend.magnitude().shifted_left(1).add(divisor);
    let quotient = doubled
        .div_rem(&divisor.shifted_left(1))
        .map(|(quotient, _)| quotient)
        .unwrap_or_default();
    Integer::from_sign_and_magnitude(dividend.is_negative(), quotient)
}

fn divide_ceiling(dividend: &Natural, divisor: &Natural) -> Natural {
    match dividend.div_rem(divisor) {
        Some((quotient, remainder)) if remainder.is_zero() => quotient,
        Some((quotient, _)) => quotient.add(&Natural::one()),
        None => Natural::zero(),
    }
}

impl Ball {
    pub(crate) fn precision(&self) -> usize {
        self.precision
    }

    pub(crate) fn exact_integer(value: &Integer, precision: usize) -> Self {
        Self {
            center: shifted_integer(value, precision),
            radius: Natural::zero(),
            precision,
        }
    }

    pub(crate) fn one(precision: usize) -> Self {
        Self::exact_integer(&Integer::one(), precision)
    }

    pub(crate) fn from_exact(value: &Number, precision: usize) -> Option<Self> {
        match value {
            Number::Integer(integer) => Some(Self::exact_integer(integer, precision)),
            Number::Rational(rational) => {
                let scaled = shifted_integer(rational.numerator(), precision);
                let denominator = rational.denominator().magnitude();
                let remainder_is_zero = scaled
                    .magnitude()
                    .div_rem(denominator)
                    .is_some_and(|(_, remainder)| remainder.is_zero());
                let radius = if remainder_is_zero {
                    Natural::zero()
                } else {
                    Natural::one()
                };
                Some(Self {
                    center: divide_rounded(&scaled, denominator),
                    radius,
                    precision,
                })
            }
            Number::F32(_) | Number::F64(_) => None,
        }
    }

    pub(crate) fn from_f64(value: f64, precision: usize) -> Option<Self> {
        let exact = Number::F64(value).to_exact().ok()?;
        Self::from_exact(&exact, precision)
    }

    pub(crate) fn hull(&self, other: &Self) -> Self {
        let sum = &self.center + &other.center;
        let center = sum
            .div_rem_euclid(&Integer::from(2_i64))
            .map(|(quotient, _)| quotient)
            .unwrap_or(sum);
        let reach = |ball: &Self| (&ball.center - &center).magnitude().add(&ball.radius);
        let (first, second) = (reach(self), reach(other));
        let radius = if first > second { first } else { second };
        Self {
            center,
            radius: radius.add(&Natural::one()),
            precision: self.precision,
        }
    }

    pub(crate) fn widened(&self, units: u64) -> Self {
        Self {
            center: self.center.clone(),
            radius: self.radius.add(&Natural::from_u64(units)),
            precision: self.precision,
        }
    }

    pub(crate) fn negated(&self) -> Self {
        Self {
            center: self.center.negated(),
            radius: self.radius.clone(),
            precision: self.precision,
        }
    }

    pub(crate) fn add(&self, other: &Self) -> Self {
        Self {
            center: &self.center + &other.center,
            radius: self.radius.add(&other.radius),
            precision: self.precision,
        }
    }

    pub(crate) fn sub(&self, other: &Self) -> Self {
        self.add(&other.negated())
    }

    pub(crate) fn mul(&self, other: &Self) -> Self {
        let scale = power_of_two(self.precision);
        let product = &self.center * &other.center;
        let spread = self
            .center
            .magnitude()
            .mul(&other.radius)
            .add(&other.center.magnitude().mul(&self.radius))
            .add(&self.radius.mul(&other.radius));
        Self {
            center: divide_rounded(&product, &scale),
            radius: divide_ceiling(&spread, &scale).add(&Natural::one()),
            precision: self.precision,
        }
    }

    pub(crate) fn mul_integer(&self, factor: &Integer) -> Self {
        Self {
            center: &self.center * factor,
            radius: self.radius.mul(factor.magnitude()),
            precision: self.precision,
        }
    }

    pub(crate) fn div_natural(&self, divisor: &Natural) -> Self {
        Self {
            center: divide_rounded(&self.center, divisor),
            radius: divide_ceiling(&self.radius, divisor).add(&Natural::one()),
            precision: self.precision,
        }
    }

    pub(crate) fn div_small(&self, divisor: u64) -> Self {
        self.div_natural(&Natural::from_u64(divisor))
    }

    pub(crate) fn mul_power_of_two(&self, exponent: i64) -> Self {
        let bits = usize_from_u64(exponent.unsigned_abs());
        if exponent >= 0 {
            return Self {
                center: shifted_integer(&self.center, bits),
                radius: self.radius.shifted_left(bits),
                precision: self.precision,
            };
        }
        self.div_natural(&power_of_two(bits))
    }

    pub(crate) fn div(&self, other: &Self) -> Option<Self> {
        let denominator = other.center.magnitude();
        let margin = denominator.checked_sub(&other.radius)?;
        if margin.is_zero() {
            return None;
        }
        let scaled = shifted_integer(&self.center, self.precision);
        let quotient_magnitude = divide_rounded(&scaled, denominator);
        let center = if other.center.is_negative() {
            quotient_magnitude.negated()
        } else {
            quotient_magnitude
        };
        let spread = self
            .radius
            .mul(denominator)
            .add(&self.center.magnitude().mul(&other.radius))
            .shifted_left(self.precision);
        let radius = divide_ceiling(&spread, &denominator.mul(&margin)).add(&Natural::one());
        Some(Self {
            center,
            radius,
            precision: self.precision,
        })
    }

    pub(crate) fn sqrt(&self) -> Option<Self> {
        let radius_as_integer = Integer::from_sign_and_magnitude(false, self.radius.clone());
        let upper = &self.center + &radius_as_integer;
        if upper.is_negative() {
            return None;
        }
        let lower = &self.center - &radius_as_integer;
        let lower_magnitude = if lower.is_negative() {
            Natural::zero()
        } else {
            lower.magnitude().clone()
        };
        let low_root = lower_magnitude.shifted_left(self.precision).floor_sqrt();
        let upper_scaled = upper.magnitude().shifted_left(self.precision);
        let upper_floor_root = upper_scaled.floor_sqrt();
        let high_root = if upper_floor_root.mul(&upper_floor_root) == upper_scaled {
            upper_floor_root
        } else {
            upper_floor_root.add(&Natural::one())
        };
        let center = low_root.add(&high_root).shifted_right(1);
        let radius = high_root.checked_sub(&center).unwrap_or_default();
        Some(Self {
            center: Integer::from_sign_and_magnitude(false, center),
            radius,
            precision: self.precision,
        })
    }

    pub(crate) fn rounded_quotient_of_centers(&self, other: &Self) -> Integer {
        if other.center.is_zero() {
            return Integer::zero();
        }
        let quotient = divide_rounded(&self.center, other.center.magnitude());
        if other.center.is_negative() {
            quotient.negated()
        } else {
            quotient
        }
    }

    pub(crate) fn is_surely_above(&self, bound: &Integer) -> bool {
        let radius = Integer::from_sign_and_magnitude(false, self.radius.clone());
        &self.center - &radius > shifted_integer(bound, self.precision)
    }

    pub(crate) fn is_surely_below(&self, bound: &Integer) -> bool {
        let radius = Integer::from_sign_and_magnitude(false, self.radius.clone());
        &self.center + &radius < shifted_integer(bound, self.precision)
    }

    pub(crate) fn with_precision(&self, precision: usize) -> Self {
        let extra = precision.saturating_sub(self.precision);
        Self {
            center: shifted_integer(&self.center, extra),
            radius: self.radius.shifted_left(extra),
            precision: self.precision + extra,
        }
    }

    pub(crate) fn reduced_to_precision(&self, precision: usize) -> Self {
        let removed = self.precision.saturating_sub(precision);
        let reduced = self.div_natural(&power_of_two(removed));
        Self {
            center: reduced.center,
            radius: reduced.radius,
            precision: self.precision - removed,
        }
    }

    pub(crate) fn is_surely_positive(&self) -> bool {
        self.is_surely_above(&Integer::zero())
    }

    pub(crate) fn is_surely_nonnegative(&self) -> bool {
        let radius = Integer::from_sign_and_magnitude(false, self.radius.clone());
        !(&self.center - &radius).is_negative()
    }

    pub(crate) fn has_radius_below_power_of_two(&self, exponent: usize) -> bool {
        self.precision
            .checked_sub(exponent)
            .is_some_and(|bits| self.radius < power_of_two(bits))
    }

    pub(crate) fn center_bit_length(&self) -> usize {
        self.center.magnitude().bit_length()
    }

    pub(crate) fn center_square_exceeds_two(&self) -> bool {
        self.center.magnitude().mul(self.center.magnitude()) > power_of_two(2 * self.precision + 1)
    }

    pub(crate) fn absolute(&self) -> Self {
        if self.is_surely_nonnegative() {
            return self.clone();
        }
        let radius = Integer::from_sign_and_magnitude(false, self.radius.clone());
        if (&self.center + &radius).is_negative() {
            return self.negated();
        }
        let highest = self.center.magnitude().add(&self.radius);
        let center = highest.shifted_right(1);
        let radius = highest.checked_sub(&center).unwrap_or_default();
        Self {
            center: Integer::from_sign_and_magnitude(false, center),
            radius,
            precision: self.precision,
        }
    }

    pub(crate) fn exact_bounds(&self) -> Option<(Number, Number)> {
        let scale = Integer::from_sign_and_magnitude(false, power_of_two(self.precision));
        let radius = Integer::from_sign_and_magnitude(false, self.radius.clone());
        Some((
            Number::fraction(&(&self.center - &radius), &scale).ok()?,
            Number::fraction(&(&self.center + &radius), &scale).ok()?,
        ))
    }

    pub(crate) fn has_radius_below_one(&self) -> bool {
        self.radius < power_of_two(self.precision)
    }

    pub(crate) fn floor_of_center(&self) -> Integer {
        let scale = Integer::from_sign_and_magnitude(false, power_of_two(self.precision));
        self.center
            .div_rem_euclid(&scale)
            .map(|(quotient, _)| quotient)
            .unwrap_or_default()
    }

    pub(crate) fn rounded_f64(&self) -> Option<f64> {
        let scale = Integer::from_sign_and_magnitude(false, power_of_two(self.precision));
        let radius = Integer::from_sign_and_magnitude(false, self.radius.clone());
        let lower = Number::fraction(&(&self.center - &radius), &scale).ok()?;
        let upper = Number::fraction(&(&self.center + &radius), &scale).ok()?;
        let lower_rounded = lower.round_to_f64_ties_even();
        let upper_rounded = upper.round_to_f64_ties_even();
        (lower_rounded.to_bits() == upper_rounded.to_bits()).then_some(lower_rounded)
    }

    #[cfg(test)]
    pub(crate) fn bounds(&self) -> (Number, Number) {
        let scale = Integer::from_sign_and_magnitude(false, power_of_two(self.precision));
        let radius = Integer::from_sign_and_magnitude(false, self.radius.clone());
        (
            Number::fraction(&(&self.center - &radius), &scale).unwrap(),
            Number::fraction(&(&self.center + &radius), &scale).unwrap(),
        )
    }

    #[cfg(test)]
    pub(crate) fn contains(&self, value: &Number) -> bool {
        let (lower, upper) = self.bounds();
        let is_at_least_lower = !value.sub_exact(&lower).unwrap().is_negative_exact();
        let is_at_most_upper = !upper.sub_exact(value).unwrap().is_negative_exact();
        is_at_least_lower && is_at_most_upper
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRECISION: usize = 64;

    fn exact(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    fn ball(numerator: i64, denominator: i64) -> Ball {
        Ball::from_exact(&exact(numerator, denominator), PRECISION).unwrap()
    }

    #[test]
    fn dyadic_rational_is_held_exactly() {
        assert!(ball(3, 8).radius.is_zero());
    }

    #[test]
    fn one_third_is_contained_in_its_ball() {
        assert!(ball(1, 3).contains(&exact(1, 3)));
    }

    #[test]
    fn sum_and_difference_contain_exact_values() {
        assert!(ball(1, 3).add(&ball(2, 7)).contains(&exact(13, 21)));
        assert!(ball(1, 3).sub(&ball(2, 7)).contains(&exact(1, 21)));
    }

    #[test]
    fn product_contains_exact_product() {
        let product = ball(1, 3).mul(&ball(-2, 7));

        assert!(product.contains(&exact(-2, 21)));
    }

    #[test]
    fn quotient_contains_exact_quotient() {
        let quotient = ball(1, 3).div(&ball(-5, 7)).unwrap();

        assert!(quotient.contains(&exact(-7, 15)));
    }

    #[test]
    fn division_by_ball_containing_zero_is_none() {
        let around_zero = Ball::one(PRECISION).sub(&Ball::one(PRECISION)).widened(1);

        assert!(ball(1, 3).div(&around_zero).is_none());
    }

    #[test]
    fn square_root_of_perfect_square_is_contained() {
        let root = ball(9, 4).sqrt().unwrap();

        assert!(root.contains(&exact(3, 2)));
    }

    #[test]
    fn square_root_of_negative_ball_is_none() {
        assert!(ball(-1, 2).sqrt().is_none());
    }

    #[test]
    fn division_by_integer_contains_exact_quotient() {
        assert!(ball(2, 3).div_small(7).contains(&exact(2, 21)));
    }

    #[test]
    fn scaling_by_negative_power_of_two_contains_exact_value() {
        assert!(ball(1, 3).mul_power_of_two(-5).contains(&exact(1, 96)));
    }

    #[test]
    fn rounding_of_tight_ball_gives_nearest_f64() {
        let tight = Ball::from_exact(&exact(1, 10), 200).unwrap();

        assert_eq!(
            tight.rounded_f64().map(f64::to_bits),
            Some(0.1f64.to_bits())
        );
    }

    #[test]
    fn rounding_of_wide_ball_is_undecided() {
        assert_eq!(ball(1, 10).widened(1 << 20).rounded_f64(), None);
    }

    #[test]
    fn bounds_compare_against_integers() {
        let seven_halves = ball(7, 2);

        assert!(seven_halves.is_surely_above(&Integer::from(3i64)));
        assert!(seven_halves.is_surely_below(&Integer::from(4i64)));
    }
}
