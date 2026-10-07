use std::cmp::Ordering;

use calc_exec::{Constant, Domain, EscapeTimePlanForm};
use calc_numbers::{Integer, Interval, Number, Truth};

use crate::exact_order::compare_exact;
use crate::primitive::Column;
use crate::record::Interval as ExactInterval;

pub const ESCAPED_CELL: u8 = 0;
pub const INSIDE_CELL: u8 = 1;
pub const UNDECIDED_CELL: u8 = 2;

const DEFAULT_BASE: u32 = 256;
const DEFAULT_PER_HALVING: u32 = 64;
const DEFAULT_CAP: u32 = 65_536;
const PARAMETER_DEFAULT_WIDTH: (i64, i64) = (7, 2);
const INITIAL_DEFAULT_WIDTH: (i64, i64) = (4, 1);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EscapeTimeForm {
    QuadraticParameter,
    QuadraticInitial { c_real: Number, c_imaginary: Number },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IterationLimitRule {
    Fixed {
        iterations: u32,
    },
    FollowingDepth {
        base: u32,
        per_halving: u32,
        cap: u32,
    },
}

impl IterationLimitRule {
    pub const DEFAULT: IterationLimitRule = IterationLimitRule::FollowingDepth {
        base: DEFAULT_BASE,
        per_halving: DEFAULT_PER_HALVING,
        cap: DEFAULT_CAP,
    };
}

fn fraction(parts: (i64, i64)) -> Option<Number> {
    Number::fraction(&Integer::from(parts.0), &Integer::from(parts.1)).ok()
}

pub fn default_view(form: &EscapeTimeForm) -> [ExactInterval; 2] {
    let exact = |parts: (i64, i64)| fraction(parts).unwrap_or_else(|| Number::from(0_i64));
    let interval = |lower: (i64, i64), upper: (i64, i64)| ExactInterval {
        lower: exact(lower),
        upper: exact(upper),
    };
    match form {
        EscapeTimeForm::QuadraticParameter => {
            [interval((-5, 2), (1, 1)), interval((-5, 4), (5, 4))]
        }
        EscapeTimeForm::QuadraticInitial { .. } => {
            [interval((-2, 1), (2, 1)), interval((-3, 2), (3, 2))]
        }
    }
}

fn default_width(form: &EscapeTimeForm) -> Option<Number> {
    match form {
        EscapeTimeForm::QuadraticParameter => fraction(PARAMETER_DEFAULT_WIDTH),
        EscapeTimeForm::QuadraticInitial { .. } => fraction(INITIAL_DEFAULT_WIDTH),
    }
}

pub(crate) fn iterations_for(
    rule: IterationLimitRule,
    form: &EscapeTimeForm,
    real_axis: &ExactInterval,
) -> Option<u32> {
    let (base, per_halving, cap) = match rule {
        IterationLimitRule::Fixed { iterations } => return Some(iterations),
        IterationLimitRule::FollowingDepth {
            base,
            per_halving,
            cap,
        } => (base, per_halving, cap),
    };
    let limit_of = |depth: u32| {
        per_halving
            .checked_mul(depth)
            .and_then(|growth| base.checked_add(growth))
            .map_or(cap, |limit| limit.min(cap))
    };
    let default = default_width(form)?;
    let two = Number::from(2_i64);
    let mut width = real_axis.upper.sub_exact(&real_axis.lower).ok()?;
    let mut depth = 0_u32;
    loop {
        if limit_of(depth) >= cap {
            return Some(cap);
        }
        let doubled = width.mul_exact(&two).ok()?;
        if compare_exact(&doubled, &default)? == Ordering::Greater {
            return Some(limit_of(depth));
        }
        width = doubled;
        depth = depth.checked_add(1)?;
    }
}

pub(crate) fn plan_form(form: &EscapeTimeForm, domain: Domain) -> EscapeTimePlanForm {
    match form {
        EscapeTimeForm::QuadraticParameter => EscapeTimePlanForm::Parameter,
        EscapeTimeForm::QuadraticInitial {
            c_real,
            c_imaginary,
        } => {
            let constant = |number: &Number| match domain {
                Domain::F32 => Constant::F32(number.round_to_f32_ties_even()),
                Domain::F64 => Constant::F64(number.round_to_f64_ties_even()),
            };
            EscapeTimePlanForm::Initial {
                c_real: constant(c_real),
                c_imaginary: constant(c_imaginary),
            }
        }
    }
}

fn is_at_most(left: Option<Number>, right: Option<Number>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => compare_exact(&left, &right) != Some(Ordering::Greater),
        _ => false,
    }
}

