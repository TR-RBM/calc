use calc_expr::{BinderKind, ExprId, ExprPool, Head, NodeView, Operator, SymbolKind};

use crate::derivative::depends_on;
use crate::exact_evaluation::closed_square_root_sum;
use crate::exact_rational::ExactRational;
use crate::polynomial::{AtomTable, Polynomial, polynomial_expression, polynomial_of, rational_of};
use crate::square_root_sum::SquareRootSum;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegralRefusal {
    NotAPolynomial,
    PoleInInterval,
    HighDegreeFactor(u32),
    RepeatedQuadratic,
    BoundNotExact,
}

fn polynomial_antiderivative(pool: &mut ExprPool, body: ExprId) -> Result<ExprId, IntegralRefusal> {
    let mut atoms = AtomTable::default();
    let polynomial =
        polynomial_of(pool, body, &mut atoms).ok_or(IntegralRefusal::NotAPolynomial)?;
    let variable = pool.bound(0).map_err(|_| IntegralRefusal::NotAPolynomial)?;
    let mut index = None;
    for (position, atom) in atoms.expressions().iter().enumerate() {
        if *atom == variable {
            index = Some(position);
            continue;
        }
        if depends_on(pool, *atom, 0) {
            return Err(IntegralRefusal::NotAPolynomial);
        }
    }
    let index = match index {
        Some(index) => index,
        None => atoms.index_of(variable),
    };
    let raised = polynomial
        .raise_in(index)
        .ok_or(IntegralRefusal::NotAPolynomial)?;
    polynomial_expression(pool, &raised, &atoms).ok_or(IntegralRefusal::NotAPolynomial)
}

fn antiderivative(
    pool: &mut ExprPool,
    body: ExprId,
) -> Result<(ExprId, Vec<crate::real_roots::RealRoot>), IntegralRefusal> {
    match polynomial_antiderivative(pool, body) {
        Ok(raised) => Ok((raised, Vec::new())),
        Err(refusal) => match rational_antiderivative(pool, body) {
            Ok(integrand) => Ok((integrand.antiderivative, integrand.denominator_roots)),
            Err(IntegralRefusal::NotAPolynomial) => Err(refusal),
            Err(other) => Err(other),
        },
    }
}

pub fn definite_integral(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Option<Result<ExprId, IntegralRefusal>> {
    let NodeView::Bind {
        binder: BinderKind::Integral,
        arguments,
        body,
    } = pool.node(expression).ok()?
    else {
        return None;
    };
    let bounds = arguments.to_vec();
    let (raised, mut poles) = match antiderivative(pool, body) {
        Ok(found) => found,
        Err(refusal) => return Some(Err(refusal)),
    };
    let [lower, upper] = bounds[..] else {
        let name = pool.bound_name(expression).unwrap_or("x").to_owned();
        let symbol = pool.intern_symbol(&name, SymbolKind::Variable).ok()?;
        let variable = pool.symbol(symbol).ok()?;
        return Some(
            crate::rule_search::replace_parameters(pool, raised, &[variable], 0)
                .ok_or(IntegralRefusal::NotAPolynomial),
        );
    };
    if let Err(refusal) = pole_between(pool, &mut poles, lower, upper) {
        return Some(Err(refusal));
    }
    let at_upper = crate::rule_search::replace_parameters(pool, raised, &[upper], 0)?;
    let at_lower = crate::rule_search::replace_parameters(pool, raised, &[lower], 0)?;
    Some(
        pool.apply(Head::Operator(Operator::Sub), &[at_upper, at_lower])
            .map_err(|_| IntegralRefusal::NotAPolynomial),
    )
}

fn univariate(polynomial: &Polynomial) -> Result<Vec<ExactRational>, IntegralRefusal> {
    let mut coefficients: Vec<ExactRational> = Vec::new();
    for (monomial, coefficient) in polynomial.terms() {
        let degree = match monomial.factors() {
            [] => 0,
            [(0, exponent)] => {
                usize::try_from(*exponent).map_err(|_| IntegralRefusal::NotAPolynomial)?
            }
            _ => return Err(IntegralRefusal::NotAPolynomial),
        };
        let coefficient = coefficient
            .as_rational()
            .ok_or(IntegralRefusal::NotAPolynomial)?;
        if coefficients.len() <= degree {
            coefficients.resize(degree + 1, ExactRational::zero());
        }
        coefficients[degree] = coefficient;
    }
    Ok(coefficients)
}

fn rational_product(left: &[ExactRational], right: &[ExactRational]) -> Vec<ExactRational> {
    if left.is_empty() || right.is_empty() {
        return Vec::new();
    }
    let mut result = vec![ExactRational::zero(); left.len() + right.len() - 1];
    for (i, a) in left.iter().enumerate() {
        for (j, b) in right.iter().enumerate() {
            result[i + j] = result[i + j].plus(&a.multiply(b));
        }
    }
    result
}

fn solved_square(mut matrix: Vec<Vec<ExactRational>>) -> Option<Vec<ExactRational>> {
    let size = matrix.len();
    for column in 0..size {
        let found = (column..size).find(|row| !matrix[*row][column].is_zero())?;
        matrix.swap(column, found);
        let pivot = matrix[column][column].clone();
        for entry in matrix[column].iter_mut() {
            *entry = entry.divide(&pivot)?;
        }
        let pivot_row = matrix[column].clone();
        for (other, row) in matrix.iter_mut().enumerate() {
            if other == column || row[column].is_zero() {
                continue;
            }
            let factor = row[column].clone();
            for (entry, pivot_entry) in row.iter_mut().zip(&pivot_row) {
                *entry = entry.subtract(&pivot_entry.multiply(&factor));
            }
        }
    }
    Some(matrix.into_iter().map(|row| row[size].clone()).collect())
}

struct Builder<'pool> {
    pool: &'pool mut ExprPool,
    atoms: &'pool AtomTable,
}

