use std::cmp::Ordering;

use calc_expr::{BuiltinConstant, ExprId, ExprPool, Head, NodeView, Operator};
use calc_numbers::{Integer, Number};

use crate::exact_evaluation::{
    ExactEvaluationError, closed_square_root_sum, evaluate_exact, holds_the_imaginary_unit,
    square_root_sum_expression, sum_within_size_limit_of,
};
use crate::exact_rational::ExactRational;
use crate::square_root_sum::SquareRootSum;

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExactComplex {
    real: SquareRootSum,
    imaginary: SquareRootSum,
}

impl ExactComplex {
    fn from_real(real: SquareRootSum) -> Self {
        ExactComplex {
            real,
            imaginary: SquareRootSum::zero(),
        }
    }

    fn unit() -> Self {
        ExactComplex {
            real: SquareRootSum::zero(),
            imaginary: SquareRootSum::from_rational(ExactRational::one()),
        }
    }

    fn one() -> Self {
        ExactComplex::from_real(SquareRootSum::from_rational(ExactRational::one()))
    }

    fn is_zero(&self) -> bool {
        self.real.is_zero() && self.imaginary.is_zero()
    }

    fn plus(&self, other: &Self) -> Self {
        ExactComplex {
            real: self.real.plus(&other.real),
            imaginary: self.imaginary.plus(&other.imaginary),
        }
    }

    fn negated(&self) -> Self {
        ExactComplex {
            real: self.real.negated(),
            imaginary: self.imaginary.negated(),
        }
    }

    fn times(&self, other: &Self) -> Self {
        ExactComplex {
            real: self
                .real
                .times(&other.real)
                .minus(&self.imaginary.times(&other.imaginary)),
            imaginary: self
                .real
                .times(&other.imaginary)
                .plus(&self.imaginary.times(&other.real)),
        }
    }

    fn reciprocal(&self) -> Option<Self> {
        let norm = self
            .real
            .times(&self.real)
            .plus(&self.imaginary.times(&self.imaginary));
        Some(ExactComplex {
            real: self.real.divided_by(&norm)?,
            imaginary: self.imaginary.negated().divided_by(&norm)?,
        })
    }

    fn modulus(&self) -> Option<SquareRootSum> {
        let norm = self
            .real
            .times(&self.real)
            .plus(&self.imaginary.times(&self.imaginary));
        match norm.as_rational() {
            Some(rational) => SquareRootSum::square_root_of_rational(&rational),
            None => norm.denested_square_root(),
        }
    }

