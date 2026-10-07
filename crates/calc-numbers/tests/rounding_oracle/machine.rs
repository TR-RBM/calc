use std::cmp::Ordering;

use calc_numbers::{Integer, Number};

use crate::arithmetic::{Arithmetic, Point, UNBOUNDED_MAGNITUDE_EXPONENT, ZERO_MAGNITUDE_EXPONENT};
use crate::bits::{binary_exponent, integral_to_i64, odd_significand_and_exponent, power_of_two};
use crate::dyadic::DyadicArithmetic;

const CONSTANT_PRECISION: u32 = 256;
const WINDOW_PRECISION: u32 = 512;
const LOWEST_WINDOW_EXPONENT: i32 = -53;
const HIGHEST_WINDOW_EXPONENT: i32 = 127;
const TURN_FRACTION_BITS: u32 = 124;
const TURN_WINDOW_BITS: u32 = 126;
const TURN_SCALE_EXPONENT: i32 = -124;
const SMALL_ARGUMENT: f64 = 0.75;
const LIMB_BITS: u32 = 32;
const LIMB_EXPONENT: i32 = 32;
const LIMB_COUNT: u32 = 4;
const LIMB_MASK: u128 = 0xffff_ffff;
const SCALING_STEP_EXPONENT: i32 = 1000;
const TRUNCATION_EXPONENT: i32 = -60;
const QUADRANT_MASK: u128 = 3;
const QUADRANT_COUNT: u8 = 4;
const HIGHEST_RADIUS_EXPONENT: i32 = 1023;
const LOWEST_RADIUS_EXPONENT: i32 = -1074;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MachineInterval {
    pub lower: f64,
    pub upper: f64,
}

impl MachineInterval {
    pub fn point(value: f64) -> Self {
        Self {
            lower: value,
            upper: value,
        }
    }

    fn whole() -> Self {
        Self {
            lower: f64::NEG_INFINITY,
            upper: f64::INFINITY,
        }
    }

    fn outward(lower: f64, upper: f64) -> Self {
        if lower.is_nan() || upper.is_nan() {
            return Self::whole();
        }
        Self {
            lower: lower.next_down(),
            upper: upper.next_up(),
        }
    }
}

fn smallest_of(values: [f64; 4]) -> f64 {
    values.into_iter().fold(f64::INFINITY, |smallest, value| {
        if value < smallest { value } else { smallest }
    })
}

fn largest_of(values: [f64; 4]) -> f64 {
    values
        .into_iter()
        .fold(f64::NEG_INFINITY, |largest, value| {
            if value > largest { value } else { largest }
        })
}

fn has_nan(values: [f64; 4]) -> bool {
    values.iter().any(|value| value.is_nan())
}

pub struct MachineArithmetic {
    pi: MachineInterval,
    ln2: MachineInterval,
    windows: Vec<u128>,
}

impl MachineArithmetic {
    pub fn new() -> Self {
        let constants = DyadicArithmetic::new(CONSTANT_PRECISION);
        let pi = Self::from_dyadic(&constants, &constants.pi());
        let ln2 = Self::from_dyadic(&constants, &constants.ln2());
        let window_source = DyadicArithmetic::new(WINDOW_PRECISION);
        let windows = (LOWEST_WINDOW_EXPONENT..=HIGHEST_WINDOW_EXPONENT)
            .map(|exponent| window_source.quarter_turn_window(exponent))
            .collect();
        Self { pi, ln2, windows }
    }

    fn from_dyadic(
        source: &DyadicArithmetic,
        value: &crate::dyadic::DyadicInterval,
    ) -> MachineInterval {
        let (lower, upper) = source.bounds(value);
        MachineInterval {
            lower: lower.round_to_f64_ties_even().next_down(),
            upper: upper.round_to_f64_ties_even().next_up(),
        }
    }

    fn interval_from_units(&self, units: i128) -> MachineInterval {
        let magnitude = units.unsigned_abs();
        let mut accumulated = MachineInterval::point(0.0);
        for index in (0..LIMB_COUNT).rev() {
            let limb =
                u32::try_from((magnitude >> (index * LIMB_BITS)) & LIMB_MASK).expect("limb fits");
            let shifted = self.times_power_of_two(&accumulated, LIMB_EXPONENT);
            accumulated = self.add(&shifted, &MachineInterval::point(f64::from(limb)));
        }
        if units < 0 {
            self.negate(&accumulated)
        } else {
            accumulated
        }
    }

    fn hull(left: &MachineInterval, right: &MachineInterval) -> MachineInterval {
        MachineInterval {
            lower: if left.lower < right.lower {
                left.lower
            } else {
                right.lower
            },
            upper: if left.upper > right.upper {
                left.upper
            } else {
                right.upper
            },
        }
    }
}