impl Builder<'_> {
    fn apply(&mut self, operator: Operator, arguments: &[ExprId]) -> Option<ExprId> {
        self.pool.apply(Head::Operator(operator), arguments).ok()
    }

    fn rational(&mut self, value: &ExactRational) -> Option<ExprId> {
        crate::solve::written_rational(self.pool, value)
    }

    fn polynomial(&mut self, coefficients: &[ExactRational]) -> Option<ExprId> {
        let sums: Vec<SquareRootSum> = coefficients
            .iter()
            .map(|coefficient| SquareRootSum::from_rational(coefficient.clone()))
            .collect();
        let polynomial = crate::polynomial::univariate_polynomial(&sums, 0)?;
        polynomial_expression(self.pool, &polynomial, self.atoms)
    }

    fn scaled(&mut self, scale: &ExactRational, term: ExprId) -> Option<ExprId> {
        if scale.is_one() {
            return Some(term);
        }
        if scale.negated().is_one() {
            return self.apply(Operator::Neg, &[term]);
        }
        let scale = self.rational(scale)?;
        self.apply(Operator::Mul, &[scale, term])
    }

    fn logarithm_of_absolute(&mut self, argument: ExprId, keep_sign: bool) -> Option<ExprId> {
        let inner = if keep_sign {
            argument
        } else {
            self.apply(Operator::Abs, &[argument])?
        };
        self.apply(Operator::Ln, &[inner])
    }

    fn sum_polynomial(&mut self, coefficients: &[SquareRootSum]) -> Option<ExprId> {
        let polynomial = crate::polynomial::univariate_polynomial(coefficients, 0)?;
        polynomial_expression(self.pool, &polynomial, self.atoms)
    }

    fn sum_scaled(&mut self, scale: &SquareRootSum, term: ExprId) -> Option<ExprId> {
        if let Some(rational) = scale.as_rational() {
            return self.scaled(&rational, term);
        }
        let scale = crate::exact_evaluation::square_root_sum_expression(self.pool, scale).ok()?;
        self.apply(Operator::Mul, &[scale, term])
    }
}

fn term_of_linear(
    builder: &mut Builder<'_>,
    factor: &[ExactRational],
    power: u32,
    numerator: &ExactRational,
) -> Option<ExprId> {
    let written = builder.polynomial(factor)?;
    let scale = numerator.divide(&factor[1])?;
    if power == 1 {
        let logarithm = builder.logarithm_of_absolute(written, false)?;
        return builder.scaled(&scale, logarithm);
    }
    let lowered = ExactRational::from_i64(i64::from(power) - 1);
    let exponent = builder.rational(&lowered)?;
    let raised = if power == 2 {
        written
    } else {
        builder.apply(Operator::Pow, &[written, exponent])?
    };
    let above = builder.rational(&scale.divide(&lowered)?.negated())?;
    builder.apply(Operator::Div, &[above, raised])
}

