use calc_expr::{BinderKind, ExprId, ExprPool, Head, LimitSide, NodeView, Operator};

use crate::derivative::{depends_on, derivative};
use crate::exact_evaluation::evaluate_exact;
use crate::exact_rational::ExactRational;

const CANCELLATION_LIMIT: u32 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitRefusal {
    NotARationalFunction,
    NoFiniteLimit(LimitSide),
    Undecided,
}

pub(crate) fn is_analytic_in_the_variable(pool: &ExprPool, expression: ExprId) -> bool {
    if !depends_on(pool, expression, 0) {
        return true;
    }
    match pool.node(expression) {
        Ok(NodeView::Bound(0)) => true,
        Ok(NodeView::Apply {
            head: Head::Operator(operator),
            arguments,
        }) => {
            let arguments = arguments.to_vec();
            match (operator, arguments.as_slice()) {
                (Operator::Add | Operator::Sub | Operator::Mul | Operator::Div, [left, right]) => {
                    is_analytic_in_the_variable(pool, *left)
                        && is_analytic_in_the_variable(pool, *right)
                }
                (
                    Operator::Neg
                    | Operator::Sin
                    | Operator::Cos
                    | Operator::Tan
                    | Operator::Exp
                    | Operator::Ln
                    | Operator::Atan,
                    [argument],
                ) => is_analytic_in_the_variable(pool, *argument),
                (Operator::Pow, [base, exponent]) => {
                    !depends_on(pool, *exponent, 0)
                        && whole_exponent(pool, *exponent)
                        && is_analytic_in_the_variable(pool, *base)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn whole_exponent(pool: &ExprPool, exponent: ExprId) -> bool {
    match pool.node(exponent) {
        Ok(NodeView::Number(number)) => pool
            .number_value(number)
            .ok()
            .and_then(|value| value.to_exact().ok())
            .and_then(|value| ExactRational::from_number(&value))
            .is_some_and(|value| value.is_integer()),
        _ => false,
    }
}

fn vanishes_at(pool: &mut ExprPool, body: ExprId, point: ExprId) -> Option<bool> {
    let placed = crate::rule_search::replace_parameters(pool, body, &[point], 0)?;
    let evaluation = evaluate_exact(pool, placed).ok()?;
    if let Some(value) = evaluation
        .rational_value()
        .and_then(ExactRational::from_number)
    {
        return Some(value.is_zero());
    }
    if let Some(sign) = crate::exact_evaluation::exact_sign(pool, placed) {
        return Some(sign == std::cmp::Ordering::Equal);
    }
    let enclosure = crate::decimal_enclosure::enclose_decimal(pool, placed, 4, &|| false).ok()?;
    let lower = enclosure
        .lower
        .to_exact()
        .ok()
        .and_then(|value| ExactRational::from_number(&value))?;
    let upper = enclosure
        .upper
        .to_exact()
        .ok()
        .and_then(|value| ExactRational::from_number(&value))?;
    (lower.sign() == std::cmp::Ordering::Greater || upper.sign() == std::cmp::Ordering::Less)
        .then_some(false)
}

fn quotient_limit(
    pool: &mut ExprPool,
    numerator: ExprId,
    denominator: ExprId,
    point: ExprId,
    side: LimitSide,
) -> Result<ExprId, LimitRefusal> {
    if !is_analytic_in_the_variable(pool, numerator)
        || !is_analytic_in_the_variable(pool, denominator)
    {
        return Err(LimitRefusal::NotARationalFunction);
    }
    let (mut numerator, mut denominator) = (numerator, denominator);
    for _ in 0..CANCELLATION_LIMIT {
        let above_vanishes = vanishes_at(pool, numerator, point).ok_or(LimitRefusal::Undecided)?;
        let below_vanishes =
            vanishes_at(pool, denominator, point).ok_or(LimitRefusal::Undecided)?;
        if !below_vanishes {
            let placed_above = crate::rule_search::replace_parameters(pool, numerator, &[point], 0)
                .ok_or(LimitRefusal::Undecided)?;
            let placed_below =
                crate::rule_search::replace_parameters(pool, denominator, &[point], 0)
                    .ok_or(LimitRefusal::Undecided)?;
            return pool
                .apply(Head::Operator(Operator::Div), &[placed_above, placed_below])
                .map_err(|_| LimitRefusal::Undecided);
        }
        if !above_vanishes {
            return Err(LimitRefusal::NoFiniteLimit(side));
        }
        numerator = derivative(pool, numerator, 0).ok_or(LimitRefusal::Undecided)?;
        denominator = derivative(pool, denominator, 0).ok_or(LimitRefusal::Undecided)?;
    }
    Err(LimitRefusal::Undecided)
}

pub fn limit_value(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Option<Result<ExprId, LimitRefusal>> {
    let NodeView::Bind {
        binder: BinderKind::Limit(side),
        arguments,
        body,
    } = pool.node(expression).ok()?
    else {
        return None;
    };
    let point = *arguments.first()?;
    if let Ok(NodeView::Apply {
        head: Head::Operator(Operator::Div),
        arguments: [numerator, denominator],
    }) = pool.node(body)
    {
        let (numerator, denominator) = (*numerator, *denominator);
        return Some(quotient_limit(pool, numerator, denominator, point, side));
    }
    if !is_analytic_in_the_variable(pool, body) {
        return Some(Err(LimitRefusal::NotARationalFunction));
    }
    Some(
        crate::rule_search::replace_parameters(pool, body, &[point], 0)
            .ok_or(LimitRefusal::Undecided),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_syntax::parse_expression;

    fn limit_of(text: &str) -> Result<Option<ExactRational>, LimitRefusal> {
        let mut pool = ExprPool::new();
        let expression = parse_expression(&mut pool, text).expect("the input parses");
        let value = limit_value(&mut pool, expression).expect("it is a limit")?;
        Ok(evaluate_exact(&mut pool, value)
            .ok()
            .and_then(|evaluation| {
                evaluation
                    .rational_value()
                    .and_then(ExactRational::from_number)
            }))
    }

    fn fraction(numerator: i64, denominator: i64) -> Result<Option<ExactRational>, LimitRefusal> {
        Ok(Some(
            ExactRational::from_i64(numerator)
                .divide(&ExactRational::from_i64(denominator))
                .expect("the expected value is a fraction"),
        ))
    }

    #[test]
    fn a_polynomial_is_continuous_so_the_limit_is_the_value() {
        assert_eq!(limit_of("limit(x^2 + 1, x, 3)"), fraction(10, 1));
    }

    #[test]
    fn a_quotient_whose_denominator_survives_is_taken_directly() {
        assert_eq!(limit_of("limit((x + 1) / (x + 2), x, 0)"), fraction(1, 2));
    }

    #[test]
    fn a_zero_over_zero_quotient_is_cancelled() {
        assert_eq!(limit_of("limit((x^2 - 1) / (x - 1), x, 1)"), fraction(2, 1));
    }

    #[test]
    fn a_double_root_is_cancelled_twice() {
        assert_eq!(
            limit_of("limit((x^3 - 3*x + 2) / (x^2 - 2*x + 1), x, 1)"),
            fraction(3, 1)
        );
    }

    #[test]
    fn a_pole_has_no_finite_limit() {
        assert_eq!(
            limit_of("limit(1 / (x - 1), x, 1)"),
            Err(LimitRefusal::NoFiniteLimit(LimitSide::Both))
        );
    }

    #[test]
    fn a_pole_refused_from_one_side_names_that_side() {
        assert_eq!(
            limit_of("limit(1 / x, x, 0, side=right)"),
            Err(LimitRefusal::NoFiniteLimit(LimitSide::Right))
        );
    }

    #[test]
    fn a_body_outside_the_analytic_set_is_refused() {
        assert_eq!(
            limit_of("limit(abs(x) / x, x, 0)"),
            Err(LimitRefusal::NotARationalFunction)
        );
    }

    #[test]
    fn an_analytic_quotient_of_two_vanishing_parts_has_its_limit() {
        assert_eq!(limit_of("limit(sin(x) / x, x, 0)"), fraction(1, 1));
    }

    #[test]
    fn a_limit_at_a_fraction_is_exact() {
        assert_eq!(limit_of("limit(x^2, x, 1/3)"), fraction(1, 9));
    }
}
