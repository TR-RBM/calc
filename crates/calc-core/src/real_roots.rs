use std::cmp::Ordering;

use calc_expr::{BinderKind, ExprId, ExprPool, NodeView};
use calc_numbers::Integer;

use crate::exact_evaluation::closed_square_root_sum;
use crate::exact_rational::ExactRational;
use crate::polynomial::{AtomTable, Polynomial, polynomial_of};
use crate::square_root_sum::SquareRootSum;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RootRefusal {
    NotAPolynomial,
    ZeroPolynomial,
    IndexNotWhole,
    IndexOutOfRange(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Dyadic {
    numerator: Integer,
    exponent: u32,
}

impl Dyadic {
    fn scaled_to(&self, exponent: u32) -> Integer {
        &self.numerator * &Integer::from(2_i64).pow(exponent - self.exponent)
    }

    fn midpoint(&self, other: &Self) -> Self {
        let exponent = self.exponent.max(other.exponent) + 1;
        Dyadic {
            numerator: &self.scaled_to(exponent - 1) + &other.scaled_to(exponent - 1),
            exponent,
        }
    }

    fn reduced(mut self) -> Self {
        let two = Integer::from(2_i64);
        while self.exponent > 0 {
            match self.numerator.div_rem_euclid(&two) {
                Ok((half, rest)) if rest.is_zero() => {
                    self.numerator = half;
                    self.exponent -= 1;
                }
                _ => break,
            }
        }
        self
    }

    fn power_of_two(exponent: u32, is_negative: bool) -> Self {
        let magnitude = Integer::from(2_i64).pow(exponent);
        Dyadic {
            numerator: if is_negative {
                magnitude.negated()
            } else {
                magnitude
            },
            exponent: 0,
        }
    }

    fn compare(&self, other: &Self) -> Ordering {
        let exponent = self.exponent.max(other.exponent);
        self.scaled_to(exponent).cmp(&other.scaled_to(exponent))
    }

    fn to_rational(&self) -> ExactRational {
        ExactRational::fraction(&self.numerator, &Integer::from(2_i64).pow(self.exponent))
            .unwrap_or_else(ExactRational::zero)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RealRoot {
    polynomial: Vec<ExactRational>,
    integers: Vec<Integer>,
    derivative: Vec<Integer>,
    lower: Dyadic,
    upper: Dyadic,
    exact: Option<ExactRational>,
}

fn integer_coefficients(coefficients: &[ExactRational]) -> Vec<Integer> {
    let mut multiple = Integer::one();
    for coefficient in coefficients {
        let denominator = coefficient.denominator();
        let common = multiple.gcd(denominator);
        let (quotient, _) = denominator
            .div_rem_euclid(&common)
            .unwrap_or((Integer::one(), Integer::zero()));
        multiple = &multiple * &quotient;
    }
    coefficients
        .iter()
        .map(|coefficient| {
            coefficient
                .multiply(&ExactRational::from_integer(multiple.clone()))
                .numerator()
                .clone()
        })
        .collect()
}

fn scaled_value(integers: &[Integer], point: &Dyadic) -> Integer {
    let power = Integer::from(2_i64).pow(point.exponent);
    let mut total = Integer::zero();
    let mut scale = Integer::one();
    for coefficient in integers.iter().rev() {
        total = &(&total * &point.numerator) + &(coefficient * &scale);
        scale = &scale * &power;
    }
    total
}

fn sign_at_dyadic(integers: &[Integer], point: &Dyadic) -> Ordering {
    scaled_value(integers, point).cmp(&Integer::zero())
}

fn integer_derivative(integers: &[Integer]) -> Vec<Integer> {
    integers
        .iter()
        .enumerate()
        .skip(1)
        .map(|(power, coefficient)| coefficient * &Integer::from(i64::try_from(power).unwrap_or(0)))
        .collect()
}

fn trimmed(mut coefficients: Vec<ExactRational>) -> Vec<ExactRational> {
    while coefficients.last().is_some_and(ExactRational::is_zero) {
        coefficients.pop();
    }
    coefficients
}

fn sign_at(coefficients: &[ExactRational], point: &ExactRational) -> Ordering {
    coefficients
        .iter()
        .rev()
        .fold(ExactRational::zero(), |total, coefficient| {
            total.multiply(point).plus(coefficient)
        })
        .sign()
}

pub(crate) fn derivative(coefficients: &[ExactRational]) -> Vec<ExactRational> {
    coefficients
        .iter()
        .enumerate()
        .skip(1)
        .map(|(power, coefficient)| {
            coefficient.multiply(&ExactRational::from_i64(i64::try_from(power).unwrap_or(0)))
        })
        .collect()
}

fn remainder(dividend: &[ExactRational], divisor: &[ExactRational]) -> Option<Vec<ExactRational>> {
    divided(dividend, divisor).map(|(_, rest)| rest)
}

pub(crate) fn divided(
    dividend: &[ExactRational],
    divisor: &[ExactRational],
) -> Option<(Vec<ExactRational>, Vec<ExactRational>)> {
    let divisor = trimmed(divisor.to_vec());
    let leading = divisor.last()?.clone();
    let mut rest = trimmed(dividend.to_vec());
    if rest.len() < divisor.len() {
        return Some((Vec::new(), rest));
    }
    let mut quotient = vec![ExactRational::zero(); rest.len() - divisor.len() + 1];
    while rest.len() >= divisor.len() && !rest.is_empty() {
        let shift = rest.len() - divisor.len();
        let factor = rest.last()?.divide(&leading)?;
        for (position, coefficient) in divisor.iter().enumerate() {
            rest[shift + position] = rest[shift + position].subtract(&factor.multiply(coefficient));
        }
        quotient[shift] = factor;
        rest.pop();
        rest = trimmed(rest);
    }
    Some((quotient, rest))
}

pub(crate) fn greatest_common_divisor(
    left: &[ExactRational],
    right: &[ExactRational],
) -> Option<Vec<ExactRational>> {
    let mut first = trimmed(left.to_vec());
    let mut second = trimmed(right.to_vec());
    while !second.is_empty() {
        let rest = remainder(&first, &second)?;
        first = second;
        second = rest;
    }
    Some(first)
}

pub(crate) struct SquareFree(Vec<ExactRational>);

impl SquareFree {
    pub(crate) fn coefficients(&self) -> &[ExactRational] {
        &self.0
    }
}

pub(crate) fn square_free_part(coefficients: &[ExactRational]) -> Option<SquareFree> {
    let common = greatest_common_divisor(coefficients, &derivative(coefficients))?;
    let (part, rest) = divided(coefficients, &common)?;
    rest.is_empty().then(|| SquareFree(trimmed(part)))
}

fn root_bound(coefficients: &[ExactRational]) -> Option<ExactRational> {
    let leading = coefficients.last()?.absolute();
    let largest = coefficients[..coefficients.len() - 1]
        .iter()
        .map(|coefficient| coefficient.absolute().divide(&leading))
        .try_fold(ExactRational::zero(), |largest, ratio| {
            let ratio = ratio?;
            Some(if ratio.compare(&largest) == Ordering::Greater {
                ratio
            } else {
                largest
            })
        })?;
    Some(ExactRational::one().plus(&largest))
}

fn scaled_on_interval(integers: &[Integer], lower: &Dyadic, upper: &Dyadic) -> Vec<Integer> {
    let exponent = lower.exponent.max(upper.exponent);
    let start = lower.scaled_to(exponent);
    let width = &upper.scaled_to(exponent) - &start;
    let step = Integer::from(2_i64).pow(exponent);
    let mut scale = Integer::one();
    let mut composed: Vec<Integer> = Vec::new();
    for coefficient in integers.iter().rev() {
        let mut next = vec![Integer::zero(); composed.len() + 1];
        for (power, term) in composed.iter().enumerate() {
            next[power] = &next[power] + &(term * &start);
            next[power + 1] = &next[power + 1] + &(term * &width);
        }
        next[0] = &next[0] + &(coefficient * &scale);
        scale = &scale * &step;
        composed = next;
    }
    composed
}

fn shifted_by_one(coefficients: &mut [Integer]) {
    let length = coefficients.len();
    for start in 0..length.saturating_sub(1) {
        for position in (start..length - 1).rev() {
            coefficients[position] = &coefficients[position] + &coefficients[position + 1];
        }
    }
}

fn sign_variations(integers: &[Integer], lower: &Dyadic, upper: &Dyadic) -> usize {
    let mut transformed = scaled_on_interval(integers, lower, upper);
    transformed.reverse();
    shifted_by_one(&mut transformed);
    let zero = Integer::zero();
    let signs: Vec<Ordering> = transformed
        .iter()
        .map(|coefficient| coefficient.cmp(&zero))
        .filter(|sign| *sign != Ordering::Equal)
        .collect();
    signs.windows(2).filter(|pair| pair[0] != pair[1]).count()
}

pub(crate) fn every_root_is_real(coefficients: &[ExactRational]) -> Option<bool> {
    let polynomial = square_free_part(&trimmed(coefficients.to_vec()))?;
    let degree = polynomial.coefficients().len().checked_sub(1)?;
    Some(real_roots(coefficients)?.len() == degree)
}

pub(crate) fn real_roots(coefficients: &[ExactRational]) -> Option<Vec<RealRoot>> {
    let square_free = square_free_part(&trimmed(coefficients.to_vec()))?;
    let polynomial = square_free.coefficients().to_vec();
    if polynomial.len() < 2 {
        return Some(Vec::new());
    }
    let bound = root_bound(&polynomial)?;
    let mut power = ExactRational::one();
    let mut exponent = 0_u32;
    while power.compare(&bound) == Ordering::Less {
        power = power.multiply(&ExactRational::from_i64(2));
        exponent += 1;
    }
    let mut pending = vec![(
        Dyadic::power_of_two(exponent, true),
        Dyadic::power_of_two(exponent, false),
    )];
    let integers = integer_coefficients(&polynomial);
    let isolated = |lower: &Dyadic, upper: &Dyadic| -> RealRoot {
        RealRoot {
            polynomial: polynomial.clone(),
            integers: integers.clone(),
            derivative: integer_derivative(&integers),
            lower: lower.clone(),
            upper: upper.clone(),
            exact: (lower == upper).then(|| lower.to_rational()),
        }
    };
    let mut found: Vec<RealRoot> = Vec::new();
    while let Some((lower, upper)) = pending.pop() {
        match sign_variations(&integers, &lower, &upper) {
            0 => {}
            1 => found.push(isolated(&lower, &upper)),
            _ => {
                let middle = lower.midpoint(&upper).reduced();
                if sign_at_dyadic(&integers, &middle) == Ordering::Equal {
                    found.push(isolated(&middle, &middle));
                }
                pending.push((lower, middle.clone()));
                pending.push((middle, upper));
            }
        }
    }
    if let Some(rational) = crate::polynomial_factoring::rational_roots_of_square_free(&square_free)
    {
        for root in &mut found {
            let (lower, upper) = (root.lower.to_rational(), root.upper.to_rational());
            if let Some(value) = rational.iter().find(|value| {
                value.compare(&lower) == Ordering::Greater
                    && value.compare(&upper) == Ordering::Less
            }) {
                root.exact = Some(value.clone());
            }
        }
    }
    found.sort_by(|left, right| left.lower.to_rational().compare(&right.lower.to_rational()));
    Some(found)
}

pub(crate) fn coefficients_in_bound_variable(
    pool: &mut ExprPool,
    body: ExprId,
) -> Result<Vec<ExactRational>, RootRefusal> {
    let variable = pool.bound(0).map_err(|_| RootRefusal::NotAPolynomial)?;
    let mut atoms = AtomTable::default();
    let polynomial = polynomial_of(pool, body, &mut atoms).ok_or(RootRefusal::NotAPolynomial)?;
    if atoms.expressions().iter().any(|atom| *atom != variable) {
        return Err(RootRefusal::NotAPolynomial);
    }
    univariate_coefficients(&polynomial).ok_or(RootRefusal::NotAPolynomial)
}

pub(crate) fn univariate_coefficients(polynomial: &Polynomial) -> Option<Vec<ExactRational>> {
    let mut coefficients: Vec<ExactRational> = Vec::new();
    for (monomial, coefficient) in polynomial.terms() {
        let degree = match monomial.factors() {
            [] => 0,
            [(_, exponent)] => usize::try_from(*exponent).ok()?,
            _ => return None,
        };
        let coefficient = coefficient.as_rational()?;
        if coefficients.len() <= degree {
            coefficients.resize(degree + 1, ExactRational::zero());
        }
        coefficients[degree] = coefficient;
    }
    Some(coefficients)
}

fn odd_multiplicity_roots(coefficients: &[ExactRational]) -> Option<Vec<RealRoot>> {
    let parts =
        crate::polynomial_factoring::square_free_parts(&integer_coefficients(coefficients))?;
    let mut roots = Vec::new();
    for (part, multiplicity) in parts {
        if multiplicity % 2 == 1 {
            let rational: Vec<ExactRational> =
                part.into_iter().map(ExactRational::from_integer).collect();
            roots.extend(real_roots(&rational)?);
        }
    }
    Some(roots)
}

pub(crate) fn sign_on_interval(
    coefficients: &[ExactRational],
    low: &ExactRational,
    high: &ExactRational,
) -> Option<Ordering> {
    let polynomial = trimmed(coefficients.to_vec());
    if polynomial.is_empty() {
        return Some(Ordering::Equal);
    }
    if low.compare(high) != Ordering::Less {
        return None;
    }
    for mut root in odd_multiplicity_roots(&polynomial)? {
        if root.compare_with(low) == Ordering::Greater && root.compare_with(high) == Ordering::Less
        {
            return None;
        }
    }
    let samples = polynomial.len();
    let width = high.subtract(low);
    let steps = Integer::from(u64::try_from(samples + 1).ok()?);
    (1..=samples).find_map(|step| {
        let share = ExactRational::fraction(&Integer::from(u64::try_from(step).ok()?), &steps)?;
        match sign_at(&polynomial, &low.plus(&width.multiply(&share))) {
            Ordering::Equal => None,
            sign => Some(sign),
        }
    })
}

pub(crate) fn vanishes_on_closed_interval(
    coefficients: &[ExactRational],
    low: &ExactRational,
    high: &ExactRational,
) -> Option<bool> {
    let polynomial = trimmed(coefficients.to_vec());
    if polynomial.is_empty() {
        return Some(true);
    }
    for mut root in real_roots(&polynomial)? {
        if root.compare_with(low) != Ordering::Less && root.compare_with(high) != Ordering::Greater
        {
            return Some(true);
        }
    }
    Some(false)
}

pub(crate) fn root_of(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Option<Result<RealRoot, RootRefusal>> {
    let NodeView::Bind {
        binder: BinderKind::Root,
        arguments,
        body,
    } = pool.node(expression).ok()?
    else {
        return None;
    };
    let (index, body) = (*arguments.first()?, body);
    Some(root_at(pool, body, index))
}

fn root_at(pool: &mut ExprPool, body: ExprId, index: ExprId) -> Result<RealRoot, RootRefusal> {
    let index = closed_square_root_sum(pool, index)
        .and_then(|value| value.as_rational())
        .filter(|value| value.is_integer() && value.sign() == Ordering::Greater)
        .and_then(|value| value.numerator().to_i64())
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(RootRefusal::IndexNotWhole)?;
    let coefficients = coefficients_in_bound_variable(pool, body)?;
    if coefficients.is_empty() {
        return Err(RootRefusal::ZeroPolynomial);
    }
    let roots = real_roots(&coefficients).ok_or(RootRefusal::NotAPolynomial)?;
    let count = roots.len();
    roots
        .into_iter()
        .nth(index - 1)
        .ok_or(RootRefusal::IndexOutOfRange(count))
}

impl RealRoot {
    pub(crate) fn exact(&self) -> Option<ExactRational> {
        self.exact.clone()
    }

    pub(crate) fn bounds(&self) -> (ExactRational, ExactRational) {
        match &self.exact {
            Some(value) => (value.clone(), value.clone()),
            None => (self.lower.to_rational(), self.upper.to_rational()),
        }
    }

    fn halved(&mut self) {
        if self.exact.is_some() {
            return;
        }
        let middle = self.lower.midpoint(&self.upper);
        let at_middle = sign_at_dyadic(&self.integers, &middle);
        if at_middle == Ordering::Equal {
            self.exact = Some(middle.to_rational());
            return;
        }
        let at_lower = sign_at_dyadic(&self.integers, &self.lower);
        let at_upper = sign_at_dyadic(&self.integers, &self.upper);
        let root_is_below = if at_lower != Ordering::Equal {
            at_lower != at_middle
        } else if at_upper != Ordering::Equal {
            at_upper == at_middle
        } else {
            sign_variations(&self.integers, &self.lower, &middle) == 1
        };
        if root_is_below {
            self.upper = middle;
        } else {
            self.lower = middle;
        }
    }

    fn width_exponent(&self) -> i64 {
        let exponent = self.lower.exponent.max(self.upper.exponent);
        let width = &self.upper.scaled_to(exponent) - &self.lower.scaled_to(exponent);
        i64::try_from(width.bit_length()).unwrap_or(i64::MAX) - i64::from(exponent)
    }

    fn newton_step(&mut self, target: u32) -> bool {
        let middle = self.lower.midpoint(&self.upper);
        let value = scaled_value(&self.integers, &middle);
        if value.is_zero() {
            self.exact = Some(middle.to_rational());
            return true;
        }
        let mut slope = scaled_value(&self.derivative, &middle);
        if slope.is_zero() || target <= middle.exponent {
            return false;
        }
        let mut numerator = &(&middle.numerator * &slope) - &value;
        if slope.is_negative() {
            slope = slope.negated();
            numerator = numerator.negated();
        }
        let scaled = &numerator * &Integer::from(2_i64).pow(target - middle.exponent);
        let Ok((center, _)) = scaled.div_rem_euclid(&slope) else {
            return false;
        };
        let lower = Dyadic {
            numerator: &center - &Integer::one(),
            exponent: target,
        };
        let upper = Dyadic {
            numerator: &center + &Integer::from(2_i64),
            exponent: target,
        };
        let inside = lower.compare(&self.lower) == Ordering::Greater
            && upper.compare(&self.upper) == Ordering::Less;
        if !inside {
            return false;
        }
        let at_lower = sign_at_dyadic(&self.integers, &lower);
        let at_upper = sign_at_dyadic(&self.integers, &upper);
        if at_lower == Ordering::Equal {
            self.exact = Some(lower.to_rational());
            return true;
        }
        if at_upper == Ordering::Equal {
            self.exact = Some(upper.to_rational());
            return true;
        }
        if at_lower == at_upper {
            return false;
        }
        self.lower = lower;
        self.upper = upper;
        true
    }

    pub(crate) fn narrowed_below(&mut self, bits: u32) {
        loop {
            if self.exact.is_some() {
                return;
            }
            let width = self.width_exponent();
            if width < -i64::from(bits) {
                return;
            }
            let reached = u32::try_from(-width.min(0)).unwrap_or(0);
            let target = reached.saturating_mul(2).max(reached + 8).min(bits + 8);
            if !self.newton_step(target) {
                self.halved();
            }
        }
    }

    pub(crate) fn compare_with_sum(&mut self, value: &SquareRootSum) -> Ordering {
        loop {
            if let Some(exact) = &self.exact {
                return SquareRootSum::from_rational(exact.clone()).compare(value);
            }
            let (lower, upper) = self.bounds();
            if SquareRootSum::from_rational(lower).compare(value) != Ordering::Less {
                return Ordering::Greater;
            }
            if SquareRootSum::from_rational(upper).compare(value) != Ordering::Greater {
                return Ordering::Less;
            }
            self.halved();
        }
    }

    pub(crate) fn separated_from(&mut self, other: &mut RealRoot) -> Ordering {
        loop {
            let (own_lower, own_upper) = self.bounds();
            let (other_lower, other_upper) = other.bounds();
            if own_upper.compare(&other_lower) == Ordering::Less {
                return Ordering::Less;
            }
            if other_upper.compare(&own_lower) == Ordering::Less {
                return Ordering::Greater;
            }
            if self.exact.is_some() && other.exact.is_some() {
                return own_lower.compare(&other_lower);
            }
            self.halved();
            other.halved();
        }
    }

    pub(crate) fn compare_with(&mut self, value: &ExactRational) -> Ordering {
        loop {
            if let Some(exact) = &self.exact {
                return exact.compare(value);
            }
            let (lower, upper) = self.bounds();
            if value.compare(&lower) != Ordering::Greater {
                return Ordering::Greater;
            }
            if value.compare(&upper) != Ordering::Less {
                return Ordering::Less;
            }
            if sign_at(&self.polynomial, value) == Ordering::Equal {
                return Ordering::Equal;
            }
            self.halved();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rational(numerator: i64, denominator: i64) -> ExactRational {
        ExactRational::fraction(&numerator.into(), &denominator.into()).unwrap()
    }

    fn whole(values: &[i64]) -> Vec<ExactRational> {
        values
            .iter()
            .map(|value| ExactRational::from_i64(*value))
            .collect()
    }

    #[test]
    fn a_cube_root_of_two_is_isolated_once() {
        let roots = real_roots(&whole(&[-2, 0, 0, 1])).unwrap();

        assert_eq!(roots.len(), 1);
        let mut root = roots[0].clone();
        assert_eq!(root.compare_with(&rational(5, 4)), Ordering::Greater);
        assert_eq!(root.compare_with(&rational(63, 50)), Ordering::Less);
    }

    #[test]
    fn rational_roots_are_found_in_order() {
        let mut roots = real_roots(&whole(&[6, -5, 1])).unwrap();

        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].compare_with(&rational(2, 1)), Ordering::Equal);
        assert_eq!(roots[1].compare_with(&rational(3, 1)), Ordering::Equal);
    }

    #[test]
    fn a_repeated_root_is_counted_once() {
        let roots = real_roots(&whole(&[1, -2, 1])).unwrap();

        assert_eq!(roots.len(), 1);
    }

    #[test]
    fn a_polynomial_with_no_real_root_has_none() {
        assert!(real_roots(&whole(&[1, 0, 1])).unwrap().is_empty());
    }

    fn rational_sign_variations(
        coefficients: &[ExactRational],
        lower: &ExactRational,
        upper: &ExactRational,
    ) -> usize {
        let composed = |values: &[ExactRational], start: &ExactRational, width: &ExactRational| {
            let mut composed: Vec<ExactRational> = Vec::new();
            for coefficient in values.iter().rev() {
                let mut next = vec![ExactRational::zero(); composed.len() + 1];
                for (power, term) in composed.iter().enumerate() {
                    next[power] = next[power].plus(&term.multiply(start));
                    next[power + 1] = next[power + 1].plus(&term.multiply(width));
                }
                next[0] = next[0].plus(coefficient);
                composed = next;
            }
            composed
        };
        let on_unit = composed(coefficients, lower, &upper.subtract(lower));
        let reversed: Vec<ExactRational> = on_unit.into_iter().rev().collect();
        let transformed = composed(&reversed, &ExactRational::one(), &ExactRational::one());
        let signs: Vec<Ordering> = transformed
            .iter()
            .map(ExactRational::sign)
            .filter(|sign| *sign != Ordering::Equal)
            .collect();
        signs.windows(2).filter(|pair| pair[0] != pair[1]).count()
    }

    #[test]
    fn integer_and_rational_counts_agree_on_every_interval_of_the_tree() {
        let mut mignotte = vec![ExactRational::zero(); 9];
        mignotte[8] = ExactRational::one();
        mignotte[2] = ExactRational::from_i64(-20000);
        mignotte[1] = ExactRational::from_i64(400);
        mignotte[0] = ExactRational::from_i64(-2);
        let polynomials = vec![
            mignotte,
            whole(&[-2, 0, 0, 1]),
            whole(&[-1, 3, 0, -1]),
            vec![
                rational(-1, 3),
                rational(5, 7),
                rational(0, 1),
                rational(3, 2),
            ],
        ];
        for polynomial in polynomials {
            let integers = integer_coefficients(&polynomial);
            for depth in 0..7_u32 {
                for index in 0..(1_i64 << depth) {
                    let lower = Dyadic {
                        numerator: Integer::from(2 * index - (1_i64 << depth)),
                        exponent: depth,
                    }
                    .reduced();
                    let upper = Dyadic {
                        numerator: Integer::from(2 * index + 2 - (1_i64 << depth)),
                        exponent: depth,
                    }
                    .reduced();
                    assert_eq!(
                        sign_variations(&integers, &lower, &upper),
                        rational_sign_variations(
                            &polynomial,
                            &lower.to_rational(),
                            &upper.to_rational()
                        ),
                    );
                }
            }
        }
    }

    #[test]
    fn three_roots_inside_the_unit_interval_give_three_variations() {
        let integers = integer_coefficients(&whole(&[-6, 44, -96, 64]));
        let lower = Dyadic {
            numerator: Integer::zero(),
            exponent: 0,
        };
        let upper = Dyadic {
            numerator: Integer::one(),
            exponent: 0,
        };

        assert_eq!(sign_variations(&integers, &lower, &upper), 3);
    }

    #[test]
    fn a_square_touching_zero_inside_keeps_its_sign() {
        let square = whole(&[3, -6, 3]);

        assert_eq!(
            sign_on_interval(&square, &rational(0, 1), &rational(2, 1)),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn a_square_touching_zero_at_an_irrational_point_keeps_its_sign() {
        let square = whole(&[4, 0, -4, 0, 1]);

        assert_eq!(
            sign_on_interval(&square, &rational(0, 1), &rational(2, 1)),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn a_simple_root_inside_changes_the_sign() {
        assert_eq!(
            sign_on_interval(&whole(&[1, -2]), &rational(0, 1), &rational(1, 1)),
            None
        );
    }

    #[test]
    fn a_simple_root_at_an_end_keeps_the_sign() {
        assert_eq!(
            sign_on_interval(&whole(&[-1, 0, 1]), &rational(1, 1), &rational(3, 1)),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn the_zero_polynomial_has_no_sign() {
        assert_eq!(
            sign_on_interval(&whole(&[0]), &rational(0, 1), &rational(1, 1)),
            Some(Ordering::Equal)
        );
    }

    #[test]
    fn a_root_at_an_end_is_on_the_closed_interval() {
        assert_eq!(
            vanishes_on_closed_interval(&whole(&[0, 1]), &rational(0, 1), &rational(1, 1)),
            Some(true)
        );
        assert_eq!(
            vanishes_on_closed_interval(&whole(&[1, 0, 1]), &rational(-1, 1), &rational(1, 1)),
            Some(false)
        );
    }

    #[test]
    fn three_close_roots_are_separated() {
        let roots = real_roots(&whole(&[-1, 3, 0, -1])).unwrap();

        assert_eq!(roots.len(), 3);
        let (first, _) = roots[0].bounds();
        let (_, last) = roots[2].bounds();
        assert_eq!(first.compare(&last), Ordering::Less);
    }
}
