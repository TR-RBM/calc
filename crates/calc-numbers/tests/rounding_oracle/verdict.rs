use std::cmp::Ordering;

use calc_numbers::Number;

use crate::arithmetic::{Arithmetic, Point};
use crate::knowledge::{ExactValue, Knowledge};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Verdict {
    WithinBound,
    ExceedsBound,
    MissingClaim,
    Undecided,
}

#[derive(Clone, Debug)]
pub struct Claim {
    pub low: Point,
    pub high: Point,
}

fn exact_of(point: &Point) -> Option<Number> {
    match (&point.exact, point.machine) {
        (Some(exact), _) => Some(exact.clone()),
        (None, Some(machine)) => Number::F64(machine).to_exact().ok(),
        (None, None) => None,
    }
}

fn sign_of(value: &Number) -> Ordering {
    match value {
        Number::Integer(integer) if integer.is_zero() => Ordering::Equal,
        Number::Integer(integer) if integer.is_negative() => Ordering::Less,
        Number::Rational(rational) if rational.numerator().is_negative() => Ordering::Less,
        _ => Ordering::Greater,
    }
}

pub fn exact_order(left: &Number, right: &Number) -> Option<Ordering> {
    left.sub_exact(right)
        .ok()
        .map(|difference| sign_of(&difference))
}

fn exact_verdict(value: &Number, result: f64, claim: Option<&Claim>, overflow: &Point) -> Verdict {
    if result.is_infinite() {
        let Some(threshold) = exact_of(overflow) else {
            return Verdict::Undecided;
        };
        let reaches = if result > 0.0 {
            exact_order(value, &threshold) != Some(Ordering::Less)
        } else {
            threshold
                .negate_exact()
                .ok()
                .and_then(|negative| exact_order(value, &negative))
                != Some(Ordering::Greater)
        };
        return if reaches {
            Verdict::WithinBound
        } else {
            Verdict::ExceedsBound
        };
    }
    let Some(claim) = claim else {
        return Verdict::MissingClaim;
    };
    let (Some(low), Some(high)) = (exact_of(&claim.low), exact_of(&claim.high)) else {
        return Verdict::Undecided;
    };
    let above_low = exact_order(value, &low) != Some(Ordering::Less);
    let below_high = exact_order(value, &high) != Some(Ordering::Greater);
    if above_low && below_high {
        Verdict::WithinBound
    } else {
        Verdict::ExceedsBound
    }
}

fn is_at_least<A: Arithmetic>(arithmetic: &A, value: &A::Value, point: &Point) -> Option<bool> {
    arithmetic
        .compare_lower(value, point)
        .map(|order| order != Ordering::Less)
}

fn is_at_most<A: Arithmetic>(arithmetic: &A, value: &A::Value, point: &Point) -> Option<bool> {
    arithmetic
        .compare_upper(value, point)
        .map(|order| order != Ordering::Greater)
}

fn is_entirely_below<A: Arithmetic>(
    arithmetic: &A,
    value: &A::Value,
    point: &Point,
) -> Option<bool> {
    arithmetic
        .compare_upper(value, point)
        .map(|order| order == Ordering::Less)
}

fn is_entirely_above<A: Arithmetic>(
    arithmetic: &A,
    value: &A::Value,
    point: &Point,
) -> Option<bool> {
    arithmetic
        .compare_lower(value, point)
        .map(|order| order == Ordering::Greater)
}

fn negated_point(point: &Point) -> Point {
    Point {
        machine: point.machine.map(|value| -value),
        exact: point
            .exact
            .as_ref()
            .and_then(|value| value.negate_exact().ok()),
    }
}

fn enclosed_verdict<A: Arithmetic>(
    arithmetic: &A,
    value: &A::Value,
    result: f64,
    claim: Option<&Claim>,
    overflow: &Point,
) -> Verdict {
    let (within, outside) = if result == f64::INFINITY {
        (
            is_at_least(arithmetic, value, overflow),
            is_entirely_below(arithmetic, value, overflow),
        )
    } else if result == f64::NEG_INFINITY {
        let negative = negated_point(overflow);
        (
            is_at_most(arithmetic, value, &negative),
            is_entirely_above(arithmetic, value, &negative),
        )
    } else {
        let Some(claim) = claim else {
            return Verdict::MissingClaim;
        };
        let within = is_at_least(arithmetic, value, &claim.low)
            .zip(is_at_most(arithmetic, value, &claim.high))
            .map(|(above, below)| above && below);
        let outside = is_entirely_below(arithmetic, value, &claim.low)
            .zip(is_entirely_above(arithmetic, value, &claim.high))
            .map(|(below, above)| below || above);
        (within, outside)
    };
    match (within, outside) {
        (Some(true), _) => Verdict::WithinBound,
        (_, Some(true)) => Verdict::ExceedsBound,
        _ => Verdict::Undecided,
    }
}

