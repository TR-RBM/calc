use std::cmp::Ordering;

use calc_numbers::{Integer, Number};

use crate::exact_order::compare_exact;
use crate::primitive::Column;
use crate::record::Interval;
use crate::view::{AxisUnit, Dimension};

const QUANTILE_DIVISOR: usize = 50;
const MOST_ROUND_STEPS: i64 = 5;
const STEP_DIGITS: [i64; 3] = [1, 2, 5];
const TEN: i64 = 10;
const MEDIAN_WINDOW: f64 = 1e-12;
const LOG_TWO_BELOW: i64 = 30_102;
const LOG_TWO_ABOVE: i64 = 30_103;
const LOG_SCALE: i64 = 100_000;
const STEP_DECADES: i64 = 8;

fn integer(value: i64) -> Number {
    Number::from(value)
}

fn interval(lower: i64, upper: i64) -> Interval {
    Interval {
        lower: integer(lower),
        upper: integer(upper),
    }
}

fn half() -> Number {
    Number::fraction(&Integer::one(), &Integer::from(2_i64)).unwrap_or_else(|_| integer(1))
}

pub(crate) fn default_axis_interval(
    dimension: Dimension,
    unit: &AxisUnit,
    is_complex: bool,
) -> Interval {
    if is_complex {
        return interval(-2, 2);
    }
    let is_temperature = dimension == Dimension::TEMPERATURE
        && matches!(
            unit,
            AxisUnit::Coherent { .. } | AxisUnit::TemperatureScale { .. }
        );
    if is_temperature {
        interval(0, 100)
    } else {
        interval(-10, 10)
    }
}

fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

fn exact(value: f64) -> Option<Number> {
    Number::F64(value).to_exact().ok()
}

pub(crate) fn finite_values(columns: &[&Column], is_left_out: &dyn Fn(usize) -> bool) -> Vec<f64> {
    let mut values = Vec::new();
    for column in columns {
        match column {
            Column::F32(column_values) => values.extend(
                column_values
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| !is_left_out(*index))
                    .filter_map(|(_, value)| finite(f64::from(*value))),
            ),
            Column::F64(column_values) => values.extend(
                column_values
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| !is_left_out(*index))
                    .filter_map(|(_, value)| finite(*value)),
            ),
        }
    }
    values
}

fn decade_lower_bound(value: &Number) -> Option<i64> {
    let (numerator, denominator) = match value {
        Number::Integer(integer) => (integer.clone(), Integer::one()),
        Number::Rational(rational) => {
            (rational.numerator().clone(), rational.denominator().clone())
        }
        Number::F32(_) | Number::F64(_) => return None,
    };
    let binary = i64::try_from(numerator.bit_length()).ok()?
        - 1
        - i64::try_from(denominator.bit_length()).ok()?;
    let scale = if binary >= 0 {
        LOG_TWO_BELOW
    } else {
        LOG_TWO_ABOVE
    };
    Some((binary * scale).div_euclid(LOG_SCALE) - 1)
}

fn first_step(lowest_decade: i64, covers: &dyn Fn(&Number) -> Option<bool>) -> Option<Number> {
    for exponent in lowest_decade..lowest_decade.saturating_add(STEP_DECADES) {
        for digit in STEP_DIGITS {
            let step = power_of_ten(exponent)?.mul_exact(&integer(digit)).ok()?;
            if covers(&step)? {
                return Some(step);
            }
        }
    }
    None
}

fn power_of_ten(exponent: i64) -> Option<Number> {
    let magnitude = u32::try_from(exponent.unsigned_abs()).ok()?;
    let power = Integer::from(TEN).pow(magnitude);
    if exponent >= 0 {
        Some(Number::from(power))
    } else {
        Number::fraction(&Integer::one(), &power).ok()
    }
}

fn floor_integer(value: &Number) -> Option<Integer> {
    match value {
        Number::Integer(integer) => Some(integer.clone()),
        Number::Rational(rational) => rational
            .numerator()
            .div_rem_euclid(rational.denominator())
            .ok()
            .map(|(quotient, _)| quotient),
        Number::F32(_) | Number::F64(_) => None,
    }
}

