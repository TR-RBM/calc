use crate::elementary::{
    acos_f64, asin_f64, atan_f64, atan2_f64, exp_f64, ln_f64, pow_f64, sin_f64, tan_f64,
};
use crate::ieee::{maximum_f64, minimum_f64};
use crate::number::Number;

const WIDE_ANGLE_WIDTH: f64 = 7.0;
const LARGE_ANGLE: f64 = 1.0e15;
const HALF_PI_LOWER: f64 = std::f64::consts::FRAC_PI_2;
const QUARTER_TURNS_SEEN: i64 = 6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    lower: f64,
    upper: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Truth {
    True,
    False,
    Unknown,
}

fn widened_down(value: f64) -> f64 {
    if value == f64::NEG_INFINITY {
        value
    } else {
        value.next_down()
    }
}

fn widened_up(value: f64) -> f64 {
    if value == f64::INFINITY {
        value
    } else {
        value.next_up()
    }
}

fn outward(first: f64, second: f64) -> Option<Interval> {
    if first.is_nan() || second.is_nan() {
        return None;
    }
    Some(Interval {
        lower: widened_down(minimum_f64(first, second)),
        upper: widened_up(maximum_f64(first, second)),
    })
}

const FRACTION_MASK: u64 = (1 << 52) - 1;

fn is_a_power_of_two(value: f64) -> bool {
    value.is_normal() && value.to_bits() & FRACTION_MASK == 0
}

fn is_above_the_smallest_normal(value: f64) -> bool {
    value.is_finite() && value.abs() > f64::MIN_POSITIVE
}

fn sum_is_exact(first: f64, second: f64, sum: f64) -> bool {
    (first == 0.0 || second == 0.0) && sum.is_finite()
}

fn product_is_exact(first: f64, second: f64, product: f64) -> bool {
    if first == 0.0 || second == 0.0 {
        return first.is_finite() && second.is_finite();
    }
    is_above_the_smallest_normal(product) && (is_a_power_of_two(first) || is_a_power_of_two(second))
}

fn quotient_is_exact(dividend: f64, divisor: f64, quotient: f64) -> bool {
    if dividend == 0.0 {
        return divisor.is_finite() && divisor != 0.0;
    }
    is_above_the_smallest_normal(quotient) && is_a_power_of_two(divisor)
}

const EXPONENT_BIAS: i64 = 1023;
const FRACTION_BITS: u32 = 52;
const LOWEST_SUBNORMAL_POWER: i64 = -1074;
const HIGHEST_FINITE_POWER: i64 = 1023;

fn exact_power_of_two(base: &Interval, exponent: &Interval) -> Option<f64> {
    if base.lower != base.upper
        || base.lower <= 0.0
        || exponent.lower != exponent.upper
        || !is_a_power_of_two(base.lower)
    {
        return None;
    }
    let count = exponent.lower;
    if !count.is_finite() || count.trunc() != count || count.abs() > f64::from(u16::MAX) {
        return None;
    }
    let base_power = i64::try_from(base.lower.to_bits() >> FRACTION_BITS).ok()? - EXPONENT_BIAS;
    let power = base_power.checked_mul(count as i64)?;
    if !(LOWEST_SUBNORMAL_POWER..=HIGHEST_FINITE_POWER).contains(&power) {
        return None;
    }
    let value = pow_f64(2.0, power as f64);
    (value != 0.0 && value.is_finite()).then_some(value)
}

fn root_is_exact(radicand: f64) -> bool {
    radicand == 0.0
}

fn exact_or_widened_down(value: f64, is_exact: bool) -> f64 {
    if is_exact { value } else { widened_down(value) }
}

fn exact_or_widened_up(value: f64, is_exact: bool) -> f64 {
    if is_exact { value } else { widened_up(value) }
}

fn exact_hull(candidates: &[(f64, bool)]) -> Option<Interval> {
    if candidates.iter().any(|(value, _)| value.is_nan()) {
        return None;
    }
    let lower = candidates
        .iter()
        .map(|(value, _)| *value)
        .fold(f64::INFINITY, minimum_f64);
    let upper = candidates
        .iter()
        .map(|(value, _)| *value)
        .fold(f64::NEG_INFINITY, maximum_f64);
    let exact_at = |end: f64| {
        candidates
            .iter()
            .filter(|(value, _)| *value == end)
            .all(|(_, is_exact)| *is_exact)
    };
    Some(Interval {
        lower: exact_or_widened_down(lower, exact_at(lower)),
        upper: exact_or_widened_up(upper, exact_at(upper)),
    })
}

fn hull_of(values: &[f64]) -> Option<Interval> {
    if values.iter().any(|value| value.is_nan()) {
        return None;
    }
    let lower = values.iter().copied().fold(f64::INFINITY, minimum_f64);
    let upper = values.iter().copied().fold(f64::NEG_INFINITY, maximum_f64);
    Some(Interval {
        lower: widened_down(lower),
        upper: widened_up(upper),
    })
}

impl Truth {
    pub fn and(self, other: Truth) -> Truth {
        match (self, other) {
            (Truth::False, _) | (_, Truth::False) => Truth::False,
            (Truth::True, Truth::True) => Truth::True,
            _ => Truth::Unknown,
        }
    }