fn term_of_quadratic(
    builder: &mut Builder<'_>,
    factor: &[ExactRational],
    linear: &ExactRational,
    constant: &ExactRational,
) -> Option<ExprId> {
    let (c, b, a) = (&factor[0], &factor[1], &factor[2]);
    let two = ExactRational::from_i64(2);
    let four = ExactRational::from_i64(4);
    let discriminant = b.multiply(b).subtract(&four.multiply(a).multiply(c));
    let written = builder.polynomial(factor)?;
    let mut terms = Vec::new();
    if !linear.is_zero() {
        let always_positive = discriminant.sign() == std::cmp::Ordering::Less
            && a.sign() == std::cmp::Ordering::Greater;
        let logarithm = builder.logarithm_of_absolute(written, always_positive)?;
        terms.push(builder.scaled(&linear.divide(&two.multiply(a))?, logarithm)?);
    }
    let rest = constant.subtract(&linear.multiply(b).divide(&two.multiply(a))?);
    if !rest.is_zero() {
        let sum = |value: &ExactRational| SquareRootSum::from_rational(value.clone());
        let negative = discriminant.sign() == std::cmp::Ordering::Less;
        let root = SquareRootSum::square_root_of_rational(&if negative {
            discriminant.negated()
        } else {
            discriminant.clone()
        })?;
        if negative {
            let argument = builder.sum_polynomial(&[
                sum(b).divided_by(&root)?,
                sum(&two.multiply(a)).divided_by(&root)?,
            ])?;
            let angle = builder.apply(Operator::Atan, &[argument])?;
            let scale = sum(&two.multiply(&rest)).divided_by(&root)?;
            terms.push(builder.sum_scaled(&scale, angle)?);
        } else {
            let twice = sum(&two.multiply(a));
            let below = builder.sum_polynomial(&[
                sum(b).minus(&root).divided_by(&twice)?,
                SquareRootSum::from_rational(ExactRational::one()),
            ])?;
            let above = builder.sum_polynomial(&[
                sum(b).plus(&root).divided_by(&twice)?,
                SquareRootSum::from_rational(ExactRational::one()),
            ])?;
            let quotient = builder.apply(Operator::Div, &[below, above])?;
            let logarithm = builder.logarithm_of_absolute(quotient, false)?;
            let scale = sum(&rest).divided_by(&root)?;
            terms.push(builder.sum_scaled(&scale, logarithm)?);
        }
    }
    let mut total: Option<ExprId> = None;
    for term in terms {
        total = Some(match total {
            Some(running) => builder.apply(Operator::Add, &[running, term])?,
            None => term,
        });
    }
    total
}

struct RationalIntegrand {
    denominator_roots: Vec<crate::real_roots::RealRoot>,
    antiderivative: ExprId,
}