fn ceil_integer(value: &Number) -> Option<Integer> {
    floor_integer(&value.negate_exact().ok()?).map(|floor| floor.negated())
}

pub(crate) fn round_step(width: &Number) -> Option<Number> {
    let target = width.div_exact(&integer(MOST_ROUND_STEPS)).ok()?;
    let lowest = decade_lower_bound(&target)?;
    first_step(lowest, &|step| {
        Some(compare_exact(step, &target) != Some(Ordering::Less))
    })
}

pub(crate) fn round_ends(lower: &Number, upper: &Number) -> Option<Interval> {
    let width = upper.sub_exact(lower).ok()?;
    let step = round_step(&width)?;
    let down = floor_integer(&lower.div_exact(&step).ok()?)?;
    let up = ceil_integer(&upper.div_exact(&step).ok()?)?;
    Some(Interval {
        lower: Number::from(down).mul_exact(&step).ok()?,
        upper: Number::from(up).mul_exact(&step).ok()?,
    })
}

pub(crate) fn value_interval(mut values: Vec<f64>) -> Interval {
    values.sort_by(f64::total_cmp);
    let count = values.len();
    let (Some(lowest_kept), Some(highest_kept)) = (
        values.get((count.saturating_sub(1)) / QUANTILE_DIVISOR),
        values.get(count.saturating_sub(1) - (count.saturating_sub(1)) / QUANTILE_DIVISOR),
    ) else {
        return interval(-1, 1);
    };
    derive(&values, *lowest_kept, *highest_kept).unwrap_or_else(|| interval(-1, 1))
}

fn derive(sorted: &[f64], lower_quantile: f64, upper_quantile: f64) -> Option<Interval> {
    let lower = exact(lower_quantile)?;
    let upper = exact(upper_quantile)?;
    let mut width = upper.sub_exact(&lower).ok()?;
    if compare_exact(&width, &integer(0)) == Some(Ordering::Equal) {
        width = match compare_exact(&lower, &integer(0)) {
            Some(Ordering::Less) => lower.negate_exact().ok()?,
            Some(Ordering::Greater) => lower.clone(),
            _ => integer(1),
        };
    }
    let reach = width.mul_exact(&half()).ok()?;
    let lower_limit = lower.sub_exact(&reach).ok()?;
    let upper_limit = upper.add_exact(&reach).ok()?;
    let first = sorted.partition_point(|value| {
        exact(*value)
            .is_some_and(|value| compare_exact(&value, &lower_limit) == Some(Ordering::Less))
    });
    let last = sorted.partition_point(|value| {
        exact(*value)
            .is_some_and(|value| compare_exact(&value, &upper_limit) != Some(Ordering::Greater))
    });
    let lowest = sorted.get(first).copied().and_then(exact)?;
    let highest = last
        .checked_sub(1)
        .and_then(|index| sorted.get(index))
        .copied()
        .and_then(exact)?;
    let (lowest, highest) = if compare_exact(&lowest, &highest) == Some(Ordering::Equal) {
        (
            lowest.sub_exact(&reach).ok()?,
            highest.add_exact(&reach).ok()?,
        )
    } else {
        (lowest, highest)
    };
    round_ends(&lowest, &highest)
}

pub(crate) fn reference_modulus(real: &Column, imaginary: &Column) -> Interval {
    let pair = |index: usize| -> Option<(f64, f64)> {
        let value = |column: &Column| match column {
            Column::F32(values) => values.get(index).map(|value| f64::from(*value)),
            Column::F64(values) => values.get(index).copied(),
        };
        Some((finite(value(real)?)?, finite(value(imaginary)?)?))
    };
    let pairs: Vec<(f64, f64)> = (0..real.len()).filter_map(pair).collect();
    median_modulus(&pairs)
        .and_then(|squared| modulus_upper_end(&squared))
        .map_or_else(
            || interval(0, 1),
            |upper| Interval {
                lower: integer(0),
                upper,
            },
        )
}

