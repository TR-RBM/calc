use calc_expr::{BinderKind, ExprId, ExprPool, Head, NodeView, Operator, SymbolKind};
use calc_numbers::Integer;

use crate::derivative::derivative;
use crate::exact_evaluation::{closed_square_root_sum, evaluate_exact};
use crate::exact_rational::ExactRational;
use crate::limit::is_analytic_in_the_variable;

const ORDER_LIMIT: i64 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaylorRefusal {
    NotAnalytic,
    OrderNotWhole,
    OrderTooHigh,
    NotDefinedAtPoint,
}

fn apply(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> Option<ExprId> {
    pool.apply(Head::Operator(operator), arguments).ok()
}

fn signed(pool: &mut ExprPool, expression: ExprId) -> (bool, ExprId) {
    match pool.node(expression) {
        Ok(NodeView::Apply {
            head: Head::Operator(Operator::Neg),
            arguments: [inner],
        }) => {
            let (negative, magnitude) = signed(pool, *inner);
            (!negative, magnitude)
        }
        Ok(NodeView::Apply {
            head: Head::Operator(Operator::Div),
            arguments: [numerator, denominator],
        }) => {
            let (numerator, denominator) = (*numerator, *denominator);
            let (negative, magnitude) = signed(pool, numerator);
            if magnitude == numerator {
                return (false, expression);
            }
            match apply(pool, Operator::Div, &[magnitude, denominator]) {
                Some(divided) => (negative, divided),
                None => (false, expression),
            }
        }
        _ => (false, expression),
    }
}

pub fn taylor_polynomial(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Option<Result<(ExprId, bool), TaylorRefusal>> {
    let NodeView::Bind {
        binder: BinderKind::Taylor,
        arguments,
        body,
    } = pool.node(expression).ok()?
    else {
        return None;
    };
    let (point, order) = (*arguments.first()?, *arguments.get(1)?);
    Some(expanded_at(pool, expression, body, point, order))
}

fn expanded_at(
    pool: &mut ExprPool,
    expression: ExprId,
    body: ExprId,
    point: ExprId,
    order: ExprId,
) -> Result<(ExprId, bool), TaylorRefusal> {
    if !is_analytic_in_the_variable(pool, body) {
        return Err(TaylorRefusal::NotAnalytic);
    }
    let order = closed_square_root_sum(pool, order)
        .and_then(|value| value.as_rational())
        .filter(|value| value.is_integer() && value.sign() != std::cmp::Ordering::Less)
        .and_then(|value| value.numerator().to_i64())
        .ok_or(TaylorRefusal::OrderNotWhole)?;
    if order > ORDER_LIMIT {
        return Err(TaylorRefusal::OrderTooHigh);
    }
    let about_zero = closed_square_root_sum(pool, point).is_some_and(|value| value.is_zero());
    let name = pool.bound_name(expression).unwrap_or("x").to_owned();
    let fail = || TaylorRefusal::NotDefinedAtPoint;
    let symbol = pool
        .intern_symbol(&name, SymbolKind::Variable)
        .map_err(|_| fail())?;
    let variable = pool.symbol(symbol).map_err(|_| fail())?;
    let shifted = if about_zero {
        variable
    } else {
        apply(pool, Operator::Sub, &[variable, point]).ok_or_else(fail)?
    };
    let mut current = body;
    let mut factorial = Integer::one();
    let mut total: Option<ExprId> = None;
    for step in 0..=order {
        if step > 0 {
            factorial = &factorial * &Integer::from(step);
        }
        let placed =
            crate::rule_search::replace_parameters(pool, current, &[point], 0).ok_or_else(fail)?;
        let evaluation = evaluate_exact(pool, placed).map_err(|_| fail())?;
        let is_zero = evaluation
            .rational_value()
            .and_then(ExactRational::from_number)
            .is_some_and(|value| value.is_zero());
        if !is_zero {
            let coefficient = evaluation.expression();
            let divisor = pool.number(factorial.clone().into()).map_err(|_| fail())?;
            let scaled = apply(pool, Operator::Div, &[coefficient, divisor]).ok_or_else(fail)?;
            let scaled = evaluate_exact(pool, scaled).map_err(|_| fail())?;
            let rational = scaled.rational_value().and_then(ExactRational::from_number);
            let (negative, magnitude) = match &rational {
                Some(value) if value.sign() == std::cmp::Ordering::Less => {
                    let positive = value.negated();
                    (true, pool.number(positive.to_number()).map_err(|_| fail())?)
                }
                Some(_) => (false, scaled.expression()),
                None => signed(pool, scaled.expression()),
            };
            let unit = rational.as_ref().is_some_and(|value| {
                value.is_integer() && value.numerator().to_i64().is_some_and(|n| n.abs() == 1)
            });
            let power = match step {
                0 => None,
                1 => Some(shifted),
                _ => {
                    let exponent = pool
                        .number(Integer::from(step).into())
                        .map_err(|_| fail())?;
                    Some(apply(pool, Operator::Pow, &[shifted, exponent]).ok_or_else(fail)?)
                }
            };
            let constant = power.is_none();
            let term = match power {
                None => magnitude,
                Some(power) if unit => power,
                Some(power) => apply(pool, Operator::Mul, &[magnitude, power]).ok_or_else(fail)?,
            };
            total = Some(match (total, negative) {
                (Some(running), true) => {
                    apply(pool, Operator::Sub, &[running, term]).ok_or_else(fail)?
                }
                (Some(running), false) => {
                    apply(pool, Operator::Add, &[running, term]).ok_or_else(fail)?
                }
                (None, true) if constant => scaled.expression(),
                (None, true) => apply(pool, Operator::Neg, &[term]).ok_or_else(fail)?,
                (None, false) => term,
            });
        }
        if step < order {
            current = derivative(pool, current, 0).ok_or_else(fail)?;
        }
    }
    let total = match total {
        Some(total) => total,
        None => pool.number(Integer::zero().into()).map_err(|_| fail())?,
    };
    Ok((total, about_zero))
}