    pub fn or(self, other: Truth) -> Truth {
        match (self, other) {
            (Truth::True, _) | (_, Truth::True) => Truth::True,
            (Truth::False, Truth::False) => Truth::False,
            _ => Truth::Unknown,
        }
    }

    pub fn negated(self) -> Truth {
        match self {
            Truth::True => Truth::False,
            Truth::False => Truth::True,
            Truth::Unknown => Truth::Unknown,
        }
    }
}

impl Interval {
    pub fn lower(&self) -> f64 {
        self.lower
    }

    pub fn upper(&self) -> f64 {
        self.upper
    }

    pub fn point(value: f64) -> Option<Interval> {
        (!value.is_nan()).then_some(Interval {
            lower: value,
            upper: value,
        })
    }

    pub fn from_exact(value: &Number) -> Option<Interval> {
        match value {
            Number::F64(machine) => return Interval::point(*machine),
            Number::F32(machine) => return Interval::point(f64::from(*machine)),
            Number::Integer(_) | Number::Rational(_) => {}
        }
        let rounded = value.round_to_f64_ties_even();
        let is_exact = Number::F64(rounded)
            .to_exact()
            .is_ok_and(|exact| &exact == value);
        if is_exact {
            Interval::point(rounded)
        } else {
            outward(rounded, rounded)
        }
    }

    pub fn contains_zero(&self) -> bool {
        self.lower <= 0.0 && self.upper >= 0.0
    }

    pub fn attained_between(points: &[Interval]) -> Option<Interval> {
        let lowest_upper = points
            .iter()
            .map(|point| point.upper)
            .fold(f64::INFINITY, minimum_f64);
        let highest_lower = points
            .iter()
            .map(|point| point.lower)
            .fold(f64::NEG_INFINITY, maximum_f64);
        (lowest_upper <= highest_lower).then_some(Interval {
            lower: lowest_upper,
            upper: highest_lower,
        })
    }

    pub fn is_bounded(&self) -> bool {
        self.lower.is_finite() && self.upper.is_finite()
    }

    pub fn hull(&self, other: &Interval) -> Interval {
        Interval {
            lower: minimum_f64(self.lower, other.lower),
            upper: maximum_f64(self.upper, other.upper),
        }
    }

    pub fn distance_bound(&self, value: f64) -> Option<f64> {
        if !self.is_bounded() || !value.is_finite() {
            return None;
        }
        let below = widened_up((value - self.lower).abs());
        let above = widened_up((self.upper - value).abs());
        let bound = maximum_f64(below, above);
        bound.is_finite().then_some(bound)
    }

    pub fn neg(&self) -> Interval {
        Interval {
            lower: -self.upper,
            upper: -self.lower,
        }
    }

    pub fn add(&self, other: &Interval) -> Option<Interval> {
        let lower = self.lower + other.lower;
        let upper = self.upper + other.upper;
        if lower.is_nan() || upper.is_nan() {
            return None;
        }
        Some(Interval {
            lower: exact_or_widened_down(lower, sum_is_exact(self.lower, other.lower, lower)),
            upper: exact_or_widened_up(upper, sum_is_exact(self.upper, other.upper, upper)),
        })
    }

    pub fn sub(&self, other: &Interval) -> Option<Interval> {
        self.add(&other.neg())
    }

    pub fn mul(&self, other: &Interval) -> Option<Interval> {
        let has_infinity = !self.is_bounded() || !other.is_bounded();
        if has_infinity && (self.contains_zero() || other.contains_zero()) {
            return None;
        }
        let product = |first: f64, second: f64| {
            let value = first * second;
            (value, product_is_exact(first, second, value))
        };
        exact_hull(&[
            product(self.lower, other.lower),
            product(self.lower, other.upper),
            product(self.upper, other.lower),
            product(self.upper, other.upper),
        ])
    }

    pub fn div(&self, other: &Interval) -> Option<Interval> {
        if other.contains_zero() {
            return None;
        }
        if !self.is_bounded() && !other.is_bounded() {
            return None;
        }
        let quotient = |dividend: f64, divisor: f64| {
            let value = dividend / divisor;
            (value, quotient_is_exact(dividend, divisor, value))
        };
        exact_hull(&[
            quotient(self.lower, other.lower),
            quotient(self.lower, other.upper),
            quotient(self.upper, other.lower),
            quotient(self.upper, other.upper),
        ])
    }

    pub fn mul_add(&self, factor: &Interval, addend: &Interval) -> Option<Interval> {
        self.mul(factor)?.add(addend)
    }

    pub fn abs(&self) -> Interval {
        if self.contains_zero() {
            return Interval {
                lower: 0.0,
                upper: maximum_f64(self.upper, -self.lower),
            };
        }
        let first = self.lower.abs();
        let second = self.upper.abs();
        Interval {
            lower: minimum_f64(first, second),
            upper: maximum_f64(first, second),
        }
    }

    pub fn sqrt(&self) -> Option<Interval> {
        if self.lower < 0.0 {
            return None;
        }
        let lower = self.lower.sqrt();
        let upper = self.upper.sqrt();
        Some(Interval {
            lower: maximum_f64(exact_or_widened_down(lower, root_is_exact(self.lower)), 0.0),
            upper: exact_or_widened_up(upper, root_is_exact(self.upper)),
        })
    }