fn exact_squared(real: f64, imaginary: f64) -> Option<Number> {
    let real = exact(real)?;
    let imaginary = exact(imaginary)?;
    real.mul_exact(&real)
        .ok()?
        .add_exact(&imaginary.mul_exact(&imaginary).ok()?)
        .ok()
}

fn median_modulus(pairs: &[(f64, f64)]) -> Option<Number> {
    let approximate: Vec<f64> = pairs
        .iter()
        .map(|(real, imaginary)| real * real + imaginary * imaginary)
        .collect();
    let mut sorted = approximate.clone();
    sorted.sort_by(f64::total_cmp);
    let index = sorted.len().checked_sub(1)? / 2;
    let middle = *sorted.get(index)?;
    let low = middle * (1.0 - MEDIAN_WINDOW);
    let high = middle * (1.0 + MEDIAN_WINDOW);
    let below = approximate.iter().filter(|key| **key < low).count();
    let mut candidates: Vec<Number> = pairs
        .iter()
        .zip(&approximate)
        .filter(|(_, key)| **key >= low && **key <= high)
        .filter_map(|((real, imaginary), _)| exact_squared(*real, *imaginary))
        .collect();
    candidates.sort_by(|left, right| compare_exact(left, right).unwrap_or(Ordering::Equal));
    candidates.get(index.checked_sub(below)?).cloned()
}