fn enclosed_inside_test(real: &Number, imaginary: &Number) -> Option<bool> {
    let a = Interval::from_exact(real)?;
    let b = Interval::from_exact(imaginary)?;
    let quarter = Interval::point(0.25)?;
    let sixteenth = Interval::point(0.0625)?;
    let one = Interval::point(1.0)?;
    let shifted = a.sub(&quarter)?;
    let b_squared = b.mul(&b)?;
    let q = shifted.mul(&shifted)?.add(&b_squared)?;
    let cardioid = q
        .mul(&q.add(&shifted)?)?
        .less_or_equal(&b_squared.mul(&quarter)?);
    let shifted_disc = a.add(&one)?;
    let disc = shifted_disc
        .mul(&shifted_disc)?
        .add(&b_squared)?
        .less_or_equal(&sixteenth);
    match cardioid.or(disc) {
        Truth::True => Some(true),
        Truth::False => Some(false),
        Truth::Unknown => None,
    }
}

pub(crate) fn is_proven_inside(real: &Number, imaginary: &Number) -> bool {
    enclosed_inside_test(real, imaginary)
        .unwrap_or_else(|| is_proven_inside_exactly(real, imaginary))
}

fn is_proven_inside_exactly(real: &Number, imaginary: &Number) -> bool {
    let quarter = fraction((1, 4));
    let sixteenth = fraction((1, 16));
    let square = |number: &Number| number.mul_exact(number).ok();
    let shifted = quarter.and_then(|quarter| real.sub_exact(&quarter).ok());
    let imaginary_squared = square(imaginary);
    let q = shifted
        .as_ref()
        .and_then(square)
        .zip(imaginary_squared.clone())
        .and_then(|(left, right)| left.add_exact(&right).ok());
    let cardioid_left = q.clone().zip(shifted).and_then(|(q, shifted)| {
        q.add_exact(&shifted)
            .ok()
            .and_then(|sum| q.mul_exact(&sum).ok())
    });
    let cardioid_right = imaginary_squared
        .clone()
        .and_then(|squared| fraction((1, 4)).and_then(|quarter| squared.mul_exact(&quarter).ok()));
    let disc_left = real
        .add_exact(&Number::from(1_i64))
        .ok()
        .as_ref()
        .and_then(square)
        .zip(imaginary_squared)
        .and_then(|(left, right)| left.add_exact(&right).ok());
    is_at_most(cardioid_left, cardioid_right) || is_at_most(disc_left, sixteenth)
}

pub(crate) struct Classified {
    pub(crate) scalar: Column,
    pub(crate) classes: Vec<u8>,
    pub(crate) escaped: u64,
    pub(crate) inside: u64,
    pub(crate) undecided: u64,
}

fn values(column: &Column) -> Vec<f64> {
    match column {
        Column::F32(values) => values.iter().map(|value| f64::from(*value)).collect(),
        Column::F64(values) => values.clone(),
    }
}

