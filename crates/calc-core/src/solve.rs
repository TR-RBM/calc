use calc_expr::{
    BinderKind, BuiltinConstant, ExprId, ExprPool, Head, NodeView, Operator, SymbolId,
};
use calc_numbers::Integer;

use crate::exact_evaluation::square_root_sum_expression;
use crate::exact_rational::ExactRational;
use crate::integer_roots::UnsplitFactor;
use crate::integer_roots::integer_root;
use crate::machine_evaluation::{SampleEnclosure, enclose_sample};
use crate::polynomial::{
    AtomTable, Polynomial, polynomial_expression, polynomial_of, rational_of, reduced_quotient,
};
use crate::square_root_sum::{SquareRootRefusal, SquareRootSum};

const DEGREE_LIMIT: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SolveRefusal {
    NotAPolynomial,
    NoName,
    SeveralNames(Vec<String>),
    EveryNumber,
    DegreeTooHigh(usize),
    RadicalCoefficients(usize),
    SquarePartUnknown(Integer),
    ConstantCoefficients(Vec<String>),
    ConstantCoefficientDegree(usize),
    UnprovenCoefficient,
    CoefficientOutsideTheField(ExprId),
}

fn real_square_root(discriminant: &ExactRational) -> Result<Option<SquareRootSum>, SolveRefusal> {
    match SquareRootSum::real_square_root_of_rational(discriminant) {
        Ok(root) => Ok(Some(root)),
        Err(SquareRootRefusal::Negative) => Ok(None),
        Err(SquareRootRefusal::UnsplitFactor(number)) => {
            Err(SolveRefusal::SquarePartUnknown(number))
        }
    }
}

fn coefficients(
    pool: &mut ExprPool,
    expression: ExprId,
    unknown: SymbolId,
) -> Result<Vec<SquareRootSum>, SolveRefusal> {
    let mut atoms = AtomTable::default();
    let polynomial =
        polynomial_of(pool, expression, &mut atoms).ok_or(SolveRefusal::NotAPolynomial)?;
    let name = pool.symbol(unknown).map_err(|_| SolveRefusal::NoName)?;
    let mut index = None;
    for (position, atom) in atoms.expressions().iter().enumerate() {
        if *atom == name {
            index = Some(position);
            continue;
        }
        return Err(SolveRefusal::NotAPolynomial);
    }
    let Some(index) = index else {
        return Ok(vec![
            polynomial
                .as_constant()
                .ok_or(SolveRefusal::NotAPolynomial)?,
        ]);
    };
    let mut found: Vec<SquareRootSum> = Vec::new();
    for (monomial, coefficient) in polynomial.terms() {
        if monomial.factors().len() > 1 {
            return Err(SolveRefusal::NotAPolynomial);
        }
        let degree = monomial.exponent_of(index) as usize;
        if degree > DEGREE_LIMIT {
            return Err(SolveRefusal::DegreeTooHigh(degree));
        }
        while found.len() <= degree {
            found.push(SquareRootSum::zero());
        }
        found[degree] = coefficient.clone();
    }
    if found.is_empty() {
        found.push(SquareRootSum::zero());
    }
    Ok(found)
}

fn radical_roots(
    pool: &mut ExprPool,
    coefficients: &[SquareRootSum],
) -> Result<Vec<ExprId>, SolveRefusal> {
    let degree = coefficients.len() - 1;
    let mut roots: Vec<SquareRootSum> = match coefficients {
        [constant, linear] => {
            vec![
                constant
                    .negated()
                    .divided_by(linear)
                    .ok_or(SolveRefusal::NotAPolynomial)?,
            ]
        }
        [constant, linear, square] => {
            let four = SquareRootSum::from_rational(ExactRational::from_i64(4));
            let discriminant = linear
                .times(linear)
                .minus(&four.times(square).times(constant));
            let Some(discriminant) = discriminant.as_rational() else {
                return Err(SolveRefusal::RadicalCoefficients(degree));
            };
            let Some(root) = real_square_root(&discriminant)? else {
                return Ok(Vec::new());
            };
            let twice = SquareRootSum::from_rational(ExactRational::from_i64(2)).times(square);
            let mut found = Vec::new();
            for candidate in [linear.negated().plus(&root), linear.negated().minus(&root)] {
                found.push(
                    candidate
                        .divided_by(&twice)
                        .ok_or(SolveRefusal::NotAPolynomial)?,
                );
            }
            found
        }
        _ => return Err(SolveRefusal::RadicalCoefficients(degree)),
    };
    roots.sort_by(SquareRootSum::compare);
    roots.dedup();
    roots
        .iter()
        .map(|root| {
            square_root_sum_expression(pool, root).map_err(|_| SolveRefusal::NotAPolynomial)
        })
        .collect()
}

fn cleared(coefficients: &[ExactRational]) -> Option<Vec<Integer>> {
    let mut multiple = Integer::one();
    for coefficient in coefficients {
        let denominator = coefficient.denominator().clone();
        let common = multiple.gcd(&denominator);
        let (quotient, _) = denominator.div_rem_euclid(&common).ok()?;
        multiple = &multiple * &quotient;
    }
    coefficients
        .iter()
        .map(|coefficient| {
            let scaled = coefficient.multiply(&ExactRational::from_integer(multiple.clone()));
            scaled.is_integer().then(|| scaled.numerator().clone())
        })
        .collect()
}

fn value_at(coefficients: &[ExactRational], point: &ExactRational) -> ExactRational {
    let mut total = ExactRational::zero();
    for coefficient in coefficients.iter().rev() {
        total = total.multiply(point).plus(coefficient);
    }
    total
}