fn modulus_upper_end(squared: &Number) -> Option<Number> {
    if compare_exact(squared, &integer(0)) != Some(Ordering::Greater) {
        return None;
    }
    let steps_squared = integer(MOST_ROUND_STEPS * MOST_ROUND_STEPS);
    let lowest = decade_lower_bound(squared)?.div_euclid(2) - 1;
    let step = first_step(lowest, &|step| {
        let covered = step.mul_exact(step).ok()?.mul_exact(&steps_squared).ok()?;
        Some(compare_exact(&covered, squared) != Some(Ordering::Less))
    })?;
    let mut multiple = Integer::one();
    let one = Integer::one();
    loop {
        let end = Number::from(multiple.clone()).mul_exact(&step).ok()?;
        let square = end.mul_exact(&end).ok()?;
        if compare_exact(&square, squared) != Some(Ordering::Less) {
            return Some(end);
        }
        multiple = &multiple + &one;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(function: impl Fn(f64) -> f64, count: u32) -> Vec<f64> {
        (0..=count)
            .map(|index| -10.0 + 20.0 * f64::from(index) / f64::from(count))
            .map(function)
            .collect()
    }

    fn ends(interval: &Interval) -> (f64, f64) {
        (
            interval.lower.round_to_f64_ties_even(),
            interval.upper.round_to_f64_ties_even(),
        )
    }

    #[test]
    fn sine_gets_minus_one_to_one() {
        let interval = value_interval(values(calc_numbers::sin_f64, 1000));

        assert_eq!(ends(&interval), (-1.0, 1.0));
    }

    #[test]
    fn square_gets_zero_to_one_hundred() {
        let interval = value_interval(values(|x| x * x, 1000));

        assert_eq!(ends(&interval), (0.0, 100.0));
    }

    #[test]
    fn reciprocal_is_not_scaled_to_its_pole() {
        let interval = value_interval(values(|x| 1.0 / x, 1000));

        assert_eq!(ends(&interval), (-6.0, 6.0));
    }

    #[test]
    fn tangent_is_not_scaled_to_its_poles() {
        let samples = values(calc_numbers::tan_f64, 1000);
        let largest = samples.iter().fold(0.0, |most: f64, value| {
            if value.abs() > most {
                value.abs()
            } else {
                most
            }
        });

        let interval = value_interval(samples);

        let (lower, upper) = ends(&interval);
        assert_eq!((lower, upper, largest > 150.0), (-30.0, 30.0, true));
    }

    #[test]
    fn narrow_peak_rises_beyond_the_value_interval() {
        let samples = values(|x| calc_numbers::exp_f64(-100.0 * x * x), 1000);

        let interval = value_interval(samples);

        assert_eq!(ends(&interval), (0.0, 0.02));
    }

    #[test]
    fn no_finite_value_gives_minus_one_to_one() {
        assert_eq!(ends(&value_interval(Vec::new())), (-1.0, 1.0));
    }

    #[test]
    fn constant_values_widen_by_their_magnitude() {
        assert_eq!(ends(&value_interval(vec![4.0; 20])), (2.0, 6.0));
    }

    #[test]
    fn value_interval_does_not_depend_on_sample_order() {
        let forward = values(|x| x * x * x, 400);
        let mut backward = forward.clone();
        backward.reverse();

        assert_eq!(value_interval(forward), value_interval(backward));
    }

    #[test]
    fn round_step_is_one_two_or_five_times_a_power_of_ten() {
        let step = round_step(&integer(7)).unwrap();

        assert_eq!(step.round_to_f64_ties_even(), 2.0);
    }

    #[test]
    fn round_step_of_a_width_below_the_subnormals_is_the_smallest_step() {
        let tiny = Number::fraction(&Integer::one(), &Integer::from(10_i64).pow(400)).unwrap();

        let step = round_step(&tiny).unwrap();

        assert_eq!(
            step,
            Number::fraction(&Integer::from(2_i64), &Integer::from(10_i64).pow(401)).unwrap()
        );
    }

    #[test]
    fn round_step_of_a_huge_width_is_exact() {
        let huge = Number::from(Integer::from(10_i64).pow(400));

        let step = round_step(&huge).unwrap();

        assert_eq!(
            step,
            integer(2)
                .mul_exact(&Number::from(Integer::from(10_i64).pow(399)))
                .unwrap()
        );
    }

    #[test]
    fn reference_modulus_step_is_chosen_from_the_exact_median() {
        let squared = integer(625);

        assert_eq!(modulus_upper_end(&squared), Some(integer(25)));
    }

    #[test]
    fn round_ends_contain_the_interval() {
        let rounded = round_ends(
            &Number::fraction(&Integer::from(-13_i64), &Integer::from(10_i64)).unwrap(),
            &Number::fraction(&Integer::from(27_i64), &Integer::from(10_i64)).unwrap(),
        )
        .unwrap();

        assert_eq!(ends(&rounded), (-2.0, 3.0));
    }

    #[test]
    fn dimensionless_axis_defaults_to_minus_ten_to_ten() {
        let axis =
            default_axis_interval(Dimension::DIMENSIONLESS, &AxisUnit::dimensionless(), false);

        assert_eq!(ends(&axis), (-10.0, 10.0));
    }

    #[test]
    fn temperature_axis_defaults_to_zero_to_one_hundred() {
        let unit = AxisUnit::Coherent {
            symbol: String::from("K"),
        };

        assert_eq!(
            ends(&default_axis_interval(Dimension::TEMPERATURE, &unit, false)),
            (0.0, 100.0)
        );
    }

    #[test]
    fn complex_plane_defaults_to_minus_two_to_two() {
        let axis =
            default_axis_interval(Dimension::DIMENSIONLESS, &AxisUnit::dimensionless(), true);

        assert_eq!(ends(&axis), (-2.0, 2.0));
    }

    #[test]
    fn reference_modulus_is_the_rounded_median_modulus() {
        let real = Column::F64(vec![3.0, 0.0, 1.0]);
        let imaginary = Column::F64(vec![4.0, 1.0, 0.0]);

        let range = reference_modulus(&real, &imaginary);

        assert_eq!(ends(&range), (0.0, 1.0));
    }

    #[test]
    fn reference_modulus_rounds_up_an_irrational_median() {
        let real = Column::F64(vec![1.0, 1.0, 5.0]);
        let imaginary = Column::F64(vec![1.0, 1.0, 5.0]);

        let range = reference_modulus(&real, &imaginary);

        assert_eq!(ends(&range), (0.0, 1.5));
    }

    #[test]
    fn zero_moduli_give_a_reference_modulus_of_one() {
        let zeros = Column::F64(vec![0.0; 3]);

        assert_eq!(ends(&reference_modulus(&zeros, &zeros)), (0.0, 1.0));
    }
}