fn rational_antiderivative(
    pool: &mut ExprPool,
    body: ExprId,
) -> Result<RationalIntegrand, IntegralRefusal> {
    let variable = pool.bound(0).map_err(|_| IntegralRefusal::NotAPolynomial)?;
    let mut atoms = AtomTable::default();
    atoms.index_of(variable);
    let function = rational_of(pool, body, &mut atoms).ok_or(IntegralRefusal::NotAPolynomial)?;
    if atoms.expressions().len() != 1 {
        return Err(IntegralRefusal::NotAPolynomial);
    }
    let numerator = univariate(function.numerator())?;
    let denominator = univariate(function.denominator())?;
    let (whole, rest) = crate::real_roots::divided(&numerator, &denominator)
        .ok_or(IntegralRefusal::NotAPolynomial)?;
    let factoring = crate::polynomial_factoring::factor_over_rationals(&denominator)
        .ok_or(IntegralRefusal::NotAPolynomial)?;
    let factors: Vec<(Vec<ExactRational>, u32)> = factoring
        .factors
        .iter()
        .map(|(factor, multiplicity)| {
            (
                factor
                    .iter()
                    .map(|value| ExactRational::from_integer(value.clone()))
                    .collect(),
                *multiplicity,
            )
        })
        .collect();
    for (factor, multiplicity) in &factors {
        let degree = factor.len() - 1;
        if degree > 2 {
            return Err(IntegralRefusal::HighDegreeFactor(
                u32::try_from(degree).unwrap_or(u32::MAX),
            ));
        }
        if degree == 2 && *multiplicity > 1 {
            return Err(IntegralRefusal::RepeatedQuadratic);
        }
    }
    let mut product = vec![ExactRational::one()];
    for (factor, multiplicity) in &factors {
        for _ in 0..*multiplicity {
            product = rational_product(&product, factor);
        }
    }
    let size = product.len() - 1;
    let scale = factoring.constant.clone();
    let target: Vec<ExactRational> = (0..size)
        .map(|position| {
            rest.get(position)
                .cloned()
                .unwrap_or_else(ExactRational::zero)
                .divide(&scale)
                .unwrap_or_else(ExactRational::zero)
        })
        .collect();
    let mut columns: Vec<(usize, u32, usize)> = Vec::new();
    let mut matrix: Vec<Vec<ExactRational>> = vec![Vec::new(); size];
    for (index, (factor, multiplicity)) in factors.iter().enumerate() {
        let mut power = vec![ExactRational::one()];
        for exponent in 1..=*multiplicity {
            power = rational_product(&power, factor);
            let (cofactor, _) = crate::real_roots::divided(&product, &power)
                .ok_or(IntegralRefusal::NotAPolynomial)?;
            for shift in 0..factor.len() - 1 {
                let mut column = vec![ExactRational::zero(); shift];
                column.extend(cofactor.iter().cloned());
                for (row, line) in matrix.iter_mut().enumerate() {
                    line.push(column.get(row).cloned().unwrap_or_else(ExactRational::zero));
                }
                columns.push((index, exponent, shift));
            }
        }
    }
    for (line, value) in matrix.iter_mut().zip(target) {
        line.push(value);
    }
    let solution = if size == 0 {
        Vec::new()
    } else {
        solved_square(matrix).ok_or(IntegralRefusal::NotAPolynomial)?
    };
    let mut builder = Builder {
        pool,
        atoms: &atoms,
    };
    let mut terms = Vec::new();
    if !whole.is_empty() {
        let mut raised = vec![ExactRational::zero()];
        for (power, coefficient) in whole.iter().enumerate() {
            let divisor = ExactRational::from_i64(i64::try_from(power + 1).unwrap_or(i64::MAX));
            raised.push(
                coefficient
                    .divide(&divisor)
                    .ok_or(IntegralRefusal::NotAPolynomial)?,
            );
        }
        terms.push(
            builder
                .polynomial(&raised)
                .ok_or(IntegralRefusal::NotAPolynomial)?,
        );
    }
    for (index, (factor, multiplicity)) in factors.iter().enumerate() {
        for exponent in 1..=*multiplicity {
            let parts: Vec<ExactRational> = columns
                .iter()
                .zip(&solution)
                .filter(|((column_factor, column_exponent, _), _)| {
                    *column_factor == index && *column_exponent == exponent
                })
                .map(|(_, value)| value.clone())
                .collect();
            if parts.iter().all(ExactRational::is_zero) {
                continue;
            }
            let term = if factor.len() == 2 {
                term_of_linear(&mut builder, factor, exponent, &parts[0])
            } else {
                term_of_quadratic(&mut builder, factor, &parts[1], &parts[0])
            };
            terms.push(term.ok_or(IntegralRefusal::NotAPolynomial)?);
        }
    }
    let mut total: Option<ExprId> = None;
    for term in terms {
        total = Some(match total {
            Some(running) => builder
                .apply(Operator::Add, &[running, term])
                .ok_or(IntegralRefusal::NotAPolynomial)?,
            None => term,
        });
    }
    let antiderivative = match total {
        Some(total) => total,
        None => builder
            .rational(&ExactRational::zero())
            .ok_or(IntegralRefusal::NotAPolynomial)?,
    };
    let denominator_roots =
        crate::real_roots::real_roots(&product).ok_or(IntegralRefusal::NotAPolynomial)?;
    Ok(RationalIntegrand {
        denominator_roots,
        antiderivative,
    })
}