    pub fn floor(&self) -> Interval {
        Interval {
            lower: self.lower.floor(),
            upper: self.upper.floor(),
        }
    }

    pub fn ceil(&self) -> Interval {
        Interval {
            lower: self.lower.ceil(),
            upper: self.upper.ceil(),
        }
    }

    pub fn trunc(&self) -> Interval {
        Interval {
            lower: self.lower.trunc(),
            upper: self.upper.trunc(),
        }
    }

    pub fn round_ties_even(&self) -> Interval {
        Interval {
            lower: self.lower.round_ties_even(),
            upper: self.upper.round_ties_even(),
        }
    }

    pub fn min(&self, other: &Interval) -> Interval {
        Interval {
            lower: minimum_f64(self.lower, other.lower),
            upper: minimum_f64(self.upper, other.upper),
        }
    }

    pub fn max(&self, other: &Interval) -> Interval {
        Interval {
            lower: maximum_f64(self.lower, other.lower),
            upper: maximum_f64(self.upper, other.upper),
        }
    }

    pub fn copy_sign(&self, sign: &Interval) -> Option<Interval> {
        if sign.lower > 0.0 {
            Some(self.abs())
        } else if sign.upper < 0.0 {
            Some(self.abs().neg())
        } else {
            None
        }
    }

    pub fn exp(&self) -> Interval {
        Interval {
            lower: maximum_f64(widened_down(exp_f64(self.lower)), 0.0),
            upper: widened_up(exp_f64(self.upper)),
        }
    }

    pub fn ln(&self) -> Option<Interval> {
        if self.lower <= 0.0 {
            return None;
        }
        outward(ln_f64(self.lower), ln_f64(self.upper))
    }

    fn quarter_turns_touching(&self) -> Option<Vec<i64>> {
        let lowest = maximum_f64(
            (self.lower / HALF_PI_LOWER.next_up()).floor() - 1.0,
            -LARGE_ANGLE,
        );
        let highest = minimum_f64((self.upper / HALF_PI_LOWER).ceil() + 1.0, LARGE_ANGLE);
        let lowest = Number::F64(lowest).to_exact().ok()?;
        let highest = Number::F64(highest).to_exact().ok()?;
        let (Number::Integer(lowest), Number::Integer(highest)) = (lowest, highest) else {
            return None;
        };
        let (lowest, highest) = (lowest.to_i64()?, highest.to_i64()?);
        let touching = (lowest..=highest)
            .filter(|turns| {
                let Some(position) = Interval::from_exact(&Number::from(*turns)) else {
                    return true;
                };
                let angle = Interval {
                    lower: HALF_PI_LOWER,
                    upper: HALF_PI_LOWER.next_up(),
                }
                .mul(&position);
                angle.is_none_or(|angle| angle.upper >= self.lower && angle.lower <= self.upper)
            })
            .collect();
        Some(touching)
    }

    fn is_wide_angle(&self) -> bool {
        !self.is_bounded()
            || self.upper - self.lower >= WIDE_ANGLE_WIDTH
            || self.lower.abs() >= LARGE_ANGLE
            || self.upper.abs() >= LARGE_ANGLE
    }

    fn trigonometric(&self, value_at: fn(f64) -> f64, extreme_phase: i64) -> Option<Interval> {
        let full = Interval {
            lower: -1.0,
            upper: 1.0,
        };
        if self.is_wide_angle() {
            return Some(full);
        }
        let mut values = vec![value_at(self.lower), value_at(self.upper)];
        let mut enclosure = hull_of(&values)?;
        for turns in self.quarter_turns_touching()? {
            match (turns - extreme_phase).rem_euclid(4) {
                0 => values.push(1.0),
                2 => values.push(-1.0),
                _ => {}
            }
        }
        enclosure = enclosure.hull(&hull_of(&values)?);
        Some(Interval {
            lower: maximum_f64(enclosure.lower, -1.0),
            upper: minimum_f64(enclosure.upper, 1.0),
        })
    }

    pub fn sin(&self) -> Option<Interval> {
        self.trigonometric(sin_f64, 1)
    }

    pub fn cos(&self) -> Option<Interval> {
        self.trigonometric(crate::elementary::cos_f64, 0)
    }

    pub fn sin_attained(&self) -> Option<Interval> {
        self.trigonometric_attained(sin_f64, 1)
    }

    pub fn cos_attained(&self) -> Option<Interval> {
        self.trigonometric_attained(crate::elementary::cos_f64, 0)
    }

    fn quarter_turns_within(&self) -> Option<Vec<i64>> {
        let first = (self.lower / HALF_PI_LOWER.next_up()).floor() - 1.0;
        let Number::Integer(first) = Number::F64(first).to_exact().ok()? else {
            return None;
        };
        let first = first.to_i64()?;
        let within = (first..=first.checked_add(QUARTER_TURNS_SEEN)?)
            .filter(|turns| {
                Interval::from_exact(&Number::from(*turns))
                    .and_then(|position| {
                        Interval {
                            lower: HALF_PI_LOWER,
                            upper: HALF_PI_LOWER.next_up(),
                        }
                        .mul(&position)
                    })
                    .is_some_and(|angle| angle.lower >= self.lower && angle.upper <= self.upper)
            })
            .collect();
        Some(within)
    }

