use std::cmp::Ordering;

use calc_expr::{ExprId, ExprPool, Head, NodeView, Operator};

use crate::complex_construction::ExactComplexValue;
use crate::decimal_enclosure::enclose_decimal;
use crate::exact_evaluation::evaluate_exact;
use crate::exact_rational::ExactRational;
use crate::polynomial::{
    AtomTable, polynomial_expression, polynomial_of, rational_of, with_algebraic_atoms_reduced,
    with_angles_expanded, with_exponentials_expanded, with_imaginary_unit_reduced,
    with_logarithms_expanded, with_pythagoras,
};

const ENCLOSURE_DIGITS: u32 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Holds,
    Fails,
    Undecided,
}

fn holds_for(operator: Operator, order: Ordering) -> Option<bool> {
    let holds = match operator {
        Operator::Equal => order == Ordering::Equal,
        Operator::NotEqual => order != Ordering::Equal,
        Operator::Less => order == Ordering::Less,
        Operator::LessOrEqual => order != Ordering::Greater,
        Operator::Greater => order == Ordering::Greater,
        Operator::GreaterOrEqual => order != Ordering::Less,
        _ => return None,
    };
    Some(holds)
}

pub(crate) fn decimal_sign(pool: &mut ExprPool, difference: ExprId) -> Option<Ordering> {
    let never_cancelled = || false;
    let enclosure = enclose_decimal(pool, difference, ENCLOSURE_DIGITS, &never_cancelled).ok()?;
    let zero = ExactRational::zero();
    let lower = ExactRational::from_number(&enclosure.lower)?;
    let upper = ExactRational::from_number(&enclosure.upper)?;
    if lower.sign() == Ordering::Greater && upper.sign() == Ordering::Greater {
        return Some(Ordering::Greater);
    }
    if lower.sign() == Ordering::Less && upper.sign() == Ordering::Less {
        return Some(Ordering::Less);
    }
    if lower == zero && upper == zero {
        return Some(Ordering::Equal);
    }
    None
}

pub fn is_relation(pool: &ExprPool, expression: ExprId) -> bool {
    match pool.node(expression) {
        Ok(NodeView::Apply {
            head: Head::Operator(operator),
            arguments: [_, _],
        }) => holds_for(operator, Ordering::Equal).is_some(),
        _ => false,
    }
}

pub fn check_relation(pool: &mut ExprPool, expression: ExprId) -> Option<Verdict> {
    let (operator, left, right) = match pool.node(expression).ok()? {
        NodeView::Apply {
            head: Head::Operator(operator),
            arguments: [left, right],
        } => (operator, *left, *right),
        _ => return None,
    };
    holds_for(operator, Ordering::Equal)?;
    let difference = pool
        .apply(Head::Operator(Operator::Sub), &[left, right])
        .ok()?;
    if let Some(verdict) = complex_verdict(pool, operator, difference) {
        return Some(verdict);
    }
    let Ok(evaluation) = evaluate_exact(pool, difference) else {
        return Some(Verdict::Undecided);
    };
    let order = match evaluation
        .rational_value()
        .and_then(ExactRational::from_number)
    {
        Some(value) => Some(value.sign()),
        None => enclosed_order(pool, left, right).or_else(|| decimal_sign(pool, difference)),
    };
    let Some(order) = order else {
        return Some(Verdict::Undecided);
    };
    Some(match holds_for(operator, order) {
        Some(true) => Verdict::Holds,
        Some(false) => Verdict::Fails,
        None => Verdict::Undecided,
    })
}

fn enclosed_order(pool: &mut ExprPool, left: ExprId, right: ExprId) -> Option<Ordering> {
    let left = crate::quantities::to_coherent_units(pool, left).ok()?;
    let right = crate::quantities::to_coherent_units(pool, right).ok()?;
    if left.dimension != right.dimension {
        return None;
    }
    let left_enclosure = crate::machine_evaluation::real_enclosure(pool, left.expression)?;
    let right_enclosure = crate::machine_evaluation::real_enclosure(pool, right.expression)?;
    if left.expression == right.expression {
        return Some(Ordering::Equal);
    }
    if left_enclosure.upper() < right_enclosure.lower() {
        Some(Ordering::Less)
    } else if right_enclosure.upper() < left_enclosure.lower() {
        Some(Ordering::Greater)
    } else {
        None
    }
}