fn pole_between(
    pool: &mut ExprPool,
    roots: &mut [crate::real_roots::RealRoot],
    lower: ExprId,
    upper: ExprId,
) -> Result<(), IntegralRefusal> {
    if roots.is_empty() {
        return Ok(());
    }
    let lower = closed_square_root_sum(pool, lower).ok_or(IntegralRefusal::BoundNotExact)?;
    let upper = closed_square_root_sum(pool, upper).ok_or(IntegralRefusal::BoundNotExact)?;
    let (low, high) = if lower.compare(&upper) == std::cmp::Ordering::Greater {
        (upper, lower)
    } else {
        (lower, upper)
    };
    for root in roots.iter_mut() {
        if root.compare_with_sum(&low) != std::cmp::Ordering::Less
            && root.compare_with_sum(&high) != std::cmp::Ordering::Greater
        {
            return Err(IntegralRefusal::PoleInInterval);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exact_evaluation::evaluate_exact;
    use calc_syntax::parse_expression;

    fn integrated(text: &str) -> Result<String, IntegralRefusal> {
        let mut pool = ExprPool::new();
        let expression = parse_expression(&mut pool, text).expect("the input parses");
        let value = definite_integral(&mut pool, expression).expect("it is an integral")?;
        let evaluation = evaluate_exact(&mut pool, value).expect("the value evaluates");
        Ok(evaluation
            .rational_value()
            .map(|number| format!("{number:?}"))
            .unwrap_or_default())
    }

    fn number(text: &str) -> Option<crate::exact_rational::ExactRational> {
        let mut pool = ExprPool::new();
        let expression = parse_expression(&mut pool, text).ok()?;
        let value = definite_integral(&mut pool, expression)?.ok()?;
        let evaluation = evaluate_exact(&mut pool, value).ok()?;
        evaluation
            .rational_value()
            .and_then(crate::exact_rational::ExactRational::from_number)
    }

    fn equals(text: &str, numerator: i64, denominator: i64) -> bool {
        let expected = crate::exact_rational::ExactRational::from_i64(numerator)
            .divide(&crate::exact_rational::ExactRational::from_i64(denominator))
            .expect("the expected value is a fraction");
        number(text) == Some(expected)
    }

    #[test]
    fn the_integral_of_a_square_over_the_unit_interval_is_a_third() {
        assert!(equals("integral(x^2, x, 0, 1)", 1, 3));
    }

    #[test]
    fn the_integral_of_a_constant_is_the_width_times_the_constant() {
        assert!(equals("integral(5, x, 2, 6)", 20, 1));
    }

    #[test]
    fn the_integral_of_the_variable_itself_is_half_the_square() {
        assert!(equals("integral(x, x, 0, 4)", 8, 1));
    }

    #[test]
    fn a_polynomial_integrates_term_by_term() {
        assert!(equals("integral(x^3 + 2*x, x, 0, 2)", 8, 1));
    }

    #[test]
    fn bounds_the_wrong_way_round_give_the_negative() {
        assert!(equals("integral(x^2, x, 1, 0)", -1, 3));
    }

    #[test]
    fn fractional_coefficients_stay_exact() {
        assert!(equals("integral(x/3, x, 0, 3)", 3, 2));
    }

    #[test]
    fn an_integrand_that_is_not_a_polynomial_is_refused() {
        assert_eq!(
            integrated("integral(sin(x), x, 0, 1)"),
            Err(IntegralRefusal::NotAPolynomial)
        );
    }

    fn antiderivative_text(text: &str) -> String {
        let mut pool = ExprPool::new();
        let expression = parse_expression(&mut pool, text).expect("the input parses");
        let value = definite_integral(&mut pool, expression)
            .expect("it is an integral")
            .expect("it integrates");
        calc_syntax::print_expression(&pool, value, calc_syntax::PrintMode::Ascii)
            .expect("it prints")
    }

    #[test]
    fn an_integral_without_bounds_is_an_antiderivative_in_its_variable() {
        assert_eq!(antiderivative_text("integral(x^2, x)"), "x^3 / 3");
    }

    #[test]
    fn a_reciprocal_integrates_to_a_logarithm() {
        assert_eq!(antiderivative_text("integral(1/x, x)"), "ln(abs(x))");
    }

    #[test]
    fn a_pole_between_the_bounds_is_refused() {
        assert_eq!(
            integrated("integral(1/x^2, x, -1, 1)"),
            Err(IntegralRefusal::PoleInInterval)
        );
    }

    #[test]
    fn a_cubic_factor_in_the_denominator_is_refused_with_its_degree() {
        assert_eq!(
            integrated("integral(1/(x^3 - 2), x, 0, 1)"),
            Err(IntegralRefusal::HighDegreeFactor(3))
        );
    }
}