    fn trigonometric_attained(
        &self,
        value_at: fn(f64) -> f64,
        extreme_phase: i64,
    ) -> Option<Interval> {
        if !self.is_bounded()
            || self.lower > self.upper
            || self.lower.abs() >= LARGE_ANGLE
            || self.upper.abs() >= LARGE_ANGLE
        {
            return None;
        }
        let mut points = vec![
            hull_of(&[value_at(self.lower)])?,
            hull_of(&[value_at(self.upper)])?,
        ];
        for turns in self.quarter_turns_within()? {
            match (turns - extreme_phase).rem_euclid(4) {
                0 => points.push(Interval::point(1.0)?),
                2 => points.push(Interval::point(-1.0)?),
                _ => {}
            }
        }
        Interval::attained_between(&points)
    }

    pub fn tan(&self) -> Option<Interval> {
        if self.is_wide_angle() {
            return None;
        }
        let crosses_pole = self
            .quarter_turns_touching()?
            .iter()
            .any(|turns| turns.rem_euclid(2) == 1);
        if crosses_pole {
            return None;
        }
        outward(tan_f64(self.lower), tan_f64(self.upper))
    }

    pub fn asin(&self) -> Option<Interval> {
        if self.lower < -1.0 || self.upper > 1.0 {
            return None;
        }
        outward(asin_f64(self.lower), asin_f64(self.upper))
    }

    pub fn acos(&self) -> Option<Interval> {
        if self.lower < -1.0 || self.upper > 1.0 {
            return None;
        }
        outward(acos_f64(self.upper), acos_f64(self.lower))
    }

    pub fn atan(&self) -> Option<Interval> {
        outward(atan_f64(self.lower), atan_f64(self.upper))
    }

    pub fn atan2(numerator: &Interval, denominator: &Interval) -> Option<Interval> {
        let mut numerator = *numerator;
        if numerator.contains_zero() && denominator.lower <= 0.0 {
            let reaches_the_cut_only_from_above = numerator.lower == 0.0 && denominator.upper < 0.0;
            if !reaches_the_cut_only_from_above {
                return None;
            }
            numerator.lower = 0.0;
        }
        hull_of(&[
            atan2_f64(numerator.lower, denominator.lower),
            atan2_f64(numerator.lower, denominator.upper),
            atan2_f64(numerator.upper, denominator.lower),
            atan2_f64(numerator.upper, denominator.upper),
        ])
    }

    pub fn pow(base: &Interval, exponent: &Interval) -> Option<Interval> {
        if let Some(power) = exact_power_of_two(base, exponent) {
            return Interval::point(power);
        }
        if base.lower > 0.0 || (base.lower == 0.0 && exponent.lower > 0.0) {
            return hull_of(&[
                pow_f64(base.lower, exponent.lower),
                pow_f64(base.lower, exponent.upper),
                pow_f64(base.upper, exponent.lower),
                pow_f64(base.upper, exponent.upper),
            ]);
        }
        let power = exponent.lower;
        let is_integer_exponent =
            exponent.lower == exponent.upper && power.is_finite() && power.trunc() == power;
        if !is_integer_exponent {
            return None;
        }
        if power == 0.0 {
            return Interval::point(1.0);
        }
        let is_even = (power / 2.0).trunc() * 2.0 == power;
        if base.upper < 0.0 {
            let magnitude = Interval::pow(&base.neg(), exponent)?;
            return Some(if is_even { magnitude } else { magnitude.neg() });
        }
        if power < 0.0 {
            return None;
        }
        if is_even {
            return hull_of(&[0.0, pow_f64(-base.lower, power), pow_f64(base.upper, power)]);
        }
        hull_of(&[pow_f64(base.lower, power), pow_f64(base.upper, power)])
    }

    pub fn less(&self, other: &Interval) -> Truth {
        if self.upper < other.lower {
            Truth::True
        } else if self.lower >= other.upper {
            Truth::False
        } else {
            Truth::Unknown
        }
    }

    pub fn less_or_equal(&self, other: &Interval) -> Truth {
        if self.upper <= other.lower {
            Truth::True
        } else if self.lower > other.upper {
            Truth::False
        } else {
            Truth::Unknown
        }
    }

