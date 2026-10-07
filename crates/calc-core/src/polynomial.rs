use std::cmp::Ordering;
use std::collections::BTreeMap;

use calc_expr::{
    BinderKind, BuiltinConstant, ExprId, ExprPool, Head, NodeView, Operator, SymbolId, SymbolKind,
};
use calc_numbers::Integer;

use crate::exact_evaluation::{
    closed_square_root_sum, least_common_denominator, square_root_sum_expression,
    sum_within_size_limit,
};
use crate::exact_rational::ExactRational;
use crate::field_gcd::{FieldPolynomial, field_gcd};
use crate::polynomial_gcd::{Exponents, IntegerPolynomial, integer_gcd};
use crate::square_root_sum::SquareRootSum;

const DEGREE_LIMIT: u32 = 64;

const TERM_LIMIT: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Monomial {
    factors: Vec<(usize, u32)>,
}

impl Ord for Monomial {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut left = self.factors.iter();
        let mut right = other.factors.iter();
        loop {
            return match (left.next(), right.next()) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some((first, one)), Some((second, two))) => match first.cmp(second) {
                    Ordering::Equal => match two.cmp(one) {
                        Ordering::Equal => continue,
                        ordering => ordering,
                    },
                    ordering => ordering,
                },
            };
        }
    }
}

impl PartialOrd for Monomial {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Monomial {
    fn constant() -> Self {
        Monomial {
            factors: Vec::new(),
        }
    }

    fn single(atom: usize) -> Self {
        Monomial {
            factors: vec![(atom, 1)],
        }
    }

    fn times(&self, other: &Self) -> Option<Self> {
        let mut factors: Vec<(usize, u32)> = Vec::new();
        let (mut left, mut right) = (0, 0);
        while left < self.factors.len() || right < other.factors.len() {
            let take_left = match (self.factors.get(left), other.factors.get(right)) {
                (Some((first, _)), Some((second, _))) => first <= second,
                (Some(_), None) => true,
                _ => false,
            };
            let merge = matches!(
                (self.factors.get(left), other.factors.get(right)),
                (Some((first, _)), Some((second, _))) if first == second
            );
            if merge {
                let (atom, one) = self.factors[left];
                let (_, two) = other.factors[right];
                let exponent = one.checked_add(two)?;
                if exponent > DEGREE_LIMIT {
                    return None;
                }
                factors.push((atom, exponent));
                left += 1;
                right += 1;
            } else if take_left {
                factors.push(self.factors[left]);
                left += 1;
            } else {
                factors.push(other.factors[right]);
                right += 1;
            }
        }
        Some(Monomial { factors })
    }

    pub fn is_constant(&self) -> bool {
        self.factors.is_empty()
    }

    pub fn factors(&self) -> &[(usize, u32)] {
        &self.factors
    }

    pub fn exponent_of(&self, atom: usize) -> u32 {
        self.factors
            .iter()
            .find(|(factor, _)| *factor == atom)
            .map_or(0, |(_, exponent)| *exponent)
    }

    pub fn with_exponent(&self, atom: usize, exponent: u32) -> Option<Monomial> {
        if exponent > DEGREE_LIMIT {
            return None;
        }
        let mut factors: Vec<(usize, u32)> = self
            .factors
            .iter()
            .copied()
            .filter(|(factor, _)| *factor != atom)
            .collect();
        if exponent > 0 {
            let position = factors
                .iter()
                .position(|(factor, _)| *factor > atom)
                .unwrap_or(factors.len());
            factors.insert(position, (atom, exponent));
        }
        Some(Monomial { factors })
    }
}

impl Monomial {
    fn divided_by(&self, divisor: &Monomial) -> Option<Monomial> {
        let mut quotient = self.clone();
        for (atom, exponent) in &divisor.factors {
            let own = quotient.exponent_of(*atom);
            quotient = quotient.with_exponent(*atom, own.checked_sub(*exponent)?)?;
        }
        Some(quotient)
    }

    fn exponents(&self, width: usize) -> Option<Exponents> {
        let mut exponents = vec![0; width];
        for (atom, exponent) in &self.factors {
            *exponents.get_mut(*atom)? = *exponent;
        }
        Some(exponents)
    }

    fn from_exponents(exponents: &[u32]) -> Monomial {
        Monomial {
            factors: exponents
                .iter()
                .enumerate()
                .filter(|(_, exponent)| **exponent > 0)
                .map(|(atom, exponent)| (atom, *exponent))
                .collect(),
        }
    }