impl Arithmetic for MachineArithmetic {
    type Value = MachineInterval;

    fn exact(&self, value: f64) -> MachineInterval {
        MachineInterval::point(value)
    }

    fn integer(&self, value: &Integer) -> MachineInterval {
        if let Some(small) = value.to_i64().and_then(|small| i32::try_from(small).ok()) {
            return MachineInterval::point(f64::from(small));
        }
        let rounded = Number::Integer(value.clone()).round_to_f64_ties_even();
        MachineInterval::outward(rounded, rounded)
    }

    fn small_integer(&self, value: i64) -> MachineInterval {
        match i32::try_from(value) {
            Ok(small) => MachineInterval::point(f64::from(small)),
            Err(_) => self.integer(&Integer::from(value)),
        }
    }

    fn add(&self, left: &MachineInterval, right: &MachineInterval) -> MachineInterval {
        MachineInterval::outward(left.lower + right.lower, left.upper + right.upper)
    }

    fn sub(&self, left: &MachineInterval, right: &MachineInterval) -> MachineInterval {
        MachineInterval::outward(left.lower - right.upper, left.upper - right.lower)
    }

    fn mul(&self, left: &MachineInterval, right: &MachineInterval) -> MachineInterval {
        let products = [
            left.lower * right.lower,
            left.lower * right.upper,
            left.upper * right.lower,
            left.upper * right.upper,
        ];
        if has_nan(products) {
            return MachineInterval::whole();
        }
        MachineInterval::outward(smallest_of(products), largest_of(products))
    }

    fn div(
        &self,
        numerator: &MachineInterval,
        denominator: &MachineInterval,
    ) -> Option<MachineInterval> {
        if !(denominator.lower > 0.0 || denominator.upper < 0.0) {
            return None;
        }
        let quotients = [
            numerator.lower / denominator.lower,
            numerator.lower / denominator.upper,
            numerator.upper / denominator.lower,
            numerator.upper / denominator.upper,
        ];
        if has_nan(quotients) {
            return Some(MachineInterval::whole());
        }
        Some(MachineInterval::outward(
            smallest_of(quotients),
            largest_of(quotients),
        ))
    }

    fn negate(&self, value: &MachineInterval) -> MachineInterval {
        MachineInterval {
            lower: -value.upper,
            upper: -value.lower,
        }
    }

    fn square_root(&self, value: &MachineInterval) -> Option<MachineInterval> {
        if value.lower.is_nan() || value.lower < 0.0 {
            return None;
        }
        Some(MachineInterval::outward(
            value.lower.sqrt(),
            value.upper.sqrt(),
        ))
    }

    fn times_power_of_two(&self, value: &MachineInterval, exponent: i32) -> MachineInterval {
        let mut scaled = *value;
        let mut remaining = exponent;
        while remaining != 0 {
            let step = remaining.clamp(-SCALING_STEP_EXPONENT, SCALING_STEP_EXPONENT);
            let factor = power_of_two(step);
            scaled = MachineInterval::outward(scaled.lower * factor, scaled.upper * factor);
            remaining -= step;
        }
        scaled
    }

    fn widen(&self, value: &MachineInterval, radius_exponent: i32) -> MachineInterval {
        if radius_exponent > HIGHEST_RADIUS_EXPONENT {
            return MachineInterval::whole();
        }
        let radius = power_of_two(radius_exponent.max(LOWEST_RADIUS_EXPONENT));
        MachineInterval::outward(value.lower - radius, value.upper + radius)
    }

    fn magnitude_exponent(&self, value: &MachineInterval) -> i32 {
        let lower = value.lower.abs();
        let upper = value.upper.abs();
        let magnitude = if lower > upper { lower } else { upper };
        if magnitude == 0.0 {
            return ZERO_MAGNITUDE_EXPONENT;
        }
        match binary_exponent(magnitude) {
            Some(exponent) => exponent + 1,
            None => UNBOUNDED_MAGNITUDE_EXPONENT,
        }
    }

    fn nearest_small_integer(&self, value: &MachineInterval) -> Option<i64> {
        integral_to_i64(value.lower.round_ties_even())
    }