fn divided_by_root(coefficients: &[ExactRational], root: &ExactRational) -> Vec<ExactRational> {
    let mut carried = ExactRational::zero();
    let mut quotient = vec![ExactRational::zero(); coefficients.len().saturating_sub(1)];
    for (position, coefficient) in coefficients.iter().enumerate().rev() {
        if position == 0 {
            break;
        }
        carried = carried.multiply(root).plus(coefficient);
        quotient[position - 1] = carried.clone();
    }
    quotient
}

pub(crate) fn rational_roots(
    coefficients: &[ExactRational],
) -> Option<(Vec<ExactRational>, Vec<ExactRational>)> {
    let mut remaining = coefficients.to_vec();
    let mut roots: Vec<ExactRational> = Vec::new();
    while remaining.len() > 1 && remaining[0].is_zero() {
        roots.push(ExactRational::zero());
        remaining.remove(0);
    }
    if remaining.len() <= 1 {
        return Some((roots, remaining));
    }
    if remaining.last()?.is_zero() {
        return None;
    }
    let square_free = crate::real_roots::square_free_part(&remaining)?;
    for root in crate::polynomial_factoring::rational_roots_of_square_free(&square_free)? {
        while remaining.len() > 1 && value_at(&remaining, &root).is_zero() {
            remaining = divided_by_root(&remaining, &root);
            roots.push(root.clone());
        }
    }
    Some((roots, remaining))
}

fn number(pool: &mut ExprPool, value: &ExactRational) -> Option<ExprId> {
    pool.number(value.to_number()).ok()
}

pub(crate) fn written_rational(pool: &mut ExprPool, value: &ExactRational) -> Option<ExprId> {
    if value.is_integer() {
        return number(pool, value);
    }
    let numerator = number(
        pool,
        &ExactRational::from_integer(value.numerator().clone()),
    )?;
    let denominator = number(
        pool,
        &ExactRational::from_integer(value.denominator().clone()),
    )?;
    pool.apply(Head::Operator(Operator::Div), &[numerator, denominator])
        .ok()
}

fn scaled_root(pool: &mut ExprPool, scale: &ExactRational, root: ExprId) -> Option<ExprId> {
    if scale.is_one() {
        return Some(root);
    }
    let above = if scale.numerator().is_one() {
        root
    } else {
        let factor = number(
            pool,
            &ExactRational::from_integer(scale.numerator().clone()),
        )?;
        pool.apply(Head::Operator(Operator::Mul), &[factor, root])
            .ok()?
    };
    if scale.is_integer() {
        return Some(above);
    }
    let denominator = number(
        pool,
        &ExactRational::from_integer(scale.denominator().clone()),
    )?;
    pool.apply(Head::Operator(Operator::Div), &[above, denominator])
        .ok()
}

fn quadratic_roots(
    pool: &mut ExprPool,
    coefficients: &[ExactRational],
) -> Result<Vec<ExprId>, SolveRefusal> {
    let (constant, linear, square) = (
        coefficients[0].clone(),
        coefficients[1].clone(),
        coefficients[2].clone(),
    );
    let four = ExactRational::from_i64(4);
    let discriminant = linear
        .multiply(&linear)
        .subtract(&four.multiply(&square).multiply(&constant));
    if discriminant.sign() == std::cmp::Ordering::Less {
        return Ok(Vec::new());
    }
    let twice = ExactRational::from_i64(2).multiply(&square);
    if let Some(root) = exact_square_root(&discriminant) {
        let mut found = Vec::new();
        for sign in [ExactRational::one(), ExactRational::one().negated()] {
            let value = linear
                .negated()
                .plus(&sign.multiply(&root))
                .divide(&twice)
                .ok_or(SolveRefusal::NotAPolynomial)?;
            if !found.contains(&value) {
                found.push(value);
            }
        }
        found.sort_by(ExactRational::compare);
        return found
            .iter()
            .map(|value| number(pool, value).ok_or(SolveRefusal::NotAPolynomial))
            .collect();
    }
    let numerator = discriminant.numerator().clone();
    let denominator = discriminant.denominator().clone();
    let decomposition =
        crate::integer_roots::square_free_decomposition(&(&numerator * &denominator))
            .map_err(|UnsplitFactor(number)| SolveRefusal::SquarePartUnknown(number))?;
    let outside = ExactRational::fraction(&decomposition.square_root_of_square_part, &denominator)
        .ok_or(SolveRefusal::NotAPolynomial)?;
    let shift = linear
        .negated()
        .divide(&twice)
        .ok_or(SolveRefusal::NotAPolynomial)?;
    let scale = outside.divide(&twice).ok_or(SolveRefusal::NotAPolynomial)?;
    let radicand = number(pool, &ExactRational::from_integer(decomposition.radicand))
        .ok_or(SolveRefusal::NotAPolynomial)?;
    let root = pool
        .apply(Head::Operator(Operator::Sqrt), &[radicand])
        .map_err(|_| SolveRefusal::NotAPolynomial)?;
    let mut found = Vec::new();
    for is_negative in [true, false] {
        let term = scaled_root(pool, &scale, root).ok_or(SolveRefusal::NotAPolynomial)?;
        if shift.is_zero() {
            found.push(if is_negative {
                pool.apply(Head::Operator(Operator::Neg), &[term])
                    .map_err(|_| SolveRefusal::NotAPolynomial)?
            } else {
                term
            });
            continue;
        }
        let base = written_rational(pool, &shift).ok_or(SolveRefusal::NotAPolynomial)?;
        let operator = if is_negative {
            Operator::Sub
        } else {
            Operator::Add
        };
        found.push(
            pool.apply(Head::Operator(operator), &[base, term])
                .map_err(|_| SolveRefusal::NotAPolynomial)?,
        );
    }
    Ok(found)
}

fn exact_square_root(value: &ExactRational) -> Option<ExactRational> {
    let (numerator, exact) = integer_root(&value.numerator().absolute(), 2);
    if !exact {
        return None;
    }
    let (denominator, exact) = integer_root(value.denominator(), 2);
    if !exact {
        return None;
    }
    ExactRational::fraction(&numerator, &denominator)
}