    fn lowest(&self, other: &Monomial) -> Monomial {
        Monomial {
            factors: self
                .factors
                .iter()
                .filter_map(|(atom, exponent)| {
                    let shared = (*exponent).min(other.exponent_of(*atom));
                    (shared > 0).then_some((*atom, shared))
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Polynomial {
    terms: BTreeMap<Monomial, SquareRootSum>,
}

impl Polynomial {
    fn constant(value: SquareRootSum) -> Self {
        let mut terms = BTreeMap::new();
        if !value.is_zero() {
            terms.insert(Monomial::constant(), value);
        }
        Polynomial { terms }
    }

    fn rational(value: ExactRational) -> Self {
        Polynomial::constant(SquareRootSum::from_rational(value))
    }

    fn one() -> Self {
        Polynomial::rational(ExactRational::one())
    }

    fn atom(index: usize) -> Self {
        let mut terms = BTreeMap::new();
        terms.insert(
            Monomial::single(index),
            SquareRootSum::from_rational(ExactRational::one()),
        );
        Polynomial { terms }
    }

    pub fn raise_in(&self, atom: usize) -> Option<Polynomial> {
        let mut raised = Polynomial::default();
        for (monomial, coefficient) in &self.terms {
            let exponent = monomial.exponent_of(atom);
            let divisor = ExactRational::from_i64(i64::from(exponent) + 1).reciprocal()?;
            raised.insert(
                monomial.with_exponent(atom, exponent + 1)?,
                coefficient.times(&SquareRootSum::from_rational(divisor)),
            )?;
        }
        Some(raised)
    }

    fn insert(&mut self, monomial: Monomial, coefficient: SquareRootSum) -> Option<()> {
        if coefficient.is_zero() {
            return Some(());
        }
        let total = match self.terms.remove(&monomial) {
            Some(existing) => existing.plus(&coefficient),
            None => coefficient,
        };
        if !sum_within_size_limit(&total) {
            return None;
        }
        if !total.is_zero() {
            self.terms.insert(monomial, total);
        }
        Some(())
    }

    pub fn plus(&self, other: &Self) -> Option<Self> {
        let mut total = self.clone();
        for (monomial, coefficient) in &other.terms {
            total.insert(monomial.clone(), coefficient.clone())?;
        }
        (total.terms.len() <= TERM_LIMIT).then_some(total)
    }

    pub fn negated(&self) -> Self {
        Polynomial {
            terms: self
                .terms
                .iter()
                .map(|(monomial, coefficient)| (monomial.clone(), coefficient.negated()))
                .collect(),
        }
    }

    fn times(&self, other: &Self) -> Option<Self> {
        let mut total = Polynomial::default();
        for (left, first) in &self.terms {
            for (right, second) in &other.terms {
                let monomial = left.times(right)?;
                total.insert(monomial, first.times(second))?;
                if total.terms.len() > TERM_LIMIT {
                    return None;
                }
            }
        }
        Some(total)
    }

    fn raised(&self, exponent: u32) -> Option<Self> {
        if exponent > DEGREE_LIMIT {
            return None;
        }
        let mut total = Polynomial::one();
        for _ in 0..exponent {
            total = total.times(self)?;
        }
        Some(total)
    }

    pub(crate) fn as_constant(&self) -> Option<SquareRootSum> {
        match self.terms.len() {
            0 => Some(SquareRootSum::zero()),
            1 => {
                let (monomial, coefficient) = self.terms.iter().next()?;
                monomial.is_constant().then(|| coefficient.clone())
            }
            _ => None,
        }
    }

    pub(crate) fn terms(&self) -> impl Iterator<Item = (&Monomial, &SquareRootSum)> {
        self.terms.iter()
    }

    fn width(&self) -> usize {
        self.terms
            .keys()
            .filter_map(|monomial| monomial.factors.last().map(|(atom, _)| atom + 1))
            .max()
            .unwrap_or(0)
    }

    fn leading(&self) -> Option<(&Monomial, &SquareRootSum)> {
        self.terms.iter().next()
    }

    fn single_term(monomial: Monomial, coefficient: SquareRootSum) -> Self {
        let mut terms = BTreeMap::new();
        if !coefficient.is_zero() {
            terms.insert(monomial, coefficient);
        }
        Polynomial { terms }
    }

    fn is_rational(&self) -> bool {
        self.terms
            .values()
            .all(|coefficient| coefficient.as_rational().is_some())
    }

    fn common_denominator(&self) -> Integer {
        self.terms
            .values()
            .fold(Integer::one(), |shared, coefficient| {
                let denominator = least_common_denominator(coefficient);
                let divisor = shared.gcd(&denominator);
                match shared.div_rem_euclid(&divisor) {
                    Ok((quotient, _)) => &quotient * &denominator,
                    Err(_) => shared,
                }
            })
    }

    fn scaled(&self, factor: &SquareRootSum) -> Self {
        Polynomial {
            terms: self
                .terms
                .iter()
                .map(|(monomial, coefficient)| (monomial.clone(), coefficient.times(factor)))
                .filter(|(_, coefficient)| !coefficient.is_zero())
                .collect(),
        }
    }

    fn as_integer(&self, width: usize) -> Option<IntegerPolynomial> {
        let scale = ExactRational::from_integer(self.common_denominator());
        self.terms
            .iter()
            .map(|(monomial, coefficient)| {
                let whole = coefficient.as_rational()?.multiply(&scale);
                whole
                    .denominator()
                    .is_one()
                    .then(|| Some((monomial.exponents(width)?, whole.numerator().clone())))
                    .flatten()
            })
            .collect()
    }

    fn from_integer(polynomial: &IntegerPolynomial) -> Self {
        Polynomial {
            terms: polynomial
                .iter()
                .map(|(exponents, coefficient)| {
                    (
                        Monomial::from_exponents(exponents),
                        SquareRootSum::from_rational(ExactRational::from_integer(
                            coefficient.clone(),
                        )),
                    )
                })
                .collect(),
        }
    }

    fn common_monomial(&self, other: &Self) -> Option<Monomial> {
        let mut monomials = self.terms.keys().chain(other.terms.keys());
        let first = monomials.next()?.clone();
        Some(monomials.fold(first, |shared, monomial| shared.lowest(monomial)))
    }

    fn without_monomial(&self, monomial: &Monomial) -> Option<Self> {
        let mut quotient = Polynomial::default();
        for (own, coefficient) in &self.terms {
            quotient
                .terms
                .insert(own.divided_by(monomial)?, coefficient.clone());
        }
        Some(quotient)
    }

    pub(crate) fn divided_exactly(&self, divisor: &Self) -> Option<Self> {
        let (divisor_monomial, divisor_coefficient) = divisor.leading()?;
        let mut remainder = self.clone();
        let mut quotient = Polynomial::default();
        while let Some((monomial, coefficient)) = remainder.leading() {
            let shift = monomial.divided_by(divisor_monomial)?;
            let factor = coefficient.divided_by(divisor_coefficient)?;
            let step = Polynomial::single_term(shift.clone(), factor.clone());
            remainder = remainder.plus(&step.times(divisor)?.negated())?;
            quotient.insert(shift, factor)?;
        }
        Some(quotient)
    }

    fn remainder_in_one_name(&self, divisor: &Self) -> Option<Self> {
        let (divisor_monomial, divisor_coefficient) = divisor.leading()?;
        let mut remainder = self.clone();
        while let Some((monomial, coefficient)) = remainder.leading() {
            let Some(shift) = monomial.divided_by(divisor_monomial) else {
                break;
            };
            let factor = coefficient.divided_by(divisor_coefficient)?;
            let step = Polynomial::single_term(shift, factor);
            remainder = remainder.plus(&step.times(divisor)?.negated())?;
        }
        Some(remainder)
    }

    fn monic(&self) -> Option<Self> {
        let (_, leading) = self.leading()?;
        Some(self.scaled(&leading.reciprocal()?))
    }

    pub(crate) fn gcd(&self, other: &Self) -> Option<Self> {
        if self.terms.is_empty() {
            return other.monic().or_else(|| Some(Polynomial::default()));
        }
        if other.terms.is_empty() {
            return self.monic();
        }
        let shared = self.common_monomial(other)?;
        let left = self.without_monomial(&shared)?;
        let right = other.without_monomial(&shared)?;
        let (left, right) = if left.is_rational() && right.is_rational() {
            (left, right)
        } else {
            (left.monic()?, right.monic()?)
        };
        let width = left.width().max(right.width());
        let rest = if left.terms.len() == 1 || right.terms.len() == 1 {
            Polynomial::one()
        } else if let (Some(one), Some(two)) = (left.as_integer(width), right.as_integer(width)) {
            Polynomial::from_integer(&integer_gcd(&one, &two)?)
        } else {
            let mut names = left
                .terms
                .keys()
                .chain(right.terms.keys())
                .flat_map(|monomial| monomial.factors.iter().map(|(atom, _)| *atom));
            let first = names.next();
            if names.any(|atom| Some(atom) != first) {
                return Polynomial::gcd_over_the_field(&left, &right, width)?.times(
                    &Polynomial::single_term(
                        shared,
                        SquareRootSum::from_rational(ExactRational::one()),
                    ),
                );
            }
            let (mut one, mut two) = (left, right);
            while !two.terms.is_empty() {
                let remainder = one.remainder_in_one_name(&two)?;
                one = two;
                two = remainder;
            }
            one.monic()?
        };
        rest.times(&Polynomial::single_term(
            shared,
            SquareRootSum::from_rational(ExactRational::one()),
        ))
    }

    fn as_field(&self, width: usize) -> Option<FieldPolynomial> {
        self.terms
            .iter()
            .map(|(monomial, coefficient)| Some((monomial.exponents(width)?, coefficient.clone())))
            .collect()
    }

    fn from_field(polynomial: &FieldPolynomial) -> Self {
        Polynomial {
            terms: polynomial
                .iter()
                .map(|(exponents, coefficient)| {
                    (Monomial::from_exponents(exponents), coefficient.clone())
                })
                .collect(),
        }
    }

    fn gcd_over_the_field(left: &Self, right: &Self, width: usize) -> Option<Self> {
        let mut generators: Vec<Integer> = left
            .terms
            .values()
            .chain(right.terms.values())
            .flat_map(|coefficient| {
                coefficient
                    .prime_terms()
                    .flat_map(|(primes, _)| primes.to_vec())
                    .collect::<Vec<Integer>>()
            })
            .collect();
        generators.sort();
        generators.dedup();
        let accepts = |candidate: &FieldPolynomial| {
            let candidate = Polynomial::from_field(candidate);
            let divides = |polynomial: &Polynomial| {
                polynomial
                    .divided_exactly(&candidate)
                    .and_then(|quotient| quotient.times(&candidate))
                    .as_ref()
                    == Some(polynomial)
            };
            divides(left) && divides(right)
        };
        let gcd = field_gcd(
            &left.as_field(width)?,
            &right.as_field(width)?,
            &generators,
            accepts,
        )?;
        Some(Polynomial::from_field(&gcd))
    }

    pub(crate) fn coefficients_in(&self, atom: usize) -> Option<Vec<Self>> {
        let mut coefficients: Vec<Polynomial> = Vec::new();
        for (monomial, coefficient) in &self.terms {
            let degree = usize::try_from(monomial.exponent_of(atom)).ok()?;
            while coefficients.len() <= degree {
                coefficients.push(Polynomial::default());
            }
            coefficients[degree].insert(monomial.with_exponent(atom, 0)?, coefficient.clone())?;
        }
        Some(coefficients)
    }

    pub(crate) fn atoms_used(&self) -> Vec<usize> {
        let mut used: Vec<usize> = self
            .terms
            .keys()
            .flat_map(|monomial| monomial.factors.iter().map(|(atom, _)| *atom))
            .collect();
        used.sort_unstable();
        used.dedup();
        used
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    pub(crate) fn derivative_in(&self, atom: usize) -> Option<Self> {
        let mut derivative = Polynomial::default();
        for (monomial, coefficient) in &self.terms {
            let exponent = monomial.exponent_of(atom);
            if exponent == 0 {
                continue;
            }
            let factor = SquareRootSum::from_rational(ExactRational::from_i64(i64::from(exponent)));
            derivative.insert(
                monomial.with_exponent(atom, exponent - 1)?,
                coefficient.times(&factor),
            )?;
        }
        Some(derivative)
    }

    fn square_free_part(&self) -> Option<Self> {
        let mut shared = self.clone();
        for atom in 0..self.width() {
            shared = shared.gcd(&self.derivative_in(atom)?)?;
        }
        self.divided_exactly(&shared)
    }
}

#[derive(Default)]
pub struct AtomTable {
    atoms: Vec<ExprId>,
}

impl AtomTable {
    fn index(&mut self, expression: ExprId) -> usize {
        match self.atoms.iter().position(|atom| *atom == expression) {
            Some(index) => index,
            None => {
                self.atoms.push(expression);
                self.atoms.len() - 1
            }
        }
    }

    pub fn expression(&self, index: usize) -> Option<ExprId> {
        self.atoms.get(index).copied()
    }

    pub fn expressions(&self) -> &[ExprId] {
        &self.atoms
    }

    pub fn index_of(&mut self, expression: ExprId) -> usize {
        self.index(expression)
    }

    pub fn holds_an_atom_outside_the_field(&self, pool: &ExprPool) -> bool {
        self.atoms.iter().any(|atom| {
            matches!(
                pool.node(*atom),
                Ok(NodeView::Symbol(symbol)) if symbol == BuiltinConstant::Infinity.symbol()
            )
        })
    }

    pub fn every_atom_is_a_free_name(&self, pool: &ExprPool) -> bool {
        self.atoms.iter().all(|atom| is_a_free_name(pool, *atom))
    }

    pub fn every_atom_is_a_free_name_or_the_imaginary_unit(&self, pool: &ExprPool) -> bool {
        self.atoms
            .iter()
            .all(|atom| is_a_free_name(pool, *atom) || is_the_imaginary_unit(pool, *atom))
    }

    pub fn holds_only_the_imaginary_unit(&self, pool: &ExprPool, polynomial: &Polynomial) -> bool {
        let used = polynomial.atoms_used();
        !used.is_empty()
            && used.iter().all(|index| {
                self.atoms
                    .get(*index)
                    .is_some_and(|atom| is_the_imaginary_unit(pool, *atom))
            })
    }
}

pub fn is_a_constant(pool: &ExprPool, symbol: SymbolId) -> bool {
    matches!(pool.symbol_kind(symbol), Ok(SymbolKind::Constant))
}

fn is_the_imaginary_unit(pool: &ExprPool, expression: ExprId) -> bool {
    matches!(
        pool.node(expression),
        Ok(NodeView::Symbol(symbol)) if symbol == BuiltinConstant::ImaginaryUnit.symbol()
    )
}

fn is_a_free_name(pool: &ExprPool, expression: ExprId) -> bool {
    match pool.node(expression) {
        Ok(NodeView::Symbol(symbol)) => !is_a_constant(pool, symbol),
        _ => false,
    }
}

fn whole_exponent(pool: &ExprPool, expression: ExprId) -> Option<u32> {
    match pool.node(expression).ok()? {
        NodeView::Number(number) => {
            let value = ExactRational::from_number(pool.number_value(number).ok()?)?;
            value
                .is_integer()
                .then(|| value.numerator().to_i64())
                .flatten()
                .and_then(|value| u32::try_from(value).ok())
        }
        _ => None,
    }
}

fn atom_or_value(pool: &mut ExprPool, expression: ExprId, atoms: &mut AtomTable) -> Polynomial {
    match closed_square_root_sum(pool, expression) {
        Some(value) if sum_within_size_limit(&value) => Polynomial::constant(value),
        _ => Polynomial::atom(atoms.index(expression)),
    }
}

fn rational_power(
    pool: &mut ExprPool,
    base: ExprId,
    exponent: ExprId,
) -> Option<(ExactRational, u32, u32)> {
    let base = closed_square_root_sum(pool, base)?.as_rational()?;
    let exponent = closed_square_root_sum(pool, exponent)?.as_rational()?;
    if base.sign() != Ordering::Greater
        || exponent.sign() != Ordering::Greater
        || exponent.is_integer()
    {
        return None;
    }
    let multiple = u32::try_from(exponent.numerator().to_i64()?).ok()?;
    let order = u32::try_from(exponent.denominator().to_i64()?).ok()?;
    Some((base, multiple, order))
}

fn written_rational(pool: &mut ExprPool, value: &ExactRational) -> Option<ExprId> {
    let numerator = pool.number(value.numerator().clone().into()).ok()?;
    if value.is_integer() {
        return Some(numerator);
    }
    let denominator = pool.number(value.denominator().clone().into()).ok()?;
    pool.apply(Head::Operator(Operator::Div), &[numerator, denominator])
        .ok()
}

fn power_or_atom(
    pool: &mut ExprPool,
    expression: ExprId,
    base: ExprId,
    exponent: ExprId,
    atoms: &mut AtomTable,
) -> Polynomial {
    if let Some(value) = closed_square_root_sum(pool, expression)
        && sum_within_size_limit(&value)
    {
        return Polynomial::constant(value);
    }
    let canonical = rational_power(pool, base, exponent).and_then(|(base, multiple, order)| {
        let base = written_rational(pool, &base)?;
        let reciprocal =
            ExactRational::fraction(&Integer::one(), &Integer::from(i64::from(order)))?;
        let exponent = written_rational(pool, &reciprocal)?;
        let root = pool
            .apply(Head::Operator(Operator::Pow), &[base, exponent])
            .ok()?;
        Polynomial::atom(atoms.index(root)).raised(multiple)
    });
    canonical.unwrap_or_else(|| Polynomial::atom(atoms.index(expression)))
}

fn defining_polynomial(pool: &mut ExprPool, atom: ExprId) -> Option<Vec<ExactRational>> {
    match pool.node(atom).ok()? {
        NodeView::Symbol(symbol) if symbol == BuiltinConstant::ImaginaryUnit.symbol() => {
            Some(vec![
                ExactRational::one(),
                ExactRational::zero(),
                ExactRational::one(),
            ])
        }
        NodeView::Bind {
            binder: BinderKind::Root,
            body,
            ..
        } => crate::real_roots::coefficients_in_bound_variable(pool, body).ok(),
        NodeView::Apply {
            head: Head::Operator(Operator::Pow),
            arguments: [base, exponent],
        } => {
            let (base, exponent) = (*base, *exponent);
            let (base, multiple, order) = rational_power(pool, base, exponent)?;
            if multiple != 1 {
                return None;
            }
            let mut coefficients = vec![ExactRational::zero(); usize::try_from(order).ok()? + 1];
            coefficients[0] = base.negated();
            coefficients[usize::try_from(order).ok()?] = ExactRational::one();
            Some(coefficients)
        }
        _ => None,
    }
}

fn powers_modulo(defining: &[ExactRational], largest: u32) -> Option<Vec<Vec<ExactRational>>> {
    let degree = defining.len().checked_sub(1)?;
    let leading = defining.last()?.clone();
    if degree == 0 || leading.is_zero() {
        return None;
    }
    let mut powers = vec![vec![ExactRational::one()]];
    for _ in 0..largest {
        let previous = powers.last()?;
        let mut next = vec![ExactRational::zero(); previous.len() + 1];
        for (position, coefficient) in previous.iter().enumerate() {
            next[position + 1] = coefficient.clone();
        }
        if next.len() > degree {
            let top = next.pop()?;
            let factor = top.divide(&leading)?;
            for (position, coefficient) in defining.iter().take(degree).enumerate() {
                next[position] = next[position].subtract(&factor.multiply(coefficient));
            }
        }
        powers.push(next);
    }
    Some(powers)
}

pub fn with_algebraic_atoms_reduced(
    pool: &mut ExprPool,
    polynomial: &Polynomial,
    atoms: &mut AtomTable,
) -> Option<Polynomial> {
    with_atoms_reduced(pool, polynomial, atoms, false)
}

pub fn with_imaginary_unit_reduced(
    pool: &mut ExprPool,
    polynomial: &Polynomial,
    atoms: &mut AtomTable,
) -> Option<Polynomial> {
    with_atoms_reduced(pool, polynomial, atoms, true)
}

fn with_atoms_reduced(
    pool: &mut ExprPool,
    polynomial: &Polynomial,
    atoms: &mut AtomTable,
    only_the_imaginary_unit: bool,
) -> Option<Polynomial> {
    let mut reduced = polynomial.clone();
    for (index, atom) in atoms.expressions().to_vec().into_iter().enumerate() {
        let largest = reduced
            .terms
            .keys()
            .map(|monomial| monomial.exponent_of(index))
            .max()
            .unwrap_or(0);
        let is_imaginary_unit = matches!(
            pool.node(atom),
            Ok(NodeView::Symbol(symbol)) if symbol == BuiltinConstant::ImaginaryUnit.symbol()
        );
        if only_the_imaginary_unit && !is_imaginary_unit {
            continue;
        }
        let Some(defining) = defining_polynomial(pool, atom) else {
            continue;
        };
        if usize::try_from(largest).ok()? + 1 < defining.len() {
            continue;
        }
        let powers = powers_modulo(&defining, largest)?;
        let mut rewritten = Polynomial::default();
        for (monomial, coefficient) in &reduced.terms {
            let exponent = monomial.exponent_of(index);
            let kept = monomial.with_exponent(index, 0)?;
            for (power, scale) in powers[usize::try_from(exponent).ok()?].iter().enumerate() {
                if scale.is_zero() {
                    continue;
                }
                let term_monomial = kept.with_exponent(index, u32::try_from(power).ok()?)?;
                let mut term = Polynomial::default();
                term.insert(
                    term_monomial,
                    coefficient.times(&SquareRootSum::from_rational(scale.clone())),
                )?;
                rewritten = rewritten.plus(&term)?;
            }
        }
        reduced = rewritten;
    }
    Some(reduced)
}

fn constant_square_root(pool: &mut ExprPool, radicand: ExprId) -> Option<SquareRootSum> {
    let mut own_atoms = AtomTable::default();
    let radicand = polynomial_of(pool, radicand, &mut own_atoms)?
        .as_constant()?
        .as_rational()?;
    let root = SquareRootSum::square_root_of_rational(&radicand)?;
    sum_within_size_limit(&root).then_some(root)
}

pub fn polynomial_of(
    pool: &mut ExprPool,
    expression: ExprId,
    atoms: &mut AtomTable,
) -> Option<Polynomial> {
    let view = pool.node(expression).ok()?;
    let (operator, arguments) = match view {
        NodeView::Number(number) => {
            let value = ExactRational::from_number(pool.number_value(number).ok()?)?;
            return Some(Polynomial::rational(value));
        }
        NodeView::Apply {
            head: Head::Operator(operator),
            arguments,
        } => (operator, arguments.to_vec()),
        NodeView::Bind {
            binder: BinderKind::Derivative,
            ..
        } => {
            let differentiated = crate::exact_evaluation::differentiated_body(pool, expression)?;
            return polynomial_of(pool, differentiated, atoms);
        }
        _ => return Some(atom_or_value(pool, expression, atoms)),
    };
    match (operator, arguments.as_slice()) {
        (Operator::Add, [left, right]) => {
            let (left, right) = (*left, *right);
            let left = polynomial_of(pool, left, atoms)?;
            let right = polynomial_of(pool, right, atoms)?;
            left.plus(&right)
        }
        (Operator::Sub, [left, right]) => {
            let (left, right) = (*left, *right);
            let left = polynomial_of(pool, left, atoms)?;
            let right = polynomial_of(pool, right, atoms)?;
            left.plus(&right.negated())
        }
        (Operator::Neg, [single]) => {
            let single = *single;
            Some(polynomial_of(pool, single, atoms)?.negated())
        }
        (Operator::Mul, [left, right]) => {
            let (left, right) = (*left, *right);
            let left = polynomial_of(pool, left, atoms)?;
            let right = polynomial_of(pool, right, atoms)?;
            left.times(&right)
        }
        (Operator::Div, [left, right]) => {
            let (left, right) = (*left, *right);
            let divisor = polynomial_of(pool, right, atoms)?;
            let Some(divisor) = divisor.as_constant() else {
                return Some(atom_or_value(pool, expression, atoms));
            };
            if divisor.is_zero() {
                return None;
            }
            let left = polynomial_of(pool, left, atoms)?;
            let reciprocal = Polynomial::constant(divisor.reciprocal()?);
            left.times(&reciprocal)
        }
        (Operator::Sqrt, [radicand]) => {
            let radicand = *radicand;
            Some(match constant_square_root(pool, radicand) {
                Some(root) => Polynomial::constant(root),
                None => atom_or_value(pool, expression, atoms),
            })
        }
        (Operator::Pow, [base, exponent]) => {
            let (base, exponent) = (*base, *exponent);
            let Some(whole) = whole_exponent(pool, exponent) else {
                return Some(power_or_atom(pool, expression, base, exponent, atoms));
            };
            let exponent = whole;
            let base = polynomial_of(pool, base, atoms)?;
            base.raised(exponent)
        }
        _ => Some(atom_or_value(pool, expression, atoms)),
    }
}

const MULTIPLE_LIMIT: i64 = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Angle {
    Sine,
    Cosine,
}

type LinearParts = Vec<(Option<ExprId>, ExactRational)>;

struct ExponentialBase {
    generator: Option<ExprId>,
    rising: Polynomial,
    falling: Polynomial,
    rising_index: usize,
    falling_index: usize,
}

struct TrigonometricAtom {
    index: usize,
    angle: Angle,
    parts: LinearParts,
}

fn trigonometric_call(pool: &ExprPool, expression: ExprId) -> Option<(Angle, ExprId)> {
    match pool.node(expression).ok()? {
        NodeView::Apply {
            head: Head::Operator(Operator::Sin),
            arguments: [argument],
        } => Some((Angle::Sine, *argument)),
        NodeView::Apply {
            head: Head::Operator(Operator::Cos),
            arguments: [argument],
        } => Some((Angle::Cosine, *argument)),
        _ => None,
    }
}

fn linear_parts(
    pool: &mut ExprPool,
    argument: ExprId,
    generators: &mut AtomTable,
) -> Option<Vec<(Option<ExprId>, ExactRational)>> {
    let form = polynomial_of(pool, argument, generators)?;
    let mut parts = Vec::new();
    for (monomial, coefficient) in form.terms() {
        let coefficient = coefficient.as_rational()?;
        match monomial.factors() {
            [] => parts.push((None, coefficient)),
            [(generator, 1)] => parts.push((Some(generators.expression(*generator)?), coefficient)),
            _ => return None,
        }
    }
    Some(parts)
}

fn least_common_multiple(left: &Integer, right: &Integer) -> Option<Integer> {
    let divisor = left.gcd(right);
    let (quotient, _) = left.div_rem_euclid(&divisor).ok()?;
    Some(&quotient * right)
}

fn base_angle(
    pool: &mut ExprPool,
    generator: Option<ExprId>,
    denominator: &Integer,
) -> Option<ExprId> {
    let Some(generator) = generator else {
        let value = ExactRational::fraction(&Integer::one(), denominator)?;
        return pool.number(value.to_number()).ok();
    };
    if denominator.is_one() {
        return Some(generator);
    }
    let divisor = pool.number(denominator.clone().into()).ok()?;
    pool.apply(Head::Operator(Operator::Div), &[generator, divisor])
        .ok()
}

fn multiple_angle(
    sine: &Polynomial,
    cosine: &Polynomial,
    multiple: i64,
) -> Option<(Polynomial, Polynomial)> {
    if multiple.abs() > MULTIPLE_LIMIT {
        return None;
    }
    let (mut running_sine, mut running_cosine) = (Polynomial::default(), Polynomial::one());
    for _ in 0..multiple.abs() {
        let next_sine = running_sine
            .times(cosine)?
            .plus(&running_cosine.times(sine)?)?;
        let next_cosine = running_cosine
            .times(cosine)?
            .plus(&running_sine.times(sine)?.negated())?;
        running_sine = next_sine;
        running_cosine = next_cosine;
    }
    if multiple < 0 {
        running_sine = running_sine.negated();
    }
    Some((running_sine, running_cosine))
}

fn substituted(
    polynomial: &Polynomial,
    replacements: &BTreeMap<usize, Polynomial>,
) -> Option<Polynomial> {
    let mut total = Polynomial::default();
    for (monomial, coefficient) in &polynomial.terms {
        let mut term = Polynomial::default();
        let mut kept = Monomial::constant();
        let mut factors = Vec::new();
        for (atom, exponent) in monomial.factors() {
            match replacements.get(atom) {
                Some(replacement) => factors.push(replacement.raised(*exponent)?),
                None => kept = kept.with_exponent(*atom, *exponent)?,
            }
        }
        term.insert(kept, coefficient.clone())?;
        for factor in factors {
            term = term.times(&factor)?;
        }
        total = total.plus(&term)?;
    }
    Some(total)
}

pub fn with_angles_expanded(
    pool: &mut ExprPool,
    polynomial: &Polynomial,
    atoms: &mut AtomTable,
) -> Option<Polynomial> {
    let mut generators = AtomTable::default();
    let mut calls = Vec::new();
    for (index, atom) in atoms.expressions().to_vec().into_iter().enumerate() {
        let Some((angle, argument)) = trigonometric_call(pool, atom) else {
            continue;
        };
        if let Some(parts) = linear_parts(pool, argument, &mut generators) {
            calls.push(TrigonometricAtom {
                index,
                angle,
                parts,
            });
        }
    }
    if calls.is_empty() {
        return Some(polynomial.clone());
    }
    let mut denominators: Vec<(Option<ExprId>, Integer)> = Vec::new();
    for call in &calls {
        for (generator, coefficient) in &call.parts {
            match denominators
                .iter_mut()
                .find(|(known, _)| known == generator)
            {
                Some((_, denominator)) => {
                    *denominator = least_common_multiple(denominator, coefficient.denominator())?;
                }
                None => denominators.push((*generator, coefficient.denominator().clone())),
            }
        }
    }
    let mut bases: Vec<(Option<ExprId>, Polynomial, Polynomial)> = Vec::new();
    for (generator, denominator) in &denominators {
        let angle = base_angle(pool, *generator, denominator)?;
        let sine = pool.apply(Head::Operator(Operator::Sin), &[angle]).ok()?;
        let cosine = pool.apply(Head::Operator(Operator::Cos), &[angle]).ok()?;
        let sine = atom_or_value(pool, sine, atoms);
        let cosine = atom_or_value(pool, cosine, atoms);
        bases.push((*generator, sine, cosine));
    }
    let mut replacements = BTreeMap::new();
    for call in &calls {
        let (mut sine, mut cosine) = (Polynomial::default(), Polynomial::one());
        for (generator, coefficient) in &call.parts {
            let (_, denominator) = denominators.iter().find(|(known, _)| known == generator)?;
            let multiple = coefficient
                .multiply(&ExactRational::from_integer(denominator.clone()))
                .numerator()
                .to_i64()?;
            let (_, base_sine, base_cosine) =
                bases.iter().find(|(known, _, _)| known == generator)?;
            let (part_sine, part_cosine) = multiple_angle(base_sine, base_cosine, multiple)?;
            let next_sine = sine.times(&part_cosine)?.plus(&cosine.times(&part_sine)?)?;
            let next_cosine = cosine
                .times(&part_cosine)?
                .plus(&sine.times(&part_sine)?.negated())?;
            sine = next_sine;
            cosine = next_cosine;
        }
        replacements.insert(
            call.index,
            match call.angle {
                Angle::Sine => sine,
                Angle::Cosine => cosine,
            },
        );
    }
    substituted(polynomial, &replacements)
}

pub fn with_logarithms_expanded(
    pool: &mut ExprPool,
    polynomial: &Polynomial,
    atoms: &mut AtomTable,
) -> Option<Polynomial> {
    let mut replacements = BTreeMap::new();
    for (index, atom) in atoms.expressions().to_vec().into_iter().enumerate() {
        let Ok(NodeView::Apply {
            head: Head::Operator(Operator::Ln),
            arguments: [argument],
        }) = pool.node(atom)
        else {
            continue;
        };
        let argument = *argument;
        let Some(value) =
            closed_square_root_sum(pool, argument).and_then(|value| value.as_rational())
        else {
            continue;
        };
        if value.sign() != Ordering::Greater {
            continue;
        }
        let mut terms: Vec<(Integer, i64)> = Vec::new();
        for (part, sign) in [(value.numerator(), 1_i64), (value.denominator(), -1_i64)] {
            let (factors, rest) = match crate::factorization::proven_prime_factors(part) {
                Ok(factors) => (factors, Integer::one()),
                Err(_) => crate::integer_roots::prime_factors(part),
            };
            for (prime, exponent) in factors {
                terms.push((prime, sign * i64::from(exponent)));
            }
            if !rest.is_one() {
                terms.push((rest, sign));
            }
        }
        let mut expansion = Polynomial::default();
        for (factor, exponent) in terms {
            let factor = pool.number(factor.into()).ok()?;
            let logarithm = pool.apply(Head::Operator(Operator::Ln), &[factor]).ok()?;
            let logarithm = atom_or_value(pool, logarithm, atoms);
            let scale = Polynomial::rational(ExactRational::from_i64(exponent));
            expansion = expansion.plus(&logarithm.times(&scale)?)?;
        }
        replacements.insert(index, expansion);
    }
    substituted(polynomial, &replacements)
}

fn exponential_argument(pool: &ExprPool, expression: ExprId) -> Option<ExprId> {
    match pool.node(expression).ok()? {
        NodeView::Apply {
            head: Head::Operator(Operator::Exp),
            arguments: [argument],
        } => Some(*argument),
        _ => None,
    }
}

pub fn with_exponentials_expanded(
    pool: &mut ExprPool,
    polynomial: &Polynomial,
    atoms: &mut AtomTable,
) -> Option<Polynomial> {
    let mut generators = AtomTable::default();
    let mut calls: Vec<(usize, LinearParts)> = Vec::new();
    for (index, atom) in atoms.expressions().to_vec().into_iter().enumerate() {
        if matches!(
            pool.node(atom),
            Ok(NodeView::Symbol(symbol)) if symbol == BuiltinConstant::E.symbol()
        ) {
            calls.push((index, vec![(None, ExactRational::one())]));
            continue;
        }
        let Some(argument) = exponential_argument(pool, atom) else {
            continue;
        };
        if let Some(parts) = linear_parts(pool, argument, &mut generators) {
            calls.push((index, parts));
        }
    }
    if calls.is_empty() {
        return Some(polynomial.clone());
    }
    let mut denominators: Vec<(Option<ExprId>, Integer)> = Vec::new();
    for (_, parts) in &calls {
        for (generator, coefficient) in parts {
            match denominators
                .iter_mut()
                .find(|(known, _)| known == generator)
            {
                Some((_, denominator)) => {
                    *denominator = least_common_multiple(denominator, coefficient.denominator())?;
                }
                None => denominators.push((*generator, coefficient.denominator().clone())),
            }
        }
    }
    let mut bases: Vec<ExponentialBase> = Vec::new();
    for (generator, denominator) in &denominators {
        let exponent = base_angle(pool, *generator, denominator)?;
        let negated = pool
            .apply(Head::Operator(Operator::Neg), &[exponent])
            .ok()?;
        let rising = pool
            .apply(Head::Operator(Operator::Exp), &[exponent])
            .ok()?;
        let falling = pool.apply(Head::Operator(Operator::Exp), &[negated]).ok()?;
        let rising_index = atoms.index(rising);
        let falling_index = atoms.index(falling);
        let rising = atom_or_value(pool, rising, atoms);
        let falling = atom_or_value(pool, falling, atoms);
        bases.push(ExponentialBase {
            generator: *generator,
            rising,
            falling,
            rising_index,
            falling_index,
        });
    }
    let mut replacements = BTreeMap::new();
    for (index, parts) in &calls {
        let mut product = Polynomial::one();
        for (generator, coefficient) in parts {
            let (_, denominator) = denominators.iter().find(|(known, _)| known == generator)?;
            let multiple = coefficient
                .multiply(&ExactRational::from_integer(denominator.clone()))
                .numerator()
                .to_i64()?;
            if multiple.abs() > MULTIPLE_LIMIT {
                return None;
            }
            let base = bases.iter().find(|base| base.generator == *generator)?;
            let factor = if multiple >= 0 {
                &base.rising
            } else {
                &base.falling
            };
            product =
                product.times(&factor.raised(u32::try_from(multiple.unsigned_abs()).ok()?)?)?;
        }
        replacements.insert(*index, product);
    }
    let mut expanded = substituted(polynomial, &replacements)?;
    for base in &bases {
        expanded = with_reciprocals_cancelled(&expanded, base.rising_index, base.falling_index)?;
    }
    Some(expanded)
}

fn with_reciprocals_cancelled(
    polynomial: &Polynomial,
    rising: usize,
    falling: usize,
) -> Option<Polynomial> {
    let mut cancelled = Polynomial::default();
    for (monomial, coefficient) in &polynomial.terms {
        let (up, down) = (monomial.exponent_of(rising), monomial.exponent_of(falling));
        let common = up.min(down);
        let kept = monomial
            .with_exponent(rising, up - common)?
            .with_exponent(falling, down - common)?;
        let mut term = Polynomial::default();
        term.insert(kept, coefficient.clone())?;
        cancelled = cancelled.plus(&term)?;
    }
    Some(cancelled)
}

fn cosine_argument(pool: &ExprPool, expression: ExprId) -> Option<ExprId> {
    match pool.node(expression).ok()? {
        NodeView::Apply {
            head: Head::Operator(Operator::Cos),
            arguments: [argument],
        } => Some(*argument),
        _ => None,
    }
}

pub fn with_pythagoras(
    pool: &mut ExprPool,
    polynomial: &Polynomial,
    atoms: &mut AtomTable,
) -> Option<Polynomial> {
    let cosines: Vec<(usize, ExprId)> = atoms
        .expressions()
        .iter()
        .enumerate()
        .filter_map(|(index, atom)| Some((index, cosine_argument(pool, *atom)?)))
        .collect();
    let mut reduced = polynomial.clone();
    for (cosine, argument) in cosines {
        if !reduced
            .terms
            .keys()
            .any(|monomial| monomial.exponent_of(cosine) >= 2)
        {
            continue;
        }
        let sine = pool
            .apply(Head::Operator(Operator::Sin), &[argument])
            .ok()?;
        let sine = Polynomial::atom(atoms.index(sine));
        let one_minus_sine_squared = Polynomial::one().plus(&sine.raised(2)?.negated())?;
        let mut rewritten = Polynomial::default();
        for (monomial, coefficient) in &reduced.terms {
            let exponent = monomial.exponent_of(cosine);
            let kept = monomial.with_exponent(cosine, exponent % 2)?;
            let mut term = Polynomial::default();
            term.insert(kept, coefficient.clone())?;
            let term = term.times(&one_minus_sine_squared.raised(exponent / 2)?)?;
            rewritten = rewritten.plus(&term)?;
        }
        reduced = rewritten;
    }
    Some(reduced)
}

#[derive(Clone, Debug)]
pub struct RationalFunction {
    numerator: Polynomial,
    denominator: Polynomial,
    excluded: Vec<ExprId>,
}

impl RationalFunction {
    fn whole(numerator: Polynomial) -> Self {
        RationalFunction {
            numerator,
            denominator: Polynomial::one(),
            excluded: Vec::new(),
        }
    }

    pub fn numerator(&self) -> &Polynomial {
        &self.numerator
    }

    pub fn denominator(&self) -> &Polynomial {
        &self.denominator
    }

    pub fn excluded(&self) -> &[ExprId] {
        &self.excluded
    }

    fn with(&self, other: &Self, numerator: Polynomial, denominator: Polynomial) -> Self {
        let mut excluded = self.excluded.clone();
        for expression in &other.excluded {
            if !excluded.contains(expression) {
                excluded.push(*expression);
            }
        }
        RationalFunction {
            numerator,
            denominator,
            excluded,
        }
    }

    pub(crate) fn plus(&self, other: &Self) -> Option<Self> {
        if let Some(shared) = self.denominator.gcd(&other.denominator) {
            let own_share = other.denominator.divided_exactly(&shared)?;
            let other_share = self.denominator.divided_exactly(&shared)?;
            let numerator = self
                .numerator
                .times(&own_share)?
                .plus(&other.numerator.times(&other_share)?)?;
            let denominator = self.denominator.times(&own_share)?;
            return Some(self.with(other, numerator, denominator));
        }
        let numerator = self
            .numerator
            .times(&other.denominator)?
            .plus(&other.numerator.times(&self.denominator)?)?;
        let denominator = self.denominator.times(&other.denominator)?;
        Some(self.with(other, numerator, denominator))
    }

    pub(crate) fn reduced(&self) -> Option<Self> {
        let shared = self.numerator.gcd(&self.denominator)?;
        let numerator = self.numerator.divided_exactly(&shared)?;
        let denominator = self.denominator.divided_exactly(&shared)?;
        let (numerator, denominator) = normalized(numerator, denominator)?;
        Some(RationalFunction {
            numerator,
            denominator,
            excluded: self.excluded.clone(),
        })
    }

    pub(crate) fn negated(&self) -> Self {
        RationalFunction {
            numerator: self.numerator.negated(),
            denominator: self.denominator.clone(),
            excluded: self.excluded.clone(),
        }
    }

    fn times(&self, other: &Self) -> Option<Self> {
        let numerator = self.numerator.times(&other.numerator)?;
        let denominator = self.denominator.times(&other.denominator)?;
        Some(self.with(other, numerator, denominator))
    }

    fn over(&self, other: &Self, written: ExprId) -> Option<Self> {
        let numerator = self.numerator.times(&other.denominator)?;
        let denominator = self.denominator.times(&other.numerator)?;
        let mut quotient = self.with(other, numerator, denominator);
        let is_constant =
            other.numerator.as_constant().is_some() && other.denominator.as_constant().is_some();
        if is_constant {
            if other.numerator.as_constant()?.is_zero() {
                return None;
            }
            return Some(quotient);
        }
        if !quotient.excluded.contains(&written) {
            quotient.excluded.push(written);
        }
        Some(quotient)
    }

    fn raised(&self, exponent: u32) -> Option<Self> {
        Some(RationalFunction {
            numerator: self.numerator.raised(exponent)?,
            denominator: self.denominator.raised(exponent)?,
            excluded: self.excluded.clone(),
        })
    }
}

pub struct ReducedQuotient {
    pub expression: ExprId,
    pub excluding: Vec<ExprId>,
}

fn is_zero_free_beyond(
    pool: &mut ExprPool,
    written: ExprId,
    denominator: &Polynomial,
    atoms: &mut AtomTable,
) -> Option<bool> {
    let written_zeros = rational_of(pool, written, atoms)?.numerator;
    let Some(square_free) = written_zeros.square_free_part() else {
        return Some(false);
    };
    Some(
        denominator
            .divided_exactly(&square_free)
            .is_some_and(|quotient| quotient.times(&square_free).as_ref() == Some(denominator)),
    )
}

pub fn reduced_quotient(pool: &mut ExprPool, expression: ExprId) -> Option<ReducedQuotient> {
    let mut atoms = AtomTable::default();
    let reduced = rational_of(pool, expression, &mut atoms)?.reduced()?;
    let mut excluding = Vec::new();
    for written in reduced.excluded.clone() {
        if !is_zero_free_beyond(pool, written, &reduced.denominator, &mut atoms)? {
            excluding.push(written);
        }
    }
    let numerator = polynomial_expression(pool, &reduced.numerator, &atoms)?;
    let expression = if reduced.denominator.as_constant().is_some() {
        numerator
    } else {
        let denominator = polynomial_expression(pool, &reduced.denominator, &atoms)?;
        pool.apply(Head::Operator(Operator::Div), &[numerator, denominator])
            .ok()?
    };
    Some(ReducedQuotient {
        expression,
        excluding,
    })
}

fn normalized(numerator: Polynomial, denominator: Polynomial) -> Option<(Polynomial, Polynomial)> {
    if let Some(constant) = denominator.as_constant() {
        return Some((numerator.scaled(&constant.reciprocal()?), Polynomial::one()));
    }
    if numerator.is_rational() && denominator.is_rational() {
        let width = numerator.width().max(denominator.width());
        let (one, two) = (
            numerator.common_denominator(),
            denominator.common_denominator(),
        );
        let (scale, _) = (&one * &two).div_rem_euclid(&one.gcd(&two)).ok()?;
        let scale = SquareRootSum::from_rational(ExactRational::from_integer(scale));
        let (numerator, denominator) = (numerator.scaled(&scale), denominator.scaled(&scale));
        let content = numerator
            .as_integer(width)?
            .values()
            .chain(denominator.as_integer(width)?.values())
            .fold(Integer::zero(), |content, coefficient| {
                content.gcd(coefficient)
            });
        let (_, leading) = denominator.leading()?;
        let sign = if leading.sign() == std::cmp::Ordering::Less {
            ExactRational::from_i64(-1)
        } else {
            ExactRational::one()
        };
        let factor =
            SquareRootSum::from_rational(sign.divide(&ExactRational::from_integer(content))?);
        return Some((numerator.scaled(&factor), denominator.scaled(&factor)));
    }
    let (_, leading) = denominator.leading()?;
    let factor = leading.reciprocal()?;
    Some((numerator.scaled(&factor), denominator.scaled(&factor)))
}

pub fn rational_of(
    pool: &mut ExprPool,
    expression: ExprId,
    atoms: &mut AtomTable,
) -> Option<RationalFunction> {
    let view = pool.node(expression).ok()?;
    let (operator, arguments) = match view {
        NodeView::Number(number) => {
            let value = ExactRational::from_number(pool.number_value(number).ok()?)?;
            return Some(RationalFunction::whole(Polynomial::rational(value)));
        }
        NodeView::Apply {
            head: Head::Operator(operator),
            arguments,
        } => (operator, arguments.to_vec()),
        NodeView::Bind {
            binder: BinderKind::Derivative,
            ..
        } => {
            let differentiated = crate::exact_evaluation::differentiated_body(pool, expression)?;
            return rational_of(pool, differentiated, atoms);
        }
        _ => {
            return Some(RationalFunction::whole(atom_or_value(
                pool, expression, atoms,
            )));
        }
    };
    let atom = |pool: &mut ExprPool, atoms: &mut AtomTable| {
        Some(RationalFunction::whole(atom_or_value(
            pool, expression, atoms,
        )))
    };
    match (operator, arguments.as_slice()) {
        (Operator::Add, [left, right]) => {
            let (left, right) = (*left, *right);
            let left = rational_of(pool, left, atoms)?;
            let right = rational_of(pool, right, atoms)?;
            left.plus(&right)
        }
        (Operator::Sub, [left, right]) => {
            let (left, right) = (*left, *right);
            let left = rational_of(pool, left, atoms)?;
            let right = rational_of(pool, right, atoms)?;
            left.plus(&right.negated())
        }
        (Operator::Neg, [single]) => {
            let single = *single;
            Some(rational_of(pool, single, atoms)?.negated())
        }
        (Operator::Mul, [left, right]) => {
            let (left, right) = (*left, *right);
            let left = rational_of(pool, left, atoms)?;
            let right = rational_of(pool, right, atoms)?;
            left.times(&right)
        }
        (Operator::Div, [left, right]) => {
            let (left, right) = (*left, *right);
            let divisor = rational_of(pool, right, atoms)?;
            let dividend = rational_of(pool, left, atoms)?;
            dividend.over(&divisor, right)
        }
        (Operator::Pow, [base, exponent]) => {
            let (base, exponent) = (*base, *exponent);
            let Some(whole) = whole_exponent(pool, exponent) else {
                return Some(RationalFunction::whole(power_or_atom(
                    pool, expression, base, exponent, atoms,
                )));
            };
            rational_of(pool, base, atoms)?.raised(whole)
        }
        (Operator::Sqrt, [radicand]) => {
            let radicand = *radicand;
            match constant_square_root(pool, radicand) {
                Some(root) => Some(RationalFunction::whole(Polynomial::constant(root))),
                None => atom(pool, atoms),
            }
        }
        (Operator::Tan, [argument]) => {
            let argument = *argument;
            let sine = pool
                .apply(Head::Operator(Operator::Sin), &[argument])
                .ok()?;
            let cosine = pool
                .apply(Head::Operator(Operator::Cos), &[argument])
                .ok()?;
            let numerator = rational_of(pool, sine, atoms)?;
            let denominator = rational_of(pool, cosine, atoms)?;
            numerator.over(&denominator, cosine)
        }
        _ => atom(pool, atoms),
    }
}

fn term_expression(
    pool: &mut ExprPool,
    monomial: &Monomial,
    coefficient: &SquareRootSum,
    atoms: &AtomTable,
) -> Option<ExprId> {
    let denominator = least_common_denominator(coefficient);
    let scaled = coefficient.times(&SquareRootSum::from_rational(ExactRational::from_integer(
        denominator.clone(),
    )));
    let is_one = scaled
        .as_rational()
        .is_some_and(|rational| rational.is_one());
    let mut product: Option<ExprId> = if is_one && !monomial.is_constant() {
        None
    } else {
        Some(square_root_sum_expression(pool, &scaled).ok()?)
    };
    for (atom, exponent) in monomial.factors() {
        let base = atoms.expression(*atom)?;
        let factor = if *exponent == 1 {
            base
        } else {
            let power = pool
                .number(Integer::from(i64::from(*exponent)).into())
                .ok()?;
            pool.apply(Head::Operator(Operator::Pow), &[base, power])
                .ok()?
        };
        product = Some(match product {
            Some(running) => pool
                .apply(Head::Operator(Operator::Mul), &[running, factor])
                .ok()?,
            None => factor,
        });
    }
    let product = product?;
    if denominator == Integer::one() {
        return Some(product);
    }
    let denominator = pool.number(denominator.into()).ok()?;
    pool.apply(Head::Operator(Operator::Div), &[product, denominator])
        .ok()
}

fn leading_sign(coefficient: &SquareRootSum) -> Ordering {
    coefficient
        .terms()
        .next()
        .map_or(Ordering::Equal, |(_, rational)| rational.sign())
}

fn written_terms(polynomial: &Polynomial) -> Vec<(Monomial, SquareRootSum)> {
    let mut written = Vec::new();
    for (monomial, coefficient) in polynomial.terms() {
        if !monomial.is_constant() {
            written.push((monomial.clone(), coefficient.clone()));
            continue;
        }
        for term in coefficient.separated_terms() {
            written.push((monomial.clone(), term));
        }
    }
    written
}

pub(crate) fn univariate_polynomial(
    coefficients: &[SquareRootSum],
    atom: usize,
) -> Option<Polynomial> {
    let mut polynomial = Polynomial::default();
    for (power, coefficient) in coefficients.iter().enumerate() {
        if coefficient.is_zero() {
            continue;
        }
        let monomial = Monomial::constant().with_exponent(atom, u32::try_from(power).ok()?)?;
        polynomial.insert(monomial, coefficient.clone())?;
    }
    Some(polynomial)
}

pub(crate) fn linear_combination(
    constant: SquareRootSum,
    terms: &[(usize, SquareRootSum)],
) -> Option<Polynomial> {
    let mut combination = Polynomial::constant(constant);
    for (atom, coefficient) in terms {
        let mut term = Polynomial::default();
        term.insert(Monomial::single(*atom), coefficient.clone())?;
        combination = combination.plus(&term)?;
    }
    Some(combination)
}

pub fn polynomial_expression(
    pool: &mut ExprPool,
    polynomial: &Polynomial,
    atoms: &AtomTable,
) -> Option<ExprId> {
    if let Some(constant) = polynomial.as_constant() {
        return square_root_sum_expression(pool, &constant).ok();
    }
    let mut total: Option<ExprId> = None;
    for (monomial, coefficient) in written_terms(polynomial) {
        let is_negative = leading_sign(&coefficient) == Ordering::Less && total.is_some();
        let coefficient = if is_negative {
            coefficient.negated()
        } else {
            coefficient
        };
        let term = term_expression(pool, &monomial, &coefficient, atoms)?;
        let operator = if is_negative {
            Operator::Sub
        } else {
            Operator::Add
        };
        total = Some(match total {
            Some(running) => pool
                .apply(Head::Operator(operator), &[running, term])
                .ok()?,
            None => term,
        });
    }
    match total {
        Some(total) => Some(total),
        None => pool.number(Integer::zero().into()).ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_syntax::{PrintMode, parse_expression, print_expression};

    fn normalized(text: &str) -> Option<String> {
        let mut pool = ExprPool::new();
        let expression = parse_expression(&mut pool, text).ok()?;
        let mut atoms = AtomTable::default();
        let polynomial = polynomial_of(&mut pool, expression, &mut atoms)?;
        let rebuilt = polynomial_expression(&mut pool, &polynomial, &atoms)?;
        print_expression(&pool, rebuilt, PrintMode::Ascii).ok()
    }

    fn difference_is_zero(left: &str, right: &str) -> bool {
        let mut pool = ExprPool::new();
        let left = parse_expression(&mut pool, left).expect("left parses");
        let right = parse_expression(&mut pool, right).expect("right parses");
        let mut atoms = AtomTable::default();
        let left = polynomial_of(&mut pool, left, &mut atoms).expect("left is a polynomial");
        let right = polynomial_of(&mut pool, right, &mut atoms).expect("right is a polynomial");
        left.plus(&right.negated())
            .expect("the difference fits")
            .as_constant()
            .is_some_and(|constant| constant.is_zero())
    }

    fn reduced(text: &str) -> Option<(String, Vec<String>)> {
        let mut pool = ExprPool::new();
        let expression = parse_expression(&mut pool, text).ok()?;
        let quotient = reduced_quotient(&mut pool, expression)?;
        let shown = print_expression(&pool, quotient.expression, PrintMode::Ascii).ok()?;
        let excluding = quotient
            .excluding
            .iter()
            .map(|written| print_expression(&pool, *written, PrintMode::Ascii).ok())
            .collect::<Option<Vec<String>>>()?;
        Some((shown, excluding))
    }

    fn reduced_to(text: &str, expected: &str, excluding: &[&str]) {
        let (shown, excluded) = reduced(text).expect("the line reduces");
        assert_eq!(shown, expected, "{text}");
        assert_eq!(excluded, excluding, "{text}");
    }

    #[test]
    fn a_sum_of_equal_quotients_keeps_its_denominator_and_no_condition() {
        reduced_to("1/x + 1/x", "2 / x", &[]);
    }

    #[test]
    fn a_name_over_itself_is_one_wherever_it_is_not_zero() {
        reduced_to("x/x", "1", &["x"]);
    }

    #[test]
    fn a_difference_of_equal_quotients_is_zero_wherever_the_name_is_not_zero() {
        reduced_to("1/x - 1/x", "0", &["x"]);
    }

    #[test]
    fn a_cancelled_linear_factor_is_named_as_written() {
        reduced_to("(x^2-1)/(x-1)", "x + 1", &["x - 1"]);
    }

    #[test]
    fn two_quotients_are_brought_over_their_least_common_multiple() {
        reduced_to("1/(x-1) + 1/(x+1)", "2 * x / (x^2 - 1)", &[]);
    }

    #[test]
    fn a_difference_of_squares_in_two_names_cancels() {
        reduced_to("(x^2 - y^2)/(x - y)", "x + y", &["x - y"]);
    }

    #[test]
    fn a_name_cancelled_from_a_product_is_named() {
        reduced_to("x/(x*y)", "1 / y", &["x * y"]);
    }

    #[test]
    fn a_constant_is_cancelled_jointly_from_both_sides() {
        reduced_to("2/(4*x)", "1 / (2 * x)", &[]);
        reduced_to("(2*x + 2)/(4*x)", "(x + 1) / (2 * x)", &[]);
    }

    #[test]
    fn a_square_root_coefficient_cancels_over_its_field() {
        reduced_to("(x^2-2)/(x - sqrt(2))", "x + sqrt(2)", &["x - sqrt(2)"]);
    }

    #[test]
    fn a_square_root_common_to_both_sides_cancels_in_two_names() {
        reduced_to("(sqrt(2)*x)/(sqrt(2)*y)", "x / y", &[]);
    }

    #[test]
    fn a_quotient_that_is_rational_once_monic_reduces_in_two_names() {
        reduced_to(
            "(sqrt(2)*x^2 - sqrt(2)*y^2)/(sqrt(3)*x - sqrt(3)*y)",
            "sqrt(6) * x / 3 + sqrt(6) * y / 3",
            &["sqrt(3) * x - sqrt(3) * y"],
        );
    }

    #[test]
    fn a_square_root_factor_in_two_names_cancels_over_the_field() {
        reduced_to(
            "(x^2 - 2*y^2)/(x - sqrt(2)*y)",
            "x + sqrt(2) * y",
            &["x - sqrt(2) * y"],
        );
    }

    #[test]
    fn a_factor_with_two_square_roots_in_two_names_cancels_over_the_field() {
        reduced_to(
            "((x + sqrt(2)*y + sqrt(3))*(x - y))/((x + sqrt(2)*y + sqrt(3))*(x + y))",
            "(x - y) / (x + y)",
            &["(x + sqrt(2) * y + sqrt(3)) * (x + y)"],
        );
    }

    #[test]
    fn a_factor_with_large_coefficients_cancels_over_the_field_within_the_bound() {
        reduced_to(
            "((x + 1000003*sqrt(2)*y + 7/9)*(x - y))/((x + 1000003*sqrt(2)*y + 7/9)*(x + y))",
            "(x - y) / (x + y)",
            &["(x + 1000003 * sqrt(2) * y + 7 / 9) * (x + y)"],
        );
    }

    #[test]
    fn the_derivative_of_a_quotient_is_reduced() {
        reduced_to("diff(x/(x+1), x)", "1 / (x^2 + 2 * x + 1)", &[]);
    }

    #[test]
    fn a_denominator_with_no_real_zero_is_still_named() {
        reduced_to("(x^3 + x)/(x^2 + 1)", "x", &["x^2 + 1"]);
    }

    #[test]
    fn a_square_of_a_sum_expands() {
        assert_eq!(normalized("(x + 1)^2").as_deref(), Some("x^2 + 2 * x + 1"));
    }

    #[test]
    fn a_binomial_identity_has_a_zero_difference() {
        assert!(difference_is_zero("(x + 1)^2", "x^2 + 2*x + 1"));
    }

    #[test]
    fn a_wrong_identity_has_a_difference_that_is_not_zero() {
        assert!(!difference_is_zero("(x + 1)^2", "x^2 + 1"));
    }

    #[test]
    fn a_difference_of_squares_factors_back_to_the_same_polynomial() {
        assert!(difference_is_zero("(x - 1) * (x + 1)", "x^2 - 1"));
    }

    #[test]
    fn a_cube_of_a_sum_expands_with_the_binomial_coefficients() {
        assert_eq!(
            normalized("(x + y)^3").as_deref(),
            Some("x^3 + 3 * x^2 * y + 3 * x * y^2 + y^3")
        );
    }

    #[test]
    fn an_opaque_call_is_treated_as_one_atom() {
        assert!(difference_is_zero(
            "(sin(x) + 1)^2",
            "sin(x)^2 + 2*sin(x) + 1"
        ));
    }

    #[test]
    fn a_division_by_a_number_stays_exact() {
        assert!(difference_is_zero("(x + 1) / 2", "x/2 + 1/2"));
    }

    #[test]
    fn a_division_by_a_variable_becomes_an_atom_rather_than_a_guess() {
        assert!(!difference_is_zero("(x^2 - 1) / (x - 1)", "x + 1"));
    }

    #[test]
    fn a_polynomial_with_no_terms_prints_as_zero() {
        assert_eq!(normalized("x - x").as_deref(), Some("0"));
    }

    #[test]
    fn the_same_expression_written_two_ways_normalizes_alike() {
        assert_eq!(normalized("2*x + 3*x"), normalized("5*x"));
    }

    #[test]
    fn a_constant_radical_expands_as_the_evaluator_writes_it() {
        assert_eq!(
            normalized("(3 + sqrt(8))^2").as_deref(),
            Some("17 + 12 * sqrt(2)")
        );
    }

    #[test]
    fn a_square_root_squared_is_its_radicand() {
        assert!(difference_is_zero("sqrt(2) * sqrt(2)", "2"));
    }

    #[test]
    fn a_radicand_is_made_squarefree() {
        assert!(difference_is_zero("sqrt(8)", "2 * sqrt(2)"));
    }

    #[test]
    fn a_radical_coefficient_multiplies_through_a_free_name() {
        assert_eq!(
            normalized("(x + sqrt(2))^2").as_deref(),
            Some("x^2 + 2 * sqrt(2) * x + 2")
        );
    }

    #[test]
    fn a_difference_of_radical_squares_is_rational() {
        assert_eq!(
            normalized("(x - sqrt(3)) * (x + sqrt(3))").as_deref(),
            Some("x^2 - 3")
        );
    }

    #[test]
    fn a_division_by_a_radical_is_rationalised() {
        assert!(difference_is_zero("1 / sqrt(2)", "sqrt(2) / 2"));
    }

    #[test]
    fn a_radicand_that_is_not_constant_stays_an_atom() {
        assert!(!difference_is_zero("sqrt(x)^2", "x"));
    }

    #[test]
    fn a_cube_root_stays_an_atom() {
        assert!(!difference_is_zero("(2^(1/3))^3", "2"));
    }

    #[test]
    fn the_square_root_of_zero_vanishes_from_a_term() {
        assert!(difference_is_zero("sqrt(0) * x", "0"));
    }

    #[test]
    fn a_closed_part_the_evaluator_carries_exactly_is_folded() {
        assert_eq!(normalized("x + sin(pi/6)").as_deref(), Some("x + 1 / 2"));
    }

    #[test]
    fn a_whole_cube_root_of_a_cube_is_folded() {
        assert_eq!(
            normalized("(x + 8^(1/3))^2").as_deref(),
            Some("x^2 + 4 * x + 4")
        );
    }

    #[test]
    fn a_closed_part_the_evaluator_cannot_carry_exactly_stays_an_atom() {
        assert_eq!(normalized("x + sin(1)").as_deref(), Some("x + sin(1)"));
    }
}
