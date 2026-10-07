use std::cmp::Ordering;

use calc_numbers::{Integer, Number, POWER_OF_TEN_MARK, power_of_ten_text};
use calc_viz::Interval;

pub const TICK_LABEL_CELLS: u16 = 11;

const STEP_DIGITS: [i64; 3] = [1, 2, 5];
const LOWEST_PLAIN_EXPONENT: i64 = -4;
const HIGHEST_PLAIN_EXPONENT: i64 = 5;
const CANDIDATE_LIMIT: usize = 4_096;
const TEN: i64 = 10;
const MOST_EXACT_PLACES: u32 = 18;
const MINUS: char = '-';
const ZERO: char = '0';
const LOG_TWO_DIGITS: i64 = 30_103;
const LOG_TWO_SCALE: i64 = 100_000;
const DECADE_SEARCH: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickLabelError {
    TooWide { significand: i64, exponent: i64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outward {
    Down,
    Up,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TickRoom {
    length: u64,
    spacing: Spacing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spacing {
    Fixed(u64),
    Labels { cell: u64, gap_cells: u64 },
}

impl TickRoom {
    pub(crate) fn at_most(count: u32) -> TickRoom {
        TickRoom {
            length: u64::from(count),
            spacing: Spacing::Fixed(1),
        }
    }

    pub(crate) fn spaced(length: u32, spacing: u32) -> TickRoom {
        TickRoom {
            length: u64::from(length),
            spacing: Spacing::Fixed(u64::from(spacing.max(1))),
        }
    }

    pub(crate) fn beside_labels(length: u32, cell: u16, gap_cells: u32) -> TickRoom {
        TickRoom {
            length: u64::from(length),
            spacing: Spacing::Labels {
                cell: u64::from(cell.max(1)),
                gap_cells: u64::from(gap_cells),
            },
        }
    }

    fn spacing(&self, widest_cells: u64) -> u64 {
        match self.spacing {
            Spacing::Fixed(spacing) => spacing,
            Spacing::Labels { cell, gap_cells } => (widest_cells + gap_cells) * cell,
        }
    }

    fn most(&self) -> u64 {
        self.length / self.spacing(1).max(1)
    }

    fn fits(&self, ticks: &[Tick]) -> bool {
        let widest = ticks
            .iter()
            .map(|tick| tick.label.chars().count())
            .max()
            .unwrap_or(0);
        let count = u64::try_from(ticks.len()).unwrap_or(u64::MAX);
        let widest = u64::try_from(widest).unwrap_or(u64::MAX);
        count.saturating_mul(self.spacing(widest)) <= self.length
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Tick {
    pub value: Number,
    pub label: String,
}

pub fn tick_label(
    significand: i64,
    exponent: i64,
    separator: char,
) -> Result<String, TickLabelError> {
    let too_wide = TickLabelError::TooWide {
        significand,
        exponent,
    };
    if significand == 0 {
        return Ok(ZERO.to_string());
    }
    let mut digits_value = significand.unsigned_abs();
    let mut last_exponent = exponent;
    while digits_value.is_multiple_of(10) {
        digits_value /= 10;
        last_exponent = last_exponent.checked_add(1).ok_or(too_wide)?;
    }
    let digits = digits_value.to_string();
    let digit_count = i64::try_from(digits.len()).map_err(|_| too_wide)?;
    let leading_exponent = last_exponent.checked_add(digit_count - 1).ok_or(too_wide)?;
    let mut label = String::new();
    if significand < 0 {
        label.push(MINUS);
    }
    let is_plain = (LOWEST_PLAIN_EXPONENT..=HIGHEST_PLAIN_EXPONENT).contains(&leading_exponent);
    if is_plain {
        push_plain(&mut label, &digits, last_exponent, separator, too_wide)?;
    } else {
        push_scientific(&mut label, &digits, leading_exponent, separator);
    }
    let cells = label.chars().count();
    if cells <= usize::from(TICK_LABEL_CELLS) {
        Ok(label)
    } else {
        Err(too_wide)
    }
}

pub(crate) fn outward_label(value: &Number, direction: Outward, separator: char) -> Option<String> {
    let zero = Number::from(0_i64);
    let magnitude = match compare(value, &zero)? {
        Ordering::Equal => return tick_label(0, 0, separator).ok(),
        Ordering::Less => value.negate_exact().ok()?,
        Ordering::Greater => value.clone(),
    };
    let leading = leading_decade(&magnitude)?;
    let place = leading.checked_sub(1)?;
    let scaled = value.div_exact(&power_of_ten(place)?).ok()?;
    let significand = match direction {
        Outward::Down => floor_integer(&scaled)?,
        Outward::Up => ceil_integer(&scaled)?,
    };
    tick_label(significand.to_i64()?, place, separator).ok()
}

fn leading_decade(magnitude: &Number) -> Option<i64> {
    let (numerator, denominator) = match magnitude {
        Number::Integer(integer) => (integer.clone(), Integer::one()),
        Number::Rational(rational) => {
            (rational.numerator().clone(), rational.denominator().clone())
        }
        Number::F32(_) | Number::F64(_) => return None,
    };
    let binary = i64::try_from(numerator.bit_length()).ok()?
        - i64::try_from(denominator.bit_length()).ok()?;
    let mut decade = binary
        .checked_mul(LOG_TWO_DIGITS)?
        .div_euclid(LOG_TWO_SCALE)
        .checked_sub(1)?;
    for _ in 0..DECADE_SEARCH {
        let below = power_of_ten(decade)?;
        let above = power_of_ten(decade.checked_add(1)?)?;
        if compare(&below, magnitude)? == Ordering::Greater {
            decade = decade.checked_sub(1)?;
        } else if compare(&above, magnitude)? != Ordering::Greater {
            decade = decade.checked_add(1)?;
        } else {
            return Some(decade);
        }
    }
    None
}

pub(crate) fn exact_label(value: &Number, separator: char) -> Option<String> {
    for places in 0..=MOST_EXACT_PLACES {
        let scaled = power_of_ten(i64::from(places))?.mul_exact(value).ok()?;
        if let Number::Integer(integer) = scaled {
            return tick_label(integer.to_i64()?, -i64::from(places), separator).ok();
        }
    }
    None
}

fn push_plain(
    label: &mut String,
    digits: &str,
    last_exponent: i64,
    separator: char,
    too_wide: TickLabelError,
) -> Result<(), TickLabelError> {
    if last_exponent >= 0 {
        label.push_str(digits);
        let zeros = usize::try_from(last_exponent).map_err(|_| too_wide)?;
        label.extend(std::iter::repeat_n(ZERO, zeros));
        return Ok(());
    }
    let places = usize::try_from(last_exponent.unsigned_abs()).map_err(|_| too_wide)?;
    if digits.len() > places {
        let (whole, fraction) = digits.split_at(digits.len() - places);
        label.push_str(whole);
        label.push(separator);
        label.push_str(fraction);
    } else {
        label.push(ZERO);
        label.push(separator);
        label.extend(std::iter::repeat_n(ZERO, places - digits.len()));
        label.push_str(digits);
    }
    Ok(())
}

fn push_scientific(label: &mut String, digits: &str, leading_exponent: i64, separator: char) {
    label.push_str(&power_of_ten_text(digits, leading_exponent, separator));
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

pub(crate) fn floor_integer(value: &Number) -> Option<Integer> {
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

pub(crate) fn decimal_exponent(value: f64) -> Option<i64> {
    if !value.is_finite() || value == 0.0 {
        return None;
    }
    let text = format!("{value:e}");
    let (_, exponent) = text.split_once(POWER_OF_TEN_MARK)?;
    exponent.parse().ok()
}

pub(crate) fn compare(left: &Number, right: &Number) -> Option<Ordering> {
    let difference = left.sub_exact(right).ok()?;
    let numerator = match &difference {
        Number::Integer(integer) => integer.clone(),
        Number::Rational(rational) => rational.numerator().clone(),
        Number::F32(_) | Number::F64(_) => return None,
    };
    Some(if numerator.is_zero() {
        Ordering::Equal
    } else if numerator.is_negative() {
        Ordering::Less
    } else {
        Ordering::Greater
    })
}

pub(crate) fn linear_ticks(range: &Interval, room: &TickRoom, separator: char) -> Vec<Tick> {
    let most = room.most();
    if most == 0 {
        return Vec::new();
    }
    let Ok(width) = range.upper.sub_exact(&range.lower) else {
        return Vec::new();
    };
    let per_tick =
        width.round_to_f64_ties_even() / f64::from(u32::try_from(most).unwrap_or(u32::MAX));
    let Some(start) = decimal_exponent(per_tick) else {
        return Vec::new();
    };
    let mut exponent = start.saturating_sub(1);
    for _ in 0..CANDIDATE_LIMIT {
        for digit in STEP_DIGITS {
            match ticks_for_step(range, digit, exponent, room, separator) {
                StepOutcome::Ticks(ticks) => return ticks,
                StepOutcome::Empty => return Vec::new(),
                StepOutcome::Coarser => {}
            }
        }
        exponent = exponent.saturating_add(1);
    }
    Vec::new()
}

enum StepOutcome {
    Ticks(Vec<Tick>),
    Empty,
    Coarser,
}

fn ticks_for_step(
    range: &Interval,
    digit: i64,
    exponent: i64,
    room: &TickRoom,
    separator: char,
) -> StepOutcome {
    let step = power_of_ten(exponent).and_then(|power| power.mul_exact(&Number::from(digit)).ok());
    let bounds = step.as_ref().and_then(|step| {
        let first = ceil_integer(&range.lower.div_exact(step).ok()?)?;
        let last = floor_integer(&range.upper.div_exact(step).ok()?)?;
        Some((first.to_i64()?, last.to_i64()?))
    });
    let (Some(step), Some((first, last))) = (step, bounds) else {
        return StepOutcome::Coarser;
    };
    if last < first {
        let exceeds = |bound: &Number| {
            let magnitude = if compare(bound, &Number::from(0_i64)) == Some(Ordering::Less) {
                bound.negate_exact().ok()
            } else {
                Some(bound.clone())
            };
            magnitude.is_some_and(|magnitude| compare(&step, &magnitude) == Some(Ordering::Greater))
        };
        return if exceeds(&range.lower) && exceeds(&range.upper) {
            StepOutcome::Empty
        } else {
            StepOutcome::Coarser
        };
    }
    let count = last.abs_diff(first).saturating_add(1);
    if count > room.most() {
        return StepOutcome::Coarser;
    }
    let mut ticks = Vec::new();
    for index in first..=last {
        let Some(significand) = index.checked_mul(digit) else {
            return StepOutcome::Coarser;
        };
        let Ok(label) = tick_label(significand, exponent, separator) else {
            return StepOutcome::Coarser;
        };
        let Some(value) = power_of_ten(exponent)
            .and_then(|power| power.mul_exact(&Number::from(significand)).ok())
        else {
            return StepOutcome::Coarser;
        };
        ticks.push(Tick { value, label });
    }
    if room.fits(&ticks) {
        StepOutcome::Ticks(ticks)
    } else {
        StepOutcome::Coarser
    }
}

pub(crate) fn logarithmic_ticks(range: &Interval, room: &TickRoom, separator: char) -> Vec<Tick> {
    let zero = Number::from(0_i64);
    if room.most() == 0 || compare(&range.lower, &zero) != Some(Ordering::Greater) {
        return Vec::new();
    }
    let (Some(lower_estimate), Some(upper_estimate)) = (
        decimal_exponent(range.lower.round_to_f64_ties_even()),
        decimal_exponent(range.upper.round_to_f64_ties_even()),
    ) else {
        return Vec::new();
    };
    let is_inside = |exponent: i64| {
        power_of_ten(exponent).is_some_and(|power| {
            compare(&power, &range.lower) != Some(Ordering::Less)
                && compare(&power, &range.upper) != Some(Ordering::Greater)
        })
    };
    let exponents: Vec<i64> = (lower_estimate.saturating_sub(1)..=upper_estimate.saturating_add(1))
        .filter(|exponent| is_inside(*exponent))
        .collect();
    let count = u64::try_from(exponents.len()).unwrap_or(u64::MAX);
    let mut stride = count.div_ceil(room.most()).max(1);
    loop {
        let stride_step = i64::try_from(stride).unwrap_or(i64::MAX);
        let ticks: Vec<Tick> = exponents
            .iter()
            .filter(|exponent| exponent.rem_euclid(stride_step) == 0)
            .filter_map(|exponent| {
                Some(Tick {
                    value: power_of_ten(*exponent)?,
                    label: tick_label(1, *exponent, separator).ok()?,
                })
            })
            .collect();
        if room.fits(&ticks) || stride >= count {
            return if room.fits(&ticks) { ticks } else { Vec::new() };
        }
        stride += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interval(lower: i64, upper: i64) -> Interval {
        Interval {
            lower: Number::from(lower),
            upper: Number::from(upper),
        }
    }

    fn labels(ticks: &[Tick]) -> Vec<&str> {
        ticks.iter().map(|tick| tick.label.as_str()).collect()
    }

    #[test]
    fn integer_label_is_its_digits() {
        assert_eq!(tick_label(250, 0, '.'), Ok(String::from("250")));
    }

    #[test]
    fn negative_label_starts_with_a_minus_sign() {
        assert_eq!(tick_label(-15, -1, '.'), Ok(String::from("-1.5")));
    }

    #[test]
    fn label_uses_the_given_decimal_separator() {
        assert_eq!(tick_label(15, -1, ','), Ok(String::from("1,5")));
    }

    #[test]
    fn small_label_keeps_leading_zeros() {
        assert_eq!(tick_label(2, -3, '.'), Ok(String::from("0.002")));
    }

    #[test]
    fn trailing_zeros_of_the_significand_are_not_printed_after_the_separator() {
        assert_eq!(tick_label(20, -1, '.'), Ok(String::from("2")));
    }

    #[test]
    fn large_label_is_written_with_a_power_of_ten_exponent() {
        assert_eq!(tick_label(5, 7, '.'), Ok(String::from("5e7")));
    }

    #[test]
    fn tiny_label_is_written_with_a_negative_exponent() {
        assert_eq!(tick_label(125, -12, '.'), Ok(String::from("1.25e-10")));
    }

    #[test]
    fn zero_label_is_a_single_digit() {
        assert_eq!(tick_label(0, -30, '.'), Ok(String::from("0")));
    }

    #[test]
    fn label_with_too_many_digits_is_too_wide() {
        assert_eq!(
            tick_label(123_456_789, -300, '.'),
            Err(TickLabelError::TooWide {
                significand: 123_456_789,
                exponent: -300
            })
        );
    }

    #[test]
    fn exact_decimal_has_an_exact_label() {
        let value = Number::fraction(&Integer::from(5_i64), &Integer::from(4_i64)).unwrap();

        assert_eq!(exact_label(&value, '.'), Some(String::from("1.25")));
    }

    #[test]
    fn third_has_no_exact_label() {
        let value = Number::fraction(&Integer::from(1_i64), &Integer::from(3_i64)).unwrap();

        assert_eq!(exact_label(&value, '.'), None);
    }

    #[test]
    fn linear_ticks_use_a_step_of_one_two_or_five() {
        let ticks = linear_ticks(&interval(0, 10), &TickRoom::at_most(6), '.');

        assert_eq!(labels(&ticks), ["0", "2", "4", "6", "8", "10"]);
    }

    #[test]
    fn linear_ticks_never_exceed_the_allowed_count() {
        let ticks = linear_ticks(&interval(-7, 13), &TickRoom::at_most(3), '.');

        assert_eq!(labels(&ticks), ["0", "10"]);
    }

    #[test]
    fn linear_tick_values_are_exact() {
        let range = Interval {
            lower: Number::fraction(&Integer::from(1_i64), &Integer::from(10_i64)).unwrap(),
            upper: Number::fraction(&Integer::from(3_i64), &Integer::from(10_i64)).unwrap(),
        };

        let ticks = linear_ticks(&range, &TickRoom::at_most(3), '.');

        assert_eq!(
            compare(&ticks[0].value, &range.lower),
            Some(Ordering::Equal)
        );
    }

    #[test]
    fn deep_zoom_without_distinct_short_labels_keeps_only_labels_that_fit() {
        let lower = Number::from(1_i64);
        let upper = Number::fraction(
            &Integer::from(10_000_000_001_i64),
            &Integer::from(10_000_000_000_i64),
        )
        .unwrap();

        let ticks = linear_ticks(&Interval { lower, upper }, &TickRoom::at_most(10), '.');

        assert_eq!(labels(&ticks), ["1"]);
    }

    #[test]
    fn range_between_round_numbers_without_room_has_no_tick() {
        let tenth_of_billionth = Integer::from(10_000_000_000_i64);
        let lower =
            Number::fraction(&Integer::from(10_000_000_001_i64), &tenth_of_billionth).unwrap();
        let upper =
            Number::fraction(&Integer::from(10_000_000_002_i64), &tenth_of_billionth).unwrap();

        let ticks = linear_ticks(&Interval { lower, upper }, &TickRoom::at_most(10), '.');

        assert!(ticks.is_empty());
    }

    #[test]
    fn narrow_range_keeps_the_round_tick_inside_it() {
        let range = Interval {
            lower: Number::fraction(&Integer::from(9_i64), &Integer::from(20_i64)).unwrap(),
            upper: Number::fraction(&Integer::from(11_i64), &Integer::from(20_i64)).unwrap(),
        };

        let ticks = linear_ticks(&range, &TickRoom::at_most(1), '.');

        assert_eq!(labels(&ticks), ["0.5"]);
    }

    #[test]
    fn labelled_ticks_are_spaced_by_their_widest_label() {
        let room = TickRoom::beside_labels(200, 10, 2);

        let ticks = linear_ticks(&interval(-4, 4), &room, '.');

        assert_eq!(labels(&ticks), ["-4", "-2", "0", "2", "4"]);
    }

    #[test]
    fn logarithmic_ticks_are_powers_of_ten() {
        let ticks = logarithmic_ticks(&interval(1, 1000), &TickRoom::at_most(10), '.');

        assert_eq!(labels(&ticks), ["1", "10", "100", "1000"]);
    }

    #[test]
    fn logarithmic_ticks_skip_powers_when_too_many() {
        let ticks = logarithmic_ticks(&interval(1, 1_000_000), &TickRoom::at_most(4), '.');

        assert_eq!(labels(&ticks), ["1", "100", "10000", "1e6"]);
    }

    #[test]
    fn logarithmic_ticks_need_a_positive_range() {
        assert!(logarithmic_ticks(&interval(0, 10), &TickRoom::at_most(4), '.').is_empty());
    }

    #[test]
    fn decimal_exponent_is_read_from_the_shortest_digits() {
        assert_eq!(decimal_exponent(0.00125), Some(-3));
    }

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    #[test]
    fn negative_value_rounded_down_grows_in_magnitude() {
        assert_eq!(
            outward_label(&fraction(-10, 3), Outward::Down, '.'),
            Some(String::from("-3.4"))
        );
    }

    #[test]
    fn positive_value_rounded_down_keeps_two_significant_digits() {
        assert_eq!(
            outward_label(&fraction(1, 3), Outward::Down, '.'),
            Some(String::from("0.33"))
        );
    }

    #[test]
    fn value_rounded_up_keeps_two_significant_digits() {
        assert_eq!(
            outward_label(&fraction(10, 3), Outward::Up, '.'),
            Some(String::from("3.4"))
        );
    }

    #[test]
    fn value_rounded_up_carries_into_the_next_decade() {
        assert_eq!(
            outward_label(&fraction(991, 10), Outward::Up, '.'),
            Some(String::from("100"))
        );
    }

    #[test]
    fn tiny_value_rounded_outward_is_written_with_an_exponent() {
        let tiny = fraction(1, 3)
            .mul_exact(&power_of_ten(-20).unwrap())
            .unwrap();

        assert_eq!(
            outward_label(&tiny, Outward::Down, '.'),
            Some(String::from("3.3e-21"))
        );
    }
}