    fn within_size_limit(self, expression: ExprId) -> Result<Self, ExactEvaluationError> {
        sum_within_size_limit_of(expression, &self.real)?;
        sum_within_size_limit_of(expression, &self.imaginary)?;
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExactComplexValue {
    Real(ExprId),
    Rational { real: Number, imaginary: Number },
    Algebraic(ExprId),
}

fn whole_exponent(pool: &mut ExprPool, exponent: ExprId) -> Option<i128> {
    if holds_the_imaginary_unit(pool, exponent) {
        return None;
    }
    let rational = closed_square_root_sum(pool, exponent)?.as_rational()?;
    rational
        .denominator()
        .is_one()
        .then(|| rational.numerator().to_i128())
        .flatten()
}

fn powered(
    base: &ExactComplex,
    exponent: i128,
    expression: ExprId,
) -> Result<ExactComplex, ExactEvaluationError> {
    let base = if exponent < 0 {
        base.reciprocal()
            .ok_or(ExactEvaluationError::DivisionByZero(expression))?
    } else {
        base.clone()
    };
    let mut remaining = exponent.unsigned_abs();
    let mut square = base;
    let mut result = ExactComplex::one();
    while remaining > 0 {
        if remaining & 1 == 1 {
            result = result.times(&square).within_size_limit(expression)?;
        }
        remaining >>= 1;
        if remaining > 0 {
            square = square.times(&square).within_size_limit(expression)?;
        }
    }
    Ok(result)
}

fn complex_of(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Option<Result<ExactComplex, ExactEvaluationError>> {
    if !holds_the_imaginary_unit(pool, expression) {
        let real = closed_square_root_sum(pool, expression)?;
        return Some(Ok(ExactComplex::from_real(real)));
    }
    if is_imaginary_unit(pool, expression) {
        return Some(Ok(ExactComplex::unit()));
    }
    let (head, arguments) = match pool.node(expression).ok()? {
        NodeView::Apply {
            head: Head::Operator(operator),
            arguments,
        } => (operator, arguments.to_vec()),
        _ => return None,
    };
    let result = match (head, arguments.as_slice()) {
        (Operator::Complex, [real, imaginary]) => {
            let (real, imaginary) = (*real, *imaginary);
            if holds_the_imaginary_unit(pool, real) || holds_the_imaginary_unit(pool, imaginary) {
                return None;
            }
            Ok(ExactComplex {
                real: closed_square_root_sum(pool, real)?,
                imaginary: closed_square_root_sum(pool, imaginary)?,
            })
        }
        (Operator::Add | Operator::Sub | Operator::Mul | Operator::Div, [left, right]) => {
            let (left, right) = (*left, *right);
            let left = match complex_of(pool, left)? {
                Ok(value) => value,
                Err(error) => return Some(Err(error)),
            };
            let right = match complex_of(pool, right)? {
                Ok(value) => value,
                Err(error) => return Some(Err(error)),
            };
            match head {
                Operator::Add => Ok(left.plus(&right)),
                Operator::Sub => Ok(left.plus(&right.negated())),
                Operator::Mul => Ok(left.times(&right)),
                _ if right.is_zero() => Err(ExactEvaluationError::DivisionByZero(expression)),
                _ => right
                    .reciprocal()
                    .map(|inverse| left.times(&inverse))
                    .ok_or(ExactEvaluationError::DivisionByZero(expression)),
            }
        }
        (Operator::Neg, [argument]) => {
            let argument = *argument;
            complex_of(pool, argument)?.map(|value| value.negated())
        }
        (
            Operator::RealPart | Operator::ImaginaryPart | Operator::Conjugate | Operator::Abs,
            [argument],
        ) => {
            let argument = *argument;
            match complex_of(pool, argument)? {
                Ok(value) => Ok(match head {
                    Operator::RealPart => ExactComplex::from_real(value.real),
                    Operator::ImaginaryPart => ExactComplex::from_real(value.imaginary),
                    Operator::Conjugate => ExactComplex {
                        real: value.real,
                        imaginary: value.imaginary.negated(),
                    },
                    _ => ExactComplex::from_real(value.modulus()?),
                }),
                Err(error) => Err(error),
            }
        }
        (Operator::Pow, [base, exponent]) => {
            let (base, exponent) = (*base, *exponent);
            if let Some(power_of_zero) = power_of_zero(pool, base, exponent, expression) {
                return Some(power_of_zero);
            }
            let exponent = whole_exponent(pool, exponent)?;
            if let Some(squared) = squared_modulus(pool, base, exponent, expression) {
                return Some(squared);
            }
            match complex_of(pool, base)? {
                Ok(base) if base.is_zero() && exponent < 0 => {
                    Err(ExactEvaluationError::DivisionByZero(expression))
                }
                Ok(base) => powered(&base, exponent, expression),
                Err(error) => Err(error),
            }
        }
        _ => return None,
    };
    Some(result.and_then(|value| value.within_size_limit(expression)))
}

fn power_of_zero(
    pool: &mut ExprPool,
    base: ExprId,
    exponent: ExprId,
    expression: ExprId,
) -> Option<Result<ExactComplex, ExactEvaluationError>> {
    if !holds_the_imaginary_unit(pool, exponent) {
        return None;
    }
    let Ok(base) = complex_of(pool, base)? else {
        return None;
    };
    if !base.is_zero() {
        return None;
    }
    let exponent = match complex_of(pool, exponent)? {
        Ok(exponent) => exponent,
        Err(error) => return Some(Err(error)),
    };
    if exponent.imaginary.is_zero() {
        return None;
    }
    Some(match exponent.real.sign() {
        Ordering::Greater => Ok(ExactComplex::from_real(SquareRootSum::zero())),
        Ordering::Less => Err(ExactEvaluationError::DivisionByZero(expression)),
        Ordering::Equal => Err(ExactEvaluationError::PowerOfZeroWithoutValue { expression }),
    })
}

fn squared_modulus(
    pool: &mut ExprPool,
    base: ExprId,
    exponent: i128,
    expression: ExprId,
) -> Option<Result<ExactComplex, ExactEvaluationError>> {
    let Ok(NodeView::Apply {
        head: Head::Operator(Operator::Abs),
        arguments: [argument],
    }) = pool.node(base)
    else {
        return None;
    };
    if exponent % 2 != 0 {
        return None;
    }
    let argument = *argument;
    let value = match complex_of(pool, argument)? {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    let norm = ExactComplex::from_real(
        value
            .real
            .times(&value.real)
            .plus(&value.imaginary.times(&value.imaginary)),
    );
    if norm.is_zero() && exponent < 0 {
        return Some(Err(ExactEvaluationError::DivisionByZero(expression)));
    }
    Some(powered(&norm, exponent / 2, expression))
}

fn modulus_outside_the_field(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Option<Result<ExactComplexValue, ExactEvaluationError>> {
    let Ok(NodeView::Apply {
        head: Head::Operator(Operator::Abs),
        arguments: [argument],
    }) = pool.node(expression)
    else {
        return None;
    };
    let argument = *argument;
    let value = match complex_of(pool, argument)? {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    if value.modulus().is_some() {
        return None;
    }
    let norm = value
        .real
        .times(&value.real)
        .plus(&value.imaginary.times(&value.imaginary));
    let norm = square_root_sum_expression(pool, &norm).ok()?;
    let root = pool.apply(Head::Operator(Operator::Sqrt), &[norm]).ok()?;
    Some(Ok(ExactComplexValue::Real(root)))
}

pub fn exact_complex(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Option<Result<ExactComplexValue, ExactEvaluationError>> {
    if !holds_the_imaginary_unit(pool, expression) || asks_for_a_machine_width(pool, expression) {
        return None;
    }
    if let Some(result) = modulus_outside_the_field(pool, expression) {
        return Some(result);
    }
    let value = match complex_of(pool, expression)? {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    let written =
        |pool: &mut ExprPool, sum: &SquareRootSum| square_root_sum_expression(pool, sum).ok();
    if value.imaginary.is_zero() {
        return Some(Ok(ExactComplexValue::Real(written(pool, &value.real)?)));
    }
    if let (Some(real), Some(imaginary)) = (value.real.as_rational(), value.imaginary.as_rational())
    {
        return Some(Ok(ExactComplexValue::Rational {
            real: real.to_number(),
            imaginary: imaginary.to_number(),
        }));
    }
    let unit = pool.symbol(BuiltinConstant::ImaginaryUnit.symbol()).ok()?;
    let imaginary = written(pool, &value.imaginary)?;
    let imaginary = pool
        .apply(Head::Operator(Operator::Mul), &[imaginary, unit])
        .ok()?;
    let expression = if value.real.is_zero() {
        imaginary
    } else {
        let real = written(pool, &value.real)?;
        pool.apply(Head::Operator(Operator::Add), &[real, imaginary])
            .ok()?
    };
    Some(Ok(ExactComplexValue::Algebraic(expression)))
}

pub fn constructed_complex(pool: &mut ExprPool, expression: ExprId) -> Option<(Number, Number)> {
    if is_imaginary_unit(pool, expression) {
        return Some((
            Number::Integer(Integer::zero()),
            Number::Integer(Integer::one()),
        ));
    }
    let (head, arguments) = match pool.node(expression).ok()? {
        NodeView::Apply { head, arguments } => (head, arguments.to_vec()),
        _ => return None,
    };
    match (head, arguments.as_slice()) {
        (Head::Operator(Operator::Complex), [real, imaginary]) => {
            let (real, imaginary) = (*real, *imaginary);
            Some((exact_part(pool, real)?, exact_part(pool, imaginary)?))
        }
        (Head::Operator(Operator::Mul), [left, right]) => {
            let (left, right) = (*left, *right);
            let scale = match (
                is_imaginary_unit(pool, left),
                is_imaginary_unit(pool, right),
            ) {
                (true, false) => right,
                (false, true) => left,
                _ => return None,
            };
            Some((Number::Integer(Integer::zero()), exact_part(pool, scale)?))
        }
        _ => None,
    }
}

fn is_imaginary_unit(pool: &ExprPool, expression: ExprId) -> bool {
    matches!(
        pool.node(expression),
        Ok(NodeView::Symbol(symbol)) if symbol == BuiltinConstant::ImaginaryUnit.symbol()
    )
}

fn asks_for_a_machine_width(pool: &ExprPool, expression: ExprId) -> bool {
    let mut pending = vec![expression];
    let mut seen: Vec<ExprId> = Vec::new();
    while let Some(current) = pending.pop() {
        if seen.contains(&current) {
            continue;
        }
        seen.push(current);
        match pool.node(current) {
            Ok(NodeView::Apply { head, arguments }) => {
                if matches!(head, Head::Operator(Operator::ToF32 | Operator::ToF64)) {
                    return true;
                }
                pending.extend(arguments.iter().copied());
            }
            Ok(NodeView::Quantity { value, .. }) => pending.push(value),
            Ok(NodeView::Array { elements, .. }) => pending.extend(elements.iter().copied()),
            Ok(NodeView::Bind {
                arguments, body, ..
            }) => {
                pending.extend(arguments.iter().copied());
                pending.push(body);
            }
            Ok(NodeView::Number(_) | NodeView::Symbol(_) | NodeView::Bound(_)) | Err(_) => {}
        }
    }
    false
}

fn exact_part(pool: &mut ExprPool, expression: ExprId) -> Option<Number> {
    if asks_for_a_machine_width(pool, expression) {
        return None;
    }
    let evaluation = evaluate_exact(pool, expression).ok()?;
    if evaluation.unit().is_some() {
        return None;
    }
    let value = evaluation.rational_value()?;
    value.is_exact().then(|| value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts_of(text: &str) -> Option<(Number, Number)> {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, text).unwrap();
        constructed_complex(&mut pool, expression)
    }

    fn whole(value: i64) -> Number {
        Number::Integer(Integer::from(value))
    }

    #[test]
    fn the_imaginary_unit_is_one_times_i() {
        assert_eq!(parts_of("i"), Some((whole(0), whole(1))));
    }

    #[test]
    fn a_product_with_the_imaginary_unit_carries_the_exact_factor() {
        assert_eq!(parts_of("i * sqrt(4)"), Some((whole(0), whole(2))));
    }

    #[test]
    fn a_factor_written_before_the_imaginary_unit_reads_the_same() {
        assert_eq!(parts_of("sqrt(4) * i"), Some((whole(0), whole(2))));
    }

    #[test]
    fn a_complex_call_carries_both_parts() {
        assert_eq!(parts_of("complex(1, 2)"), Some((whole(1), whole(2))));
    }

    #[test]
    fn a_part_that_is_not_exact_is_no_construction() {
        assert_eq!(parts_of("i * sqrt(2)"), None);
    }

    #[test]
    fn a_part_that_carries_a_unit_is_no_construction() {
        assert_eq!(parts_of("i * 2 m"), None);
    }

    #[test]
    fn a_product_of_two_imaginary_units_is_no_construction() {
        assert_eq!(parts_of("i * i"), None);
    }

    #[test]
    fn a_part_asked_for_in_a_machine_width_is_no_construction() {
        assert_eq!(parts_of("to_f64(1) * i"), None);
    }

    #[test]
    fn a_machine_width_inside_a_part_is_no_construction() {
        assert_eq!(parts_of("complex(1 + to_f32(1), 2)"), None);
    }

    fn complex(text: &str) -> Option<Result<ExactComplexValue, ExactEvaluationError>> {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, text).unwrap();
        exact_complex(&mut pool, expression)
    }

    fn rational(text: &str) -> Option<(Number, Number)> {
        match complex(text)? {
            Ok(ExactComplexValue::Rational { real, imaginary }) => Some((real, imaginary)),
            _ => None,
        }
    }

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    #[test]
    fn the_square_of_the_imaginary_unit_is_the_real_minus_one() {
        assert!(matches!(
            complex("i*i"),
            Some(Ok(ExactComplexValue::Real(_)))
        ));
    }

    #[test]
    fn a_square_of_one_plus_i_is_two_i() {
        assert_eq!(rational("(1+i)^2"), Some((whole(0), whole(2))));
    }

    #[test]
    fn a_quotient_of_gaussian_integers_is_exact() {
        assert_eq!(
            rational("(1+2*i)/(3-i)"),
            Some((fraction(1, 10), fraction(7, 10)))
        );
    }

    #[test]
    fn a_negative_power_is_the_power_of_the_reciprocal() {
        assert_eq!(rational("(1+i)^-2"), Some((whole(0), fraction(-1, 2))));
    }

    #[test]
    fn a_complex_call_is_computed_with() {
        assert_eq!(rational("complex(1,2)^2"), Some((whole(-3), whole(4))));
    }

    #[test]
    fn a_zero_imaginary_part_is_dropped() {
        assert!(matches!(
            complex("complex(2, 0)"),
            Some(Ok(ExactComplexValue::Real(_)))
        ));
    }

    #[test]
    fn a_square_root_part_is_carried_exactly() {
        assert!(matches!(
            complex("i*sqrt(2)"),
            Some(Ok(ExactComplexValue::Algebraic(_)))
        ));
        assert!(matches!(
            complex("(sqrt(2)*i)^2"),
            Some(Ok(ExactComplexValue::Real(_)))
        ));
    }

    #[test]
    fn a_division_by_complex_zero_is_a_division_by_zero() {
        assert!(matches!(
            complex("1/(0*i)"),
            Some(Err(ExactEvaluationError::DivisionByZero(_)))
        ));
    }

    #[test]
    fn zero_to_an_exponent_with_positive_real_part_is_zero() {
        assert!(matches!(
            complex("0^(1+i)"),
            Some(Ok(
                ExactComplexValue::Rational { .. } | ExactComplexValue::Real(_)
            ))
        ));
    }

    #[test]
    fn zero_to_an_imaginary_exponent_has_no_value() {
        assert!(matches!(
            complex("0^i"),
            Some(Err(ExactEvaluationError::PowerOfZeroWithoutValue { .. }))
        ));
    }

    #[test]
    fn zero_to_an_exponent_with_negative_real_part_divides_by_zero() {
        assert!(matches!(
            complex("0^(-1+2*i)"),
            Some(Err(ExactEvaluationError::DivisionByZero(_)))
        ));
    }

    #[test]
    fn a_power_past_the_size_limit_is_refused_and_not_computed() {
        assert!(matches!(
            complex("(1+i)^100000000"),
            Some(Err(ExactEvaluationError::ResultTooLarge { .. }))
        ));
    }

    fn real_text(text: &str) -> Option<String> {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, text).unwrap();
        match exact_complex(&mut pool, expression)? {
            Ok(ExactComplexValue::Real(real)) => {
                calc_syntax::print_expression(&pool, real, calc_syntax::PrintMode::Ascii).ok()
            }
            _ => None,
        }
    }

    #[test]
    fn the_modulus_of_three_plus_four_i_is_five() {
        assert_eq!(real_text("abs(3+4*i)").as_deref(), Some("5"));
    }

    #[test]
    fn a_modulus_outside_the_field_is_its_exact_root() {
        assert_eq!(
            real_text("abs(1+sqrt(2)+i)").as_deref(),
            Some("sqrt(4 + 2 * sqrt(2))")
        );
    }

    #[test]
    fn a_squared_modulus_needs_no_root() {
        assert_eq!(
            real_text("abs(1+sqrt(2)+i)^2").as_deref(),
            Some("4 + 2 * sqrt(2)")
        );
    }

    #[test]
    fn the_parts_of_a_complex_number_are_real() {
        assert_eq!(real_text("re(3+4*i)").as_deref(), Some("3"));
        assert_eq!(real_text("im(3+4*i)").as_deref(), Some("4"));
    }

    #[test]
    fn the_conjugate_negates_the_imaginary_part() {
        assert_eq!(rational("conj(3+4*i)"), Some((whole(3), whole(-4))));
    }

    #[test]
    fn a_line_with_pi_or_a_function_of_i_is_left_to_the_machine() {
        assert_eq!(complex("pi*i"), None);
        assert_eq!(complex("exp(i)"), None);
        assert_eq!(complex("i^(1/2)"), None);
    }

    #[test]
    fn a_sum_that_holds_the_imaginary_unit_is_no_construction() {
        assert_eq!(parts_of("1 + i"), None);
    }
}