pub fn solutions(
    pool: &mut ExprPool,
    equation: ExprId,
    unknown: SymbolId,
) -> Result<Vec<ExprId>, SolveRefusal> {
    let difference = match pool
        .node(equation)
        .map_err(|_| SolveRefusal::NotAPolynomial)?
    {
        NodeView::Apply {
            head: Head::Operator(Operator::Equal),
            arguments: [left, right],
        } => {
            let (left, right) = (*left, *right);
            pool.apply(Head::Operator(Operator::Sub), &[left, right])
                .map_err(|_| SolveRefusal::NotAPolynomial)?
        }
        _ => equation,
    };
    let coefficients = coefficients(pool, difference, unknown)?;
    roots_of_coefficients(pool, coefficients, unknown)
}

fn holds_symbol(pool: &ExprPool, expression: ExprId, symbol: SymbolId) -> bool {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        match pool.node(node) {
            Ok(NodeView::Symbol(found)) if found == symbol => return true,
            Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().copied()),
            _ => {}
        }
    }
    false
}

fn transcendental_constant(pool: &ExprPool, atom: ExprId) -> Option<BuiltinConstant> {
    match pool.node(atom).ok()? {
        NodeView::Symbol(symbol) if symbol == BuiltinConstant::Pi.symbol() => {
            Some(BuiltinConstant::Pi)
        }
        NodeView::Symbol(symbol) if symbol == BuiltinConstant::E.symbol() => {
            Some(BuiltinConstant::E)
        }
        _ => None,
    }
}

fn is_proven_nonzero(pool: &mut ExprPool, coefficient: &Polynomial, atoms: &AtomTable) -> bool {
    if coefficient.is_zero() {
        return false;
    }
    if coefficient.atoms_used().len() <= 1 {
        return true;
    }
    let Some(expression) = polynomial_expression(pool, coefficient, atoms) else {
        return false;
    };
    matches!(
        enclose_sample(pool, expression, &[]),
        SampleEnclosure::Real(Some(interval)) if interval.lower() > 0.0 || interval.upper() < 0.0
    )
}

pub fn constant_coefficient_solutions(
    pool: &mut ExprPool,
    equation: ExprId,
    unknown: SymbolId,
) -> Result<Vec<ExprId>, SolveRefusal> {
    let difference = match pool
        .node(equation)
        .map_err(|_| SolveRefusal::NotAPolynomial)?
    {
        NodeView::Apply {
            head: Head::Operator(Operator::Equal),
            arguments: [left, right],
        } => {
            let (left, right) = (*left, *right);
            pool.apply(Head::Operator(Operator::Sub), &[left, right])
                .map_err(|_| SolveRefusal::NotAPolynomial)?
        }
        _ => equation,
    };
    let mut atoms = AtomTable::default();
    let quotient = rational_of(pool, difference, &mut atoms).ok_or(SolveRefusal::NotAPolynomial)?;
    let name = pool.symbol(unknown).map_err(|_| SolveRefusal::NoName)?;
    let mut unknown_atom = None;
    for (position, atom) in atoms.expressions().iter().enumerate() {
        if *atom == name {
            unknown_atom = Some(position);
        } else if holds_symbol(pool, *atom, unknown) {
            return Err(SolveRefusal::NotAPolynomial);
        } else if transcendental_constant(pool, *atom).is_none() {
            return Err(SolveRefusal::CoefficientOutsideTheField(*atom));
        }
    }
    if unknown_atom.is_some_and(|atom| quotient.denominator().atoms_used().contains(&atom)) {
        return Err(SolveRefusal::NotAPolynomial);
    }
    if !is_proven_nonzero(pool, quotient.denominator(), &atoms) {
        return Err(SolveRefusal::UnprovenCoefficient);
    }
    let polynomial = quotient.numerator().clone();
    let coefficients = match unknown_atom {
        Some(atom) => polynomial
            .coefficients_in(atom)
            .ok_or(SolveRefusal::NotAPolynomial)?,
        None => vec![polynomial],
    };
    let Some(degree) = coefficients
        .iter()
        .rposition(|coefficient| !coefficient.is_zero())
    else {
        return Err(SolveRefusal::EveryNumber);
    };
    if !is_proven_nonzero(pool, &coefficients[degree], &atoms) {
        return Err(SolveRefusal::UnprovenCoefficient);
    }
    match degree {
        0 => Ok(Vec::new()),
        1 => {
            let constant = polynomial_expression(pool, &coefficients[0].negated(), &atoms)
                .ok_or(SolveRefusal::NotAPolynomial)?;
            let linear = polynomial_expression(pool, &coefficients[1], &atoms)
                .ok_or(SolveRefusal::NotAPolynomial)?;
            let quotient = pool
                .apply(Head::Operator(Operator::Div), &[constant, linear])
                .map_err(|_| SolveRefusal::NotAPolynomial)?;
            Ok(vec![
                reduced_quotient(pool, quotient).map_or(quotient, |reduced| reduced.expression),
            ])
        }
        _ => Err(SolveRefusal::ConstantCoefficientDegree(degree)),
    }
}

pub fn leaves_out_non_real_roots(
    pool: &mut ExprPool,
    equation: ExprId,
    unknown: SymbolId,
    real_roots: usize,
) -> Option<bool> {
    let difference = match pool.node(equation).ok()? {
        NodeView::Apply {
            head: Head::Operator(Operator::Equal),
            arguments: [left, right],
        } => {
            let (left, right) = (*left, *right);
            pool.apply(Head::Operator(Operator::Sub), &[left, right])
                .ok()?
        }
        _ => equation,
    };
    let coefficients = coefficients(pool, difference, unknown).ok()?;
    let degree = coefficients.len().checked_sub(1)?;
    if degree == 0 {
        return Some(false);
    }
    if real_roots == 0 {
        return Some(true);
    }
    let rational: Vec<ExactRational> = coefficients
        .iter()
        .map(SquareRootSum::as_rational)
        .collect::<Option<_>>()?;
    let square_free = crate::real_roots::square_free_part(&rational)?;
    let distinct_roots = square_free.coefficients().len().checked_sub(1)?;
    Some(distinct_roots > real_roots)
}

