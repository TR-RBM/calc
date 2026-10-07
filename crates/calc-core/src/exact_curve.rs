use std::cmp::Ordering;

use calc_expr::{ExprId, ExprPool, SymbolId};
use calc_numbers::Number;

use crate::exact_rational::ExactRational;

const TURN_PRECISION_BITS: u32 = 32;
const LARGEST_TURN_BITS: u32 = 4096;
use crate::polynomial::{AtomTable, rational_of};
use crate::real_roots::{RealRoot, derivative, real_roots, univariate_coefficients};

#[derive(Clone, Debug)]
pub struct ExactCurve {
    numerator: Vec<ExactRational>,
    denominator: Vec<ExactRational>,
    poles: Vec<RealRoot>,
    turns: Vec<RealRoot>,
}

#[derive(Clone, Debug)]
struct Span {
    low: ExactRational,
    high: ExactRational,
}

impl Span {
    fn point(value: ExactRational) -> Span {
        Span {
            low: value.clone(),
            high: value,
        }
    }

    fn plus(&self, other: &Span) -> Span {
        Span {
            low: self.low.plus(&other.low),
            high: self.high.plus(&other.high),
        }
    }

    fn times(&self, other: &Span) -> Span {
        let products = [
            self.low.multiply(&other.low),
            self.low.multiply(&other.high),
            self.high.multiply(&other.low),
            self.high.multiply(&other.high),
        ];
        let mut span = Span::point(products[0].clone());
        for product in &products[1..] {
            if product.compare(&span.low) == Ordering::Less {
                span.low = product.clone();
            }
            if product.compare(&span.high) == Ordering::Greater {
                span.high = product.clone();
            }
        }
        span
    }

    fn holds_zero(&self) -> bool {
        self.low.sign() != Ordering::Greater && self.high.sign() != Ordering::Less
    }

    fn over(&self, divisor: &Span) -> Option<Span> {
        if divisor.holds_zero() {
            return None;
        }
        let reciprocal = Span {
            low: divisor.high.reciprocal()?,
            high: divisor.low.reciprocal()?,
        };
        Some(self.times(&reciprocal))
    }
}

