use std::cell::RefCell;
use std::collections::HashMap;

use calc_expr::{
    AccessError, BinderKind, BuiltinConstant, ExprId, ExprPool, Head, NodeView, Operator,
};
use calc_numbers::{
    BallError, DecimalDigits, DecimalEnclosure, ENCLOSURE_BUDGET_BITS, EnclosureError,
    EnclosureStep, Expansion, Number, RealBall, SIGNIFICANT_DIGITS_LIMIT, at_least_one,
    correctly_rounded_to_significant_digits, decimal_digits, enclose_exact,
    enclose_to_significant_digits, maximum_f32, maximum_f64, minimum_f32, minimum_f64,
    truncated_at_places,
};

use crate::exact_evaluation::{ExactEvaluationError, evaluate_exact};
use crate::quantities::{QuantityError, to_coherent_units};
use crate::real_roots::{RealRoot, root_of};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecimalEnclosureError {
    Access(AccessError),
    Quantity(QuantityError),
    NotApplicable { expression: ExprId },
    Enclosure(EnclosureError),
}

type Checked<T> = Result<T, DecimalEnclosureError>;

fn not_applicable(expression: ExprId) -> DecimalEnclosureError {
    DecimalEnclosureError::NotApplicable { expression }
}

fn has_enclosure(operator: Operator) -> bool {
    matches!(
        operator,
        Operator::Add
            | Operator::Sub
            | Operator::Mul
            | Operator::Div
            | Operator::Neg
            | Operator::Abs
            | Operator::Sqrt
            | Operator::Pow
            | Operator::Exp
            | Operator::Ln
            | Operator::Sin
            | Operator::Cos
            | Operator::Tan
            | Operator::Atan
            | Operator::Atan2
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MachineDomain {
    Single,
    Double,
}

fn domain_of(operator: Operator) -> Option<MachineDomain> {
    match operator {
        Operator::ToF32 => Some(MachineDomain::Single),
        Operator::ToF64 => Some(MachineDomain::Double),
        _ => None,
    }
}

fn is_pi(symbol: calc_expr::SymbolId) -> bool {
    symbol == BuiltinConstant::Pi.symbol()
}

fn is_euler(symbol: calc_expr::SymbolId) -> bool {
    symbol == BuiltinConstant::E.symbol()
}

fn machine_double(pool: &ExprPool, expression: ExprId) -> Option<f64> {
    match pool.node(expression).ok()? {
        NodeView::Number(number) => match pool.number_value(number).ok()? {
            Number::F64(value) => Some(*value),
            Number::F32(_) => None,
            exact => Some(exact.round_to_f64_ties_even()),
        },
        NodeView::Symbol(symbol) if is_pi(symbol) => Some(std::f64::consts::PI),
        NodeView::Symbol(symbol) if is_euler(symbol) => Some(std::f64::consts::E),
        NodeView::Apply {
            head: Head::Operator(operator),
            arguments,
        } => {
            let values = arguments
                .iter()
                .map(|argument| machine_double(pool, *argument))
                .collect::<Option<Vec<f64>>>()?;
            match (operator, values.as_slice()) {
                (Operator::ToF64, [value]) => Some(*value),
                (Operator::Add, [left, right]) => Some(left + right),
                (Operator::Sub, [left, right]) => Some(left - right),
                (Operator::Mul, [left, right]) => Some(left * right),
                (Operator::Div, [left, right]) => Some(left / right),
                (Operator::Neg, [value]) => Some(-value),
                (Operator::Abs, [value]) => Some(value.abs()),
                (Operator::Sqrt, [value]) => Some(value.sqrt()),
                (Operator::MulAdd, [factor, other, addend]) => {
                    Some(factor.mul_add(*other, *addend))
                }
                (Operator::Floor, [value]) => Some(value.floor()),
                (Operator::Ceil, [value]) => Some(value.ceil()),
                (Operator::Trunc, [value]) => Some(value.trunc()),
                (Operator::RoundTiesEven, [value]) => Some(value.round_ties_even()),
                (Operator::CopySign, [value, sign]) => Some(value.copysign(*sign)),
                (Operator::Min, [left, right]) => Some(minimum_f64(*left, *right)),
                (Operator::Max, [left, right]) => Some(maximum_f64(*left, *right)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn machine_single(pool: &ExprPool, expression: ExprId) -> Option<f32> {
    match pool.node(expression).ok()? {
        NodeView::Number(number) => match pool.number_value(number).ok()? {
            Number::F32(value) => Some(*value),
            Number::F64(_) => None,
            exact => Some(exact.round_to_f32_ties_even()),
        },
        NodeView::Symbol(symbol) if is_pi(symbol) => Some(std::f32::consts::PI),
        NodeView::Symbol(symbol) if is_euler(symbol) => Some(std::f32::consts::E),
        NodeView::Apply {
            head: Head::Operator(operator),
            arguments,
        } => {
            let values = arguments
                .iter()
                .map(|argument| machine_single(pool, *argument))
                .collect::<Option<Vec<f32>>>()?;
            match (operator, values.as_slice()) {
                (Operator::ToF32, [value]) => Some(*value),
                (Operator::Add, [left, right]) => Some(left + right),
                (Operator::Sub, [left, right]) => Some(left - right),
                (Operator::Mul, [left, right]) => Some(left * right),
                (Operator::Div, [left, right]) => Some(left / right),
                (Operator::Neg, [value]) => Some(-value),
                (Operator::Abs, [value]) => Some(value.abs()),
                (Operator::Sqrt, [value]) => Some(value.sqrt()),
                (Operator::MulAdd, [factor, other, addend]) => {
                    Some(factor.mul_add(*other, *addend))
                }
                (Operator::Floor, [value]) => Some(value.floor()),
                (Operator::Ceil, [value]) => Some(value.ceil()),
                (Operator::Trunc, [value]) => Some(value.trunc()),
                (Operator::RoundTiesEven, [value]) => Some(value.round_ties_even()),
                (Operator::CopySign, [value, sign]) => Some(value.copysign(*sign)),
                (Operator::Min, [left, right]) => Some(minimum_f32(*left, *right)),
                (Operator::Max, [left, right]) => Some(maximum_f32(*left, *right)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn machine_value(pool: &ExprPool, domain: MachineDomain, argument: ExprId) -> Option<Number> {
    let value = match domain {
        MachineDomain::Double => Number::F64(machine_double(pool, argument)?),
        MachineDomain::Single => Number::F32(machine_single(pool, argument)?),
    };
    value.to_exact().ok()
}

fn exact_integer_exponent(pool: &mut ExprPool, exponent: ExprId) -> Option<i64> {
    let evaluation = evaluate_exact(pool, exponent).ok()?;
    match evaluation.rational_value()? {
        Number::Integer(integer) => integer.to_i64(),
        _ => None,
    }
}

#[derive(Default)]
struct Prepared {
    machine_values: HashMap<ExprId, Number>,
    integer_exponents: HashMap<ExprId, i64>,
    roots: HashMap<ExprId, RefCell<RealRoot>>,
}

fn prepare(pool: &mut ExprPool, root: ExprId) -> Checked<Prepared> {
    let mut prepared = Prepared::default();
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        let (operator, arguments) = match pool
            .node(expression)
            .map_err(DecimalEnclosureError::Access)?
        {
            NodeView::Number(number) => {
                let value = pool
                    .number_value(number)
                    .map_err(DecimalEnclosureError::Access)?;
                if value.to_exact().is_err() {
                    return Err(not_applicable(expression));
                }
                continue;
            }
            NodeView::Symbol(symbol) if is_pi(symbol) || is_euler(symbol) => continue,
            NodeView::Apply {
                head: Head::Operator(operator),
                arguments,
            } => (operator, arguments.to_vec()),
            NodeView::Bind {
                binder: BinderKind::Root,
                ..
            } => {
                let root = root_of(pool, expression)
                    .and_then(Result::ok)
                    .ok_or_else(|| not_applicable(expression))?;
                prepared.roots.insert(expression, RefCell::new(root));
                continue;
            }
            _ => return Err(not_applicable(expression)),
        };
        if let (Some(domain), [argument]) = (domain_of(operator), arguments.as_slice()) {
            let value =
                machine_value(pool, domain, *argument).ok_or_else(|| not_applicable(expression))?;
            prepared.machine_values.insert(expression, value);
            continue;
        }
        if !has_enclosure(operator) {
            return Err(not_applicable(expression));
        }
        if let (Operator::Pow, [_, exponent]) = (operator, arguments.as_slice())
            && let Some(integer) = exact_integer_exponent(pool, *exponent)
        {
            prepared.integer_exponents.insert(expression, integer);
        }
        pending.extend(arguments);
    }
    Ok(prepared)
}

struct BallEvaluator<'pool, 'step> {
    pool: &'pool ExprPool,
    prepared: &'pool Prepared,
    step: &'pool EnclosureStep<'step>,
    memo: HashMap<ExprId, Result<RealBall<'step>, BallError>>,
}

impl<'step> BallEvaluator<'_, 'step> {
    fn ball(&mut self, expression: ExprId) -> Result<RealBall<'step>, BallError> {
        if let Some(ball) = self.memo.get(&expression) {
            return ball.clone();
        }
        let ball = self.evaluate(expression);
        self.memo.insert(expression, ball.clone());
        ball
    }

    fn evaluate(&mut self, expression: ExprId) -> Result<RealBall<'step>, BallError> {
        if let Some(value) = self.prepared.machine_values.get(&expression) {
            return self.step.exact(value);
        }
        match self.pool.node(expression) {
            Ok(NodeView::Number(number)) => self.step.exact(
                self.pool
                    .number_value(number)
                    .map_err(|_| BallError::NoEnclosure)?,
            ),
            Ok(NodeView::Symbol(symbol)) if is_pi(symbol) => self.step.pi(),
            Ok(NodeView::Symbol(symbol)) if is_euler(symbol) => self.step.e(),
            Ok(NodeView::Apply {
                head: Head::Operator(operator),
                arguments,
            }) => self.apply(expression, operator, arguments),
            Ok(NodeView::Bind {
                binder: BinderKind::Root,
                ..
            }) => self.root(expression),
            _ => Err(BallError::NoEnclosure),
        }
    }

    fn root(&self, expression: ExprId) -> Result<RealBall<'step>, BallError> {
        let mut root = self
            .prepared
            .roots
            .get(&expression)
            .ok_or(BallError::NoEnclosure)?
            .borrow_mut();
        let bits = u32::try_from(self.step.precision_bits.saturating_add(2))
            .map_err(|_| BallError::NoEnclosure)?;
        root.narrowed_below(bits);
        let (lower, upper) = root.bounds();
        self.step.between(&lower.to_number(), &upper.to_number())
    }

    fn apply(
        &mut self,
        expression: ExprId,
        operator: Operator,
        arguments: &[ExprId],
    ) -> Result<RealBall<'step>, BallError> {
        if let (Some(integer), [base, _]) =
            (self.prepared.integer_exponents.get(&expression), arguments)
        {
            return self.ball(*base)?.power(*integer);
        }
        match (operator, arguments) {
            (Operator::Add, [left, right]) => self.ball(*left)?.add(&self.ball(*right)?),
            (Operator::Sub, [left, right]) => self.ball(*left)?.sub(&self.ball(*right)?),
            (Operator::Mul, [left, right]) => self.ball(*left)?.mul(&self.ball(*right)?),
            (Operator::Div, [left, right]) => self.ball(*left)?.div(&self.ball(*right)?),
            (Operator::Pow, [base, exponent]) => {
                let logarithm = self.ball(*base)?.ln()?;
                self.ball(*exponent)?.mul(&logarithm)?.exp()
            }
            (Operator::Neg, [argument]) => Ok(self.ball(*argument)?.negated()),
            (Operator::Abs, [argument]) => Ok(self.ball(*argument)?.absolute()),
            (Operator::Sqrt, [argument]) => self.ball(*argument)?.sqrt(),
            (Operator::Exp, [argument]) => self.ball(*argument)?.exp(),
            (Operator::Ln, [argument]) => self.ball(*argument)?.ln(),
            (Operator::Sin, [argument]) => self.ball(*argument)?.sin(),
            (Operator::Cos, [argument]) => self.ball(*argument)?.cos(),
            (Operator::Tan, [argument]) => self.ball(*argument)?.tan(),
            (Operator::Atan, [argument]) => self.ball(*argument)?.atan(),
            (Operator::Atan2, [ordinate, abscissa]) => {
                let abscissa = self.ball(*abscissa)?;
                self.ball(*ordinate)?.atan2(&abscissa)
            }
            _ => Err(BallError::NoEnclosure),
        }
    }
}

fn ball_enclosure(
    pool: &mut ExprPool,
    expression: ExprId,
    significant_digits: u32,
    budget_bits: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Checked<DecimalEnclosure> {
    let prepared = prepare(pool, expression)?;
    let pool: &ExprPool = pool;
    enclose_to_significant_digits(significant_digits, budget_bits, is_cancelled, |step| {
        BallEvaluator {
            pool,
            prepared: &prepared,
            step,
            memo: HashMap::new(),
        }
        .ball(expression)
    })
    .map_err(|error| match error {
        EnclosureError::NoEnclosure { .. } | EnclosureError::NotFinite => {
            not_applicable(expression)
        }
        other => DecimalEnclosureError::Enclosure(other),
    })
}

fn scaled_ball<'step>(
    evaluator: &mut BallEvaluator<'_, 'step>,
    expression: ExprId,
    factor: &Number,
    pi_exponent: i32,
) -> Result<RealBall<'step>, BallError> {
    let step = evaluator.step;
    let value = evaluator.ball(expression)?.mul(&step.exact(factor)?)?;
    if pi_exponent == 0 {
        return Ok(value);
    }
    let pi_power = step.pi()?.power(i64::from(pi_exponent.unsigned_abs()))?;
    if pi_exponent < 0 {
        value.div(&pi_power)
    } else {
        value.mul(&pi_power)
    }
}

fn never_cancelled() -> bool {
    false
}

pub fn scaled_expression_at_least_one(
    pool: &mut ExprPool,
    expression: ExprId,
    factor: &Number,
    pi_exponent: i32,
) -> Option<bool> {
    let prepared = prepare(pool, expression).ok()?;
    let pool: &ExprPool = pool;
    at_least_one(ENCLOSURE_BUDGET_BITS, &never_cancelled, |step| {
        let mut evaluator = BallEvaluator {
            pool,
            prepared: &prepared,
            step,
            memo: HashMap::new(),
        };
        Ok(scaled_ball(&mut evaluator, expression, factor, pi_exponent)?.absolute())
    })
    .ok()
    .flatten()
}

pub fn round_scaled_expression(
    pool: &mut ExprPool,
    expression: ExprId,
    factor: &Number,
    pi_exponent: i32,
    significant_digits: u32,
) -> Option<Number> {
    let prepared = prepare(pool, expression).ok()?;
    let pool: &ExprPool = pool;
    correctly_rounded_to_significant_digits(
        significant_digits,
        ENCLOSURE_BUDGET_BITS,
        &never_cancelled,
        |step| {
            let mut evaluator = BallEvaluator {
                pool,
                prepared: &prepared,
                step,
                memo: HashMap::new(),
            };
            scaled_ball(&mut evaluator, expression, factor, pi_exponent)
        },
    )
    .ok()
}

fn coherent_expression(pool: &mut ExprPool, expression: ExprId) -> Checked<ExprId> {
    to_coherent_units(pool, expression)
        .map(|coherent| coherent.expression)
        .map_err(|error| match error {
            QuantityError::Access(access) => DecimalEnclosureError::Access(access),
            other => DecimalEnclosureError::Quantity(other),
        })
}

pub fn digits_of_expression(
    pool: &mut ExprPool,
    expression: ExprId,
    places: u32,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<DecimalDigits, DecimalEnclosureError> {
    match evaluate_exact(pool, expression) {
        Ok(evaluation) => match evaluation.rational_value() {
            Some(value) => decimal_digits(value, places).map_err(|_| not_applicable(expression)),
            None => ball_digits(
                pool,
                evaluation.expression(),
                places,
                ENCLOSURE_BUDGET_BITS,
                is_cancelled,
            ),
        },
        Err(
            ExactEvaluationError::MachineNumber(_)
            | ExactEvaluationError::UnsupportedNode(_)
            | ExactEvaluationError::UnsupportedOperator { .. }
            | ExactEvaluationError::UnsupportedConstant(_),
        ) => {
            let coherent = coherent_expression(pool, expression)?;
            ball_digits(pool, coherent, places, ENCLOSURE_BUDGET_BITS, is_cancelled)
        }
        Err(
            ExactEvaluationError::DivisionByZero(failed)
            | ExactEvaluationError::CannotDecide {
                expression: failed, ..
            }
            | ExactEvaluationError::NotFinite(failed)
            | ExactEvaluationError::OutsideDomain {
                expression: failed, ..
            }
            | ExactEvaluationError::IntegerForm {
                expression: failed, ..
            }
            | ExactEvaluationError::IntegerArgumentOutOfRange {
                expression: failed, ..
            }
            | ExactEvaluationError::ResultTooLarge {
                expression: failed, ..
            }
            | ExactEvaluationError::NotInRadicalField {
                expression: failed, ..
            }
            | ExactEvaluationError::RemainderNotComputed { expression: failed }
            | ExactEvaluationError::PowerOfZeroWithoutValue { expression: failed }
            | ExactEvaluationError::PowerOfZeroSignUndecided { expression: failed }
            | ExactEvaluationError::NotASquareRootTerm { term: failed, .. },
        ) => Err(not_applicable(failed)),
        Err(ExactEvaluationError::Access(access)) => Err(DecimalEnclosureError::Access(access)),
        Err(ExactEvaluationError::Quantity(quantity)) => {
            Err(DecimalEnclosureError::Quantity(quantity))
        }
        Err(ExactEvaluationError::Build(_)) => Err(not_applicable(expression)),
    }
}

fn ball_digits(
    pool: &mut ExprPool,
    expression: ExprId,
    places: u32,
    budget_bits: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Checked<DecimalDigits> {
    let prepared = prepare(pool, expression)?;
    let pool: &ExprPool = pool;
    truncated_at_places(
        places,
        SIGNIFICANT_DIGITS_LIMIT,
        budget_bits,
        is_cancelled,
        |step| {
            BallEvaluator {
                pool,
                prepared: &prepared,
                step,
                memo: HashMap::new(),
            }
            .ball(expression)
        },
    )
    .map(|digits| DecimalDigits {
        places,
        digits,
        terminates: false,
        remainder: None,
        expansion: Expansion::NotKnownToRecur,
    })
    .map_err(|error| match error {
        EnclosureError::NotFinite => not_applicable(expression),
        other => DecimalEnclosureError::Enclosure(other),
    })
}

pub fn enclose_decimal(
    pool: &mut ExprPool,
    expression: ExprId,
    significant_digits: u32,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<DecimalEnclosure, DecimalEnclosureError> {
    enclose_decimal_within(
        pool,
        expression,
        significant_digits,
        ENCLOSURE_BUDGET_BITS,
        is_cancelled,
    )
}

pub(crate) fn enclose_decimal_within(
    pool: &mut ExprPool,
    expression: ExprId,
    significant_digits: u32,
    budget_bits: usize,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<DecimalEnclosure, DecimalEnclosureError> {
    match evaluate_exact(pool, expression) {
        Ok(evaluation) => match evaluation.rational_value() {
            Some(value) => {
                enclose_exact(value, significant_digits).map_err(DecimalEnclosureError::Enclosure)
            }
            None => ball_enclosure(
                pool,
                evaluation.expression(),
                significant_digits,
                budget_bits,
                is_cancelled,
            ),
        },
        Err(
            ExactEvaluationError::MachineNumber(_)
            | ExactEvaluationError::UnsupportedNode(_)
            | ExactEvaluationError::UnsupportedOperator { .. }
            | ExactEvaluationError::UnsupportedConstant(_),
        ) => {
            let coherent = coherent_expression(pool, expression)?;
            ball_enclosure(
                pool,
                coherent,
                significant_digits,
                budget_bits,
                is_cancelled,
            )
        }
        Err(
            ExactEvaluationError::DivisionByZero(failed)
            | ExactEvaluationError::CannotDecide {
                expression: failed, ..
            }
            | ExactEvaluationError::NotFinite(failed)
            | ExactEvaluationError::OutsideDomain {
                expression: failed, ..
            }
            | ExactEvaluationError::IntegerForm {
                expression: failed, ..
            }
            | ExactEvaluationError::IntegerArgumentOutOfRange {
                expression: failed, ..
            }
            | ExactEvaluationError::ResultTooLarge {
                expression: failed, ..
            }
            | ExactEvaluationError::NotInRadicalField {
                expression: failed, ..
            }
            | ExactEvaluationError::RemainderNotComputed { expression: failed }
            | ExactEvaluationError::PowerOfZeroWithoutValue { expression: failed }
            | ExactEvaluationError::PowerOfZeroSignUndecided { expression: failed }
            | ExactEvaluationError::NotASquareRootTerm { term: failed, .. },
        ) => Err(not_applicable(failed)),
        Err(ExactEvaluationError::Access(access)) => Err(DecimalEnclosureError::Access(access)),
        Err(ExactEvaluationError::Quantity(quantity)) => {
            Err(DecimalEnclosureError::Quantity(quantity))
        }
        Err(ExactEvaluationError::Build(_)) => Err(not_applicable(expression)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_expr::SymbolKind;
    use calc_numbers::Integer;

    use crate::computed_result::ResultKind;

    fn fraction(pool: &mut ExprPool, numerator: i64, denominator: i64) -> ExprId {
        let value =
            Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap();
        pool.number(value).unwrap()
    }

    fn apply(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> ExprId {
        pool.apply(Head::Operator(operator), arguments).unwrap()
    }

    fn decimal(digits: i128, scale: u32) -> Number {
        Number::fraction(&Integer::from(digits), &Integer::from(10_i64).pow(scale)).unwrap()
    }

    fn is_negative_number(value: &Number) -> bool {
        match value {
            Number::Integer(integer) => integer.is_negative(),
            Number::Rational(rational) => rational.numerator().is_negative(),
            Number::F32(_) | Number::F64(_) => true,
        }
    }

    fn kind(pool: &mut ExprPool, expression: ExprId) -> ResultKind {
        evaluate_exact(pool, expression).unwrap().kind()
    }

    fn ends(enclosure: &DecimalEnclosure) -> (Number, Number, bool) {
        (
            enclosure.lower.clone(),
            enclosure.upper.clone(),
            enclosure.reached,
        )
    }

    #[test]
    fn exact_rational_is_rounded_outward() {
        let mut pool = ExprPool::new();
        let third = fraction(&mut pool, 1, 3);

        let enclosure = enclose_decimal(&mut pool, third, 6, &|| false).unwrap();

        assert_eq!(
            ends(&enclosure),
            (decimal(333_333, 6), decimal(333_334, 6), true)
        );
    }

    #[test]
    fn algebraic_result_is_enclosed_from_its_simplified_form() {
        let mut pool = ExprPool::new();
        let eight = fraction(&mut pool, 8, 1);
        let root = apply(&mut pool, Operator::Sqrt, &[eight]);
        assert_eq!(kind(&mut pool, root), ResultKind::Algebraic);

        let enclosure = enclose_decimal(&mut pool, root, 20, &|| false).unwrap();

        assert_eq!(
            ends(&enclosure),
            (
                decimal(28_284_271_247_461_900_976, 19),
                decimal(28_284_271_247_461_900_977, 19),
                true
            )
        );
    }

    #[test]
    fn symbolic_result_is_enclosed() {
        let mut pool = ExprPool::new();
        let one = fraction(&mut pool, 1, 1);
        let sine = apply(&mut pool, Operator::Sin, &[one]);
        let pi = pool.symbol(BuiltinConstant::Pi.symbol()).unwrap();
        let sum = apply(&mut pool, Operator::Add, &[sine, pi]);
        assert_eq!(kind(&mut pool, sum), ResultKind::Symbolic);

        let enclosure = enclose_decimal(&mut pool, sum, 20, &|| false).unwrap();

        assert_eq!(
            ends(&enclosure),
            (
                decimal(39_830_636_383_976_897_451, 19),
                decimal(39_830_636_383_976_897_452, 19),
                true
            )
        );
    }

    #[test]
    fn machine_literal_is_enclosed_as_its_exact_binary_value() {
        let mut pool = ExprPool::new();
        let tenth = pool.number(Number::F64(0.1)).unwrap();
        let three = fraction(&mut pool, 3, 1);
        let product = apply(&mut pool, Operator::Mul, &[tenth, three]);

        let enclosure = enclose_decimal(&mut pool, product, 20, &|| false).unwrap();

        assert_eq!(
            ends(&enclosure),
            (
                decimal(30_000_000_000_000_001_665, 20),
                decimal(30_000_000_000_000_001_666, 20),
                true
            )
        );
    }

    #[test]
    fn value_on_the_grid_without_exact_proof_stops_unreached_at_the_limit() {
        let mut pool = ExprPool::new();
        let two = fraction(&mut pool, 2, 1);
        let logarithm = apply(&mut pool, Operator::Ln, &[two]);
        let exponential = apply(&mut pool, Operator::Exp, &[logarithm]);

        let enclosure = enclose_decimal_within(&mut pool, exponential, 5, 512, &|| false).unwrap();

        let two = Number::from(2_i64);
        let below = two.sub_exact(&enclosure.lower).unwrap();
        let above = enclosure.upper.sub_exact(&two).unwrap();
        assert!(!enclosure.reached);
        assert!(!is_negative_number(&below) && !is_negative_number(&above));
    }

    #[test]
    fn division_by_zero_is_not_applicable() {
        let mut pool = ExprPool::new();
        let one = fraction(&mut pool, 1, 1);
        let zero = fraction(&mut pool, 0, 1);
        let quotient = apply(&mut pool, Operator::Div, &[one, zero]);

        assert!(matches!(
            enclose_decimal(&mut pool, quotient, 5, &|| false),
            Err(DecimalEnclosureError::NotApplicable { .. })
        ));
    }

    #[test]
    fn free_variable_is_not_applicable() {
        let mut pool = ExprPool::new();
        let symbol = pool.intern_symbol("x", SymbolKind::Variable).unwrap();
        let x = pool.symbol(symbol).unwrap();
        let tenth = pool.number(Number::F64(0.1)).unwrap();
        let sum = apply(&mut pool, Operator::Add, &[x, tenth]);

        assert!(matches!(
            enclose_decimal(&mut pool, sum, 5, &|| false),
            Err(DecimalEnclosureError::NotApplicable { .. })
        ));
    }

    #[test]
    fn conversion_encloses_the_machine_result_of_its_rounded_leaves() {
        let mut pool = ExprPool::new();
        let one = fraction(&mut pool, 1, 1);
        let three = fraction(&mut pool, 3, 1);
        let quotient = apply(&mut pool, Operator::Div, &[one, three]);
        let converted = apply(&mut pool, Operator::ToF64, &[quotient]);

        let enclosure = enclose_decimal(&mut pool, converted, 20, &|| false).unwrap();

        assert_eq!(
            ends(&enclosure),
            (
                decimal(33_333_333_333_333_331_482, 20),
                decimal(33_333_333_333_333_331_483, 20),
                true
            )
        );
    }

    #[test]
    fn conversion_of_a_square_root_answers_with_the_machine_root() {
        let mut pool = ExprPool::new();
        let eight = fraction(&mut pool, 8, 1);
        let root = apply(&mut pool, Operator::Sqrt, &[eight]);
        let converted = apply(&mut pool, Operator::ToF64, &[root]);

        let enclosure = enclose_decimal(&mut pool, converted, 20, &|| false).unwrap();

        assert_eq!(
            ends(&enclosure),
            (
                decimal(28_284_271_247_461_902_909, 19),
                decimal(28_284_271_247_461_902_910, 19),
                true
            )
        );
    }

    #[test]
    fn conversion_around_an_approximate_operation_is_not_applicable() {
        let mut pool = ExprPool::new();
        let one = fraction(&mut pool, 1, 1);
        let exponential = apply(&mut pool, Operator::Exp, &[one]);
        let converted = apply(&mut pool, Operator::ToF64, &[exponential]);

        assert!(matches!(
            enclose_decimal(&mut pool, converted, 5, &|| false),
            Err(DecimalEnclosureError::NotApplicable { .. })
        ));
    }

    #[test]
    fn exponent_that_evaluates_to_an_integer_keeps_a_negative_base() {
        let mut pool = ExprPool::new();
        let base = pool.number(Number::F64(-1.5)).unwrap();
        let one = fraction(&mut pool, 1, 1);
        let two = fraction(&mut pool, 2, 1);
        let three = apply(&mut pool, Operator::Add, &[one, two]);
        let power = apply(&mut pool, Operator::Pow, &[base, three]);

        let enclosure = enclose_decimal(&mut pool, power, 2, &|| false).unwrap();

        assert_eq!(ends(&enclosure), (decimal(-34, 1), decimal(-33, 1), true));
    }

    #[test]
    fn significant_digits_above_the_limit_are_an_error() {
        let mut pool = ExprPool::new();
        let third = fraction(&mut pool, 1, 3);

        assert_eq!(
            enclose_decimal(&mut pool, third, 5_001, &|| false),
            Err(DecimalEnclosureError::Enclosure(
                EnclosureError::DigitsOutOfRange {
                    digits: 5_001,
                    limit: 5_000
                }
            ))
        );
    }

    #[test]
    fn enclosure_stops_when_cancelled_mid_refinement() {
        let mut pool = ExprPool::new();
        let two = fraction(&mut pool, 2, 1);
        let logarithm = apply(&mut pool, Operator::Ln, &[two]);
        let exponential = apply(&mut pool, Operator::Exp, &[logarithm]);
        let checks = std::cell::Cell::new(0_u32);
        let is_cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() > 1
        };

        let result = enclose_decimal(&mut pool, exponential, 5, &is_cancelled);

        assert_eq!(
            result,
            Err(DecimalEnclosureError::Enclosure(EnclosureError::Cancelled))
        );
    }

    #[test]
    fn large_magnitude_beyond_the_budget_is_the_size_error() {
        let mut pool = ExprPool::new();
        let one = fraction(&mut pool, 1, 1);
        let sine = apply(&mut pool, Operator::Sin, &[one]);
        let ten = fraction(&mut pool, 10, 1);
        let exponent = fraction(&mut pool, 2000, 1);
        let scale = apply(&mut pool, Operator::Pow, &[ten, exponent]);
        let product = apply(&mut pool, Operator::Mul, &[scale, sine]);

        assert_eq!(
            enclose_decimal_within(&mut pool, product, 5, 1024, &|| false),
            Err(DecimalEnclosureError::Enclosure(
                EnclosureError::OverBudget { budget_bits: 1024 }
            ))
        );
    }

    #[test]
    fn square_root_of_two_is_at_least_one() {
        let mut pool = ExprPool::new();
        let two = fraction(&mut pool, 2, 1);
        let root = apply(&mut pool, Operator::Sqrt, &[two]);

        assert_eq!(
            scaled_expression_at_least_one(&mut pool, root, &Number::from(1_i64), 0),
            Some(true)
        );
    }

    #[test]
    fn square_root_of_two_millimetres_in_kilometres_is_below_one() {
        let mut pool = ExprPool::new();
        let two = fraction(&mut pool, 2, 1);
        let root = apply(&mut pool, Operator::Sqrt, &[two]);
        let per_thousand = Number::fraction(
            &calc_numbers::Integer::from(1_i64),
            &calc_numbers::Integer::from(1000_i64),
        )
        .unwrap();

        assert_eq!(
            scaled_expression_at_least_one(&mut pool, root, &per_thousand, 0),
            Some(false)
        );
    }

    #[test]
    fn square_root_of_two_rounds_to_four_digits() {
        let mut pool = ExprPool::new();
        let two = fraction(&mut pool, 2, 1);
        let root = apply(&mut pool, Operator::Sqrt, &[two]);

        assert_eq!(
            round_scaled_expression(&mut pool, root, &Number::from(1_i64), 0, 4),
            Some(decimal(1414, 3))
        );
    }

    #[test]
    fn scaling_by_a_power_of_pi_is_included() {
        let mut pool = ExprPool::new();
        let one = fraction(&mut pool, 1, 1);

        assert_eq!(
            round_scaled_expression(&mut pool, one, &Number::from(180_i64), -1, 4),
            Some(decimal(5730, 2))
        );
    }
}