pub(crate) fn roots_of_coefficients(
    pool: &mut ExprPool,
    coefficients: Vec<SquareRootSum>,
    unknown: SymbolId,
) -> Result<Vec<ExprId>, SolveRefusal> {
    let degree = coefficients.len() - 1;
    if degree == 0 {
        return if coefficients[0].is_zero() {
            Err(SolveRefusal::EveryNumber)
        } else {
            Ok(Vec::new())
        };
    }
    let rational: Option<Vec<ExactRational>> = coefficients
        .iter()
        .map(SquareRootSum::as_rational)
        .collect();
    let Some(coefficients) = rational else {
        return radical_roots(pool, &coefficients);
    };
    if degree == 2 {
        return quadratic_roots(pool, &coefficients);
    }
    let (mut roots, remaining) =
        rational_roots(&coefficients).ok_or(SolveRefusal::DegreeTooHigh(degree))?;
    roots.sort_by(ExactRational::compare);
    roots.dedup();
    match remaining.len().saturating_sub(1) {
        0 => roots
            .iter()
            .map(|value| number(pool, value).ok_or(SolveRefusal::NotAPolynomial))
            .collect(),
        2 => with_quadratic_rest(pool, &roots, &remaining),
        _ => with_algebraic_rest(pool, &roots, &remaining, unknown),
    }
}

fn with_quadratic_rest(
    pool: &mut ExprPool,
    rational: &[ExactRational],
    rest: &[ExactRational],
) -> Result<Vec<ExprId>, SolveRefusal> {
    let (constant, linear, square) = (
        SquareRootSum::from_rational(rest[0].clone()),
        SquareRootSum::from_rational(rest[1].clone()),
        SquareRootSum::from_rational(rest[2].clone()),
    );
    let four = SquareRootSum::from_rational(ExactRational::from_i64(4));
    let discriminant = linear
        .times(&linear)
        .minus(&four.times(&square).times(&constant))
        .as_rational()
        .ok_or(SolveRefusal::NotAPolynomial)?;
    let mut values: Vec<SquareRootSum> = rational
        .iter()
        .cloned()
        .map(SquareRootSum::from_rational)
        .collect();
    if let Some(root) = real_square_root(&discriminant)? {
        let twice = SquareRootSum::from_rational(ExactRational::from_i64(2)).times(&square);
        for candidate in [linear.negated().plus(&root), linear.negated().minus(&root)] {
            values.push(
                candidate
                    .divided_by(&twice)
                    .ok_or(SolveRefusal::NotAPolynomial)?,
            );
        }
    }
    values.sort_by(SquareRootSum::compare);
    values.dedup();
    values
        .iter()
        .map(|value| {
            square_root_sum_expression(pool, value).map_err(|_| SolveRefusal::NotAPolynomial)
        })
        .collect()
}

enum FoundRoot {
    Exact(SquareRootSum),
    Algebraic(crate::real_roots::RealRoot, ExprId),
}

fn ordered(left: &mut FoundRoot, right: &mut FoundRoot) -> std::cmp::Ordering {
    match (left, right) {
        (FoundRoot::Exact(a), FoundRoot::Exact(b)) => a.compare(b),
        (FoundRoot::Algebraic(root, _), FoundRoot::Exact(value)) => root.compare_with_sum(value),
        (FoundRoot::Exact(value), FoundRoot::Algebraic(root, _)) => {
            root.compare_with_sum(value).reverse()
        }
        (FoundRoot::Algebraic(a, _), FoundRoot::Algebraic(b, _)) => a.separated_from(b),
    }
}

fn roots_of_factor(
    pool: &mut ExprPool,
    factor: &[ExactRational],
    name: &str,
    found: &mut Vec<FoundRoot>,
) -> Result<(), SolveRefusal> {
    let as_sum = |value: &ExactRational| SquareRootSum::from_rational(value.clone());
    match factor.len().saturating_sub(1) {
        0 => {}
        1 => found.push(FoundRoot::Exact(
            as_sum(&factor[0])
                .negated()
                .divided_by(&as_sum(&factor[1]))
                .ok_or(SolveRefusal::NotAPolynomial)?,
        )),
        2 => {
            let (constant, linear, square) =
                (as_sum(&factor[0]), as_sum(&factor[1]), as_sum(&factor[2]));
            let four = SquareRootSum::from_rational(ExactRational::from_i64(4));
            let discriminant = linear
                .times(&linear)
                .minus(&four.times(&square).times(&constant))
                .as_rational()
                .ok_or(SolveRefusal::NotAPolynomial)?;
            if let Some(root) = real_square_root(&discriminant)? {
                let twice = SquareRootSum::from_rational(ExactRational::from_i64(2)).times(&square);
                for candidate in [linear.negated().minus(&root), linear.negated().plus(&root)] {
                    found.push(FoundRoot::Exact(
                        candidate
                            .divided_by(&twice)
                            .ok_or(SolveRefusal::NotAPolynomial)?,
                    ));
                }
            }
        }
        _ => {
            let body = primitive_polynomial(pool, factor)?;
            let roots =
                crate::real_roots::real_roots(factor).ok_or(SolveRefusal::NotAPolynomial)?;
            for (position, root) in roots.into_iter().enumerate() {
                let index =
                    ExactRational::from_i64(i64::try_from(position + 1).unwrap_or(i64::MAX));
                let index = number(pool, &index).ok_or(SolveRefusal::NotAPolynomial)?;
                let binder = pool
                    .bind(BinderKind::Root, &[index], body)
                    .map_err(|_| SolveRefusal::NotAPolynomial)?;
                pool.record_bound_name(binder, name)
                    .map_err(|_| SolveRefusal::NotAPolynomial)?;
                found.push(FoundRoot::Algebraic(root, binder));
            }
        }
    }
    Ok(())
}