    pub fn equal(&self, other: &Interval) -> Truth {
        let is_same_point =
            self.lower == self.upper && other.lower == other.upper && self.lower == other.lower;
        if is_same_point {
            Truth::True
        } else if self.upper < other.lower || other.upper < self.lower {
            Truth::False
        } else {
            Truth::Unknown
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integer::Integer;
    use crate::test_support::is_at_most;

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    fn interval(lower: f64, upper: f64) -> Interval {
        Interval { lower, upper }
    }

    fn contains_exact(enclosure: &Interval, value: &Number) -> bool {
        let lower = Number::F64(enclosure.lower).to_exact().unwrap();
        let upper = Number::F64(enclosure.upper).to_exact().unwrap();
        is_at_most(&lower, value) && is_at_most(value, &upper)
    }

    #[test]
    fn exact_one_tenth_is_enclosed_by_neighbouring_machine_numbers() {
        let tenth = fraction(1, 10);

        let enclosure = Interval::from_exact(&tenth).unwrap();

        assert!(contains_exact(&enclosure, &tenth));
        assert!(enclosure.lower < enclosure.upper);
    }

    #[test]
    fn exactly_representable_number_is_a_point() {
        let enclosure = Interval::from_exact(&fraction(3, 8)).unwrap();

        assert_eq!(enclosure, interval(0.375, 0.375));
    }

    #[test]
    fn sum_of_rounded_tenths_encloses_three_tenths() {
        let tenth = Interval::from_exact(&fraction(1, 10)).unwrap();
        let fifth = Interval::from_exact(&fraction(1, 5)).unwrap();

        let sum = tenth.add(&fifth).unwrap();

        assert!(contains_exact(&sum, &fraction(3, 10)));
    }

    #[test]
    fn product_of_mixed_signs_takes_extreme_corners() {
        let product = interval(-2.0, 3.0).mul(&interval(-5.0, 1.0)).unwrap();

        assert!(product.lower <= -15.0 && product.upper >= 10.0);
    }

    #[test]
    fn infinity_times_interval_with_zero_is_undefined() {
        assert_eq!(interval(0.0, 1.0).mul(&interval(1.0, f64::INFINITY)), None);
    }

    #[test]
    fn division_by_interval_containing_zero_is_undefined() {
        assert_eq!(interval(1.0, 2.0).div(&interval(-1.0, 1.0)), None);
    }

    #[test]
    fn quotient_encloses_exact_quotient() {
        let third = interval(1.0, 1.0).div(&interval(3.0, 3.0)).unwrap();

        assert!(contains_exact(&third, &fraction(1, 3)));
    }

    #[test]
    fn square_root_of_negative_part_is_undefined() {
        assert_eq!(interval(-1.0, 4.0).sqrt(), None);
    }

    #[test]
    fn absolute_value_of_interval_across_zero_starts_at_zero() {
        assert_eq!(interval(-3.0, 2.0).abs(), interval(0.0, 3.0));
    }

    #[test]
    fn floor_is_applied_to_both_ends() {
        assert_eq!(interval(-0.5, 2.5).floor(), interval(-1.0, 2.0));
    }

    #[test]
    fn copy_sign_with_sign_across_zero_is_undefined() {
        assert_eq!(interval(1.0, 2.0).copy_sign(&interval(-1.0, 1.0)), None);
    }

    #[test]
    fn exponential_encloses_e() {
        let enclosure = interval(1.0, 1.0).exp();

        assert!(enclosure.lower < std::f64::consts::E && std::f64::consts::E < enclosure.upper);
    }

    #[test]
    fn logarithm_of_interval_touching_zero_is_undefined() {
        assert_eq!(interval(0.0, 1.0).ln(), None);
    }

    #[test]
    fn sine_over_maximum_reaches_one() {
        let enclosure = interval(1.5, 1.7).sin().unwrap();

        assert_eq!(enclosure.upper, 1.0);
        assert!(enclosure.lower <= sin_f64(1.7));
    }

    #[test]
    fn cosine_over_pi_reaches_minus_one() {
        let enclosure = interval(3.0, 3.3).cos().unwrap();

        assert_eq!(enclosure.lower, -1.0);
    }

    #[test]
    fn sine_of_wide_interval_is_unit_interval() {
        assert_eq!(interval(0.0, 10.0).sin(), Some(interval(-1.0, 1.0)));
    }

    #[test]
    fn sine_attains_every_value_over_a_full_period() {
        assert_eq!(interval(0.0, 7.0).sin_attained(), Some(interval(-1.0, 1.0)));
    }

    #[test]
    fn sine_attains_every_value_over_a_column_far_from_zero() {
        assert_eq!(
            interval(100_000.0, 100_148.0).sin_attained(),
            Some(interval(-1.0, 1.0))
        );
    }

    #[test]
    fn sine_attains_its_maximum_inside_and_stays_within_its_range() {
        let attained = interval(1.5, 1.7).sin_attained().unwrap();

        assert_eq!(attained.upper, 1.0);
        assert!(attained.lower > sin_f64(1.7) && attained.lower < 1.0);
    }

    #[test]
    fn sine_without_an_extreme_attains_between_its_end_values() {
        let attained = interval(0.1, 0.2).sin_attained().unwrap();

        assert!(attained.lower > sin_f64(0.1) && attained.upper < sin_f64(0.2));
    }

    #[test]
    fn sine_at_one_inexact_point_attains_nothing_proven() {
        assert_eq!(interval(0.5, 0.5).sin_attained(), None);
    }

    #[test]
    fn sine_starting_just_past_a_maximum_finds_the_next_one() {
        let start = std::f64::consts::FRAC_PI_2 + 1.0e-9;

        assert_eq!(
            interval(start, start + 6.4).sin_attained(),
            Some(interval(-1.0, 1.0))
        );
    }

    #[test]
    fn sine_starting_just_past_a_negative_maximum_finds_the_next_one() {
        let start = -3.0 * std::f64::consts::FRAC_PI_2 + 1.0e-9;

        assert_eq!(
            interval(start, start + 6.4).sin_attained(),
            Some(interval(-1.0, 1.0))
        );
    }

    #[test]
    fn sine_starting_on_an_unprovable_maximum_finds_the_next_one() {
        let start = 4001.0 * std::f64::consts::FRAC_PI_2;

        assert_eq!(
            interval(start, start + 6.4).sin_attained(),
            Some(interval(-1.0, 1.0))
        );
    }

    #[test]
    fn cosine_attains_minus_one_over_pi() {
        assert_eq!(interval(3.0, 3.3).cos_attained().unwrap().lower, -1.0);
    }

    #[test]
    fn values_that_do_not_overlap_attain_between_them() {
        let attained = Interval::attained_between(&[interval(0.0, 0.25), interval(0.75, 1.0)]);

        assert_eq!(attained, Some(interval(0.25, 0.75)));
    }

    #[test]
    fn tangent_across_pole_is_undefined() {
        assert_eq!(interval(1.5, 1.6).tan(), None);
    }

    #[test]
    fn tangent_without_pole_is_monotone() {
        let enclosure = interval(0.5, 1.0).tan().unwrap();

        assert!(enclosure.lower < tan_f64(0.5) && enclosure.upper > tan_f64(1.0));
    }

    #[test]
    fn arctangent_across_branch_cut_is_undefined() {
        assert_eq!(
            Interval::atan2(&interval(-1.0, 1.0), &interval(-2.0, -1.0)),
            None
        );
    }

    #[test]
    fn arctangent_in_right_half_plane_takes_corners() {
        let enclosure = Interval::atan2(&interval(-1.0, 1.0), &interval(1.0, 1.0)).unwrap();

        assert!(enclosure.lower < -std::f64::consts::FRAC_PI_4);
        assert!(enclosure.upper > std::f64::consts::FRAC_PI_4);
    }

    #[test]
    fn arcsine_outside_its_domain_is_undefined() {
        assert_eq!(interval(0.5, 2.0).asin(), None);
    }

    #[test]
    fn arcsine_rises_with_its_argument() {
        let enclosure = interval(0.25, 0.5).asin().unwrap();

        assert!(enclosure.lower < asin_f64(0.25) && enclosure.upper > asin_f64(0.5));
    }

    #[test]
    fn arcsine_of_a_point_encloses_the_rounded_value() {
        let enclosure = interval(0.3, 0.3).asin().unwrap();

        assert!(enclosure.lower < asin_f64(0.3) && enclosure.upper > asin_f64(0.3));
    }

    #[test]
    fn arccosine_falls_as_its_argument_rises() {
        let enclosure = interval(0.25, 0.5).acos().unwrap();

        assert!(enclosure.lower < acos_f64(0.5) && enclosure.upper > acos_f64(0.25));
    }

    #[test]
    fn arccosine_outside_its_domain_is_undefined() {
        assert_eq!(interval(-2.0, 0.0).acos(), None);
    }

    #[test]
    fn arctangent_of_an_unbounded_interval_is_bounded_by_the_half_turn() {
        let enclosure = interval(f64::NEG_INFINITY, f64::INFINITY).atan().unwrap();

        assert!(enclosure.lower < -std::f64::consts::FRAC_PI_2);
        assert!(enclosure.upper > std::f64::consts::FRAC_PI_2);
    }

    #[test]
    fn power_of_positive_base_takes_corners() {
        let enclosure = Interval::pow(&interval(2.0, 3.0), &interval(-1.0, 2.0)).unwrap();

        assert!(enclosure.lower < 1.0 / 3.0 && enclosure.upper > 9.0);
    }

    #[test]
    fn power_of_negative_point_with_integer_exponent_is_defined() {
        let enclosure = Interval::pow(&interval(-2.0, -2.0), &interval(3.0, 3.0)).unwrap();

        assert!(enclosure.lower <= -8.0 && enclosure.upper >= -8.0);
    }

    #[test]
    fn square_of_a_negative_interval_equals_the_square_of_its_mirror() {
        let negative = Interval::pow(&interval(-1.0e-5, -0.9e-5), &interval(2.0, 2.0));
        let positive = Interval::pow(&interval(0.9e-5, 1.0e-5), &interval(2.0, 2.0));

        assert_eq!(negative, positive);
    }

    #[test]
    fn cube_of_a_negative_interval_is_the_negated_cube_of_its_mirror() {
        let negative = Interval::pow(&interval(-3.0, -2.0), &interval(3.0, 3.0)).unwrap();
        let positive = Interval::pow(&interval(2.0, 3.0), &interval(3.0, 3.0)).unwrap();

        assert_eq!(negative, positive.neg());
    }

    #[test]
    fn square_of_an_interval_across_zero_starts_at_zero() {
        let enclosure = Interval::pow(&interval(-1.0, 2.0), &interval(2.0, 2.0)).unwrap();

        assert!(enclosure.lower <= 0.0 && enclosure.lower > -1.0e-300 && enclosure.upper >= 4.0);
    }

    #[test]
    fn cube_of_an_interval_across_zero_spans_both_signs() {
        let enclosure = Interval::pow(&interval(-1.0, 2.0), &interval(3.0, 3.0)).unwrap();

        assert!(enclosure.lower <= -1.0 && enclosure.upper >= 8.0);
    }

    #[test]
    fn negative_power_of_an_interval_across_zero_is_undefined() {
        assert_eq!(
            Interval::pow(&interval(-1.0, 1.0), &interval(-2.0, -2.0)),
            None
        );
    }

    #[test]
    fn fractional_power_of_a_negative_interval_is_undefined() {
        assert_eq!(
            Interval::pow(&interval(-2.0, -1.0), &interval(0.5, 0.5)),
            None
        );
    }

    #[test]
    fn comparison_of_disjoint_intervals_is_decided() {
        assert_eq!(interval(0.0, 1.0).less(&interval(2.0, 3.0)), Truth::True);
        assert_eq!(interval(2.0, 3.0).less(&interval(0.0, 1.0)), Truth::False);
    }

    #[test]
    fn comparison_of_overlapping_intervals_is_unknown() {
        assert_eq!(interval(0.0, 2.0).less(&interval(1.0, 3.0)), Truth::Unknown);
    }

    #[test]
    fn equality_of_equal_points_is_true() {
        assert_eq!(interval(1.0, 1.0).equal(&interval(1.0, 1.0)), Truth::True);
    }

    #[test]
    fn unknown_and_false_is_false() {
        assert_eq!(Truth::Unknown.and(Truth::False), Truth::False);
    }

    #[test]
    fn distance_bound_covers_both_ends() {
        let bound = interval(1.0, 2.0).distance_bound(1.25).unwrap();

        assert!(bound >= 0.75);
    }

    #[test]
    fn distance_bound_covers_the_lower_end() {
        let bound = interval(1.0, 2.0).distance_bound(1.75).unwrap();

        assert!(bound >= 0.75);
    }

    #[test]
    fn sum_is_widened_outward() {
        let sum = interval(0.1, 0.1).add(&interval(0.2, 0.2)).unwrap();

        assert!(sum.lower < 0.1 + 0.2 && sum.upper > 0.1 + 0.2);
    }

    #[test]
    fn distance_bound_of_unbounded_interval_is_none() {
        assert_eq!(interval(1.0, f64::INFINITY).distance_bound(1.0), None);
    }

    fn point(value: f64) -> Interval {
        Interval::point(value).unwrap()
    }

    fn is_the_point(enclosure: &Interval, value: f64) -> bool {
        enclosure.lower == value && enclosure.upper == value
    }

    #[test]
    fn a_product_with_an_exact_zero_factor_stays_zero() {
        assert!(is_the_point(&point(0.0).mul(&point(3.0)).unwrap(), 0.0));
        assert!(is_the_point(&point(0.0).mul(&point(1e-310)).unwrap(), 0.0));
    }

    #[test]
    fn a_sum_with_zero_and_a_scaling_by_a_power_of_two_are_not_widened() {
        assert!(is_the_point(&point(0.1).add(&point(0.0)).unwrap(), 0.1));
        assert!(is_the_point(&point(0.0).sub(&point(0.1)).unwrap(), -0.1));
        assert!(is_the_point(&point(0.1).mul(&point(4.0)).unwrap(), 0.4));
        assert!(is_the_point(&point(0.1).div(&point(0.5)).unwrap(), 0.2));
        assert!(is_the_point(&point(1.0).div(&point(1.0)).unwrap(), 1.0));
        assert!(is_the_point(&point(0.0).div(&point(3.0)).unwrap(), 0.0));
        assert!(is_the_point(&point(0.0).sqrt().unwrap(), 0.0));
    }

    #[test]
    fn a_scaling_that_reaches_the_smallest_normal_only_by_rounding_is_widened() {
        let all_ones = f64::from_bits(0x3fef_ffff_ffff_ffff);
        let exact_product = Number::F64(all_ones)
            .to_exact()
            .unwrap()
            .mul_exact(&Number::F64(f64::MIN_POSITIVE).to_exact().unwrap())
            .unwrap();
        for sign in [1.0, -1.0] {
            let value = point(sign * all_ones);
            let product = value.mul(&point(f64::MIN_POSITIVE)).unwrap();
            let quotient = value
                .div(&point(f64::from_bits(0x7fd0_0000_0000_0000)))
                .unwrap();
            let exact = if sign > 0.0 {
                exact_product.clone()
            } else {
                Number::from(0_i64).sub_exact(&exact_product).unwrap()
            };
            assert!(contains_where_finite(&product, &exact));
            assert!(contains_where_finite(&quotient, &exact));
        }
    }

    #[test]
    fn a_scaling_into_the_subnormal_range_is_widened() {
        let enclosure = point(f64::from_bits(3)).mul(&point(0.5)).unwrap();
        assert!(enclosure.lower < enclosure.upper);
    }

    #[test]
    fn inexact_results_are_still_widened() {
        for enclosure in [
            point(0.1).add(&point(0.2)).unwrap(),
            point(1.0).div(&point(3.0)).unwrap(),
            point(0.1).mul(&point(0.1)).unwrap(),
            point(2.0).sqrt().unwrap(),
        ] {
            assert!(enclosure.lower < enclosure.upper);
        }
    }

    #[test]
    fn a_product_that_underflows_is_widened_even_where_its_residual_rounds_to_zero() {
        let tiny = f64::from_bits(0x1a70_0000_0000_0000);
        let enclosure = point(tiny).mul(&point(tiny)).unwrap();
        assert!(enclosure.lower < 0.0 && enclosure.upper > 0.0);
    }

    #[test]
    fn atan2_on_the_cut_reached_from_above_encloses_pi() {
        let numerator = Interval {
            lower: -0.0,
            upper: f64::from_bits(2),
        };
        let enclosure = Interval::atan2(&numerator, &point(-1.0)).unwrap();
        assert!(enclosure.lower <= std::f64::consts::PI && std::f64::consts::PI <= enclosure.upper);
        assert!(enclosure.upper - enclosure.lower < 1e-15);
    }

    #[test]
    fn atan2_across_the_cut_has_no_enclosure() {
        let numerator = Interval {
            lower: -1e-300,
            upper: 1e-300,
        };
        assert!(Interval::atan2(&numerator, &point(-1.0)).is_none());
    }

    #[test]
    fn atan2_with_the_origin_inside_has_no_enclosure() {
        let numerator = Interval {
            lower: 0.0,
            upper: 1.0,
        };
        let denominator = Interval {
            lower: -1.0,
            upper: 1.0,
        };
        assert!(Interval::atan2(&numerator, &denominator).is_none());
    }

    #[test]
    fn a_representable_power_of_two_raised_to_a_whole_number_is_exact() {
        let enclosure = Interval::pow(&point(2.0), &point(-1074.0)).unwrap();
        assert_eq!(
            (enclosure.lower, enclosure.upper),
            (f64::from_bits(1), f64::from_bits(1))
        );
        let enclosure = Interval::pow(&point(4.0), &point(511.0)).unwrap();
        assert_eq!(enclosure.lower, enclosure.upper);
    }

    #[test]
    fn a_power_of_two_raised_past_the_range_is_widened() {
        let enclosure = Interval::pow(&point(2.0), &point(-1075.0)).unwrap();
        assert!(enclosure.lower < enclosure.upper);
    }

    #[test]
    fn a_sum_that_overflows_is_widened_to_the_largest_finite_value() {
        let half_ulp = f64::from_bits(0x7c90_0000_0000_0000);
        let enclosure = point(f64::MAX).add(&point(half_ulp)).unwrap();
        assert_eq!(enclosure.lower, f64::MAX);
        assert_eq!(enclosure.upper, f64::INFINITY);
    }

    fn samples() -> Vec<f64> {
        let mut values = vec![
            0.0,
            -0.0,
            1.0,
            -1.0,
            2.0,
            3.0,
            0.5,
            0.1,
            f64::MAX,
            -f64::MAX,
            f64::MIN_POSITIVE,
            f64::from_bits(1),
            f64::from_bits(2),
            f64::from_bits(0x000f_ffff_ffff_ffff),
            f64::from_bits(0x1a70_0000_0000_0000),
            f64::from_bits(0x7c90_0000_0000_0000),
            f64::from_bits(0x3cb0_0000_0000_0000),
            f64::from_bits(0x3fef_ffff_ffff_ffff),
            f64::from_bits(0x0010_0000_0000_0001),
            f64::from_bits(0x7fd0_0000_0000_0000),
        ];
        let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
        while values.len() < 60 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let value = f64::from_bits(state);
            if value.is_finite() {
                values.push(value);
            }
        }
        values
    }

    fn contains_where_finite(enclosure: &Interval, value: &Number) -> bool {
        let lower_holds = !enclosure.lower.is_finite()
            || is_at_most(&Number::F64(enclosure.lower).to_exact().unwrap(), value);
        let upper_holds = !enclosure.upper.is_finite()
            || is_at_most(value, &Number::F64(enclosure.upper).to_exact().unwrap());
        lower_holds && upper_holds
    }

    #[test]
    fn every_basic_operation_encloses_its_exact_result_on_hard_operands() {
        let values = samples();
        for first in &values {
            let exact_first = Number::F64(*first).to_exact().unwrap();
            for second in &values {
                let exact_second = Number::F64(*second).to_exact().unwrap();
                let (one, two) = (point(*first), point(*second));
                let sum = one.add(&two).unwrap();
                assert!(
                    contains_where_finite(&sum, &exact_first.add_exact(&exact_second).unwrap()),
                    "{first:e} + {second:e}"
                );
                let difference = one.sub(&two).unwrap();
                assert!(
                    contains_where_finite(
                        &difference,
                        &exact_first.sub_exact(&exact_second).unwrap()
                    ),
                    "{first:e} - {second:e}"
                );
                let product = one.mul(&two).unwrap();
                assert!(
                    contains_where_finite(&product, &exact_first.mul_exact(&exact_second).unwrap()),
                    "{first:e} * {second:e}"
                );
                if *second != 0.0 {
                    let quotient = one.div(&two).unwrap();
                    assert!(
                        contains_where_finite(
                            &quotient,
                            &exact_first.div_exact(&exact_second).unwrap()
                        ),
                        "{first:e} / {second:e}"
                    );
                }
            }
            if *first >= 0.0 {
                let root = point(*first).sqrt().unwrap();
                let lower = Number::F64(root.lower).to_exact().unwrap();
                let upper = Number::F64(root.upper).to_exact().unwrap();
                assert!(
                    is_at_most(&lower.mul_exact(&lower).unwrap(), &exact_first)
                        && is_at_most(&exact_first, &upper.mul_exact(&upper).unwrap()),
                    "sqrt({first:e})"
                );
            }
        }
    }
}