fn complex_verdict(pool: &mut ExprPool, operator: Operator, difference: ExprId) -> Option<Verdict> {
    let value = crate::complex_construction::exact_complex(pool, difference)?;
    Some(match value {
        Ok(ExactComplexValue::Real(real)) => match exact_sign(pool, real) {
            Some(order) => match holds_for(operator, order) {
                Some(true) => Verdict::Holds,
                Some(false) => Verdict::Fails,
                None => Verdict::Undecided,
            },
            None => Verdict::Undecided,
        },
        Ok(ExactComplexValue::Rational { .. } | ExactComplexValue::Algebraic(_)) => {
            match operator {
                Operator::Equal => Verdict::Fails,
                Operator::NotEqual => Verdict::Holds,
                _ => Verdict::Undecided,
            }
        }
        Err(_) => Verdict::Undecided,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeVerdict {
    HoldsThroughout,
    FailsThroughout,
    HoldsOnlyInPart,
}

fn exact_sign(pool: &mut ExprPool, expression: ExprId) -> Option<Ordering> {
    let evaluation = evaluate_exact(pool, expression).ok()?;
    match evaluation
        .rational_value()
        .and_then(ExactRational::from_number)
    {
        Some(value) => Some(value.sign()),
        None => decimal_sign(pool, expression),
    }
}

fn signs_between(low: Ordering, high: Ordering) -> Vec<Ordering> {
    [Ordering::Less, Ordering::Equal, Ordering::Greater]
        .into_iter()
        .filter(|sign| *sign >= low && *sign <= high)
        .collect()
}

pub fn check_relation_over_ranges(pool: &mut ExprPool, expression: ExprId) -> Option<RangeVerdict> {
    let (operator, left, right) = match pool.node(expression).ok()? {
        NodeView::Apply {
            head: Head::Operator(operator),
            arguments: [left, right],
        } => (operator, *left, *right),
        _ => return None,
    };
    holds_for(operator, Ordering::Equal)?;
    let difference = pool
        .apply(Head::Operator(Operator::Sub), &[left, right])
        .ok()?;
    let bounds = crate::worst_case::worst_case(pool, difference)?.ok()?;
    let low = exact_sign(pool, bounds.low)?;
    let high = exact_sign(pool, bounds.high)?;
    let covered: Vec<bool> = signs_between(low, high)
        .into_iter()
        .map(|sign| holds_for(operator, sign))
        .collect::<Option<_>>()?;
    let at_ends = (holds_for(operator, low)?, holds_for(operator, high)?);
    if covered.iter().all(|holds| *holds) {
        Some(RangeVerdict::HoldsThroughout)
    } else if covered.iter().all(|holds| !*holds) {
        Some(RangeVerdict::FailsThroughout)
    } else if at_ends.0 != at_ends.1 {
        Some(RangeVerdict::HoldsOnlyInPart)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityVerdict {
    HoldsEverywhere,
    FailsEverywhere,
    NotEverywhere,
    Undecided,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Identity {
    Decided(IdentityVerdict),
    HoldsWhereDefined(Vec<ExprId>),
    FailsWhereDefined(Vec<ExprId>),
}

pub fn check_identity(pool: &mut ExprPool, expression: ExprId) -> Option<Identity> {
    let (operator, left, right) = match pool.node(expression).ok()? {
        NodeView::Apply {
            head: Head::Operator(operator),
            arguments: [left, right],
        } => (operator, *left, *right),
        _ => return None,
    };
    holds_for(operator, Ordering::Equal)?;
    let mut atoms = AtomTable::default();
    let left = rational_of(pool, left, &mut atoms)?;
    let right = rational_of(pool, right, &mut atoms)?;
    let difference = left.plus(&right.negated())?;
    if atoms.holds_an_atom_outside_the_field(pool) {
        return Some(Identity::Decided(IdentityVerdict::Undecided));
    }
    let denominator = with_algebraic_atoms_reduced(pool, difference.denominator(), &mut atoms)
        .unwrap_or_else(|| difference.denominator().clone());
    if denominator
        .as_constant()
        .is_some_and(|constant| constant.is_zero())
    {
        return Some(Identity::Decided(IdentityVerdict::Undecided));
    }
    let expanded = with_angles_expanded(pool, difference.numerator(), &mut atoms)
        .unwrap_or_else(|| difference.numerator().clone());
    let expanded = with_exponentials_expanded(pool, &expanded, &mut atoms).unwrap_or(expanded);
    let expanded = with_logarithms_expanded(pool, &expanded, &mut atoms).unwrap_or(expanded);
    let numerator = with_pythagoras(pool, &expanded, &mut atoms)?;
    let numerator = with_algebraic_atoms_reduced(pool, &numerator, &mut atoms).unwrap_or(numerator);
    let Some(constant) = numerator.as_constant() else {
        if atoms.holds_only_the_imaginary_unit(pool, &numerator) {
            return Some(Identity::Decided(match operator {
                Operator::Equal => IdentityVerdict::FailsEverywhere,
                Operator::NotEqual => IdentityVerdict::HoldsEverywhere,
                _ => IdentityVerdict::Undecided,
            }));
        }
        let decidable = operator == Operator::Equal
            && atoms.every_atom_is_a_free_name_or_the_imaginary_unit(pool);
        return Some(Identity::Decided(if decidable {
            IdentityVerdict::NotEverywhere
        } else {
            IdentityVerdict::Undecided
        }));
    };
    let holds = holds_for(operator, constant.sign());
    if !difference.excluded().is_empty() {
        return Some(match holds {
            Some(true) if constant.is_zero() => {
                Identity::HoldsWhereDefined(difference.excluded().to_vec())
            }
            Some(false) if constant.is_zero() => {
                Identity::FailsWhereDefined(difference.excluded().to_vec())
            }
            Some(false) if operator == Operator::Equal && atoms.every_atom_is_a_free_name(pool) => {
                Identity::Decided(IdentityVerdict::NotEverywhere)
            }
            _ => Identity::Decided(IdentityVerdict::Undecided),
        });
    }
    Some(Identity::Decided(match holds {
        Some(true) => IdentityVerdict::HoldsEverywhere,
        Some(false) => IdentityVerdict::FailsEverywhere,
        None => IdentityVerdict::Undecided,
    }))
}

fn expanded_side(pool: &mut ExprPool, expression: ExprId) -> Option<ExprId> {
    let mut atoms = AtomTable::default();
    let polynomial = polynomial_of(pool, expression, &mut atoms)?;
    let polynomial =
        with_imaginary_unit_reduced(pool, &polynomial, &mut atoms).unwrap_or(polynomial);
    polynomial_expression(pool, &polynomial, &atoms)
}

pub fn expanded(pool: &mut ExprPool, expression: ExprId) -> Option<ExprId> {
    if let Ok(NodeView::Apply {
        head: Head::Operator(operator),
        arguments: [left, right],
    }) = pool.node(expression)
        && holds_for(operator, Ordering::Equal).is_some()
    {
        let (left, right) = (*left, *right);
        let left = expanded_side(pool, left)?;
        let right = expanded_side(pool, right)?;
        return pool.apply(Head::Operator(operator), &[left, right]).ok();
    }
    expanded_side(pool, expression)
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_numbers::Integer;

    fn identity_of(text: &str) -> Option<IdentityVerdict> {
        match identity(text)? {
            Identity::Decided(verdict) => Some(verdict),
            Identity::HoldsWhereDefined(_) | Identity::FailsWhereDefined(_) => None,
        }
    }

    fn identity(text: &str) -> Option<Identity> {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, text).ok()?;
        check_identity(&mut pool, expression)
    }

    fn telescoping(pairs: u32) -> String {
        let terms: Vec<String> = (0..pairs)
            .map(|k| format!("(1/(x+{}) - 1/(x+{}))", k + 1, k + 2))
            .collect();
        format!("{} = 1/(x+1) - 1/(x+{})", terms.join(" + "), pairs + 1)
    }

    #[test]
    fn a_telescoping_sum_of_forty_pairs_holds_where_defined() {
        assert!(matches!(
            identity(&telescoping(40)),
            Some(Identity::HoldsWhereDefined(_))
        ));
    }

    #[test]
    fn a_telescoping_sum_of_sixty_three_pairs_holds_where_defined() {
        assert!(matches!(
            identity(&telescoping(63)),
            Some(Identity::HoldsWhereDefined(_))
        ));
    }

    #[test]
    fn a_product_of_conjugates_is_the_sum_of_squares_for_every_value() {
        assert!(matches!(
            identity("(x+i)*(x-i) = x^2 + 1"),
            Some(Identity::Decided(IdentityVerdict::HoldsEverywhere))
        ));
    }

    #[test]
    fn a_claim_about_i_alone_is_decided() {
        assert_eq!(
            identity_of("i*i = -1"),
            Some(IdentityVerdict::HoldsEverywhere)
        );
        assert_eq!(
            identity_of("i != 1"),
            Some(IdentityVerdict::HoldsEverywhere)
        );
        assert_eq!(identity_of("i = 1"), Some(IdentityVerdict::FailsEverywhere));
    }

    #[test]
    fn a_denominator_that_vanishes_through_i_squared_decides_nothing() {
        assert_eq!(
            identity_of("x/(i^2+1) = 0"),
            Some(IdentityVerdict::Undecided)
        );
    }

    #[test]
    fn a_complex_difference_in_a_name_is_no_identity() {
        assert_eq!(
            identity_of("(x+i)^2 = x^2 + 1"),
            Some(IdentityVerdict::NotEverywhere)
        );
    }

    #[test]
    fn an_expansion_uses_i_squared_as_minus_one() {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, "(x+i)^2").unwrap();
        let expanded = expanded(&mut pool, expression).unwrap();

        assert_eq!(
            calc_syntax::print_expression(&pool, expanded, calc_syntax::PrintMode::Ascii).unwrap(),
            "x^2 + 2 * x * i - 1"
        );
    }

    fn written(pool: &ExprPool, denominators: &[ExprId]) -> Vec<String> {
        denominators
            .iter()
            .map(|denominator| {
                calc_syntax::print_expression(pool, *denominator, calc_syntax::PrintMode::Ascii)
                    .expect("a denominator is printable")
            })
            .collect()
    }

    fn excluded(text: &str) -> Vec<String> {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, text).expect("claim reads");
        match check_identity(&mut pool, expression) {
            Some(Identity::HoldsWhereDefined(denominators)) => written(&pool, &denominators),
            other => panic!("{text} is {other:?} and not a conditional identity"),
        }
    }

    #[test]
    fn a_cancelling_quotient_names_the_denominator_it_was_written_with() {
        assert_eq!(excluded("(x^2-1)/(x-1) = x+1"), ["x - 1"]);
    }

    #[test]
    fn every_denominator_is_named_once_in_the_order_it_appears() {
        assert_eq!(
            excluded("1/(x-1) + 1/(x+1) = 2*x/(x^2-1)"),
            ["x - 1", "x + 1", "x^2 - 1"]
        );
    }

    #[test]
    fn the_same_denominator_on_both_sides_is_named_once() {
        assert_eq!(excluded("1/x = 1/x"), ["x"]);
    }

    fn fails_excluding(text: &str) -> Vec<String> {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, text).expect("claim reads");
        match check_identity(&mut pool, expression) {
            Some(Identity::FailsWhereDefined(denominators)) => written(&pool, &denominators),
            other => panic!("{text} is {other:?} and not a conditional failure"),
        }
    }

    #[test]
    fn a_relation_that_fails_where_it_is_defined_says_where_it_fails() {
        assert_eq!(fails_excluding("x/x != 1"), ["x"]);
        assert_eq!(fails_excluding("1/x != 1/x"), ["x"]);
        assert_eq!(fails_excluding("1/x > 1/x"), ["x"]);
    }

    #[test]
    fn a_relation_that_fails_everywhere_keeps_the_unconditional_verdict() {
        assert_eq!(
            identity("x/2 != 0.5*x"),
            Some(Identity::Decided(IdentityVerdict::FailsEverywhere))
        );
    }

    #[test]
    fn a_difference_holding_a_constant_is_undecided_and_not_no_identity() {
        for claim in ["3 = pi", "pi = 4*atan(1)", "3*x = pi*x"] {
            assert_eq!(
                identity_of(claim),
                Some(IdentityVerdict::Undecided),
                "{claim}"
            );
        }
    }

    #[test]
    fn the_algebra_reads_no_verdict_off_a_difference_holding_infinity() {
        for claim in ["inf + 1 = inf", "inf > 0", "inf - inf = 0", "1/inf = 0"] {
            assert_eq!(
                identity_of(claim),
                Some(IdentityVerdict::Undecided),
                "{claim}"
            );
        }
    }

    #[test]
    fn algebra_decides_a_claim_over_constants_before_anything_reads_their_values() {
        for claim in [
            "(pi+1)^2 = pi^2 + 2*pi + 1",
            "e*pi = pi*e",
            "2*pi = pi + pi",
        ] {
            assert_eq!(
                identity_of(claim),
                Some(IdentityVerdict::HoldsEverywhere),
                "{claim}"
            );
        }
    }

    #[test]
    fn a_difference_that_is_not_zero_takes_no_conditional_verdict() {
        for claim in ["1/x > 0", "1/x + 5 > 5"] {
            assert_eq!(
                identity(claim),
                Some(Identity::Decided(IdentityVerdict::Undecided)),
                "{claim}"
            );
        }
    }

    #[test]
    fn an_equation_whose_difference_is_never_zero_is_not_an_identity() {
        for claim in ["1/x + 5 = 5", "1/x = 2/x"] {
            assert_eq!(
                identity(claim),
                Some(Identity::Decided(IdentityVerdict::NotEverywhere)),
                "{claim}"
            );
        }
    }

    #[test]
    fn a_constant_denominator_leaves_the_verdict_unconditional() {
        assert_eq!(
            identity("x/2 = 0.5*x"),
            Some(Identity::Decided(IdentityVerdict::HoldsEverywhere))
        );
    }

    #[test]
    fn a_conditional_verdict_is_never_reached_with_nothing_to_exclude() {
        for claim in [
            "1/x = 1/x",
            "x/x = 1",
            "(x^2-1)/(x-1) = x+1",
            "1/(x-1) + 1/(x+1) = 2*x/(x^2-1)",
            "x/2 = 0.5*x",
            "(x+1)^2 = x^2 + 2*x + 1",
            "x/y = x/y",
            "x/x != 1",
            "1/x > 1/x",
            "x/2 != 0.5*x",
        ] {
            match identity(claim) {
                Some(Identity::HoldsWhereDefined(denominators))
                | Some(Identity::FailsWhereDefined(denominators)) => {
                    assert!(!denominators.is_empty(), "{claim} excludes nothing");
                }
                _ => {}
            }
        }
    }

    #[test]
    fn a_binomial_identity_holds_for_every_value() {
        assert_eq!(
            identity_of("(x + 1)^2 = x^2 + 2*x + 1"),
            Some(IdentityVerdict::HoldsEverywhere)
        );
    }

    #[test]
    fn a_wrong_binomial_claim_is_shown_not_to_be_an_identity() {
        assert_eq!(
            identity_of("(x + 1)^2 = x^2 + 1"),
            Some(IdentityVerdict::NotEverywhere)
        );
    }

    #[test]
    fn two_constants_that_differ_are_decided_everywhere() {
        assert_eq!(
            identity_of("2 + 2 = 5"),
            Some(IdentityVerdict::FailsEverywhere)
        );
    }

    #[test]
    fn an_inequality_between_constants_is_decided_everywhere() {
        assert_eq!(
            identity_of("x + 1 > x"),
            Some(IdentityVerdict::HoldsEverywhere)
        );
    }

    #[test]
    fn an_identity_over_an_opaque_call_is_still_decided() {
        assert_eq!(
            identity_of("(sin(x) + 1)^2 = sin(x)^2 + 2*sin(x) + 1"),
            Some(IdentityVerdict::HoldsEverywhere)
        );
    }

    #[test]
    fn a_true_identity_that_needs_more_than_algebra_stays_undecided() {
        assert_eq!(
            identity_of("sqrt(x^2) = abs(x)"),
            Some(IdentityVerdict::Undecided)
        );
    }

    #[test]
    fn an_expression_that_is_not_a_relation_has_no_identity_verdict() {
        assert_eq!(identity_of("x + 1"), None);
    }

    fn checked(text: &str) -> Option<Verdict> {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, text).ok()?;
        check_relation(&mut pool, expression)
    }

    #[test]
    fn an_equation_between_exact_complex_values_is_decided() {
        assert_eq!(checked("i*i = -1"), Some(Verdict::Holds));
        assert_eq!(checked("(1+i)^2 = 2*i"), Some(Verdict::Holds));
        assert_eq!(checked("i = 1"), Some(Verdict::Fails));
    }

    #[test]
    fn a_complex_value_that_differs_is_not_equal() {
        assert_eq!(checked("i != 1"), Some(Verdict::Holds));
    }

    #[test]
    fn an_order_with_a_non_real_difference_stays_undecided_here() {
        assert_eq!(checked("1 < i"), Some(Verdict::Undecided));
    }

    #[test]
    fn disjoint_f64_enclosures_decide_a_claim_about_arcsines() {
        assert_eq!(checked("asin(1/2) > asin(1/3)"), Some(Verdict::Holds));
        assert_eq!(checked("asin(1/2) <= asin(1/3)"), Some(Verdict::Fails));
        assert_eq!(checked("acos(1/2) > acos(2/3)"), Some(Verdict::Holds));
    }

    #[test]
    fn a_claim_between_two_identical_sides_with_a_value_holds() {
        assert_eq!(checked("exp(1) = exp(1)"), Some(Verdict::Holds));
        assert_eq!(checked("asin(2) = asin(2)"), Some(Verdict::Undecided));
    }

    #[test]
    fn overlapping_enclosures_fall_to_the_decimal_enclosure() {
        assert_eq!(
            checked("e > 2.718281828459045235360287471352662497757"),
            Some(Verdict::Holds)
        );
    }

    #[test]
    fn the_enclosures_of_quantities_are_compared_in_coherent_units() {
        assert_eq!(
            checked("asin(1/3) * 1 km < asin(1/2) * 100 m"),
            Some(Verdict::Fails)
        );
    }

    fn verdict_of(build: impl FnOnce(&mut ExprPool) -> ExprId) -> Option<Verdict> {
        let mut pool = ExprPool::new();
        let expression = build(&mut pool);
        check_relation(&mut pool, expression)
    }

    fn whole(pool: &mut ExprPool, value: i64) -> ExprId {
        pool.number(Integer::from(value).into()).unwrap()
    }

    fn relation(operator: Operator, left: i64, right: i64) -> Option<Verdict> {
        verdict_of(|pool| {
            let left = whole(pool, left);
            let right = whole(pool, right);
            pool.apply(Head::Operator(operator), &[left, right])
                .unwrap()
        })
    }

    #[test]
    fn an_equality_that_holds_is_stated_as_holding() {
        assert_eq!(relation(Operator::Equal, 2, 2), Some(Verdict::Holds));
    }

    #[test]
    fn an_equality_that_does_not_hold_fails() {
        assert_eq!(relation(Operator::Equal, 2, 3), Some(Verdict::Fails));
    }

    #[test]
    fn a_strict_order_that_holds_is_stated_as_holding() {
        assert_eq!(relation(Operator::Less, 2, 3), Some(Verdict::Holds));
    }

    #[test]
    fn an_order_that_is_not_strict_admits_equality() {
        assert_eq!(relation(Operator::LessOrEqual, 5, 5), Some(Verdict::Holds));
    }

    #[test]
    fn an_expression_that_is_not_a_relation_has_no_verdict() {
        assert_eq!(relation(Operator::Add, 1, 2), None);
    }

    #[test]
    fn the_trigonometric_pythagoras_holds_for_every_value() {
        assert_eq!(
            identity_of("sin(x)^2 + cos(x)^2 = 1"),
            Some(IdentityVerdict::HoldsEverywhere)
        );
    }

    #[test]
    fn a_higher_power_of_the_cosine_is_reduced_by_the_same_rewrite() {
        assert_eq!(
            identity_of("cos(x)^4 - sin(x)^4 = cos(x)^2 - sin(x)^2"),
            Some(IdentityVerdict::HoldsEverywhere)
        );
    }

    #[test]
    fn the_rewrite_list_holds_no_inverse_function() {
        assert_eq!(
            identity_of("asin(sin(x)) = x"),
            Some(IdentityVerdict::Undecided)
        );
    }

    #[test]
    fn a_triple_angle_is_expanded_over_its_base_angle() {
        assert_eq!(
            identity_of("cos(3*x) = 4*cos(x)^3 - 3*cos(x)"),
            Some(IdentityVerdict::HoldsEverywhere)
        );
    }

    #[test]
    fn a_half_angle_becomes_the_base_angle() {
        assert_eq!(
            identity_of("sin(x/2)^2 = (1 - cos(x))/2"),
            Some(IdentityVerdict::HoldsEverywhere)
        );
    }

    #[test]
    fn a_shift_by_a_special_angle_folds_to_its_exact_values() {
        assert_eq!(
            identity_of("sin(x + pi/3) = sin(x)/2 + sqrt(3)*cos(x)/2"),
            Some(IdentityVerdict::HoldsEverywhere)
        );
    }

    #[test]
    fn a_non_linear_argument_is_not_expanded() {
        assert_ne!(
            identity_of("sin(x^2) = sin(x)^2"),
            Some(IdentityVerdict::HoldsEverywhere)
        );
    }

    #[test]
    fn a_sine_and_a_cosine_of_different_arguments_are_not_tied() {
        assert_ne!(
            identity_of("sin(x)^2 + cos(y)^2 = 1"),
            Some(IdentityVerdict::HoldsEverywhere)
        );
    }
}