pub fn verdict<A: Arithmetic>(
    arithmetic: &A,
    knowledge: &Knowledge<A::Value>,
    result: f64,
    claim: Option<&Claim>,
    overflow: &Point,
) -> Verdict {
    match knowledge {
        Knowledge::Unknown => Verdict::Undecided,
        Knowledge::NotANumber if result.is_nan() => Verdict::WithinBound,
        Knowledge::NotANumber => Verdict::ExceedsBound,
        _ if result.is_nan() => Verdict::ExceedsBound,
        Knowledge::Exact(ExactValue::PositiveInfinity) if result == f64::INFINITY => {
            Verdict::WithinBound
        }
        Knowledge::Exact(ExactValue::NegativeInfinity) if result == f64::NEG_INFINITY => {
            Verdict::WithinBound
        }
        Knowledge::Exact(ExactValue::PositiveInfinity | ExactValue::NegativeInfinity) => {
            Verdict::ExceedsBound
        }
        Knowledge::Overflow { is_negative } => {
            let expected = if *is_negative {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
            if result == expected {
                Verdict::WithinBound
            } else {
                Verdict::ExceedsBound
            }
        }
        Knowledge::Exact(ExactValue::Finite(value)) => {
            exact_verdict(value, result, claim, overflow)
        }
        Knowledge::Enclosed(value) => enclosed_verdict(arithmetic, value, result, claim, overflow),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::{MachineArithmetic, MachineInterval};

    fn claim_around(value: f64, radius: f64) -> Claim {
        Claim {
            low: Point::machine(value - radius),
            high: Point::machine(value + radius),
        }
    }

    fn overflow() -> Point {
        Point::machine(f64::from(f32::MAX))
    }

    #[test]
    fn enclosure_inside_claim_is_within_bound() {
        let arithmetic = MachineArithmetic::new();
        let knowledge = Knowledge::Enclosed(MachineInterval {
            lower: 0.9,
            upper: 1.1,
        });

        let found = verdict(
            &arithmetic,
            &knowledge,
            1.0,
            Some(&claim_around(1.0, 0.25)),
            &overflow(),
        );

        assert_eq!(found, Verdict::WithinBound);
    }

    #[test]
    fn enclosure_beyond_claim_exceeds_bound() {
        let arithmetic = MachineArithmetic::new();
        let knowledge = Knowledge::Enclosed(MachineInterval {
            lower: 1.3,
            upper: 1.4,
        });

        let found = verdict(
            &arithmetic,
            &knowledge,
            1.0,
            Some(&claim_around(1.0, 0.25)),
            &overflow(),
        );

        assert_eq!(found, Verdict::ExceedsBound);
    }

    #[test]
    fn enclosure_straddling_claim_edge_is_undecided() {
        let arithmetic = MachineArithmetic::new();
        let knowledge = Knowledge::Enclosed(MachineInterval {
            lower: 1.2,
            upper: 1.3,
        });

        let found = verdict(
            &arithmetic,
            &knowledge,
            1.0,
            Some(&claim_around(1.0, 0.25)),
            &overflow(),
        );

        assert_eq!(found, Verdict::Undecided);
    }

    #[test]
    fn exact_value_on_claim_edge_is_within_bound() {
        let arithmetic = MachineArithmetic::new();
        let knowledge = Knowledge::Exact(ExactValue::Finite(Number::from(2i64)));

        let found = verdict(
            &arithmetic,
            &knowledge,
            1.5,
            Some(&claim_around(1.5, 0.5)),
            &overflow(),
        );

        assert_eq!(found, Verdict::WithinBound);
    }

    #[test]
    fn infinity_for_value_below_overflow_threshold_exceeds_bound() {
        let arithmetic = MachineArithmetic::new();
        let knowledge = Knowledge::Enclosed(MachineInterval {
            lower: 1.0e30,
            upper: 1.1e30,
        });

        let found = verdict(&arithmetic, &knowledge, f64::INFINITY, None, &overflow());

        assert_eq!(found, Verdict::ExceedsBound);
    }

    #[test]
    fn number_where_not_a_number_is_expected_exceeds_bound() {
        let arithmetic = MachineArithmetic::new();
        let knowledge = Knowledge::NotANumber;

        let found = verdict(
            &arithmetic,
            &knowledge,
            0.0,
            Some(&claim_around(0.0, 1.0)),
            &overflow(),
        );

        assert_eq!(found, Verdict::ExceedsBound);
    }

    #[test]
    fn finite_result_without_claim_is_missing_claim() {
        let arithmetic = MachineArithmetic::new();
        let knowledge = Knowledge::Enclosed(MachineInterval {
            lower: 0.9,
            upper: 1.1,
        });

        let found = verdict(&arithmetic, &knowledge, 1.0, None, &overflow());

        assert_eq!(found, Verdict::MissingClaim);
    }
}
