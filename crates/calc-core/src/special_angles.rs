use std::cmp::Ordering;

use calc_numbers::Integer;

use crate::exact_rational::ExactRational;
use crate::square_root_sum::SquareRootSum;

const REFERENCE_DENOMINATORS: [i64; 4] = [6, 4, 3, 2];
const FULL_TURN_IN_HALF_TURNS: i64 = 2;

fn fraction(numerator: i64, denominator: i64) -> ExactRational {
    match ExactRational::fraction(&Integer::from(numerator), &Integer::from(denominator)) {
        Some(value) => value,
        None => unreachable!(),
    }
}

fn half_root(radicand: i64) -> SquareRootSum {
    match SquareRootSum::square_root_of_rational(&ExactRational::from_i64(radicand)) {
        Some(root) => root.times(&SquareRootSum::from_rational(fraction(1, 2))),
        None => unreachable!(),
    }
}

fn sine_of_reference_angle(half_turns: &ExactRational) -> Option<SquareRootSum> {
    if half_turns.is_zero() {
        return Some(SquareRootSum::zero());
    }
    let value = match REFERENCE_DENOMINATORS
        .iter()
        .position(|denominator| *half_turns == fraction(1, *denominator))?
    {
        0 => SquareRootSum::from_rational(fraction(1, 2)),
        1 => half_root(2),
        2 => half_root(3),
        _ => SquareRootSum::from_rational(ExactRational::one()),
    };
    Some(value)
}

fn reduced_to_one_turn(half_turns: &ExactRational) -> ExactRational {
    let turn = ExactRational::from_i64(FULL_TURN_IN_HALF_TURNS);
    let whole_turns = match half_turns.divide(&turn) {
        Some(turns) => ExactRational::from_integer(turns.floor()),
        None => unreachable!(),
    };
    half_turns.subtract(&whole_turns.multiply(&turn))
}

pub(crate) fn sine_of_pi_multiple(half_turns: &ExactRational) -> Option<SquareRootSum> {
    let reduced = reduced_to_one_turn(half_turns);
    let one = ExactRational::one();
    let (angle, is_negated) = if reduced.compare(&one) != Ordering::Less {
        (reduced.subtract(&one), true)
    } else {
        (reduced, false)
    };
    let reference = if angle.compare(&fraction(1, 2)) == Ordering::Greater {
        one.subtract(&angle)
    } else {
        angle
    };
    let value = sine_of_reference_angle(&reference)?;
    Some(if is_negated { value.negated() } else { value })
}

pub(crate) fn cosine_of_pi_multiple(half_turns: &ExactRational) -> Option<SquareRootSum> {
    sine_of_pi_multiple(&half_turns.plus(&fraction(1, 2)))
}

pub(crate) enum TangentOfPiMultiple {
    Value(SquareRootSum),
    Pole,
    NotSpecial,
}

pub(crate) fn tangent_of_pi_multiple(half_turns: &ExactRational) -> TangentOfPiMultiple {
    let (Some(sine), Some(cosine)) = (
        sine_of_pi_multiple(half_turns),
        cosine_of_pi_multiple(half_turns),
    ) else {
        return TangentOfPiMultiple::NotSpecial;
    };
    match sine.divided_by(&cosine) {
        Some(value) => TangentOfPiMultiple::Value(value),
        None => TangentOfPiMultiple::Pole,
    }
}

fn principal_angles() -> Vec<ExactRational> {
    let positive = REFERENCE_DENOMINATORS.map(|denominator| fraction(1, denominator));
    positive
        .iter()
        .map(ExactRational::negated)
        .chain([ExactRational::zero()])
        .chain(positive.iter().cloned())
        .collect()
}

pub(crate) fn arcsine_as_pi_multiple(value: &SquareRootSum) -> Option<ExactRational> {
    principal_angles()
        .into_iter()
        .find(|angle| sine_of_pi_multiple(angle).as_ref() == Some(value))
}

pub(crate) fn arccosine_as_pi_multiple(value: &SquareRootSum) -> Option<ExactRational> {
    arcsine_as_pi_multiple(value).map(|angle| fraction(1, 2).subtract(&angle))
}