fn product(left: &[ExactRational], right: &[ExactRational]) -> Vec<ExactRational> {
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

fn difference(left: &[ExactRational], right: &[ExactRational]) -> Vec<ExactRational> {
    let mut result = vec![ExactRational::zero(); left.len().max(right.len())];
    for (index, value) in left.iter().enumerate() {
        result[index] = result[index].plus(value);
    }
    for (index, value) in right.iter().enumerate() {
        result[index] = result[index].subtract(value);
    }
    while result.last().is_some_and(ExactRational::is_zero) {
        result.pop();
    }
    result
}

fn roots_of(coefficients: &[ExactRational]) -> Option<Vec<RealRoot>> {
    if coefficients.len() > 1 {
        real_roots(coefficients)
    } else {
        Some(Vec::new())
    }
}

fn turn_bits(width: &ExactRational) -> u32 {
    let two = ExactRational::from_i64(2);
    let mut scaled = width.absolute();
    let mut bits = TURN_PRECISION_BITS;
    while bits < LARGEST_TURN_BITS
        && !scaled.is_zero()
        && scaled.compare(&ExactRational::one()) == Ordering::Less
    {
        scaled = scaled.multiply(&two);
        bits += 1;
    }
    bits
}

fn value_at(coefficients: &[ExactRational], at: &ExactRational) -> ExactRational {
    coefficients
        .iter()
        .rev()
        .fold(ExactRational::zero(), |total, coefficient| {
            total.multiply(at).plus(coefficient)
        })
}

fn naive_span(coefficients: &[ExactRational], over: &Span) -> Span {
    coefficients
        .iter()
        .rev()
        .fold(Span::point(ExactRational::zero()), |total, coefficient| {
            total.times(over).plus(&Span::point(coefficient.clone()))
        })
}

fn mean_value_span(coefficients: &[ExactRational], over: &Span) -> Span {
    let centre = over
        .low
        .plus(&over.high)
        .multiply(&ExactRational::one_half());
    let radius = over
        .high
        .subtract(&over.low)
        .multiply(&ExactRational::one_half());
    let slope = naive_span(&derivative(coefficients), over);
    let steepest = if slope.low.absolute().compare(&slope.high.absolute()) == Ordering::Greater {
        slope.low.absolute()
    } else {
        slope.high.absolute()
    };
    let reach = steepest.multiply(&radius);
    let middle = value_at(coefficients, &centre);
    Span {
        low: middle.subtract(&reach),
        high: middle.plus(&reach),
    }
}

impl ExactCurve {
    pub fn value_at(&self, at: &Number) -> Option<Number> {
        let at = ExactRational::from_number(at)?;
        let denominator = value_at(&self.denominator, &at);
        if denominator.is_zero() {
            return None;
        }
        Some(
            value_at(&self.numerator, &at)
                .divide(&denominator)?
                .to_number(),
        )
    }

    fn has_pole_on(&mut self, low: &ExactRational, high: &ExactRational) -> bool {
        self.poles.iter_mut().any(|pole| {
            pole.compare_with(low) != Ordering::Less && pole.compare_with(high) != Ordering::Greater
        })
    }

    fn turns_inside(&mut self, low: &ExactRational, high: &ExactRational) -> Vec<usize> {
        (0..self.turns.len())
            .filter(|index| {
                let turn = &mut self.turns[*index];
                turn.compare_with(low) == Ordering::Greater
                    && turn.compare_with(high) == Ordering::Less
            })
            .collect()
    }

    fn span_over(&self, over: &Span) -> Option<Span> {
        mean_value_span(&self.numerator, over).over(&mean_value_span(&self.denominator, over))
    }

    fn exact_value(&self, at: &ExactRational) -> Option<ExactRational> {
        value_at(&self.numerator, at).divide(&value_at(&self.denominator, at))
    }

    pub fn enclose(&mut self, low: &Number, high: &Number) -> Option<(Number, Number)> {
        let low = ExactRational::from_number(low)?;
        let high = ExactRational::from_number(high)?;
        if self.has_pole_on(&low, &high) {
            return None;
        }
        let inside = self.turns_inside(&low, &high);
        let (first, last) = (self.exact_value(&low)?, self.exact_value(&high)?);
        let mut span = if first.compare(&last) == Ordering::Greater {
            Span {
                low: last,
                high: first,
            }
        } else {
            Span {
                low: first,
                high: last,
            }
        };
        let bits = turn_bits(&high.subtract(&low));
        for index in inside {
            let turn = &mut self.turns[index];
            turn.narrowed_below(bits);
            let at_turn = match turn.exact() {
                Some(at) => Span::point(self.exact_value(&at)?),
                None => {
                    let (lower, upper) = turn.bounds();
                    self.span_over(&Span {
                        low: lower,
                        high: upper,
                    })?
                }
            };
            if at_turn.low.compare(&span.low) == Ordering::Less {
                span.low = at_turn.low;
            }
            if at_turn.high.compare(&span.high) == Ordering::Greater {
                span.high = at_turn.high;
            }
        }
        Some((span.low.to_number(), span.high.to_number()))
    }
}

pub fn exact_curve(
    pool: &mut ExprPool,
    expression: ExprId,
    symbol: SymbolId,
) -> Option<ExactCurve> {
    let variable = pool.symbol(symbol).ok()?;
    let mut atoms = AtomTable::default();
    let function = rational_of(pool, expression, &mut atoms)?;
    if atoms.expressions().iter().any(|atom| *atom != variable) {
        return None;
    }
    let numerator = univariate_coefficients(function.numerator())?;
    let denominator = univariate_coefficients(function.denominator())?;
    if denominator.iter().all(ExactRational::is_zero) {
        return None;
    }
    let poles = roots_of(&denominator)?;
    let slope = difference(
        &product(&derivative(&numerator), &denominator),
        &product(&numerator, &derivative(&denominator)),
    );
    let turns = roots_of(&slope)?;
    Some(ExactCurve {
        numerator,
        denominator,
        poles,
        turns,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_expr::SymbolKind;
    use calc_numbers::Integer;

    fn curve(text: &str) -> Option<ExactCurve> {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, text).unwrap();
        let symbol = pool.intern_symbol("x", SymbolKind::Variable).unwrap();
        exact_curve(&mut pool, expression, symbol)
    }

    fn number(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    #[test]
    fn a_cancelling_line_is_its_exact_value() {
        let curve = curve("(x + 1) - 1").unwrap();

        assert_eq!(
            curve.value_at(&number(3, 200_000_000_000_000_000)),
            Some(number(3, 200_000_000_000_000_000))
        );
    }

    #[test]
    fn a_monotone_column_is_enclosed_by_its_end_values() {
        let mut curve = curve("x^2").unwrap();

        assert_eq!(
            curve.enclose(&number(1, 1), &number(2, 1)),
            Some((number(1, 1), number(4, 1)))
        );
    }

    #[test]
    fn a_turning_point_inside_is_enclosed_within_a_tiny_margin() {
        let mut curve = curve("x^2 - 2*x").unwrap();

        let (low, high) = curve.enclose(&number(0, 1), &number(3, 1)).unwrap();

        assert_eq!(high, number(3, 1));
        let margin = low.add_exact(&number(1, 1)).unwrap();
        assert!(margin.round_to_f64_ties_even().abs() < 1e-6);
        assert!(margin.round_to_f64_ties_even() <= 0.0);
    }

    #[test]
    fn a_pole_on_the_column_leaves_it_without_an_enclosure() {
        let mut curve = curve("1/(x - 1)").unwrap();

        assert_eq!(curve.enclose(&number(0, 1), &number(2, 1)), None);
        assert_eq!(curve.value_at(&number(1, 1)), None);
    }

    #[test]
    fn a_line_with_another_atom_is_not_an_exact_curve() {
        assert!(curve("x + sin(x)").is_none());
    }
}