    fn quarter_turns(&self, value: f64) -> Option<(MachineInterval, u8)> {
        if value.abs() < SMALL_ARGUMENT {
            return Some((MachineInterval::point(value), 0));
        }
        let (significand, exponent) = odd_significand_and_exponent(value.abs())?;
        let index = usize::try_from(exponent - LOWEST_WINDOW_EXPONENT).ok()?;
        let window = *self.windows.get(index)?;
        let window_mask = (1u128 << TURN_WINDOW_BITS) - 1;
        let fraction_mask = (1u128 << TURN_FRACTION_BITS) - 1;
        let half_turn_unit = 1u128 << (TURN_FRACTION_BITS - 1);
        let product = u128::from(significand).wrapping_mul(window) & window_mask;
        let shifted = product.wrapping_add(half_turn_unit) & window_mask;
        let quadrant = u8::try_from((shifted >> TURN_FRACTION_BITS) & QUADRANT_MASK).ok()?;
        let offset = (shifted & fraction_mask).cast_signed() - half_turn_unit.cast_signed();
        let lower = self.interval_from_units(offset);
        let upper = self.interval_from_units(offset + i128::from(significand));
        let turns = self.times_power_of_two(&Self::hull(&lower, &upper), TURN_SCALE_EXPONENT);
        let half_pi = self.times_power_of_two(&self.pi, -1);
        let reduced = self.mul(&turns, &half_pi);
        if value < 0.0 {
            Some((
                self.negate(&reduced),
                (QUADRANT_COUNT - quadrant) % QUADRANT_COUNT,
            ))
        } else {
            Some((reduced, quadrant))
        }
    }

    fn pi(&self) -> MachineInterval {
        self.pi
    }

    fn ln2(&self) -> MachineInterval {
        self.ln2
    }

    fn truncation_exponent(&self) -> i32 {
        TRUNCATION_EXPONENT
    }

    fn compare_lower(&self, value: &MachineInterval, point: &Point) -> Option<Ordering> {
        value.lower.partial_cmp(&point.machine?)
    }

    fn compare_upper(&self, value: &MachineInterval, point: &Point) -> Option<Ordering> {
        value.upper.partial_cmp(&point.machine?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contains(interval: &MachineInterval, value: f64) -> bool {
        interval.lower <= value && value <= interval.upper
    }

    #[test]
    fn pi_interval_contains_standard_constant() {
        let arithmetic = MachineArithmetic::new();

        let pi = arithmetic.pi();

        assert!(
            contains(&pi, std::f64::consts::PI) && pi.upper.next_down().next_down() <= pi.lower
        );
    }

    #[test]
    fn product_widens_outward_by_one_step() {
        let arithmetic = MachineArithmetic::new();

        let product = arithmetic.mul(&MachineInterval::point(3.0), &MachineInterval::point(0.1));

        assert!(product.lower < 0.30000000000000004 && product.upper > 0.30000000000000004);
    }

    #[test]
    fn overflowing_product_keeps_largest_finite_lower_bound() {
        let arithmetic = MachineArithmetic::new();

        let product = arithmetic.mul(
            &MachineInterval::point(f64::MAX),
            &MachineInterval::point(2.0),
        );

        assert!(product.lower == f64::MAX && product.upper == f64::INFINITY);
    }

    #[test]
    fn division_by_interval_containing_zero_is_refused() {
        let arithmetic = MachineArithmetic::new();
        let straddling = MachineInterval {
            lower: -1.0,
            upper: 1.0,
        };

        let quotient = arithmetic.div(&MachineInterval::point(1.0), &straddling);

        assert_eq!(quotient, None);
    }

    #[test]
    fn units_beyond_sixty_four_bits_convert_within_one_step() {
        let arithmetic = MachineArithmetic::new();
        let units = (1i128 << 100) + 12345;

        let interval = arithmetic.interval_from_units(-units);

        assert!(contains(&interval, -power_of_two(100)));
    }

    #[test]
    fn quarter_turns_agree_with_dyadic_reduction_for_large_argument() {
        let machine = MachineArithmetic::new();
        let dyadic = DyadicArithmetic::new(300);
        let argument = f64::from(f32::MAX);

        let (machine_reduced, machine_quadrant) =
            machine.quarter_turns(argument).expect("in table");
        let (dyadic_reduced, dyadic_quadrant) = dyadic.quarter_turns(argument).expect("precise");
        let (dyadic_lower, dyadic_upper) = dyadic.bounds(&dyadic_reduced);

        assert!(
            machine_quadrant == dyadic_quadrant
                && machine_reduced.lower <= dyadic_lower.round_to_f64_ties_even()
                && dyadic_upper.round_to_f64_ties_even() <= machine_reduced.upper
        );
    }

    #[test]
    fn quarter_turns_cover_largest_power_of_two_in_f32() {
        let arithmetic = MachineArithmetic::new();

        let reduction = arithmetic.quarter_turns(power_of_two(127));

        assert!(reduction.is_some());
    }

    #[test]
    fn quarter_turns_of_negative_argument_mirror_the_quadrant() {
        let arithmetic = MachineArithmetic::new();

        let (_, quadrant) = arithmetic.quarter_turns(-2.0).expect("in table");

        assert_eq!(quadrant, 3);
    }

    #[test]
    fn integer_beyond_i32_is_enclosed() {
        let arithmetic = MachineArithmetic::new();

        let interval = arithmetic.small_integer(1 << 40);

        assert!(contains(&interval, power_of_two(40)));
    }
}