pub(crate) fn arctangent_as_pi_multiple(value: &SquareRootSum) -> Option<ExactRational> {
    let right_angle = fraction(1, 2);
    principal_angles()
        .into_iter()
        .filter(|angle| angle.absolute() != right_angle)
        .find(|angle| match tangent_of_pi_multiple(angle) {
            TangentOfPiMultiple::Value(tangent) => tangent == *value,
            TangentOfPiMultiple::Pole | TangentOfPiMultiple::NotSpecial => false,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(radicand: i64) -> SquareRootSum {
        SquareRootSum::square_root_of_rational(&ExactRational::from_i64(radicand)).unwrap()
    }

    fn rational(numerator: i64, denominator: i64) -> SquareRootSum {
        SquareRootSum::from_rational(fraction(numerator, denominator))
    }

    #[test]
    fn sine_of_sixth_of_pi_is_one_half() {
        assert_eq!(sine_of_pi_multiple(&fraction(1, 6)), Some(rational(1, 2)));
    }

    #[test]
    fn sine_of_quarter_pi_is_half_root_two() {
        assert_eq!(sine_of_pi_multiple(&fraction(1, 4)), Some(half_root(2)));
    }

    #[test]
    fn sine_of_pi_is_zero() {
        assert_eq!(
            sine_of_pi_multiple(&fraction(1, 1)),
            Some(SquareRootSum::zero())
        );
    }

    #[test]
    fn sine_of_fifth_of_pi_is_not_special() {
        assert_eq!(sine_of_pi_multiple(&fraction(1, 5)), None);
    }

    fn table_angles() -> Vec<ExactRational> {
        (-12..=12)
            .flat_map(|numerator| {
                [1, 2, 3, 4, 6].map(|denominator| fraction(numerator, denominator))
            })
            .collect()
    }

    #[test]
    fn sine_and_cosine_satisfy_the_trigonometric_pythagoras_on_all_table_angles() {
        let one = rational(1, 1);

        let failures: Vec<ExactRational> = table_angles()
            .into_iter()
            .filter(|angle| {
                let sine = sine_of_pi_multiple(angle).unwrap();
                let cosine = cosine_of_pi_multiple(angle).unwrap();
                sine.times(&sine).plus(&cosine.times(&cosine)) != one
            })
            .collect();

        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn sine_satisfies_the_addition_theorem_on_all_table_angle_pairs() {
        let angles = table_angles();
        let mut failures = Vec::new();

        for left in &angles {
            for right in &angles {
                let Some(sum_sine) = sine_of_pi_multiple(&left.plus(right)) else {
                    continue;
                };
                let expansion = sine_of_pi_multiple(left)
                    .unwrap()
                    .times(&cosine_of_pi_multiple(right).unwrap())
                    .plus(
                        &cosine_of_pi_multiple(left)
                            .unwrap()
                            .times(&sine_of_pi_multiple(right).unwrap()),
                    );
                if sum_sine != expansion {
                    failures.push((left.clone(), right.clone()));
                }
            }
        }

        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn sine_is_positive_exactly_on_the_open_upper_half_turn() {
        let failures: Vec<ExactRational> = table_angles()
            .into_iter()
            .filter(|angle| {
                let reduced = reduced_to_one_turn(angle);
                let is_upper = reduced.sign() == Ordering::Greater
                    && reduced.compare(&ExactRational::one()) == Ordering::Less;
                let is_positive = sine_of_pi_multiple(angle).unwrap().sign() == Ordering::Greater;
                is_upper != is_positive
            })
            .collect();

        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn sine_of_third_of_pi_is_half_root_three() {
        assert_eq!(sine_of_pi_multiple(&fraction(1, 3)), Some(half_root(3)));
    }

    #[test]
    fn cosine_of_third_of_pi_is_one_half() {
        assert_eq!(cosine_of_pi_multiple(&fraction(1, 3)), Some(rational(1, 2)));
    }

    #[test]
    fn tangent_of_quarter_pi_is_one() {
        assert!(matches!(
            tangent_of_pi_multiple(&fraction(1, 4)),
            TangentOfPiMultiple::Value(value) if value == rational(1, 1)
        ));
    }

    #[test]
    fn tangent_of_sixth_of_pi_is_root_three_over_three() {
        let expected = root(3).times(&rational(1, 3));

        assert!(matches!(
            tangent_of_pi_multiple(&fraction(1, 6)),
            TangentOfPiMultiple::Value(value) if value == expected
        ));
    }

    #[test]
    fn tangent_of_half_pi_is_a_pole() {
        assert!(matches!(
            tangent_of_pi_multiple(&fraction(-3, 2)),
            TangentOfPiMultiple::Pole
        ));
    }

    #[test]
    fn arcsine_of_minus_half_root_three_is_minus_third_of_pi() {
        assert_eq!(
            arcsine_as_pi_multiple(&half_root(3).negated()),
            Some(fraction(-1, 3))
        );
    }

    #[test]
    fn arcsine_of_non_special_value_is_none() {
        assert_eq!(arcsine_as_pi_multiple(&rational(1, 3)), None);
    }

    #[test]
    fn arccosine_of_minus_one_is_pi() {
        assert_eq!(
            arccosine_as_pi_multiple(&rational(-1, 1)),
            Some(fraction(1, 1))
        );
    }

    #[test]
    fn arctangent_of_root_three_is_third_of_pi() {
        assert_eq!(arctangent_as_pi_multiple(&root(3)), Some(fraction(1, 3)));
    }

    #[test]
    fn arctangent_of_minus_one_is_minus_quarter_pi() {
        assert_eq!(
            arctangent_as_pi_multiple(&rational(-1, 1)),
            Some(fraction(-1, 4))
        );
    }
}