fn with_algebraic_rest(
    pool: &mut ExprPool,
    rational: &[ExactRational],
    rest: &[ExactRational],
    unknown: SymbolId,
) -> Result<Vec<ExprId>, SolveRefusal> {
    let name = pool
        .symbol_name(unknown)
        .map_err(|_| SolveRefusal::NoName)?
        .to_owned();
    let mut found: Vec<FoundRoot> = rational
        .iter()
        .map(|value| FoundRoot::Exact(SquareRootSum::from_rational(value.clone())))
        .collect();
    match crate::polynomial_factoring::factor_over_rationals(rest) {
        Some(factoring) => {
            for (factor, _) in &factoring.factors {
                let factor: Vec<ExactRational> = factor
                    .iter()
                    .map(|coefficient| ExactRational::from_integer(coefficient.clone()))
                    .collect();
                roots_of_factor(pool, &factor, &name, &mut found)?;
            }
        }
        None => roots_of_factor(pool, rest, &name, &mut found)?,
    }
    for position in 1..found.len() {
        let mut current = position;
        while current > 0 {
            let (before, after) = found.split_at_mut(current);
            if ordered(&mut before[current - 1], &mut after[0]) != std::cmp::Ordering::Greater {
                break;
            }
            found.swap(current - 1, current);
            current -= 1;
        }
    }
    let mut written = Vec::with_capacity(found.len());
    for root in found {
        written.push(match root {
            FoundRoot::Exact(value) => square_root_sum_expression(pool, &value)
                .map_err(|_| SolveRefusal::NotAPolynomial)?,
            FoundRoot::Algebraic(_, binder) => binder,
        });
    }
    Ok(written)
}

fn primitive_polynomial(
    pool: &mut ExprPool,
    coefficients: &[ExactRational],
) -> Result<ExprId, SolveRefusal> {
    let variable = pool.bound(0).map_err(|_| SolveRefusal::NotAPolynomial)?;
    polynomial_in(pool, coefficients, variable)
}

pub(crate) fn polynomial_in(
    pool: &mut ExprPool,
    coefficients: &[ExactRational],
    variable: ExprId,
) -> Result<ExprId, SolveRefusal> {
    let mut whole = cleared(coefficients).ok_or(SolveRefusal::NotAPolynomial)?;
    let common = whole.iter().fold(Integer::zero(), |common, coefficient| {
        common.gcd(coefficient)
    });
    if !common.is_zero() && !common.is_one() {
        whole = whole
            .iter()
            .map(|coefficient| {
                coefficient
                    .div_rem_euclid(&common)
                    .map(|(quotient, _)| quotient)
                    .unwrap_or_else(|_| coefficient.clone())
            })
            .collect();
    }
    if whole.last().is_some_and(Integer::is_negative) {
        whole = whole.iter().map(Integer::negated).collect();
    }
    let fail = |_| SolveRefusal::NotAPolynomial;
    let mut total: Option<ExprId> = None;
    for (power, coefficient) in whole.iter().enumerate().rev() {
        if coefficient.is_zero() {
            continue;
        }
        let is_negative = coefficient.is_negative() && total.is_some();
        let magnitude = if is_negative {
            coefficient.negated()
        } else {
            coefficient.clone()
        };
        let factor = match power {
            0 => None,
            1 => Some(variable),
            _ => {
                let exponent = pool
                    .number(Integer::from(i64::try_from(power).unwrap_or(i64::MAX)).into())
                    .map_err(fail)?;
                Some(
                    pool.apply(Head::Operator(Operator::Pow), &[variable, exponent])
                        .map_err(fail)?,
                )
            }
        };
        let term = match factor {
            Some(factor) if magnitude.is_one() => factor,
            Some(factor) if magnitude == Integer::one().negated() => pool
                .apply(Head::Operator(Operator::Neg), &[factor])
                .map_err(fail)?,
            Some(factor) => {
                let scale = pool.number(magnitude.into()).map_err(fail)?;
                pool.apply(Head::Operator(Operator::Mul), &[scale, factor])
                    .map_err(fail)?
            }
            None => pool.number(magnitude.into()).map_err(fail)?,
        };
        total = Some(match total {
            None => term,
            Some(running) => pool
                .apply(
                    Head::Operator(if is_negative {
                        Operator::Sub
                    } else {
                        Operator::Add
                    }),
                    &[running, term],
                )
                .map_err(fail)?,
        });
    }
    total.ok_or(SolveRefusal::NotAPolynomial)
}