pub(crate) fn classify(counts: &Column, escaped_flags: &Column, inside: &[bool]) -> Classified {
    let flags = values(escaped_flags);
    let mut classes = Vec::with_capacity(flags.len());
    let mut escaped = 0_u64;
    let mut proven_inside = 0_u64;
    let mut undecided = 0_u64;
    let class_of = |index: usize| {
        if inside.get(index).copied().unwrap_or(false) {
            INSIDE_CELL
        } else if flags.get(index).is_some_and(|flag| *flag == 1.0) {
            ESCAPED_CELL
        } else {
            UNDECIDED_CELL
        }
    };
    for index in 0..flags.len() {
        let class = class_of(index);
        match class {
            ESCAPED_CELL => escaped = escaped.saturating_add(1),
            INSIDE_CELL => proven_inside = proven_inside.saturating_add(1),
            _ => undecided = undecided.saturating_add(1),
        }
        classes.push(class);
    }
    let scalar = match counts {
        Column::F32(values) => Column::F32(
            values
                .iter()
                .zip(&classes)
                .map(|(count, class)| {
                    if *class == ESCAPED_CELL {
                        *count
                    } else {
                        f32::NAN
                    }
                })
                .collect(),
        ),
        Column::F64(values) => Column::F64(
            values
                .iter()
                .zip(&classes)
                .map(|(count, class)| {
                    if *class == ESCAPED_CELL {
                        *count
                    } else {
                        f64::NAN
                    }
                })
                .collect(),
        ),
    };
    Classified {
        scalar,
        classes,
        escaped,
        inside: proven_inside,
        undecided,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact(numerator: i64, denominator: i64) -> Number {
        fraction((numerator, denominator)).unwrap()
    }

    fn real_axis(lower: Number, upper: Number) -> ExactInterval {
        ExactInterval { lower, upper }
    }

    #[test]
    fn origin_is_proven_inside_the_main_cardioid() {
        assert!(is_proven_inside(&Number::from(0_i64), &Number::from(0_i64)));
    }

    #[test]
    fn minus_one_is_proven_inside_the_period_two_disc() {
        assert!(is_proven_inside(
            &Number::from(-1_i64),
            &Number::from(0_i64)
        ));
    }

    #[test]
    fn cusp_of_the_cardioid_at_one_quarter_is_inside() {
        assert!(is_proven_inside(&exact(1, 4), &Number::from(0_i64)));
    }

    #[test]
    fn point_just_right_of_the_cusp_is_not_proven_inside() {
        assert!(!is_proven_inside(&exact(26, 100), &Number::from(0_i64)));
    }

    #[test]
    fn default_view_uses_the_base_limit() {
        let limit = iterations_for(
            IterationLimitRule::DEFAULT,
            &EscapeTimeForm::QuadraticParameter,
            &real_axis(exact(-5, 2), Number::from(1_i64)),
        );

        assert_eq!(limit, Some(256));
    }

    #[test]
    fn view_halved_three_times_adds_three_steps() {
        let limit = iterations_for(
            IterationLimitRule::DEFAULT,
            &EscapeTimeForm::QuadraticParameter,
            &real_axis(Number::from(0_i64), exact(7, 16)),
        );

        assert_eq!(limit, Some(256 + 3 * 64));
    }

    #[test]
    fn deep_view_is_capped() {
        let tiny = Number::fraction(&Integer::one(), &Integer::from(2_i64).pow(2000)).unwrap();

        let limit = iterations_for(
            IterationLimitRule::DEFAULT,
            &EscapeTimeForm::QuadraticParameter,
            &real_axis(Number::from(0_i64), tiny),
        );

        assert_eq!(limit, Some(65_536));
    }

    #[test]
    fn fixed_rule_ignores_the_depth() {
        let limit = iterations_for(
            IterationLimitRule::Fixed { iterations: 64 },
            &EscapeTimeForm::QuadraticParameter,
            &real_axis(Number::from(0_i64), exact(1, 1000)),
        );

        assert_eq!(limit, Some(64));
    }

    #[test]
    fn classes_put_proof_first_then_escape_then_undecided() {
        let counts = Column::F64(vec![3.0, 9.0, 9.0]);
        let flags = Column::F64(vec![1.0, 0.0, 1.0]);

        let classified = classify(&counts, &flags, &[false, false, true]);

        assert_eq!(
            (
                classified.classes,
                classified.escaped,
                classified.inside,
                classified.undecided
            ),
            (vec![ESCAPED_CELL, UNDECIDED_CELL, INSIDE_CELL], 1, 1, 1)
        );
        assert!(
            matches!(classified.scalar, Column::F64(values) if values[0] == 3.0 && values[1].is_nan() && values[2].is_nan())
        );
    }

    #[test]
    fn enclosed_test_agrees_with_the_exact_test_on_a_grid_around_the_set() {
        let mut disagreements = 0;
        for real_step in -40..=20 {
            for imaginary_step in -20..=20 {
                let real = exact(real_step, 16);
                let imaginary = exact(imaginary_step, 16);
                if let Some(enclosed) = enclosed_inside_test(&real, &imaginary)
                    && enclosed != is_proven_inside_exactly(&real, &imaginary)
                {
                    disagreements += 1;
                }
            }
        }

        assert_eq!(disagreements, 0);
    }
}