pub fn factored_polynomial(
    pool: &mut ExprPool,
    expression: ExprId,
    unknown: SymbolId,
) -> Result<ExprId, SolveRefusal> {
    let coefficients: Vec<ExactRational> = coefficients(pool, expression, unknown)?
        .iter()
        .map(SquareRootSum::as_rational)
        .collect::<Option<_>>()
        .ok_or(SolveRefusal::NotAPolynomial)?;
    let factoring = crate::polynomial_factoring::factor_over_rationals(&coefficients)
        .ok_or(SolveRefusal::NotAPolynomial)?;
    let variable = pool
        .symbol(unknown)
        .map_err(|_| SolveRefusal::NotAPolynomial)?;
    let fail = |_| SolveRefusal::NotAPolynomial;
    let constant = &factoring.constant;
    let mut product: Option<ExprId> = if constant.is_one() && !factoring.factors.is_empty() {
        None
    } else {
        Some(written_rational(pool, constant).ok_or(SolveRefusal::NotAPolynomial)?)
    };
    for (factor, multiplicity) in &factoring.factors {
        let factor: Vec<ExactRational> = factor
            .iter()
            .map(|coefficient| ExactRational::from_integer(coefficient.clone()))
            .collect();
        let mut term = polynomial_in(pool, &factor, variable)?;
        if *multiplicity > 1 {
            let power = pool
                .number(Integer::from(i64::from(*multiplicity)).into())
                .map_err(fail)?;
            term = pool
                .apply(Head::Operator(Operator::Pow), &[term, power])
                .map_err(fail)?;
        }
        product = Some(match product {
            Some(running) => pool
                .apply(Head::Operator(Operator::Mul), &[running, term])
                .map_err(fail)?,
            None => term,
        });
    }
    product.ok_or(SolveRefusal::NotAPolynomial)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntervalEnd {
    pub value: ExprId,
    pub included: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interval {
    pub from: Option<IntervalEnd>,
    pub to: Option<IntervalEnd>,
}

fn wanted_signs(operator: Operator) -> Option<(std::cmp::Ordering, bool)> {
    use std::cmp::Ordering;
    match operator {
        Operator::Greater => Some((Ordering::Greater, false)),
        Operator::GreaterOrEqual => Some((Ordering::Greater, true)),
        Operator::Less => Some((Ordering::Less, false)),
        Operator::LessOrEqual => Some((Ordering::Less, true)),
        _ => None,
    }
}

pub fn is_inequality(pool: &ExprPool, expression: ExprId) -> bool {
    matches!(
        pool.node(expression),
        Ok(NodeView::Apply {
            head: Head::Operator(operator),
            arguments: [_, _],
        }) if wanted_signs(operator).is_some()
    )
}

pub fn inequality_solutions(
    pool: &mut ExprPool,
    relation: ExprId,
    unknown: SymbolId,
) -> Result<Vec<Interval>, SolveRefusal> {
    use std::cmp::Ordering;
    let (operator, left, right) = match pool
        .node(relation)
        .map_err(|_| SolveRefusal::NotAPolynomial)?
    {
        NodeView::Apply {
            head: Head::Operator(operator),
            arguments: [left, right],
        } => (operator, *left, *right),
        _ => return Err(SolveRefusal::NotAPolynomial),
    };
    let (wanted, boundary_holds) = wanted_signs(operator).ok_or(SolveRefusal::NotAPolynomial)?;
    let difference = pool
        .apply(Head::Operator(Operator::Sub), &[left, right])
        .map_err(|_| SolveRefusal::NotAPolynomial)?;
    let coefficients = coefficients(pool, difference, unknown)?;
    let degree = coefficients.len() - 1;
    let coefficients: Vec<ExactRational> = coefficients
        .iter()
        .map(SquareRootSum::as_rational)
        .collect::<Option<_>>()
        .ok_or(SolveRefusal::RadicalCoefficients(degree))?;
    let everything = vec![Interval {
        from: None,
        to: None,
    }];
    let leading_sign = coefficients
        .last()
        .map_or(Ordering::Equal, ExactRational::sign);
    if degree == 0 {
        let holds = leading_sign == wanted || (leading_sign == Ordering::Equal && boundary_holds);
        return Ok(if holds { everything } else { Vec::new() });
    }
    let name = pool
        .symbol_name(unknown)
        .map_err(|_| SolveRefusal::NoName)?
        .to_owned();
    let factoring = crate::polynomial_factoring::factor_over_rationals(&coefficients)
        .ok_or(SolveRefusal::NotAPolynomial)?;
    let mut found: Vec<(FoundRoot, u32)> = Vec::new();
    for (factor, multiplicity) in &factoring.factors {
        let factor: Vec<ExactRational> = factor
            .iter()
            .map(|coefficient| ExactRational::from_integer(coefficient.clone()))
            .collect();
        let mut roots = Vec::new();
        roots_of_factor(pool, &factor, &name, &mut roots)?;
        found.extend(roots.into_iter().map(|root| (root, *multiplicity)));
    }
    for position in 1..found.len() {
        let mut current = position;
        while current > 0 {
            let (before, after) = found.split_at_mut(current);
            if ordered(&mut before[current - 1].0, &mut after[0].0) != Ordering::Greater {
                break;
            }
            found.swap(current - 1, current);
            current -= 1;
        }
    }
    let mut ends = Vec::with_capacity(found.len());
    let mut flips = Vec::with_capacity(found.len());
    for (root, multiplicity) in found {
        ends.push(match root {
            FoundRoot::Exact(value) => square_root_sum_expression(pool, &value)
                .map_err(|_| SolveRefusal::NotAPolynomial)?,
            FoundRoot::Algebraic(_, binder) => binder,
        });
        flips.push(multiplicity % 2 == 1);
    }
    let count = ends.len();
    let mut signs = vec![leading_sign; count + 1];
    for index in (0..count).rev() {
        signs[index] = if flips[index] {
            signs[index + 1].reverse()
        } else {
            signs[index + 1]
        };
    }
    let mut intervals = Vec::new();
    let mut open: Option<Option<IntervalEnd>> = None;
    for index in 0..=count {
        let segment_holds = signs[index] == wanted;
        match (&open, segment_holds) {
            (None, true) => {
                open = Some(if index == 0 {
                    None
                } else if boundary_holds {
                    Some(IntervalEnd {
                        value: ends[index - 1],
                        included: true,
                    })
                } else {
                    Some(IntervalEnd {
                        value: ends[index - 1],
                        included: false,
                    })
                });
            }
            (Some(_), false) => {
                let from = open.take().flatten();
                intervals.push(Interval {
                    from,
                    to: Some(IntervalEnd {
                        value: ends[index - 1],
                        included: boundary_holds,
                    }),
                });
            }
            _ => {}
        }
        if index == count {
            break;
        }
        let point_holds = boundary_holds;
        let next_holds = signs[index + 1] == wanted;
        match (&open, point_holds, next_holds) {
            (Some(_), false, _) => {
                let from = open.take().flatten();
                intervals.push(Interval {
                    from,
                    to: Some(IntervalEnd {
                        value: ends[index],
                        included: false,
                    }),
                });
            }
            (None, true, false) => intervals.push(Interval {
                from: Some(IntervalEnd {
                    value: ends[index],
                    included: true,
                }),
                to: Some(IntervalEnd {
                    value: ends[index],
                    included: true,
                }),
            }),
            _ => {}
        }
    }
    if let Some(from) = open {
        intervals.push(Interval { from, to: None });
    }
    Ok(intervals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_expr::SymbolKind;
    use calc_syntax::{PrintMode, parse_expression, print_expression};

    fn solved(text: &str) -> Result<Vec<String>, SolveRefusal> {
        let mut pool = ExprPool::new();
        let expression = parse_expression(&mut pool, text).expect("the input parses");
        let unknown = pool
            .intern_symbol("x", SymbolKind::Variable)
            .expect("the name interns");
        let roots = solutions(&mut pool, expression, unknown)?;
        Ok(roots
            .iter()
            .map(|root| print_expression(&pool, *root, PrintMode::Ascii).expect("it prints"))
            .collect())
    }

    fn solved_with_constants(text: &str) -> Result<Vec<String>, SolveRefusal> {
        let mut pool = ExprPool::new();
        let expression = parse_expression(&mut pool, text).expect("the input parses");
        let unknown = pool
            .intern_symbol("x", SymbolKind::Variable)
            .expect("the name interns");
        let roots = constant_coefficient_solutions(&mut pool, expression, unknown)?;
        Ok(roots
            .iter()
            .map(|root| print_expression(&pool, *root, PrintMode::Ascii).expect("it prints"))
            .collect())
    }

    #[test]
    fn a_linear_equation_over_pi_is_solved() {
        assert_eq!(
            solved_with_constants("pi*x = 1"),
            Ok(vec!["1 / pi".to_owned()])
        );
    }

    #[test]
    fn a_common_factor_in_pi_is_cancelled_from_the_solution() {
        assert_eq!(
            solved_with_constants("(pi - 1)*x = pi^2 - 1"),
            Ok(vec!["pi + 1".to_owned()])
        );
    }

    #[test]
    fn a_square_with_a_constant_coefficient_is_refused_for_its_degree() {
        assert_eq!(
            solved_with_constants("x^2 = pi"),
            Err(SolveRefusal::ConstantCoefficientDegree(2))
        );
    }

    #[test]
    fn a_divisor_whose_enclosure_holds_zero_is_refused_as_unproven() {
        assert_eq!(
            solved_with_constants("(pi - e - 0.4233108251307480031)*x = 1"),
            Err(SolveRefusal::UnprovenCoefficient)
        );
    }

    #[test]
    fn a_constant_written_as_a_divisor_is_solved() {
        assert_eq!(
            solved_with_constants("x/pi = 2"),
            Ok(vec!["2 * pi".to_owned()])
        );
    }

    #[test]
    fn a_constant_that_cancels_leaves_every_number() {
        assert_eq!(
            solved_with_constants("0*x = pi - pi"),
            Err(SolveRefusal::EveryNumber)
        );
    }

    #[test]
    fn a_nonzero_constant_with_no_unknown_has_no_solution() {
        assert_eq!(solved_with_constants("0*x = pi"), Ok(Vec::new()));
    }

    #[test]
    fn an_atom_other_than_pi_or_e_is_named_in_the_refusal() {
        assert!(matches!(
            solved_with_constants("sin(1)*x = pi"),
            Err(SolveRefusal::CoefficientOutsideTheField(_))
        ));
    }

    #[test]
    fn an_atom_that_holds_the_unknown_is_not_a_polynomial() {
        assert_eq!(
            solved_with_constants("x*sin(x) = pi"),
            Err(SolveRefusal::NotAPolynomial)
        );
    }

    #[test]
    fn a_division_by_a_constant_zero_is_not_a_polynomial() {
        assert_eq!(
            solved_with_constants("x/(pi - pi) = 1"),
            Err(SolveRefusal::NotAPolynomial)
        );
    }

    #[test]
    fn a_rational_root_whose_coefficients_pass_one_hundred_thousand_is_found() {
        assert_eq!(
            solved("(1000003*x - 999983)*(x^2 + 1) = 0").as_deref(),
            Ok(["q'999983/1000003'".to_string()].as_slice())
        );
    }

    #[test]
    fn a_repeated_rational_root_is_divided_out_as_often_as_it_occurs() {
        assert_eq!(
            solved("(x - 100003)^2*(x + 1)*(x^3 - 2) = 0").as_deref(),
            Ok([
                "-1".to_string(),
                "rootof(x^3 - 2, x, 1)".to_string(),
                "100003".to_string()
            ]
            .as_slice())
        );
    }

    #[test]
    fn a_linear_equation_has_one_solution() {
        assert_eq!(
            solved("2*x + 6 = 0").as_deref(),
            Ok(["-3".to_string()].as_slice())
        );
    }

    #[test]
    fn a_square_equation_has_two_solutions_in_order() {
        assert_eq!(
            solved("x^2 = 4").as_deref(),
            Ok(["-2".to_string(), "2".to_string()].as_slice())
        );
    }

    #[test]
    fn a_factorable_quadratic_gives_both_roots() {
        assert_eq!(
            solved("x^2 - 5*x + 6 = 0").as_deref(),
            Ok(["2".to_string(), "3".to_string()].as_slice())
        );
    }

    #[test]
    fn a_double_root_is_named_once() {
        assert_eq!(
            solved("x^2 - 2*x + 1 = 0").as_deref(),
            Ok(["1".to_string()].as_slice())
        );
    }

    #[test]
    fn a_quadratic_with_no_real_root_has_no_solution() {
        assert_eq!(solved("x^2 + 1 = 0").as_deref(), Ok([].as_slice()));
    }

    #[test]
    fn a_fractional_solution_stays_a_fraction() {
        assert_eq!(
            solved("3*x - 1 = 0").as_deref(),
            Ok(["q'1/3'".to_string()].as_slice())
        );
    }

    #[test]
    fn a_cubic_with_rational_roots_is_solved_completely() {
        assert_eq!(
            solved("x^3 - 6*x^2 + 11*x - 6 = 0").as_deref(),
            Ok(["1".to_string(), "2".to_string(), "3".to_string()].as_slice())
        );
    }

    #[test]
    fn a_cubic_with_one_real_root_answers_it_as_rootof() {
        assert_eq!(
            solved("x^3 - 2 = 0").as_deref(),
            Ok(["rootof(x^3 - 2, x, 1)".to_string()].as_slice())
        );
    }

    #[test]
    fn a_rational_root_and_a_quadratic_rest_are_answered_in_order() {
        assert_eq!(
            solved("x^3 - 2*x = 0").as_deref(),
            Ok([
                "-sqrt(2)".to_string(),
                "0".to_string(),
                "sqrt(2)".to_string()
            ]
            .as_slice())
        );
    }

    #[test]
    fn a_rational_root_is_placed_among_the_algebraic_ones() {
        assert_eq!(
            solved("(x - 1)^2 * (x^3 - 2) = 0").as_deref(),
            Ok(["1".to_string(), "rootof(x^3 - 2, x, 1)".to_string()].as_slice())
        );
    }

    #[test]
    fn a_quartic_with_no_real_root_has_no_solution() {
        assert_eq!(solved("x^4 + 1 = 0").as_deref(), Ok([].as_slice()));
    }

    #[test]
    fn an_equation_that_holds_for_every_number_says_so() {
        assert_eq!(solved("x + 1 = 1 + x"), Err(SolveRefusal::EveryNumber));
    }

    #[test]
    fn an_equation_that_holds_for_no_number_has_no_solution() {
        assert_eq!(solved("x + 1 = x + 2").as_deref(), Ok([].as_slice()));
    }

    #[test]
    fn a_linear_equation_with_a_radical_coefficient_is_solved() {
        assert_eq!(
            solved("sqrt(2)*x = 1").as_deref(),
            Ok(["sqrt(2) / 2".to_string()].as_slice())
        );
    }

    #[test]
    fn a_quadratic_with_radical_coefficients_and_a_rational_discriminant_is_solved() {
        assert_eq!(
            solved("x^2 - 2*sqrt(2)*x + 1 = 0").as_deref(),
            Ok(["-1 + sqrt(2)".to_string(), "1 + sqrt(2)".to_string()].as_slice())
        );
    }

    #[test]
    fn a_quadratic_whose_discriminant_holds_a_radical_is_refused_by_its_degree() {
        assert_eq!(
            solved("x^2 = sqrt(2)"),
            Err(SolveRefusal::RadicalCoefficients(2))
        );
    }

    #[test]
    fn a_cubic_with_a_radical_coefficient_is_refused_by_its_degree() {
        assert_eq!(
            solved("sqrt(2)*x^3 = 1"),
            Err(SolveRefusal::RadicalCoefficients(3))
        );
    }

    fn intervals(text: &str) -> Vec<String> {
        let mut pool = ExprPool::new();
        let expression = parse_expression(&mut pool, text).expect("the input parses");
        let unknown = pool
            .intern_symbol("x", SymbolKind::Variable)
            .expect("the name interns");
        let found = inequality_solutions(&mut pool, expression, unknown).expect("it solves");
        let end = |pool: &ExprPool, end: &Option<IntervalEnd>| match end {
            None => "inf".to_string(),
            Some(end) => format!(
                "{}{}",
                print_expression(pool, end.value, PrintMode::Ascii).expect("it prints"),
                if end.included { "]" } else { ")" }
            ),
        };
        found
            .iter()
            .map(|interval| {
                format!(
                    "{} .. {}",
                    end(&pool, &interval.from),
                    end(&pool, &interval.to)
                )
            })
            .collect()
    }

    #[test]
    fn a_quadratic_is_positive_outside_its_roots() {
        assert_eq!(
            intervals("x^2 - 2 > 0"),
            vec![
                "inf .. -sqrt(2))".to_string(),
                "sqrt(2)) .. inf".to_string()
            ]
        );
    }

    #[test]
    fn a_closed_inequality_includes_its_roots() {
        assert_eq!(
            intervals("x^2 - 5*x + 6 <= 0"),
            vec!["2] .. 3]".to_string()]
        );
    }

    #[test]
    fn a_double_root_does_not_change_the_sign() {
        assert_eq!(
            intervals("(x - 1)^2 > 0"),
            vec!["inf .. 1)".to_string(), "1) .. inf".to_string()]
        );
    }

    #[test]
    fn a_double_root_alone_satisfies_a_closed_inequality_at_one_point() {
        assert_eq!(intervals("(x - 1)^2 <= 0"), vec!["1] .. 1]".to_string()]);
    }

    #[test]
    fn a_polynomial_without_real_roots_holds_everywhere_or_nowhere() {
        assert_eq!(intervals("x^2 + 1 > 0"), vec!["inf .. inf".to_string()]);
        assert!(intervals("x^2 + 1 < 0").is_empty());
    }

    #[test]
    fn a_double_root_inside_a_closed_region_joins_it() {
        assert_eq!(
            intervals("x^2 * (x - 2) <= 0"),
            vec!["inf .. 2]".to_string()]
        );
    }
}
