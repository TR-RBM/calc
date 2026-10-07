use std::cmp::Ordering;
use std::collections::HashMap;

use calc_expr::{
    AccessError, BinderKind, BuildError, BuiltinConstant, ExprId, ExprPool, Head,
    IntegerTypeSpelling, LARGEST_INTEGER_WIDTH, LimitSide, NodeView, Operator, SymbolKind,
};
use calc_numbers::{Integer, Number};
use calc_units::UnitId;

use crate::computed_result::ResultKind;
use crate::exact_rational::ExactRational;
use crate::integer_roots::integer_root;
use crate::quantities::{QuantityError, to_coherent_units};
use crate::real_roots::RootRefusal;
use crate::special_angles::{
    TangentOfPiMultiple, arccosine_as_pi_multiple, arcsine_as_pi_multiple,
    arctangent_as_pi_multiple, cosine_of_pi_multiple, sine_of_pi_multiple, tangent_of_pi_multiple,
};
use crate::square_root_sum::SquareRootSum;

const SQUARE_ROOT_DEGREE: u32 = 2;
const LOG10_OF_TWO_NUMERATOR: u64 = 30_103;
const LOG10_OF_TWO_DENOMINATOR: u64 = 100_000;
const LOG2_FRACTION_BITS: u32 = 32;
const NORMALIZED_POINT: u32 = 62;

pub const EXACT_RESULT_BIT_LIMIT: u64 = 1 << 18;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactEvaluation {
    kind: ResultKind,
    expression: ExprId,
    rational_value: Option<Number>,
    pi_power: Option<(Number, i32)>,
    unit: Option<UnitId>,
}

impl ExactEvaluation {
    pub fn kind(&self) -> ResultKind {
        self.kind
    }

    pub fn expression(&self) -> ExprId {
        self.expression
    }

    pub fn rational_value(&self) -> Option<&Number> {
        self.rational_value.as_ref()
    }

    pub fn pi_power(&self) -> Option<(&Number, i32)> {
        self.pi_power
            .as_ref()
            .map(|(coefficient, exponent)| (coefficient, *exponent))
    }

    pub fn unit(&self) -> Option<UnitId> {
        self.unit
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegerProblem {
    NotWhole,
    Negative,
    OutsideType { bits: u32, signed: bool },
    NotWholeBytes { bits: u32, signed: bool },
    ShiftTooLarge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NoFiniteLimit(LimitSide),
    NoEnclosure,
    EnclosureDigitsOutOfRange(u32),
    IntegrandNotAPolynomial,
    IntegrandPoleInInterval,
    IntegrandHighDegreeFactor(u32),
    IntegrandRepeatedQuadratic,
    IntegralBoundNotExact,
    LimitNotARationalFunction,
    Undecided,
    RootNotAPolynomial,
    RootOfZero,
    RootIndexNotWhole,
    RootIndexOutOfRange(u32),
}

impl Refusal {
    pub fn side_name(self) -> Option<&'static str> {
        match self {
            Refusal::NoFiniteLimit(LimitSide::Left) => Some("left"),
            Refusal::NoFiniteLimit(LimitSide::Right) => Some("right"),
            _ => None,
        }
    }

    pub fn digits(self) -> Option<u32> {
        match self {
            Refusal::EnclosureDigitsOutOfRange(digits) => Some(digits),
            _ => None,
        }
    }

    pub fn count(self) -> Option<u32> {
        match self {
            Refusal::RootIndexOutOfRange(count) | Refusal::IntegrandHighDegreeFactor(count) => {
                Some(count)
            }
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Refusal::NoFiniteLimit(_) => "limit_not_finite",
            Refusal::NoEnclosure => "enclosure_not_applicable",
            Refusal::EnclosureDigitsOutOfRange(_) => "enclosure_digits_out_of_range",
            Refusal::IntegrandNotAPolynomial => "integrand_not_a_polynomial",
            Refusal::IntegrandPoleInInterval => "integrand_pole_in_interval",
            Refusal::IntegrandHighDegreeFactor(_) => "integrand_high_degree_factor",
            Refusal::IntegrandRepeatedQuadratic => "integrand_repeated_quadratic",
            Refusal::IntegralBoundNotExact => "integral_bound_not_exact",
            Refusal::LimitNotARationalFunction => "limit_not_a_rational_function",
            Refusal::Undecided => "limit_undecided",
            Refusal::RootNotAPolynomial => "root_not_a_polynomial",
            Refusal::RootOfZero => "root_of_zero",
            Refusal::RootIndexNotWhole => "root_index_not_whole",
            Refusal::RootIndexOutOfRange(0) => "root_none_real",
            Refusal::RootIndexOutOfRange(_) => "root_index_out_of_range",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExactEvaluationError {
    Access(AccessError),
    Build(BuildError),
    MachineNumber(ExprId),
    NotFinite(ExprId),
    UnsupportedNode(ExprId),
    CannotDecide {
        expression: ExprId,
        reason: Refusal,
    },
    Quantity(QuantityError),
    UnsupportedOperator {
        expression: ExprId,
        operator: Operator,
    },
    UnsupportedConstant(ExprId),
    DivisionByZero(ExprId),
    OutsideDomain {
        expression: ExprId,
        operator: Operator,
    },
    IntegerArgumentOutOfRange {
        expression: ExprId,
        operator: Operator,
        argument: Integer,
        limit: u64,
    },
    IntegerForm {
        expression: ExprId,
        problem: IntegerProblem,
    },
    ResultTooLarge {
        expression: ExprId,
        limit_bits: u64,
        estimated_digits: u64,
    },
    NotInRadicalField {
        expression: ExprId,
        operator: Operator,
    },
    RemainderNotComputed {
        expression: ExprId,
    },
    PowerOfZeroWithoutValue {
        expression: ExprId,
    },
    PowerOfZeroSignUndecided {
        expression: ExprId,
    },
    NotASquareRootTerm {
        term: ExprId,
        radicand: Option<Integer>,
    },
}

type Evaluated<T> = Result<T, ExactEvaluationError>;

#[derive(Clone, Debug)]
enum Value {
    Exact(SquareRootSum),
    PiMultiple(ExactRational),
    PiPower {
        coefficient: ExactRational,
        exponent: i32,
    },
    Opaque {
        expression: ExprId,
        is_algebraic: bool,
    },
}

impl Value {
    fn rational(value: ExactRational) -> Self {
        Self::Exact(SquareRootSum::from_rational(value))
    }

    fn integer(value: Integer) -> Self {
        Self::rational(ExactRational::from_integer(value))
    }

    fn pi_multiple(half_turns: ExactRational) -> Self {
        if half_turns.is_zero() {
            Self::Exact(SquareRootSum::zero())
        } else {
            Self::PiMultiple(half_turns)
        }
    }

    fn pi_power(coefficient: ExactRational, exponent: i32) -> Self {
        if coefficient.is_zero() {
            return Self::Exact(SquareRootSum::zero());
        }
        match exponent {
            0 => Self::rational(coefficient),
            1 => Self::PiMultiple(coefficient),
            _ => Self::PiPower {
                coefficient,
                exponent,
            },
        }
    }

    fn is_pi_form(&self) -> bool {
        matches!(self, Self::PiMultiple(_) | Self::PiPower { .. })
    }

    fn as_pi_power(&self) -> Option<(ExactRational, i32)> {
        match self {
            Self::Exact(sum) => sum.as_rational().map(|rational| (rational, 0)),
            Self::PiMultiple(coefficient) => Some((coefficient.clone(), 1)),
            Self::PiPower {
                coefficient,
                exponent,
            } => Some((coefficient.clone(), *exponent)),
            Self::Opaque { .. } => None,
        }
    }

    fn pi_form_sign(&self) -> Option<Ordering> {
        match self {
            Self::PiMultiple(coefficient) | Self::PiPower { coefficient, .. } => {
                Some(coefficient.sign())
            }
            Self::Exact(_) | Self::Opaque { .. } => None,
        }
    }

    fn is_algebraic(&self) -> bool {
        match self {
            Self::Exact(_) => true,
            Self::PiMultiple(_) | Self::PiPower { .. } => false,
            Self::Opaque { is_algebraic, .. } => *is_algebraic,
        }
    }

    fn as_rational(&self) -> Option<ExactRational> {
        match self {
            Self::Exact(sum) => sum.as_rational(),
            Self::PiMultiple(_) | Self::PiPower { .. } | Self::Opaque { .. } => None,
        }
    }

    fn is_exact_zero(&self) -> bool {
        matches!(self, Self::Exact(sum) if sum.is_zero())
    }

    fn angle_as_pi_multiple(&self) -> Option<ExactRational> {
        match self {
            Self::Exact(sum) if sum.is_zero() => Some(ExactRational::zero()),
            Self::PiMultiple(half_turns) => Some(half_turns.clone()),
            Self::Exact(_) | Self::PiPower { .. } | Self::Opaque { .. } => None,
        }
    }
}

fn pi_pair(left: &Value, right: &Value) -> Option<((ExactRational, i32), (ExactRational, i32))> {
    if !left.is_pi_form() && !right.is_pi_form() {
        return None;
    }
    Some((left.as_pi_power()?, right.as_pi_power()?))
}

pub fn evaluate_exact(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Result<ExactEvaluation, ExactEvaluationError> {
    let coherent = to_coherent_units(pool, expression).map_err(|error| match error {
        QuantityError::Access(access) => ExactEvaluationError::Access(access),
        QuantityError::Build(build) => ExactEvaluationError::Build(build),
        other => ExactEvaluationError::Quantity(other),
    })?;
    let mut evaluation = evaluate_coherent(pool, coherent.expression)?;
    evaluation.unit = coherent.unit;
    Ok(evaluation)
}

fn evaluate_coherent(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Result<ExactEvaluation, ExactEvaluationError> {
    let value = evaluate_value(pool, expression)?;
    match &value {
        Value::Exact(sum) => match sum.as_rational() {
            Some(rational) => {
                let number = rational.to_number();
                Ok(ExactEvaluation {
                    kind: ResultKind::ExactRational,
                    expression: pool
                        .number(number.clone())
                        .map_err(ExactEvaluationError::Build)?,
                    rational_value: Some(number),
                    pi_power: None,
                    unit: None,
                })
            }
            None => Ok(ExactEvaluation {
                kind: ResultKind::Algebraic,
                expression: expression_of(pool, &value)?,
                rational_value: None,
                pi_power: None,
                unit: None,
            }),
        },
        Value::PiMultiple(_) | Value::PiPower { .. } | Value::Opaque { .. } => {
            Ok(ExactEvaluation {
                kind: if value.is_algebraic() {
                    ResultKind::Algebraic
                } else {
                    ResultKind::Symbolic
                },
                expression: expression_of(pool, &value)?,
                rational_value: None,
                pi_power: value
                    .as_pi_power()
                    .filter(|(_, exponent)| *exponent != 0)
                    .map(|(coefficient, exponent)| (coefficient.to_number(), exponent)),
                unit: None,
            })
        }
    }
}

pub(crate) fn exact_sign(pool: &mut ExprPool, expression: ExprId) -> Option<Ordering> {
    let coherent = to_coherent_units(pool, expression).ok()?;
    match evaluate_value(pool, coherent.expression).ok()? {
        Value::Exact(sum) => Some(sum.sign()),
        Value::PiMultiple(half_turns) => Some(half_turns.sign()),
        Value::PiPower { coefficient, .. } => Some(coefficient.sign()),
        Value::Opaque { .. } => None,
    }
}

pub(crate) fn closed_square_root_sum(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Option<SquareRootSum> {
    match evaluate_value(pool, expression).ok()? {
        Value::Exact(sum) => Some(sum),
        Value::PiMultiple(_) | Value::PiPower { .. } | Value::Opaque { .. } => None,
    }
}

fn evaluate_value(pool: &mut ExprPool, root: ExprId) -> Evaluated<Value> {
    let mut values: HashMap<ExprId, Value> = HashMap::new();
    let mut pending = vec![(root, false)];
    while let Some((expression, children_are_done)) = pending.pop() {
        if values.contains_key(&expression) {
            continue;
        }
        if children_are_done {
            let value = evaluate_node(pool, expression, &values)?;
            within_size_limit(expression, value_bits(&value))?;
            values.insert(expression, value);
        } else {
            let children = children_to_evaluate(pool, expression)?;
            pending.push((expression, true));
            pending.extend(
                children
                    .into_iter()
                    .rev()
                    .filter(|child| !values.contains_key(child))
                    .map(|child| (child, false)),
            );
        }
    }
    values
        .remove(&root)
        .ok_or(ExactEvaluationError::Access(AccessError::UnknownExprId(
            root,
        )))
}

fn machine_number_argument(pool: &ExprPool, argument: ExprId) -> Evaluated<Option<Number>> {
    match pool.node(argument).map_err(ExactEvaluationError::Access)? {
        NodeView::Number(number) => {
            let value = pool
                .number_value(number)
                .map_err(ExactEvaluationError::Access)?;
            Ok((!value.is_exact()).then(|| value.clone()))
        }
        _ => Ok(None),
    }
}

const REDUCTION_TERM_LIMIT: usize = 100_000;

const REDUCTION_WORK_LIMIT: u64 = 2_000_000;

struct Reduction {
    is_product: bool,
    lower_bound: Integer,
    count: usize,
    body: ExprId,
}

fn integer_constant(pool: &ExprPool, expression: ExprId) -> Option<Integer> {
    match pool.node(expression).ok()? {
        NodeView::Number(number) => match pool.number_value(number).ok()? {
            Number::Integer(integer) => Some(integer.clone()),
            _ => None,
        },
        _ => None,
    }
}

const DERIVATIVE_VARIABLE: &str = "x";

pub(crate) fn differentiated_body(pool: &mut ExprPool, expression: ExprId) -> Option<ExprId> {
    let NodeView::Bind {
        binder: BinderKind::Derivative,
        arguments,
        body,
    } = pool.node(expression).ok()?
    else {
        return None;
    };
    let point = arguments.first().copied();
    let differentiated = crate::derivative::derivative(pool, body, 0)?;
    let replacement = match point {
        Some(point) => point,
        None => {
            let name = pool
                .bound_name(expression)
                .unwrap_or(DERIVATIVE_VARIABLE)
                .to_owned();
            let symbol = pool.intern_symbol(&name, SymbolKind::Variable).ok()?;
            pool.symbol(symbol).ok()?
        }
    };
    let replaced = crate::rule_search::replace_parameters(pool, differentiated, &[replacement], 0)?;
    crate::derivative::folded(pool, replaced)
}

fn reduction_of(pool: &ExprPool, expression: ExprId) -> Option<Reduction> {
    let NodeView::Bind {
        binder,
        arguments,
        body,
    } = pool.node(expression).ok()?
    else {
        return None;
    };
    let is_product = match binder {
        BinderKind::Sum(_) => false,
        BinderKind::Product(_) => true,
        _ => return None,
    };
    let [lower, upper] = arguments else {
        return None;
    };
    let lower_bound = integer_constant(pool, *lower)?;
    let upper_bound = integer_constant(pool, *upper)?;
    let span = (&upper_bound - &lower_bound).to_i64()?;
    let count = usize::try_from(span.checked_add(1)?.max(0))
        .ok()
        .filter(|count| *count <= REDUCTION_TERM_LIMIT)?;
    Some(Reduction {
        is_product,
        lower_bound,
        count,
        body,
    })
}

fn is_whole(value: &Value) -> bool {
    match value {
        Value::Exact(sum) => sum
            .as_rational()
            .is_some_and(|rational| rational.is_integer()),
        Value::PiMultiple(_) | Value::PiPower { .. } | Value::Opaque { .. } => false,
    }
}

fn evaluate_reduction(
    pool: &mut ExprPool,
    expression: ExprId,
    reduction: &Reduction,
) -> Evaluated<Value> {
    let mut total = if reduction.is_product {
        Value::rational(ExactRational::one())
    } else {
        Value::rational(ExactRational::zero())
    };
    let mut work = 0_u64;
    for offset in 0..reduction.count {
        let index = &reduction.lower_bound + &Integer::from(u64::try_from(offset).unwrap_or(0));
        let index = pool
            .number(Number::Integer(index))
            .map_err(ExactEvaluationError::Build)?;
        let term = crate::rule_search::replace_parameters(pool, reduction.body, &[index], 0)
            .ok_or(ExactEvaluationError::UnsupportedNode(expression))?;
        let term = evaluate_value(pool, term)?;
        total = if reduction.is_product {
            multiply(pool, &total, &term)?
        } else {
            add(pool, &total, &term)?
        };
        let bits = value_bits(&total);
        within_size_limit(expression, bits)?;
        if !is_whole(&total) {
            work = work.saturating_add(bits.max(1));
            if work > REDUCTION_WORK_LIMIT {
                return Err(ExactEvaluationError::UnsupportedNode(expression));
            }
        }
    }
    Ok(total)
}

fn children_to_evaluate(pool: &ExprPool, expression: ExprId) -> Evaluated<Vec<ExprId>> {
    match pool
        .node(expression)
        .map_err(ExactEvaluationError::Access)?
    {
        NodeView::Number(_) | NodeView::Symbol(_) => Ok(Vec::new()),
        NodeView::Apply {
            head: Head::Operator(Operator::ToExact),
            arguments: [argument],
        } if machine_number_argument(pool, *argument)?.is_some() => Ok(Vec::new()),
        NodeView::Apply { arguments, .. } => Ok(arguments.to_vec()),
        NodeView::Bind { .. } if reduction_of(pool, expression).is_some() => Ok(Vec::new()),
        NodeView::Bind {
            binder:
                BinderKind::Derivative | BinderKind::Integral | BinderKind::Limit(_) | BinderKind::Root,
            ..
        } => Ok(Vec::new()),
        NodeView::Bound(_)
        | NodeView::Bind { .. }
        | NodeView::Quantity { .. }
        | NodeView::Array { .. } => Err(ExactEvaluationError::UnsupportedNode(expression)),
    }
}

fn evaluate_node(
    pool: &mut ExprPool,
    expression: ExprId,
    values: &HashMap<ExprId, Value>,
) -> Evaluated<Value> {
    let (head, arguments) = match pool
        .node(expression)
        .map_err(ExactEvaluationError::Access)?
    {
        NodeView::Number(number) => {
            let value = pool
                .number_value(number)
                .map_err(ExactEvaluationError::Access)?;
            let exact = value
                .to_exact()
                .map_err(|_| ExactEvaluationError::MachineNumber(expression))?;
            return ExactRational::from_number(&exact)
                .map(Value::rational)
                .ok_or(ExactEvaluationError::MachineNumber(expression));
        }
        NodeView::Symbol(symbol) if symbol == BuiltinConstant::Pi.symbol() => {
            return Ok(Value::PiMultiple(ExactRational::one()));
        }
        NodeView::Symbol(symbol) if symbol == BuiltinConstant::Infinity.symbol() => {
            return Err(ExactEvaluationError::UnsupportedConstant(expression));
        }
        NodeView::Symbol(_) => {
            return Ok(Value::Opaque {
                expression,
                is_algebraic: false,
            });
        }
        NodeView::Apply { head, arguments } => (head, arguments.to_vec()),
        NodeView::Bind {
            binder: BinderKind::Integral,
            ..
        } => {
            let value = match crate::integral::definite_integral(pool, expression) {
                Some(Ok(value)) => value,
                Some(Err(refusal)) => {
                    return Err(ExactEvaluationError::CannotDecide {
                        expression,
                        reason: match refusal {
                            crate::integral::IntegralRefusal::NotAPolynomial => {
                                Refusal::IntegrandNotAPolynomial
                            }
                            crate::integral::IntegralRefusal::PoleInInterval => {
                                Refusal::IntegrandPoleInInterval
                            }
                            crate::integral::IntegralRefusal::HighDegreeFactor(degree) => {
                                Refusal::IntegrandHighDegreeFactor(degree)
                            }
                            crate::integral::IntegralRefusal::RepeatedQuadratic => {
                                Refusal::IntegrandRepeatedQuadratic
                            }
                            crate::integral::IntegralRefusal::BoundNotExact => {
                                Refusal::IntegralBoundNotExact
                            }
                        },
                    });
                }
                None => return Err(ExactEvaluationError::UnsupportedNode(expression)),
            };
            return evaluate_value(pool, value);
        }
        NodeView::Bind {
            binder: BinderKind::Limit(_),
            ..
        } => {
            let value = match crate::limit::limit_value(pool, expression) {
                Some(Ok(value)) => value,
                Some(Err(refusal)) => {
                    return Err(ExactEvaluationError::CannotDecide {
                        expression,
                        reason: match refusal {
                            crate::limit::LimitRefusal::NoFiniteLimit(side) => {
                                Refusal::NoFiniteLimit(side)
                            }
                            crate::limit::LimitRefusal::NotARationalFunction => {
                                Refusal::LimitNotARationalFunction
                            }
                            crate::limit::LimitRefusal::Undecided => Refusal::Undecided,
                        },
                    });
                }
                None => return Err(ExactEvaluationError::UnsupportedNode(expression)),
            };
            return evaluate_value(pool, value);
        }
        NodeView::Bind {
            binder: BinderKind::Derivative,
            ..
        } => {
            let differentiated = differentiated_body(pool, expression)
                .ok_or(ExactEvaluationError::UnsupportedNode(expression))?;
            return evaluate_value(pool, differentiated);
        }
        NodeView::Bind {
            binder: BinderKind::Root,
            ..
        } => {
            let root = crate::real_roots::root_of(pool, expression)
                .ok_or(ExactEvaluationError::UnsupportedNode(expression))?;
            let reason = match root {
                Ok(root) => {
                    return Ok(match root.exact() {
                        Some(value) => Value::rational(value),
                        None => Value::Opaque {
                            expression,
                            is_algebraic: true,
                        },
                    });
                }
                Err(RootRefusal::NotAPolynomial) => Refusal::RootNotAPolynomial,
                Err(RootRefusal::ZeroPolynomial) => Refusal::RootOfZero,
                Err(RootRefusal::IndexNotWhole) => Refusal::RootIndexNotWhole,
                Err(RootRefusal::IndexOutOfRange(count)) => {
                    Refusal::RootIndexOutOfRange(u32::try_from(count).unwrap_or(u32::MAX))
                }
            };
            return Err(ExactEvaluationError::CannotDecide { expression, reason });
        }
        NodeView::Bind { .. } => {
            let reduction = reduction_of(pool, expression)
                .ok_or(ExactEvaluationError::UnsupportedNode(expression))?;
            return evaluate_reduction(pool, expression, &reduction);
        }
        NodeView::Bound(_) | NodeView::Quantity { .. } | NodeView::Array { .. } => {
            return Err(ExactEvaluationError::UnsupportedNode(expression));
        }
    };
    if let (Head::Operator(Operator::ToExact), [argument]) = (head, arguments.as_slice())
        && let Some(machine) = machine_number_argument(pool, *argument)?
    {
        return machine
            .to_exact()
            .ok()
            .as_ref()
            .and_then(ExactRational::from_number)
            .map(Value::rational)
            .ok_or(ExactEvaluationError::NotFinite(expression));
    }
    let arguments: Vec<ExprId> = match head {
        Head::Operator(Operator::EnclosureLower | Operator::EnclosureUpper) => {
            arguments.iter().skip(1).copied().collect()
        }
        _ => arguments,
    };
    let argument_values = arguments
        .iter()
        .map(|argument| {
            values
                .get(argument)
                .cloned()
                .ok_or(ExactEvaluationError::Access(AccessError::UnknownExprId(
                    *argument,
                )))
        })
        .collect::<Evaluated<Vec<Value>>>()?;
    match head {
        Head::Operator(operator) => apply_operator(pool, expression, operator, &argument_values),
        Head::Function(_) => opaque(pool, head, &argument_values, false),
    }
}

fn expression_of(pool: &mut ExprPool, value: &Value) -> Evaluated<ExprId> {
    match value {
        Value::Exact(sum) => square_root_sum_expression(pool, sum),
        Value::PiMultiple(half_turns) => {
            let pi = pool
                .symbol(BuiltinConstant::Pi.symbol())
                .map_err(ExactEvaluationError::Build)?;
            if half_turns.is_one() {
                return Ok(pi);
            }
            let factor = number_node(pool, half_turns)?;
            build(pool, Operator::Mul, &[factor, pi])
        }
        Value::PiPower {
            coefficient,
            exponent,
        } => {
            let pi = pool
                .symbol(BuiltinConstant::Pi.symbol())
                .map_err(ExactEvaluationError::Build)?;
            let power_of_pi = number_node(
                pool,
                &ExactRational::from_integer(Integer::from(i64::from(*exponent))),
            )?;
            let pi_power = build(pool, Operator::Pow, &[pi, power_of_pi])?;
            let factor = number_node(pool, coefficient)?;
            build(pool, Operator::Mul, &[factor, pi_power])
        }
        Value::Opaque { expression, .. } => Ok(*expression),
    }
}

fn number_node(pool: &mut ExprPool, value: &ExactRational) -> Evaluated<ExprId> {
    pool.number(value.to_number())
        .map_err(ExactEvaluationError::Build)
}

fn build(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> Evaluated<ExprId> {
    pool.apply(Head::Operator(operator), arguments)
        .map_err(ExactEvaluationError::Build)
}

pub(crate) fn least_common_denominator(sum: &SquareRootSum) -> Integer {
    let mut shared = Integer::one();
    for (_, coefficient) in sum.terms() {
        let denominator = coefficient.denominator();
        let divisor = shared.gcd(denominator);
        shared = match shared.div_rem_euclid(&divisor) {
            Ok((quotient, _)) => &quotient * denominator,
            Err(_) => shared,
        };
    }
    shared
}

pub(crate) fn square_root_sum_expression(
    pool: &mut ExprPool,
    sum: &SquareRootSum,
) -> Evaluated<ExprId> {
    if let Some(rational) = sum.as_rational() {
        return number_node(pool, &rational);
    }
    let denominator = least_common_denominator(sum);
    let scale = ExactRational::from_integer(denominator.clone());
    let mut accumulated: Option<ExprId> = None;
    for (radicand, coefficient) in sum.terms() {
        let scaled = coefficient.multiply(&scale);
        let subtracts = accumulated.is_some() && scaled.sign() == Ordering::Less;
        let written_coefficient = if subtracts { scaled.absolute() } else { scaled };
        let term = term_expression(pool, radicand, &written_coefficient)?;
        accumulated = Some(match accumulated {
            None => term,
            Some(left) if subtracts => build(pool, Operator::Sub, &[left, term])?,
            Some(left) => build(pool, Operator::Add, &[left, term])?,
        });
    }
    let Some(expression) = accumulated else {
        return number_node(pool, &ExactRational::zero());
    };
    if denominator.is_one() {
        return Ok(expression);
    }
    let divisor = number_node(pool, &ExactRational::from_integer(denominator))?;
    build(pool, Operator::Div, &[expression, divisor])
}

fn term_expression(
    pool: &mut ExprPool,
    radicand: &Integer,
    coefficient: &ExactRational,
) -> Evaluated<ExprId> {
    if radicand.is_one() {
        return number_node(pool, coefficient);
    }
    let radicand_node = number_node(pool, &ExactRational::from_integer(radicand.clone()))?;
    let root = build(pool, Operator::Sqrt, &[radicand_node])?;
    if coefficient.is_one() {
        return Ok(root);
    }
    if coefficient.negated().is_one() {
        return build(pool, Operator::Neg, &[root]);
    }
    let factor = number_node(pool, coefficient)?;
    build(pool, Operator::Mul, &[factor, root])
}

fn exact_rational_of(value: &Value) -> Option<ExactRational> {
    match value {
        Value::Exact(sum) => sum.as_rational(),
        _ => None,
    }
}

fn is_exactly(value: &Value, wanted: &ExactRational) -> bool {
    exact_rational_of(value).is_some_and(|rational| rational == *wanted)
}

fn negated_product(pool: &mut ExprPool, expression: ExprId) -> Option<ExprId> {
    let NodeView::Apply {
        head: Head::Operator(Operator::Mul),
        arguments: [left, right],
    } = pool.node(expression).ok()?
    else {
        return None;
    };
    let (left, right) = (*left, *right);
    let NodeView::Number(number) = pool.node(left).ok()? else {
        return None;
    };
    let factor = ExactRational::from_number(pool.number_value(number).ok()?)?;
    let negated = pool.number(factor.negated().to_number()).ok()?;
    pool.apply(Head::Operator(Operator::Mul), &[negated, right])
        .ok()
}

fn negated(pool: &mut ExprPool, expression: ExprId) -> Option<ExprId> {
    if let Some(written) = negated_product(pool, expression) {
        return Some(written);
    }
    pool.apply(Head::Operator(Operator::Neg), &[expression])
        .ok()
}

fn written_form(
    pool: &mut ExprPool,
    head: Head,
    arguments: &[Value],
    expressions: &[ExprId],
) -> Option<ExprId> {
    let Head::Operator(operator) = head else {
        return None;
    };
    let one = ExactRational::one();
    let zero = ExactRational::zero();
    let minus_one = one.negated();
    match (operator, arguments, expressions) {
        (Operator::Add, [left, right], [written_left, written_right]) => {
            if is_exactly(left, &zero) {
                return Some(*written_right);
            }
            is_exactly(right, &zero).then_some(*written_left)
        }
        (Operator::Sub, [_, right], [written_left, _]) => {
            is_exactly(right, &zero).then_some(*written_left)
        }
        (Operator::Mul, [left, right], [written_left, written_right]) => {
            if is_exactly(left, &one) {
                return Some(*written_right);
            }
            if is_exactly(right, &one) {
                return Some(*written_left);
            }
            if is_exactly(left, &minus_one) {
                return negated(pool, *written_right);
            }
            if is_exactly(right, &minus_one) {
                return negated(pool, *written_left);
            }
            None
        }
        (Operator::Div | Operator::Pow, [_, right], [written_left, _]) => {
            is_exactly(right, &one).then_some(*written_left)
        }
        (Operator::Neg, [_], [written]) => negated_product(pool, *written),
        _ => None,
    }
}

fn opaque(
    pool: &mut ExprPool,
    head: Head,
    arguments: &[Value],
    is_algebraic: bool,
) -> Evaluated<Value> {
    let argument_expressions = arguments
        .iter()
        .map(|argument| expression_of(pool, argument))
        .collect::<Evaluated<Vec<ExprId>>>()?;
    let expression = match written_form(pool, head, arguments, &argument_expressions) {
        Some(written) => written,
        None => pool
            .apply(head, &argument_expressions)
            .map_err(ExactEvaluationError::Build)?,
    };
    Ok(Value::Opaque {
        expression,
        is_algebraic,
    })
}

fn opaque_operator(
    pool: &mut ExprPool,
    operator: Operator,
    arguments: &[Value],
    is_algebraic: bool,
) -> Evaluated<Value> {
    opaque(pool, Head::Operator(operator), arguments, is_algebraic)
}

fn algebraic_opaque(
    pool: &mut ExprPool,
    operator: Operator,
    arguments: &[Value],
) -> Evaluated<Value> {
    let is_algebraic = arguments.iter().all(Value::is_algebraic);
    opaque_operator(pool, operator, arguments, is_algebraic)
}

fn enclosure_endpoint(
    pool: &mut ExprPool,
    expression: ExprId,
    operator: Operator,
    digits: &Value,
) -> Evaluated<Value> {
    let refused = |reason| ExactEvaluationError::CannotDecide { expression, reason };
    let significant_digits = whole_digits(digits).ok_or_else(|| {
        refused(Refusal::EnclosureDigitsOutOfRange(whole_digits_shown(
            digits,
        )))
    })?;
    let NodeView::Apply { arguments, .. } = pool
        .node(expression)
        .map_err(ExactEvaluationError::Access)?
    else {
        return Err(ExactEvaluationError::UnsupportedNode(expression));
    };
    let value = *arguments.first().ok_or(refused(Refusal::NoEnclosure))?;
    let enclosure =
        crate::decimal_enclosure::enclose_decimal(pool, value, significant_digits, &|| false)
            .map_err(|error| match error {
                crate::decimal_enclosure::DecimalEnclosureError::Enclosure(
                    calc_numbers::EnclosureError::DigitsOutOfRange { digits, .. },
                ) => refused(Refusal::EnclosureDigitsOutOfRange(digits)),
                _ => refused(Refusal::NoEnclosure),
            })?;
    let endpoint = match operator {
        Operator::EnclosureUpper => enclosure.upper,
        _ => enclosure.lower,
    };
    ExactRational::from_number(&endpoint)
        .map(Value::rational)
        .ok_or(refused(Refusal::NoEnclosure))
}

fn whole_digits(digits: &Value) -> Option<u32> {
    let rational = digits.as_rational()?;
    if !rational.is_integer() {
        return None;
    }
    u32::try_from(rational.numerator().to_i64()?).ok()
}

fn whole_digits_shown(digits: &Value) -> u32 {
    whole_digits(digits).unwrap_or_default()
}

fn apply_operator(
    pool: &mut ExprPool,
    expression: ExprId,
    operator: Operator,
    arguments: &[Value],
) -> Evaluated<Value> {
    let outside_domain = ExactEvaluationError::OutsideDomain {
        expression,
        operator,
    };
    match (operator, arguments) {
        (Operator::Add, [left, right]) => add(pool, left, right),
        (Operator::Sub, [left, right]) => subtract(pool, left, right),
        (Operator::Mul, [left, right]) => multiply(pool, left, right),
        (Operator::Div, [left, right]) => divide(pool, expression, left, right),
        (Operator::Neg, [argument]) => negate(pool, argument),
        (Operator::MulAdd, [left, right, addend]) => {
            let product = multiply(pool, left, right)?;
            add(pool, &product, addend)
        }
        (Operator::Pow, [base, exponent]) => power(pool, expression, base, exponent),
        (Operator::Sqrt, [argument]) => square_root(pool, argument, outside_domain),
        (Operator::Abs, [argument]) => absolute(pool, argument),
        (
            Operator::Floor
            | Operator::Ceil
            | Operator::Trunc
            | Operator::RoundTiesEven
            | Operator::Round,
            [argument],
        ) => round(pool, operator, argument),
        (Operator::Percent, [argument]) => per_hundred(pool, argument),
        (Operator::Sinh | Operator::Cosh | Operator::Tanh, [argument]) => {
            hyperbolic(pool, operator, argument)
        }
        (Operator::Binomial, [count, chosen]) => binomial(pool, expression, count, chosen),
        (Operator::CopySign, [magnitude, sign]) => copy_sign(pool, magnitude, sign),
        (Operator::Min | Operator::Max, [left, right]) => extremum(pool, operator, left, right),
        (Operator::Factorial, [argument]) => factorial(pool, expression, argument),
        (
            Operator::BitAnd
            | Operator::BitOr
            | Operator::BitXor
            | Operator::ShiftLeft
            | Operator::ShiftRight,
            [left, right],
        ) => integer_bits(expression, operator, left, right),
        (Operator::BitNot | Operator::Wrap | Operator::InType, [value, bits, signed]) => {
            typed_integer(expression, operator, value, bits, signed)
        }
        (Operator::Radix, [value, _base, bits, signed]) => {
            typed_integer(expression, operator, value, bits, signed)
        }
        (Operator::Bytes, [value, bits, signed, _order]) => {
            typed_integer(expression, operator, value, bits, signed)
        }
        (Operator::Gcd | Operator::Lcm | Operator::Mod, [left, right]) => {
            whole_number_pair(pool, expression, operator, left, right)
        }
        (Operator::Exp, [argument]) => exponential(pool, argument),
        (Operator::Ln, [argument]) => logarithm(pool, argument, outside_domain),
        (Operator::Log2, [argument]) => {
            based_logarithm(pool, expression, operator, argument, &Integer::from(2_i64))
        }
        (Operator::Log10, [argument]) => {
            based_logarithm(pool, expression, operator, argument, &Integer::from(10_i64))
        }
        (Operator::Log, [argument, base]) => {
            let whole_base = match base {
                Value::Exact(sum) => sum
                    .as_rational()
                    .filter(|rational| rational.is_integer())
                    .map(|rational| rational.numerator().clone()),
                _ => None,
            };
            match whole_base {
                Some(whole_base) => {
                    based_logarithm(pool, expression, operator, argument, &whole_base)
                }
                None => quotient_of_logarithms(pool, expression, argument, base),
            }
        }
        (Operator::Sin | Operator::Cos | Operator::Tan, [argument]) => {
            trigonometric(pool, operator, argument, outside_domain)
        }
        (Operator::Asin | Operator::Acos | Operator::Atan, [argument]) => {
            inverse_trigonometric(pool, operator, argument, outside_domain)
        }
        (Operator::Atan2, [ordinate, abscissa]) => {
            two_argument_arctangent(pool, ordinate, abscissa, outside_domain)
        }
        (Operator::ToExact, [argument]) => Ok(argument.clone()),
        (Operator::EnclosureLower | Operator::EnclosureUpper, [digits]) => {
            enclosure_endpoint(pool, expression, operator, digits)
        }
        (Operator::RationalPart | Operator::CoefficientOf, _)
            if taken_apart_outside_the_field(pool, expression).is_some() =>
        {
            Err(ExactEvaluationError::NotInRadicalField {
                expression,
                operator,
            })
        }
        (Operator::RealPart | Operator::Conjugate | Operator::ImaginaryPart, _)
            if holds_the_imaginary_unit(pool, expression) =>
        {
            Err(ExactEvaluationError::UnsupportedOperator {
                expression,
                operator,
            })
        }
        (Operator::RealPart | Operator::Conjugate, [value]) => Ok(value.clone()),
        (Operator::ImaginaryPart, [_]) => Ok(Value::rational(ExactRational::zero())),
        (Operator::RationalPart, [Value::Exact(sum)]) => Ok(Value::Exact(
            SquareRootSum::from_rational(sum.rational_part()),
        )),
        (Operator::CoefficientOf, [value, term]) => coefficient_of(pool, expression, value, term),
        (Operator::RationalPart, [_]) => Err(ExactEvaluationError::NotInRadicalField {
            expression,
            operator,
        }),
        _ => Err(ExactEvaluationError::UnsupportedOperator {
            expression,
            operator,
        }),
    }
}

fn add(pool: &mut ExprPool, left: &Value, right: &Value) -> Evaluated<Value> {
    match (left, right) {
        (Value::Exact(left_sum), Value::Exact(right_sum)) => {
            Ok(Value::Exact(left_sum.plus(right_sum)))
        }
        _ if let Some(((left_coefficient, left_exponent), (right_coefficient, right_exponent))) =
            pi_pair(left, right)
            && left_exponent == right_exponent =>
        {
            Ok(Value::pi_power(
                left_coefficient.plus(&right_coefficient),
                left_exponent,
            ))
        }
        (zero, other) | (other, zero) if zero.is_exact_zero() => Ok(other.clone()),
        _ => algebraic_opaque(pool, Operator::Add, &[left.clone(), right.clone()]),
    }
}

fn subtract(pool: &mut ExprPool, left: &Value, right: &Value) -> Evaluated<Value> {
    match (left, right) {
        (Value::Exact(left_sum), Value::Exact(right_sum)) => {
            Ok(Value::Exact(left_sum.minus(right_sum)))
        }
        _ if let Some(((left_coefficient, left_exponent), (right_coefficient, right_exponent))) =
            pi_pair(left, right)
            && left_exponent == right_exponent =>
        {
            Ok(Value::pi_power(
                left_coefficient.subtract(&right_coefficient),
                left_exponent,
            ))
        }
        (other, zero) if zero.is_exact_zero() => Ok(other.clone()),
        _ => algebraic_opaque(pool, Operator::Sub, &[left.clone(), right.clone()]),
    }
}

fn multiply(pool: &mut ExprPool, left: &Value, right: &Value) -> Evaluated<Value> {
    match (left, right, left.as_rational(), right.as_rational()) {
        (Value::Exact(left_sum), Value::Exact(right_sum), _, _) => {
            Ok(Value::Exact(left_sum.times(right_sum)))
        }
        _ if let Some(((left_coefficient, left_exponent), (right_coefficient, right_exponent))) =
            pi_pair(left, right)
            && let Some(exponent) = left_exponent.checked_add(right_exponent) =>
        {
            Ok(Value::pi_power(
                left_coefficient.multiply(&right_coefficient),
                exponent,
            ))
        }
        _ => algebraic_opaque(pool, Operator::Mul, &[left.clone(), right.clone()]),
    }
}

fn divide(
    pool: &mut ExprPool,
    expression: ExprId,
    left: &Value,
    right: &Value,
) -> Evaluated<Value> {
    if right.is_exact_zero() {
        return Err(ExactEvaluationError::DivisionByZero(expression));
    }
    let division_by_zero = || ExactEvaluationError::DivisionByZero(expression);
    match (left, right, right.as_rational()) {
        (Value::Exact(left_sum), Value::Exact(right_sum), _) => left_sum
            .divided_by(right_sum)
            .map(Value::Exact)
            .ok_or_else(division_by_zero),
        _ if let Some(((left_coefficient, left_exponent), (right_coefficient, right_exponent))) =
            pi_pair(left, right)
            && let Some(exponent) = left_exponent.checked_sub(right_exponent) =>
        {
            left_coefficient
                .divide(&right_coefficient)
                .map(|coefficient| Value::pi_power(coefficient, exponent))
                .ok_or_else(division_by_zero)
        }
        _ => algebraic_opaque(pool, Operator::Div, &[left.clone(), right.clone()]),
    }
}

fn negate(pool: &mut ExprPool, argument: &Value) -> Evaluated<Value> {
    match argument {
        Value::Exact(sum) => Ok(Value::Exact(sum.negated())),
        Value::PiMultiple(turns) => Ok(Value::pi_multiple(turns.negated())),
        Value::PiPower {
            coefficient,
            exponent,
        } => Ok(Value::pi_power(coefficient.negated(), *exponent)),
        Value::Opaque { .. } => {
            algebraic_opaque(pool, Operator::Neg, std::slice::from_ref(argument))
        }
    }
}

pub fn estimated_digits_of(bits: u64) -> u64 {
    estimated_digits(bits)
}

fn estimated_digits(bits: u64) -> u64 {
    bits.saturating_mul(LOG10_OF_TWO_NUMERATOR) / LOG10_OF_TWO_DENOMINATOR + 1
}

pub(crate) fn sum_within_size_limit_of(
    expression: ExprId,
    sum: &SquareRootSum,
) -> Result<(), ExactEvaluationError> {
    within_size_limit(expression, sum_bits(sum))
}

fn within_size_limit(expression: ExprId, bits: u64) -> Evaluated<()> {
    within_estimated_size_limit(expression, bits, || estimated_digits(bits))
}

fn within_estimated_size_limit(
    expression: ExprId,
    lower_bound_bits: u64,
    digits: impl FnOnce() -> u64,
) -> Evaluated<()> {
    if lower_bound_bits > EXACT_RESULT_BIT_LIMIT {
        return Err(ExactEvaluationError::ResultTooLarge {
            expression,
            limit_bits: EXACT_RESULT_BIT_LIMIT,
            estimated_digits: digits(),
        });
    }
    Ok(())
}

fn fixed_point_log2(value: &Integer) -> u128 {
    let bits = value.bit_length();
    let Some(highest_bit) = bits.checked_sub(1) else {
        return 0;
    };
    let shift = bits.saturating_sub(u64::from(NORMALIZED_POINT + 1));
    let top = u32::try_from(shift)
        .ok()
        .and_then(|shift| {
            value
                .absolute()
                .div_rem_euclid(&Integer::from(2_i64).pow(shift))
                .ok()
        })
        .and_then(|(quotient, _)| quotient.to_i128())
        .and_then(|quotient| u128::try_from(quotient).ok())
        .unwrap_or(0);
    let top_highest_bit = u64::from(u128::BITS - 1 - top.leading_zeros());
    let normalization = u64::from(NORMALIZED_POINT) - top_highest_bit;
    let mut normalized = top << normalization;
    let mut fraction = 0_u128;
    for _ in 0..LOG2_FRACTION_BITS {
        normalized = (normalized * normalized) >> NORMALIZED_POINT;
        fraction <<= 1;
        if normalized >> (NORMALIZED_POINT + 1) != 0 {
            normalized >>= 1;
            fraction |= 1;
        }
    }
    (u128::from(highest_bit) << LOG2_FRACTION_BITS) | fraction
}

fn digits_of_power(log2_of_base: u128, exponent: u32) -> u64 {
    let scaled = log2_of_base
        .saturating_mul(u128::from(exponent))
        .saturating_mul(u128::from(LOG10_OF_TWO_NUMERATOR))
        / u128::from(LOG10_OF_TWO_DENOMINATOR);
    let digits = scaled >> LOG2_FRACTION_BITS;
    u64::try_from(digits).unwrap_or(u64::MAX).saturating_add(1)
}

fn rational_bits(value: &ExactRational) -> u64 {
    value
        .numerator()
        .bit_length()
        .max(value.denominator().bit_length())
}

pub(crate) fn sum_within_size_limit(sum: &SquareRootSum) -> bool {
    sum_bits(sum) <= EXACT_RESULT_BIT_LIMIT
}

fn sum_bits(sum: &SquareRootSum) -> u64 {
    sum.terms()
        .map(|(radicand, coefficient)| radicand.bit_length().max(rational_bits(coefficient)))
        .max()
        .unwrap_or(0)
}

fn value_bits(value: &Value) -> u64 {
    match value {
        Value::Exact(sum) => sum_bits(sum),
        Value::PiMultiple(coefficient) | Value::PiPower { coefficient, .. } => {
            rational_bits(coefficient)
        }
        Value::Opaque { .. } => 0,
    }
}

fn argument_out_of_range(
    expression: ExprId,
    operator: Operator,
    argument: &Integer,
) -> ExactEvaluationError {
    ExactEvaluationError::IntegerArgumentOutOfRange {
        expression,
        operator,
        argument: argument.clone(),
        limit: u64::from(u32::MAX),
    }
}

fn exponent_magnitude(expression: ExprId, value: &Integer) -> Evaluated<u32> {
    value
        .absolute()
        .to_i64()
        .and_then(|magnitude| u32::try_from(magnitude).ok())
        .ok_or_else(|| argument_out_of_range(expression, Operator::Pow, value))
}

fn rational_power_size_lower_bound(base: &ExactRational, exponent: u32) -> u64 {
    match rational_bits(base).checked_sub(1) {
        Some(bits_below_top) => bits_below_top.saturating_mul(u64::from(exponent)) + 1,
        None => 0,
    }
}

fn size_checked_power(
    expression: ExprId,
    base: &SquareRootSum,
    exponent: u32,
) -> Evaluated<SquareRootSum> {
    let mut result = SquareRootSum::from_rational(ExactRational::one());
    let mut square = base.clone();
    let mut remaining = exponent;
    while remaining > 0 {
        if remaining % 2 == 1 {
            result = result.times(&square);
            within_size_limit(expression, sum_bits(&result))?;
        }
        remaining /= 2;
        if remaining > 0 {
            square = square.times(&square);
            within_size_limit(expression, sum_bits(&square))?;
        }
    }
    Ok(result)
}

fn integer_power(
    expression: ExprId,
    base: &SquareRootSum,
    exponent: &Integer,
) -> Evaluated<SquareRootSum> {
    let magnitude = exponent_magnitude(expression, exponent)?;
    let oriented = if exponent.is_negative() {
        base.reciprocal()
            .ok_or(ExactEvaluationError::DivisionByZero(expression))?
    } else {
        base.clone()
    };
    let Some(rational) = oriented.as_rational() else {
        return size_checked_power(expression, &oriented, magnitude);
    };
    within_estimated_size_limit(
        expression,
        rational_power_size_lower_bound(&rational, magnitude),
        || {
            digits_of_power(
                fixed_point_log2(rational.numerator())
                    .max(fixed_point_log2(rational.denominator())),
                magnitude,
            )
        },
    )?;
    Ok(oriented.power(magnitude))
}

fn power(
    pool: &mut ExprPool,
    expression: ExprId,
    base: &Value,
    exponent: &Value,
) -> Evaluated<Value> {
    let outside_domain = || ExactEvaluationError::OutsideDomain {
        expression,
        operator: Operator::Pow,
    };
    let arguments = [base.clone(), exponent.clone()];
    if let Value::Exact(base_sum) = base
        && base_sum.is_zero()
        && !exponent
            .as_rational()
            .is_some_and(|rational| rational.is_integer())
    {
        let sign = match exponent {
            Value::Exact(exponent_sum) => Some(exponent_sum.sign()),
            Value::Opaque {
                expression: exponent_expression,
                ..
            } => match constant_sign(pool, *exponent_expression) {
                ConstantSign::Decided(sign) => Some(sign),
                ConstantSign::Undecided => {
                    return Err(ExactEvaluationError::PowerOfZeroSignUndecided { expression });
                }
                ConstantSign::NotAConstant => None,
            },
            _ => exponent.pi_form_sign(),
        };
        match sign {
            Some(Ordering::Greater) => return Ok(Value::rational(ExactRational::zero())),
            Some(Ordering::Less) => return Err(ExactEvaluationError::DivisionByZero(expression)),
            _ => {}
        }
    }
    let Some(exponent_rational) = exponent.as_rational() else {
        return match (base, exponent) {
            (Value::Exact(base_sum), Value::Exact(_)) => {
                if base_sum
                    .as_rational()
                    .is_some_and(|rational| rational.is_one())
                {
                    return Ok(Value::rational(ExactRational::one()));
                }
                if base_sum.sign() != Ordering::Greater {
                    return Err(outside_domain());
                }
                opaque_operator(pool, Operator::Pow, &arguments, false)
            }
            _ if value_sign(pool, base) == Some(Ordering::Less) => Err(outside_domain()),
            _ => opaque_operator(pool, Operator::Pow, &arguments, false),
        };
    };
    if exponent_rational.is_integer() {
        let exponent_integer = exponent_rational.numerator();
        return match base {
            Value::Exact(base_sum) => {
                integer_power(expression, base_sum, exponent_integer).map(Value::Exact)
            }
            _ if exponent_integer.is_zero() => Ok(Value::rational(ExactRational::one())),
            _ if exponent_integer.is_one() => Ok(base.clone()),
            _ if let Some((coefficient, pi_exponent)) = base.as_pi_power() => {
                let power_of_pi = exponent_integer
                    .to_i64()
                    .and_then(|power| i32::try_from(power).ok())
                    .and_then(|power| pi_exponent.checked_mul(power))
                    .ok_or_else(|| {
                        argument_out_of_range(expression, Operator::Pow, exponent_integer)
                    })?;
                let powered = integer_power(
                    expression,
                    &SquareRootSum::from_rational(coefficient),
                    exponent_integer,
                )?;
                let coefficient = powered
                    .as_rational()
                    .ok_or(ExactEvaluationError::DivisionByZero(expression))?;
                Ok(Value::pi_power(coefficient, power_of_pi))
            }
            _ => algebraic_opaque(pool, Operator::Pow, &arguments),
        };
    }
    match base {
        Value::Exact(base_sum) => {
            if base_sum.sign() != Ordering::Greater {
                return Err(outside_domain());
            }
            let Some(base_rational) = base_sum.as_rational() else {
                return opaque_operator(pool, Operator::Pow, &arguments, true);
            };
            let degree = exponent_magnitude(expression, exponent_rational.denominator())?;
            let root = if degree == SQUARE_ROOT_DEGREE {
                SquareRootSum::square_root_of_rational(&base_rational)
            } else {
                rational_root(&base_rational, degree).map(SquareRootSum::from_rational)
            };
            match root {
                Some(root) => integer_power(expression, &root, exponent_rational.numerator())
                    .map(Value::Exact),
                None => opaque_operator(pool, Operator::Pow, &arguments, true),
            }
        }
        _ if value_sign(pool, base) == Some(Ordering::Less) => Err(outside_domain()),
        _ => algebraic_opaque(pool, Operator::Pow, &arguments),
    }
}

fn value_sign(pool: &ExprPool, value: &Value) -> Option<Ordering> {
    match value {
        Value::Exact(sum) => Some(sum.sign()),
        Value::Opaque { expression, .. } => enclosed_sign(pool, *expression),
        _ => value.pi_form_sign(),
    }
}

fn enclosed_sign(pool: &ExprPool, expression: ExprId) -> Option<Ordering> {
    match crate::machine_evaluation::enclose_sample(pool, expression, &[]) {
        crate::machine_evaluation::SampleEnclosure::Real(Some(interval)) => {
            if interval.lower() > 0.0 {
                Some(Ordering::Greater)
            } else if interval.upper() < 0.0 {
                Some(Ordering::Less)
            } else {
                None
            }
        }
        _ => None,
    }
}

enum ConstantSign {
    Decided(Ordering),
    Undecided,
    NotAConstant,
}

fn constant_sign(pool: &mut ExprPool, expression: ExprId) -> ConstantSign {
    if holds_a_name(pool, expression) {
        return ConstantSign::NotAConstant;
    }
    if let crate::machine_evaluation::SampleEnclosure::Real(Some(interval)) =
        crate::machine_evaluation::enclose_sample(pool, expression, &[])
    {
        if interval.lower() > 0.0 {
            return ConstantSign::Decided(Ordering::Greater);
        }
        if interval.upper() < 0.0 {
            return ConstantSign::Decided(Ordering::Less);
        }
    }
    match crate::claim::decimal_sign(pool, expression) {
        Some(sign @ (Ordering::Greater | Ordering::Less)) => ConstantSign::Decided(sign),
        _ => ConstantSign::Undecided,
    }
}

fn holds_a_name(pool: &ExprPool, expression: ExprId) -> bool {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        match pool.node(node) {
            Ok(NodeView::Symbol(symbol)) if !crate::polynomial::is_a_constant(pool, symbol) => {
                return true;
            }
            Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().copied()),
            Ok(NodeView::Bind {
                arguments, body, ..
            }) => {
                pending.extend(arguments.iter().copied());
                pending.push(body);
            }
            Ok(NodeView::Quantity { value, .. }) => pending.push(value),
            Ok(NodeView::Array { elements, .. }) => pending.extend(elements.iter().copied()),
            _ => {}
        }
    }
    false
}

fn rational_root(value: &ExactRational, degree: u32) -> Option<ExactRational> {
    let (numerator_root, numerator_is_exact) = integer_root(value.numerator(), degree);
    let (denominator_root, denominator_is_exact) = integer_root(value.denominator(), degree);
    if numerator_is_exact && denominator_is_exact {
        ExactRational::fraction(&numerator_root, &denominator_root)
    } else {
        None
    }
}

fn is_outside_the_field(pool: &ExprPool, expression: ExprId) -> bool {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        match pool.node(node) {
            Ok(NodeView::Number(number)) => {
                if pool
                    .number_value(number)
                    .is_ok_and(|value| !value.is_exact())
                {
                    return true;
                }
            }
            Ok(NodeView::Symbol(symbol)) => {
                if !BuiltinConstant::ALL
                    .iter()
                    .any(|constant| constant.symbol() == symbol)
                {
                    return true;
                }
            }
            Ok(NodeView::Apply {
                head: Head::Operator(Operator::ToF64 | Operator::ToF32),
                ..
            }) => return true,
            Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().copied()),
            _ => {}
        }
    }
    false
}

pub(crate) fn holds_the_imaginary_unit(pool: &ExprPool, expression: ExprId) -> bool {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        match pool.node(node) {
            Ok(NodeView::Symbol(symbol)) if symbol == BuiltinConstant::ImaginaryUnit.symbol() => {
                return true;
            }
            Ok(NodeView::Apply {
                head: Head::Operator(Operator::Complex),
                ..
            }) => return true,
            Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().copied()),
            Ok(NodeView::Bind {
                arguments, body, ..
            }) => {
                pending.extend(arguments.iter().copied());
                pending.push(body);
            }
            _ => {}
        }
    }
    false
}

pub fn taken_apart_outside_the_field(
    pool: &ExprPool,
    expression: ExprId,
) -> Option<ExactEvaluationError> {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        let Ok(NodeView::Apply { head, arguments }) = pool.node(node) else {
            continue;
        };
        if let Head::Operator(operator @ (Operator::RationalPart | Operator::CoefficientOf)) = head
            && arguments
                .first()
                .is_some_and(|value| is_outside_the_field(pool, *value))
        {
            return Some(ExactEvaluationError::NotInRadicalField {
                expression: node,
                operator,
            });
        }
        pending.extend(arguments.iter().copied());
    }
    None
}

fn coefficient_of(
    pool: &ExprPool,
    expression: ExprId,
    value: &Value,
    term: &Value,
) -> Evaluated<Value> {
    let term_node = match pool.node(expression) {
        Ok(NodeView::Apply { arguments, .. }) => arguments.get(1).copied(),
        _ => None,
    }
    .unwrap_or(expression);
    let radicand = match term {
        Value::Exact(sum) => sum.as_single_root(),
        _ => None,
    }
    .ok_or_else(|| ExactEvaluationError::NotASquareRootTerm {
        term: term_node,
        radicand: match term {
            Value::Exact(sum) => sum.single_radicand(),
            _ => None,
        },
    })?;
    match value {
        Value::Exact(sum) => Ok(Value::Exact(SquareRootSum::from_rational(
            sum.coefficient_of(&radicand),
        ))),
        _ => Err(ExactEvaluationError::NotInRadicalField {
            expression,
            operator: Operator::CoefficientOf,
        }),
    }
}

fn square_root(
    pool: &mut ExprPool,
    argument: &Value,
    outside_domain: ExactEvaluationError,
) -> Evaluated<Value> {
    let arguments = [argument.clone()];
    match argument {
        Value::Exact(sum) => {
            if sum.sign() == Ordering::Less {
                return Err(outside_domain);
            }
            let root = match sum.as_rational() {
                Some(rational) => SquareRootSum::square_root_of_rational(&rational),
                None => sum.denested_square_root(),
            };
            match root {
                Some(root) => Ok(Value::Exact(root)),
                None => opaque_operator(pool, Operator::Sqrt, &arguments, true),
            }
        }
        _ if argument.pi_form_sign() == Some(Ordering::Less) => Err(outside_domain),
        _ => algebraic_opaque(pool, Operator::Sqrt, &arguments),
    }
}

fn absolute(pool: &mut ExprPool, argument: &Value) -> Evaluated<Value> {
    match argument {
        Value::Exact(sum) => Ok(Value::Exact(sum.absolute())),
        Value::PiMultiple(turns) => Ok(Value::PiMultiple(turns.absolute())),
        Value::PiPower {
            coefficient,
            exponent,
        } => Ok(Value::pi_power(coefficient.absolute(), *exponent)),
        Value::Opaque { .. } => {
            algebraic_opaque(pool, Operator::Abs, std::slice::from_ref(argument))
        }
    }
}

fn is_odd(value: &Integer) -> bool {
    match value.div_rem_euclid(&Integer::from(2_u64)) {
        Ok((_, remainder)) => !remainder.is_zero(),
        Err(_) => unreachable!(),
    }
}

fn round(pool: &mut ExprPool, operator: Operator, argument: &Value) -> Evaluated<Value> {
    let Value::Exact(sum) = argument else {
        return algebraic_opaque(pool, operator, std::slice::from_ref(argument));
    };
    let one = Integer::one();
    let rounded = match operator {
        Operator::Floor => sum.floor(),
        Operator::Ceil => sum.negated().floor().negated(),
        Operator::Trunc if sum.sign() == Ordering::Less => sum.negated().floor().negated(),
        Operator::Trunc => sum.floor(),
        Operator::Round if sum.sign() == Ordering::Less => {
            let half = SquareRootSum::from_rational(ExactRational::one_half());
            sum.negated().plus(&half).floor().negated()
        }
        Operator::Round => {
            let half = SquareRootSum::from_rational(ExactRational::one_half());
            sum.plus(&half).floor()
        }
        _ => {
            let half = SquareRootSum::from_rational(ExactRational::one_half());
            let shifted = sum.plus(&half);
            let candidate = shifted.floor();
            let is_tie =
                shifted.as_rational() == Some(ExactRational::from_integer(candidate.clone()));
            if is_tie && is_odd(&candidate) {
                &candidate - &one
            } else {
                candidate
            }
        }
    };
    Ok(Value::integer(rounded))
}

fn copy_sign(pool: &mut ExprPool, magnitude: &Value, sign: &Value) -> Evaluated<Value> {
    match (magnitude, sign) {
        (Value::Exact(magnitude_sum), Value::Exact(sign_sum)) => {
            let absolute = magnitude_sum.absolute();
            Ok(Value::Exact(if sign_sum.sign() == Ordering::Less {
                absolute.negated()
            } else {
                absolute
            }))
        }
        _ => algebraic_opaque(pool, Operator::CopySign, &[magnitude.clone(), sign.clone()]),
    }
}

fn exact_order(left: &Value, right: &Value) -> Option<Ordering> {
    match (left, right) {
        (Value::Exact(left_sum), Value::Exact(right_sum)) => Some(left_sum.compare(right_sum)),
        (Value::PiMultiple(left_turns), Value::PiMultiple(right_turns)) => {
            Some(left_turns.compare(right_turns))
        }
        (
            Value::PiPower {
                coefficient: left_coefficient,
                exponent: left_exponent,
            },
            Value::PiPower {
                coefficient: right_coefficient,
                exponent: right_exponent,
            },
        ) if left_exponent == right_exponent => Some(left_coefficient.compare(right_coefficient)),
        (pi_form, zero) if zero.is_exact_zero() && pi_form.is_pi_form() => pi_form.pi_form_sign(),
        (zero, pi_form) if zero.is_exact_zero() && pi_form.is_pi_form() => {
            pi_form.pi_form_sign().map(Ordering::reverse)
        }
        _ => None,
    }
}

fn extremum(
    pool: &mut ExprPool,
    operator: Operator,
    left: &Value,
    right: &Value,
) -> Evaluated<Value> {
    let Some(order) = exact_order(left, right) else {
        return algebraic_opaque(pool, operator, &[left.clone(), right.clone()]);
    };
    let left_is_chosen = match operator {
        Operator::Min => order != Ordering::Greater,
        _ => order != Ordering::Less,
    };
    Ok(if left_is_chosen {
        left.clone()
    } else {
        right.clone()
    })
}

fn factorial_size_lower_bound(count: u32) -> u64 {
    let count = u64::from(count);
    let mut bits = 0_u64;
    let mut block_start = 2_u64;
    let mut extra_bits = 1_u64;
    while block_start <= count {
        let block_end = block_start.saturating_mul(2).saturating_sub(1).min(count);
        bits = bits.saturating_add((block_end - block_start + 1).saturating_mul(extra_bits));
        block_start = block_start.saturating_mul(2);
        extra_bits += 1;
    }
    bits + 1
}

fn per_hundred(pool: &mut ExprPool, argument: &Value) -> Evaluated<Value> {
    let hundred = SquareRootSum::from_rational(ExactRational::from_i64(100));
    match argument {
        Value::Exact(sum) => match sum.divided_by(&hundred) {
            Some(divided) => Ok(Value::Exact(divided)),
            None => algebraic_opaque(pool, Operator::Percent, std::slice::from_ref(argument)),
        },
        _ => algebraic_opaque(pool, Operator::Percent, std::slice::from_ref(argument)),
    }
}

fn hyperbolic(pool: &mut ExprPool, operator: Operator, argument: &Value) -> Evaluated<Value> {
    let is_zero = matches!(argument, Value::Exact(sum) if sum.is_zero());
    if is_zero {
        return Ok(match operator {
            Operator::Cosh => Value::rational(ExactRational::one()),
            _ => Value::rational(ExactRational::zero()),
        });
    }
    opaque_operator(pool, operator, std::slice::from_ref(argument), false)
}

fn binomial(
    pool: &mut ExprPool,
    expression: ExprId,
    count: &Value,
    chosen: &Value,
) -> Evaluated<Value> {
    if matches!(count, Value::Opaque { .. }) || matches!(chosen, Value::Opaque { .. }) {
        return algebraic_opaque(pool, Operator::Binomial, &[count.clone(), chosen.clone()]);
    }
    let count = whole_number_of(expression, Operator::Binomial, count)?;
    let chosen = whole_number_of(expression, Operator::Binomial, chosen)?;
    let outside_domain = || ExactEvaluationError::OutsideDomain {
        expression,
        operator: Operator::Binomial,
    };
    if count.is_negative() || chosen.is_negative() {
        return Err(outside_domain());
    }
    let difference = &count - &chosen;
    if difference.is_negative() {
        return Ok(Value::integer(Integer::zero()));
    }
    let steps = chosen
        .to_i64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| argument_out_of_range(expression, Operator::Binomial, &chosen))?;
    within_size_limit(expression, factorial_size_lower_bound(steps))?;
    let mut result = Integer::one();
    for step in 0..steps {
        let step = Integer::from(i64::from(step));
        result = &result * &(&count - &step);
        let divisor = &step + &Integer::one();
        let (quotient, _) = result
            .div_rem_euclid(&divisor)
            .map_err(|_| outside_domain())?;
        result = quotient;
    }
    Ok(Value::integer(result))
}

fn whole_number_of(expression: ExprId, operator: Operator, value: &Value) -> Evaluated<Integer> {
    let outside_domain = || ExactEvaluationError::OutsideDomain {
        expression,
        operator,
    };
    let rational = match value {
        Value::Exact(sum) => sum.as_rational().ok_or_else(outside_domain)?,
        Value::PiMultiple(_) | Value::PiPower { .. } | Value::Opaque { .. } => {
            return Err(outside_domain());
        }
    };
    if !rational.is_integer() {
        return Err(outside_domain());
    }
    Ok(rational.numerator().clone())
}

fn integer_form(expression: ExprId, problem: IntegerProblem) -> ExactEvaluationError {
    ExactEvaluationError::IntegerForm {
        expression,
        problem,
    }
}

fn whole_integer(expression: ExprId, value: &Value) -> Evaluated<Integer> {
    let rational = match value {
        Value::Exact(sum) => sum.as_rational(),
        _ => None,
    }
    .ok_or_else(|| integer_form(expression, IntegerProblem::NotWhole))?;
    if !rational.is_integer() {
        return Err(integer_form(expression, IntegerProblem::NotWhole));
    }
    Ok(rational.numerator().clone())
}

fn small_whole(expression: ExprId, value: &Value) -> Evaluated<u64> {
    whole_integer(expression, value)?
        .to_i64()
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| integer_form(expression, IntegerProblem::ShiftTooLarge))
}

#[inline(never)]
fn integer_bits(
    expression: ExprId,
    operator: Operator,
    left: &Value,
    right: &Value,
) -> Evaluated<Value> {
    let left_integer = whole_integer(expression, left)?;
    let negative = || integer_form(expression, IntegerProblem::Negative);
    let result = match operator {
        Operator::ShiftLeft | Operator::ShiftRight => {
            if left_integer.is_negative() {
                return Err(negative());
            }
            let amount = small_whole(expression, right)?;
            if amount > u64::from(LARGEST_INTEGER_WIDTH) {
                return Err(integer_form(expression, IntegerProblem::ShiftTooLarge));
            }
            let amount = usize::try_from(amount)
                .map_err(|_| integer_form(expression, IntegerProblem::ShiftTooLarge))?;
            if operator == Operator::ShiftLeft {
                left_integer.shifted_left(amount)
            } else {
                left_integer.shifted_right(amount).ok_or_else(negative)?
            }
        }
        _ => {
            let right_integer = whole_integer(expression, right)?;
            match operator {
                Operator::BitAnd => left_integer.bit_and(&right_integer),
                Operator::BitOr => left_integer.bit_or(&right_integer),
                _ => left_integer.bit_xor(&right_integer),
            }
            .ok_or_else(negative)?
        }
    };
    Ok(Value::integer(result))
}

fn type_range(bits: u32, signed: bool) -> (Integer, Integer) {
    let span = Integer::one().shifted_left(bits as usize);
    if signed {
        let half = Integer::one().shifted_left(bits as usize - 1);
        (half.negated(), &half - &Integer::one())
    } else {
        (Integer::zero(), &span - &Integer::one())
    }
}

#[inline(never)]
fn typed_integer(
    expression: ExprId,
    operator: Operator,
    value: &Value,
    bits: &Value,
    signed: &Value,
) -> Evaluated<Value> {
    let integer = whole_integer(expression, value)?;
    let bits = u32::try_from(small_whole(expression, bits)?)
        .map_err(|_| integer_form(expression, IntegerProblem::ShiftTooLarge))?;
    let signed = i64::try_from(small_whole(expression, signed)?)
        .ok()
        .and_then(IntegerTypeSpelling::from_code)
        .unwrap_or(IntegerTypeSpelling::Width)
        .is_signed();
    if bits == 0 {
        return match operator {
            Operator::Bytes if integer.is_negative() => {
                Err(integer_form(expression, IntegerProblem::Negative))
            }
            Operator::Radix | Operator::Bytes => Ok(Value::integer(integer)),
            _ => Err(integer_form(
                expression,
                IntegerProblem::OutsideType { bits, signed },
            )),
        };
    }
    if operator == Operator::Bytes && bits % 8 != 0 {
        return Err(integer_form(
            expression,
            IntegerProblem::NotWholeBytes { bits, signed },
        ));
    }
    let (low, high) = type_range(bits, signed);
    if operator == Operator::Wrap {
        let span = Integer::one().shifted_left(bits as usize);
        let (_, mut wrapped) = integer
            .div_rem_euclid(&span)
            .map_err(|_| integer_form(expression, IntegerProblem::ShiftTooLarge))?;
        if wrapped > high {
            wrapped = &wrapped - &span;
        }
        return Ok(Value::integer(wrapped));
    }
    if integer < low || integer > high {
        return Err(integer_form(
            expression,
            IntegerProblem::OutsideType { bits, signed },
        ));
    }
    Ok(Value::integer(match operator {
        Operator::BitNot if signed => &integer.negated() - &Integer::one(),
        Operator::BitNot => &high - &integer,
        _ => integer,
    }))
}

fn whole_number_pair(
    pool: &mut ExprPool,
    expression: ExprId,
    operator: Operator,
    left: &Value,
    right: &Value,
) -> Evaluated<Value> {
    if matches!(left, Value::Opaque { .. }) || matches!(right, Value::Opaque { .. }) {
        return algebraic_opaque(pool, operator, &[left.clone(), right.clone()]);
    }
    if operator == Operator::Mod {
        return remainder(expression, left, right);
    }
    let left = whole_number_of(expression, operator, left)?;
    let right = whole_number_of(expression, operator, right)?;
    match operator {
        Operator::Gcd => Ok(Value::integer(left.gcd(&right))),
        Operator::Lcm => {
            if left.is_zero() || right.is_zero() {
                return Ok(Value::integer(Integer::zero()));
            }
            let divisor = left.gcd(&right);
            let (quotient, _) = left.absolute().div_rem_euclid(&divisor).map_err(|_| {
                ExactEvaluationError::OutsideDomain {
                    expression,
                    operator,
                }
            })?;
            Ok(Value::integer(&quotient * &right.absolute()))
        }
        Operator::Mod => {
            let (_, remainder) = left
                .div_rem_euclid(&right.absolute())
                .map_err(|_| ExactEvaluationError::DivisionByZero(expression))?;
            Ok(Value::integer(remainder))
        }
        _ => Err(ExactEvaluationError::OutsideDomain {
            expression,
            operator,
        }),
    }
}

fn remainder(expression: ExprId, left: &Value, right: &Value) -> Evaluated<Value> {
    let (Value::Exact(dividend), Value::Exact(divisor)) = (left, right) else {
        return Err(ExactEvaluationError::RemainderNotComputed { expression });
    };
    if divisor.is_zero() {
        return Err(ExactEvaluationError::DivisionByZero(expression));
    }
    let magnitude = divisor.absolute();
    let quotient = dividend
        .divided_by(&magnitude)
        .ok_or(ExactEvaluationError::DivisionByZero(expression))?;
    let whole = SquareRootSum::from_rational(ExactRational::from_integer(quotient.floor()));
    Ok(Value::Exact(dividend.minus(&magnitude.times(&whole))))
}

fn factorial(pool: &mut ExprPool, expression: ExprId, argument: &Value) -> Evaluated<Value> {
    let outside_domain = || ExactEvaluationError::OutsideDomain {
        expression,
        operator: Operator::Factorial,
    };
    match argument {
        Value::Exact(sum) => {
            let rational = sum.as_rational().ok_or_else(outside_domain)?;
            if !rational.is_integer() || rational.sign() == Ordering::Less {
                return Err(outside_domain());
            }
            let count = rational
                .numerator()
                .to_i64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| {
                    argument_out_of_range(expression, Operator::Factorial, rational.numerator())
                })?;
            within_size_limit(expression, factorial_size_lower_bound(count))?;
            let product = (1..=count).fold(Integer::one(), |accumulated, factor| {
                &accumulated * &Integer::from(u64::from(factor))
            });
            Ok(Value::integer(product))
        }
        Value::PiMultiple(_) | Value::PiPower { .. } => Err(outside_domain()),
        Value::Opaque { .. } => {
            algebraic_opaque(pool, Operator::Factorial, std::slice::from_ref(argument))
        }
    }
}

fn is_euler_number(pool: &ExprPool, value: &Value) -> Evaluated<bool> {
    let Value::Opaque { expression, .. } = value else {
        return Ok(false);
    };
    let node = pool
        .node(*expression)
        .map_err(ExactEvaluationError::Access)?;
    Ok(node == NodeView::Symbol(BuiltinConstant::E.symbol()))
}

fn exponential(pool: &mut ExprPool, argument: &Value) -> Evaluated<Value> {
    match argument.as_rational() {
        Some(rational) if rational.is_zero() => Ok(Value::rational(ExactRational::one())),
        Some(rational) if rational.is_one() => Ok(Value::Opaque {
            expression: pool
                .symbol(BuiltinConstant::E.symbol())
                .map_err(ExactEvaluationError::Build)?,
            is_algebraic: false,
        }),
        _ => opaque_operator(pool, Operator::Exp, std::slice::from_ref(argument), false),
    }
}

fn whole_power_exponent(value: &ExactRational, base: &Integer) -> Option<Integer> {
    if value.is_one() {
        return Some(Integer::zero());
    }
    let (magnitude, is_reciprocal) = if value.is_integer() {
        (value.numerator().clone(), false)
    } else if value.numerator().is_one() {
        (value.denominator().clone(), true)
    } else {
        return None;
    };
    let mut accumulated = Integer::one();
    let mut exponent: u32 = 0;
    while accumulated.bit_length() <= magnitude.bit_length() {
        if accumulated == magnitude {
            let exponent = Integer::from(i64::from(exponent));
            return Some(if is_reciprocal {
                exponent.negated()
            } else {
                exponent
            });
        }
        accumulated = &accumulated * base;
        exponent = exponent.checked_add(1)?;
    }
    None
}

fn exponent_over_a_common_base(value: &ExactRational, base: &Integer) -> Option<ExactRational> {
    let highest_degree = u32::try_from(base.bit_length()).ok()?;
    for degree in (2..=highest_degree).rev() {
        let (root, is_exact) = integer_root(base, degree);
        if !is_exact || root.is_one() {
            continue;
        }
        let exponent = whole_power_exponent(value, &root)?;
        return ExactRational::fraction(&exponent, &Integer::from(i64::from(degree)));
    }
    None
}

fn quotient_of_logarithms(
    pool: &mut ExprPool,
    expression: ExprId,
    argument: &Value,
    base: &Value,
) -> Evaluated<Value> {
    let outside_domain = |operator| ExactEvaluationError::OutsideDomain {
        expression,
        operator,
    };
    let numerator = logarithm(pool, argument, outside_domain(Operator::Log))?;
    let denominator = logarithm(pool, base, outside_domain(Operator::Log))?;
    apply_operator(pool, expression, Operator::Div, &[numerator, denominator])
}

fn based_logarithm(
    pool: &mut ExprPool,
    expression: ExprId,
    operator: Operator,
    argument: &Value,
    base: &Integer,
) -> Evaluated<Value> {
    let outside_domain = || ExactEvaluationError::OutsideDomain {
        expression,
        operator,
    };
    if base.is_negative() || base.is_zero() || base.is_one() {
        return Err(outside_domain());
    }
    if let Value::Exact(sum) = argument
        && let Some(rational) = sum.as_rational()
    {
        if rational.sign() != Ordering::Greater {
            return Err(outside_domain());
        }
        if let Some(exponent) = whole_power_exponent(&rational, base) {
            return Ok(Value::integer(exponent));
        }
        if let Some(exponent) = exponent_over_a_common_base(&rational, base) {
            return Ok(Value::rational(exponent));
        }
    }
    let base_value = Value::integer(base.clone());
    quotient_of_logarithms(pool, expression, argument, &base_value)
}

fn logarithm(
    pool: &mut ExprPool,
    argument: &Value,
    outside_domain: ExactEvaluationError,
) -> Evaluated<Value> {
    match argument {
        Value::Exact(sum) if sum.sign() != Ordering::Greater => Err(outside_domain),
        _ if argument.pi_form_sign() == Some(Ordering::Less) => Err(outside_domain),
        Value::Exact(sum) if sum.as_rational().is_some_and(|rational| rational.is_one()) => {
            Ok(Value::rational(ExactRational::zero()))
        }
        _ if is_euler_number(pool, argument)? => Ok(Value::rational(ExactRational::one())),
        _ => opaque_operator(pool, Operator::Ln, std::slice::from_ref(argument), false),
    }
}

fn trigonometric(
    pool: &mut ExprPool,
    operator: Operator,
    argument: &Value,
    outside_domain: ExactEvaluationError,
) -> Evaluated<Value> {
    let arguments = [argument.clone()];
    let Some(half_turns) = argument.angle_as_pi_multiple() else {
        return opaque_operator(pool, operator, &arguments, false);
    };
    let special = match operator {
        Operator::Sin => sine_of_pi_multiple(&half_turns),
        Operator::Cos => cosine_of_pi_multiple(&half_turns),
        _ => match tangent_of_pi_multiple(&half_turns) {
            TangentOfPiMultiple::Value(value) => Some(value),
            TangentOfPiMultiple::Pole => return Err(outside_domain),
            TangentOfPiMultiple::NotSpecial => None,
        },
    };
    match special {
        Some(value) => Ok(Value::Exact(value)),
        None => opaque_operator(pool, operator, &arguments, true),
    }
}

fn inverse_trigonometric(
    pool: &mut ExprPool,
    operator: Operator,
    argument: &Value,
    outside_domain: ExactEvaluationError,
) -> Evaluated<Value> {
    let arguments = [argument.clone()];
    let Value::Exact(sum) = argument else {
        return opaque_operator(pool, operator, &arguments, false);
    };
    let one = SquareRootSum::from_rational(ExactRational::one());
    if operator != Operator::Atan && sum.times(sum).compare(&one) == Ordering::Greater {
        return Err(outside_domain);
    }
    let angle = match operator {
        Operator::Asin => arcsine_as_pi_multiple(sum),
        Operator::Acos => arccosine_as_pi_multiple(sum),
        _ => arctangent_as_pi_multiple(sum),
    };
    match angle {
        Some(half_turns) => Ok(Value::pi_multiple(half_turns)),
        None => opaque_operator(pool, operator, &arguments, false),
    }
}

fn two_argument_arctangent(
    pool: &mut ExprPool,
    ordinate: &Value,
    abscissa: &Value,
    outside_domain: ExactEvaluationError,
) -> Evaluated<Value> {
    let arguments = [ordinate.clone(), abscissa.clone()];
    let (Value::Exact(ordinate_sum), Value::Exact(abscissa_sum)) = (ordinate, abscissa) else {
        return opaque_operator(pool, Operator::Atan2, &arguments, false);
    };
    let half = ExactRational::one_half();
    let ordinate_sign = ordinate_sum.sign();
    let Some(ratio) = ordinate_sum.divided_by(abscissa_sum) else {
        return match ordinate_sign {
            Ordering::Greater => Ok(Value::pi_multiple(half)),
            Ordering::Less => Ok(Value::pi_multiple(half.negated())),
            Ordering::Equal => Err(outside_domain),
        };
    };
    let Some(principal) = arctangent_as_pi_multiple(&ratio) else {
        return opaque_operator(pool, Operator::Atan2, &arguments, false);
    };
    let one = ExactRational::one();
    let angle = match (abscissa_sum.sign(), ordinate_sign) {
        (Ordering::Less, Ordering::Less) => principal.subtract(&one),
        (Ordering::Less, _) => principal.plus(&one),
        _ => principal,
    };
    Ok(Value::pi_multiple(angle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sum_of_squares_is_exact() {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, "sum(i^2, i, 1, 10)").unwrap();

        let evaluation = evaluate_exact(&mut pool, expression).unwrap();

        assert_eq!(
            evaluation.rational_value().cloned(),
            Some(Number::from(385))
        );
    }

    #[test]
    fn a_long_product_of_whole_numbers_stays_exact() {
        let mut pool = ExprPool::new();
        let expression =
            calc_syntax::parse_expression(&mut pool, "product(k, k, 1, 2000)").unwrap();

        let evaluation = evaluate_exact(&mut pool, expression).unwrap();

        let Some(Number::Integer(value)) = evaluation.rational_value().cloned() else {
            panic!("expected an integer");
        };
        assert!(value.bit_length() > 19_000, "{}", value.bit_length());
    }

    #[test]
    fn a_long_sum_of_whole_numbers_stays_exact() {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, "sum(i, i, 1, 50000)").unwrap();

        let evaluation = evaluate_exact(&mut pool, expression).unwrap();

        assert_eq!(
            evaluation.rational_value().cloned(),
            Some(Number::from(1_250_025_000))
        );
    }

    #[test]
    fn a_product_of_the_first_two_hundred_whole_numbers_is_exact() {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, "product(k, k, 1, 200)").unwrap();

        let evaluation = evaluate_exact(&mut pool, expression).unwrap();

        let Some(Number::Integer(value)) = evaluation.rational_value().cloned() else {
            panic!("expected an integer");
        };
        assert_eq!(value.bit_length(), 1246);
    }

    #[test]
    fn a_sum_takes_the_same_value_in_either_shape() {
        let mut pool = ExprPool::new();
        let left = calc_syntax::parse_expression(&mut pool, "sum(1/i, i, 1, 20)").unwrap();
        let right =
            calc_syntax::parse_expression(&mut pool, "sum(1/i, i, 1, 20, shape=halving)").unwrap();

        let left = evaluate_exact(&mut pool, left).unwrap();
        let right = evaluate_exact(&mut pool, right).unwrap();

        assert_eq!(
            left.rational_value().cloned(),
            right.rational_value().cloned()
        );
    }

    #[test]
    fn an_empty_range_gives_the_empty_sum() {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, "sum(i, i, 3, 1)").unwrap();

        let evaluation = evaluate_exact(&mut pool, expression).unwrap();

        assert_eq!(evaluation.rational_value().cloned(), Some(Number::from(0)));
    }

    #[test]
    fn an_empty_range_gives_the_empty_product() {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, "product(k, k, 3, 2)").unwrap();

        let evaluation = evaluate_exact(&mut pool, expression).unwrap();

        assert_eq!(evaluation.rational_value().cloned(), Some(Number::from(1)));
    }

    #[test]
    fn a_sum_of_reciprocals_beyond_the_work_limit_is_left_to_the_plan() {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, "sum(1/i, i, 1, 20000)").unwrap();

        let evaluation = evaluate_exact(&mut pool, expression);

        assert!(
            matches!(evaluation, Err(ExactEvaluationError::UnsupportedNode(_))),
            "{evaluation:?}"
        );
    }

    #[test]
    fn a_range_beyond_the_term_limit_is_left_to_the_plan() {
        let mut pool = ExprPool::new();
        let text = format!("sum(i, i, 1, {})", REDUCTION_TERM_LIMIT + 1);
        let expression = calc_syntax::parse_expression(&mut pool, &text).unwrap();

        let evaluation = evaluate_exact(&mut pool, expression);

        assert!(
            matches!(evaluation, Err(ExactEvaluationError::UnsupportedNode(_))),
            "{evaluation:?}"
        );
    }
    use calc_expr::SymbolKind;

    fn integer(pool: &mut ExprPool, value: i64) -> ExprId {
        pool.number(Number::from(value)).unwrap()
    }

    fn fraction(pool: &mut ExprPool, numerator: i64, denominator: i64) -> ExprId {
        let value =
            Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap();
        pool.number(value).unwrap()
    }

    fn apply(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> ExprId {
        pool.apply(Head::Operator(operator), arguments).unwrap()
    }

    fn pi(pool: &mut ExprPool) -> ExprId {
        pool.symbol(BuiltinConstant::Pi.symbol()).unwrap()
    }

    fn euler(pool: &mut ExprPool) -> ExprId {
        pool.symbol(BuiltinConstant::E.symbol()).unwrap()
    }

    fn root(pool: &mut ExprPool, radicand: i64) -> ExprId {
        let radicand = integer(pool, radicand);
        apply(pool, Operator::Sqrt, &[radicand])
    }

    fn pi_over(pool: &mut ExprPool, denominator: i64) -> ExprId {
        let pi = pi(pool);
        let denominator = integer(pool, denominator);
        apply(pool, Operator::Div, &[pi, denominator])
    }

    fn pi_times(pool: &mut ExprPool, numerator: i64, denominator: i64) -> ExprId {
        let factor = fraction(pool, numerator, denominator);
        let pi = pi(pool);
        apply(pool, Operator::Mul, &[factor, pi])
    }

    #[test]
    fn a_percentage_is_the_number_divided_by_one_hundred() {
        let (mut pool, expression) = unary(Operator::Percent, |pool| integer(pool, 19));

        expect_rational(&mut pool, expression, 19, 100);
    }

    #[test]
    fn a_percentage_of_a_value_stays_exact() {
        let mut pool = ExprPool::new();
        let whole = integer(&mut pool, 1000);
        let rate = integer(&mut pool, 19);
        let percentage = apply(&mut pool, Operator::Percent, &[rate]);
        let expression = apply(&mut pool, Operator::Mul, &[whole, percentage]);

        expect_rational(&mut pool, expression, 190, 1);
    }

    #[test]
    fn round_sends_a_half_away_from_zero() {
        let (mut pool, expression) = unary(Operator::Round, |pool| fraction(pool, 5, 2));

        expect_rational(&mut pool, expression, 3, 1);
    }

    #[test]
    fn round_of_a_negative_half_goes_away_from_zero_too() {
        let (mut pool, expression) = unary(Operator::Round, |pool| fraction(pool, -5, 2));

        expect_rational(&mut pool, expression, -3, 1);
    }

    #[test]
    fn binomial_counts_the_ways_of_choosing() {
        let (mut pool, expression) = whole_pair(Operator::Binomial, 10, 3);

        expect_rational(&mut pool, expression, 120, 1);
    }

    #[test]
    fn binomial_choosing_more_than_there_are_is_zero() {
        let (mut pool, expression) = whole_pair(Operator::Binomial, 3, 5);

        expect_rational(&mut pool, expression, 0, 1);
    }

    #[test]
    fn cosh_of_zero_is_one() {
        let (mut pool, expression) = unary(Operator::Cosh, |pool| integer(pool, 0));

        expect_rational(&mut pool, expression, 1, 1);
    }

    #[test]
    fn sinh_of_zero_is_zero() {
        let (mut pool, expression) = unary(Operator::Sinh, |pool| integer(pool, 0));

        expect_rational(&mut pool, expression, 0, 1);
    }

    #[test]
    fn log10_of_a_power_of_ten_is_exact() {
        let (mut pool, expression) = unary(Operator::Log10, |pool| integer(pool, 1000));

        expect_rational(&mut pool, expression, 3, 1);
    }

    #[test]
    fn log10_of_a_reciprocal_power_of_ten_is_negative_and_exact() {
        let (mut pool, expression) = unary(Operator::Log10, |pool| fraction(pool, 1, 1000));

        expect_rational(&mut pool, expression, -3, 1);
    }

    #[test]
    fn log2_of_a_power_of_two_is_exact() {
        let (mut pool, expression) = unary(Operator::Log2, |pool| integer(pool, 1024));

        expect_rational(&mut pool, expression, 10, 1);
    }

    #[test]
    fn a_logarithm_to_a_whole_base_is_exact_for_a_power_of_that_base() {
        let (mut pool, expression) = whole_pair(Operator::Log, 8, 2);

        expect_rational(&mut pool, expression, 3, 1);
    }

    #[test]
    fn log10_of_one_is_zero() {
        let (mut pool, expression) = unary(Operator::Log10, |pool| integer(pool, 1));

        expect_rational(&mut pool, expression, 0, 1);
    }

    #[test]
    fn log10_of_zero_is_outside_its_domain() {
        let (mut pool, expression) = unary(Operator::Log10, |pool| integer(pool, 0));

        assert_eq!(
            evaluate_exact(&mut pool, expression),
            Err(ExactEvaluationError::OutsideDomain {
                expression,
                operator: Operator::Log10,
            })
        );
    }

    #[test]
    fn a_logarithm_to_base_one_is_outside_its_domain() {
        let (mut pool, expression) = whole_pair(Operator::Log, 8, 1);

        assert_eq!(
            evaluate_exact(&mut pool, expression),
            Err(ExactEvaluationError::OutsideDomain {
                expression,
                operator: Operator::Log,
            })
        );
    }

    fn whole_pair(operator: Operator, left: i64, right: i64) -> (ExprPool, ExprId) {
        let mut pool = ExprPool::new();
        let left = integer(&mut pool, left);
        let right = integer(&mut pool, right);
        let expression = apply(&mut pool, operator, &[left, right]);
        (pool, expression)
    }

    #[test]
    fn log_over_a_common_base_is_the_ratio_of_the_exponents() {
        let (mut pool, expression) = whole_pair(Operator::Log, 4, 8);

        expect_rational(&mut pool, expression, 2, 3);
    }

    #[test]
    fn log_of_a_number_by_its_own_power_is_a_unit_fraction() {
        let (mut pool, expression) = whole_pair(Operator::Log, 10, 100);

        expect_rational(&mut pool, expression, 1, 2);
    }

    #[test]
    fn gcd_of_two_whole_numbers_is_their_greatest_common_divisor() {
        let (mut pool, expression) = whole_pair(Operator::Gcd, 12, 18);

        expect_rational(&mut pool, expression, 6, 1);
    }

    #[test]
    fn gcd_ignores_the_signs_of_its_arguments() {
        let (mut pool, expression) = whole_pair(Operator::Gcd, -12, 18);

        expect_rational(&mut pool, expression, 6, 1);
    }

    #[test]
    fn lcm_of_two_whole_numbers_is_their_least_common_multiple() {
        let (mut pool, expression) = whole_pair(Operator::Lcm, 21, 6);

        expect_rational(&mut pool, expression, 42, 1);
    }

    #[test]
    fn lcm_with_zero_is_zero() {
        let (mut pool, expression) = whole_pair(Operator::Lcm, 0, 5);

        expect_rational(&mut pool, expression, 0, 1);
    }

    #[test]
    fn mod_of_a_negative_number_is_not_negative() {
        let (mut pool, expression) = whole_pair(Operator::Mod, -3, 5);

        expect_rational(&mut pool, expression, 2, 1);
    }

    fn remainder_of(pool: &mut ExprPool, dividend: ExprId, divisor: ExprId) -> ExprId {
        apply(pool, Operator::Mod, &[dividend, divisor])
    }

    #[test]
    fn a_remainder_of_a_fraction_is_exact() {
        let mut pool = ExprPool::new();
        let dividend = fraction(&mut pool, 15, 2);
        let divisor = integer(&mut pool, 2);
        let remainder = remainder_of(&mut pool, dividend, divisor);

        expect_rational(&mut pool, remainder, 3, 2);
    }

    #[test]
    fn a_remainder_by_a_fraction_is_exact() {
        let mut pool = ExprPool::new();
        let dividend = integer(&mut pool, 7);
        let divisor = fraction(&mut pool, 5, 2);
        let remainder = remainder_of(&mut pool, dividend, divisor);

        expect_rational(&mut pool, remainder, 2, 1);
    }

    #[test]
    fn a_remainder_of_a_negative_fraction_is_not_negative() {
        let mut pool = ExprPool::new();
        let dividend = fraction(&mut pool, -15, 2);
        let divisor = integer(&mut pool, 2);
        let remainder = remainder_of(&mut pool, dividend, divisor);

        expect_rational(&mut pool, remainder, 1, 2);
    }

    #[test]
    fn a_remainder_by_a_negative_fraction_is_taken_by_its_magnitude() {
        let mut pool = ExprPool::new();
        let dividend = fraction(&mut pool, 15, 2);
        let divisor = integer(&mut pool, -2);
        let remainder = remainder_of(&mut pool, dividend, divisor);

        expect_rational(&mut pool, remainder, 3, 2);
    }

    #[test]
    fn a_remainder_with_pi_is_refused_for_what_it_holds() {
        let mut pool = ExprPool::new();
        let dividend = pi(&mut pool);
        let divisor = integer(&mut pool, 1);
        let remainder = remainder_of(&mut pool, dividend, divisor);

        assert_eq!(
            evaluate_exact(&mut pool, remainder),
            Err(ExactEvaluationError::RemainderNotComputed {
                expression: remainder
            })
        );
    }

    #[test]
    fn zero_to_a_positive_fraction_is_zero() {
        let mut pool = ExprPool::new();
        let zero = integer(&mut pool, 0);
        let half = fraction(&mut pool, 1, 2);
        let power = apply(&mut pool, Operator::Pow, &[zero, half]);

        expect_rational(&mut pool, power, 0, 1);
    }

    #[test]
    fn zero_to_a_negative_fraction_divides_by_zero() {
        let mut pool = ExprPool::new();
        let zero = integer(&mut pool, 0);
        let exponent = fraction(&mut pool, -1, 2);
        let power = apply(&mut pool, Operator::Pow, &[zero, exponent]);

        assert_eq!(
            evaluate_exact(&mut pool, power),
            Err(ExactEvaluationError::DivisionByZero(power))
        );
    }

    #[test]
    fn zero_to_a_positive_expression_in_pi_is_zero() {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, "0^(pi - 3)").unwrap();

        expect_rational(&mut pool, expression, 0, 1);
    }

    #[test]
    fn zero_to_a_negative_expression_in_pi_divides_by_zero() {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, "0^(3 - pi)").unwrap();

        assert_eq!(
            evaluate_exact(&mut pool, expression),
            Err(ExactEvaluationError::DivisionByZero(expression))
        );
    }

    #[test]
    fn zero_to_pi_is_zero() {
        let mut pool = ExprPool::new();
        let zero = integer(&mut pool, 0);
        let exponent = pi(&mut pool);
        let power = apply(&mut pool, Operator::Pow, &[zero, exponent]);

        expect_rational(&mut pool, power, 0, 1);
    }

    #[test]
    fn zero_to_an_exponent_proven_positive_past_its_first_enclosure_is_zero() {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(
            &mut pool,
            "0^(e - 2.718281828459045235360287471352662497757)",
        )
        .unwrap();

        expect_rational(&mut pool, expression, 0, 1);
    }

    #[test]
    fn zero_to_an_exponent_proven_negative_past_its_first_enclosure_divides_by_zero() {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(
            &mut pool,
            "0^(e - 2.718281828459045235360287471352662497758)",
        )
        .unwrap();

        assert_eq!(
            evaluate_exact(&mut pool, expression),
            Err(ExactEvaluationError::DivisionByZero(expression))
        );
    }

    #[test]
    fn zero_to_a_constant_exponent_without_an_f64_enclosure_is_decided_past_it() {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, "0^(tan(pi/2 - 1e-30))").unwrap();

        expect_rational(&mut pool, expression, 0, 1);
    }

    #[test]
    fn zero_to_an_exponent_that_divides_by_an_undecided_zero_is_refused() {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, "0^(1/(e - e))").unwrap();

        assert_eq!(
            evaluate_exact(&mut pool, expression),
            Err(ExactEvaluationError::PowerOfZeroSignUndecided { expression })
        );
    }

    #[test]
    fn zero_to_an_exponent_of_undecided_sign_is_refused() {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, "0^(e - e)").unwrap();

        assert_eq!(
            evaluate_exact(&mut pool, expression),
            Err(ExactEvaluationError::PowerOfZeroSignUndecided { expression })
        );
    }

    #[test]
    fn mod_by_zero_is_a_division_by_zero() {
        let (mut pool, expression) = whole_pair(Operator::Mod, 17, 0);

        assert_eq!(
            evaluate_exact(&mut pool, expression),
            Err(ExactEvaluationError::DivisionByZero(expression))
        );
    }

    #[test]
    fn gcd_of_a_fraction_is_outside_its_domain() {
        let mut pool = ExprPool::new();
        let left = fraction(&mut pool, 1, 2);
        let right = integer(&mut pool, 3);
        let expression = apply(&mut pool, Operator::Gcd, &[left, right]);

        assert_eq!(
            evaluate_exact(&mut pool, expression),
            Err(ExactEvaluationError::OutsideDomain {
                expression,
                operator: Operator::Gcd,
            })
        );
    }

    fn unary(
        operator: Operator,
        argument: impl FnOnce(&mut ExprPool) -> ExprId,
    ) -> (ExprPool, ExprId) {
        let mut pool = ExprPool::new();
        let argument = argument(&mut pool);
        let expression = apply(&mut pool, operator, &[argument]);
        (pool, expression)
    }

    fn rational_of(pool: &mut ExprPool, expression: ExprId) -> Option<Number> {
        evaluate_exact(pool, expression)
            .unwrap()
            .rational_value()
            .cloned()
    }

    fn expect_rational(pool: &mut ExprPool, expression: ExprId, numerator: i64, denominator: i64) {
        let expected =
            Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap();
        assert_eq!(rational_of(pool, expression), Some(expected));
    }

    fn expect_expression(
        pool: &mut ExprPool,
        expression: ExprId,
        kind: ResultKind,
        expected: ExprId,
    ) {
        let evaluation = evaluate_exact(pool, expression).unwrap();
        assert_eq!(
            (evaluation.kind(), evaluation.expression()),
            (kind, expected)
        );
    }

    fn outside_domain(expression: ExprId, operator: Operator) -> ExactEvaluationError {
        ExactEvaluationError::OutsideDomain {
            expression,
            operator,
        }
    }

    #[test]
    fn add_sums_rationals_exactly() {
        let mut pool = ExprPool::new();
        let third = fraction(&mut pool, 1, 3);
        let sixth = fraction(&mut pool, 1, 6);
        let sum = apply(&mut pool, Operator::Add, &[third, sixth]);

        expect_rational(&mut pool, sum, 1, 2);
    }

    #[test]
    fn decimal_tenths_add_exactly() {
        let mut pool = ExprPool::new();
        let one_tenth = fraction(&mut pool, 1, 10);
        let two_tenths = fraction(&mut pool, 2, 10);
        let sum = apply(&mut pool, Operator::Add, &[one_tenth, two_tenths]);

        expect_rational(&mut pool, sum, 3, 10);
    }

    #[test]
    fn sub_of_equal_roots_is_exact_zero() {
        let mut pool = ExprPool::new();
        let left = root(&mut pool, 2);
        let right = root(&mut pool, 8);
        let half = fraction(&mut pool, 1, 2);
        let scaled = apply(&mut pool, Operator::Mul, &[half, right]);
        let difference = apply(&mut pool, Operator::Sub, &[left, scaled]);

        let evaluation = evaluate_exact(&mut pool, difference).unwrap();

        assert_eq!(
            (evaluation.kind(), evaluation.rational_value()),
            (ResultKind::ExactRational, Some(&Number::from(0_i64)))
        );
    }

    #[test]
    fn mul_of_coprime_roots_is_root_of_product() {
        let mut pool = ExprPool::new();
        let left = root(&mut pool, 2);
        let right = root(&mut pool, 3);
        let product = apply(&mut pool, Operator::Mul, &[left, right]);
        let expected = root(&mut pool, 6);

        expect_expression(&mut pool, product, ResultKind::Algebraic, expected);
    }

    #[test]
    fn div_rationalizes_the_denominator() {
        let mut pool = ExprPool::new();
        let one = integer(&mut pool, 1);
        let square_root_two = root(&mut pool, 2);
        let denominator = apply(&mut pool, Operator::Add, &[one, square_root_two]);
        let quotient = apply(&mut pool, Operator::Div, &[one, denominator]);
        let minus_one = integer(&mut pool, -1);
        let expected = apply(&mut pool, Operator::Add, &[minus_one, square_root_two]);

        expect_expression(&mut pool, quotient, ResultKind::Algebraic, expected);
    }

    #[test]
    fn div_by_zero_is_an_error() {
        let mut pool = ExprPool::new();
        let one = integer(&mut pool, 1);
        let two = root(&mut pool, 2);
        let zero = apply(&mut pool, Operator::Sub, &[two, two]);
        let quotient = apply(&mut pool, Operator::Div, &[one, zero]);

        assert_eq!(
            evaluate_exact(&mut pool, quotient),
            Err(ExactEvaluationError::DivisionByZero(quotient))
        );
    }

    #[test]
    fn neg_negates_a_rational() {
        let (mut pool, negation) = unary(Operator::Neg, |pool| fraction(pool, 1, 2));

        expect_rational(&mut pool, negation, -1, 2);
    }

    #[test]
    fn pow_with_negative_integer_exponent_inverts() {
        let mut pool = ExprPool::new();
        let base = fraction(&mut pool, 2, 3);
        let exponent = integer(&mut pool, -2);
        let power = apply(&mut pool, Operator::Pow, &[base, exponent]);

        expect_rational(&mut pool, power, 9, 4);
    }

    #[test]
    fn pow_with_cube_root_of_fraction_with_non_cube_denominator_stays_algebraic() {
        let mut pool = ExprPool::new();
        let base = fraction(&mut pool, 8, 3);
        let exponent = fraction(&mut pool, 1, 3);
        let power = apply(&mut pool, Operator::Pow, &[base, exponent]);

        expect_expression(&mut pool, power, ResultKind::Algebraic, power);
    }

    #[test]
    fn pow_with_rational_exponent_of_perfect_cube_is_rational() {
        let mut pool = ExprPool::new();
        let base = integer(&mut pool, 8);
        let exponent = fraction(&mut pool, -2, 3);
        let power = apply(&mut pool, Operator::Pow, &[base, exponent]);

        expect_rational(&mut pool, power, 1, 4);
    }

    #[test]
    fn pow_with_exponent_one_half_is_square_root() {
        let mut pool = ExprPool::new();
        let base = integer(&mut pool, 18);
        let exponent = fraction(&mut pool, 1, 2);
        let power = apply(&mut pool, Operator::Pow, &[base, exponent]);
        let three = integer(&mut pool, 3);
        let square_root_two = root(&mut pool, 2);
        let expected = apply(&mut pool, Operator::Mul, &[three, square_root_two]);

        expect_expression(&mut pool, power, ResultKind::Algebraic, expected);
    }

    #[test]
    fn pow_with_cube_root_of_non_cube_stays_algebraic() {
        let mut pool = ExprPool::new();
        let base = integer(&mut pool, 2);
        let exponent = fraction(&mut pool, 1, 3);
        let power = apply(&mut pool, Operator::Pow, &[base, exponent]);

        expect_expression(&mut pool, power, ResultKind::Algebraic, power);
    }

    #[test]
    fn pow_of_negative_base_with_fractional_exponent_is_outside_domain() {
        let mut pool = ExprPool::new();
        let base = integer(&mut pool, -8);
        let exponent = fraction(&mut pool, 1, 3);
        let power = apply(&mut pool, Operator::Pow, &[base, exponent]);

        assert_eq!(
            evaluate_exact(&mut pool, power),
            Err(outside_domain(power, Operator::Pow))
        );
    }

    #[test]
    fn pow_of_zero_with_negative_exponent_is_division_by_zero() {
        let mut pool = ExprPool::new();
        let base = integer(&mut pool, 0);
        let exponent = integer(&mut pool, -1);
        let power = apply(&mut pool, Operator::Pow, &[base, exponent]);

        assert_eq!(
            evaluate_exact(&mut pool, power),
            Err(ExactEvaluationError::DivisionByZero(power))
        );
    }

    #[test]
    fn pow_of_zero_with_exponent_zero_is_one() {
        let mut pool = ExprPool::new();
        let zero = integer(&mut pool, 0);
        let power = apply(&mut pool, Operator::Pow, &[zero, zero]);

        expect_rational(&mut pool, power, 1, 1);
    }

    #[test]
    fn pow_with_transcendental_exponent_is_symbolic() {
        let mut pool = ExprPool::new();
        let base = integer(&mut pool, 2);
        let exponent = pi(&mut pool);
        let power = apply(&mut pool, Operator::Pow, &[base, exponent]);

        expect_expression(&mut pool, power, ResultKind::Symbolic, power);
    }

    fn power_of(pool: &mut ExprPool, base: i64, exponent: i64) -> ExprId {
        let base = integer(pool, base);
        let exponent = integer(pool, exponent);
        apply(pool, Operator::Pow, &[base, exponent])
    }

    fn too_large_digits(pool: &mut ExprPool, expression: ExprId) -> (ExprId, u64, u64) {
        match evaluate_exact(pool, expression) {
            Err(ExactEvaluationError::ResultTooLarge {
                expression,
                limit_bits,
                estimated_digits,
            }) => (expression, limit_bits, estimated_digits),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn exact_result_limit_is_two_to_the_eighteen_bits() {
        assert_eq!(EXACT_RESULT_BIT_LIMIT, 262_144);
    }

    #[test]
    fn ten_to_the_hundred_to_the_ten_names_the_exponent_and_the_limit() {
        let mut pool = ExprPool::new();
        let ten = integer(&mut pool, 10);
        let exponent = power_of(&mut pool, 100, 10);
        let power = apply(&mut pool, Operator::Pow, &[ten, exponent]);

        assert_eq!(
            evaluate_exact(&mut pool, power),
            Err(ExactEvaluationError::IntegerArgumentOutOfRange {
                expression: power,
                operator: Operator::Pow,
                argument: Integer::from(10_i64).pow(20),
                limit: 4_294_967_295
            })
        );
    }

    #[test]
    fn exponent_of_u32_max_is_within_range() {
        let mut pool = ExprPool::new();
        let power = power_of(&mut pool, -1, 4_294_967_295);

        expect_rational(&mut pool, power, -1, 1);
    }

    #[test]
    fn power_with_exactly_the_limit_in_bits_is_evaluated() {
        let mut pool = ExprPool::new();
        let power = power_of(&mut pool, 2, 262_143);

        let value = rational_of(&mut pool, power);

        assert!(matches!(value, Some(Number::Integer(integer)) if integer.bit_length() == 262_144));
    }

    #[test]
    fn power_one_bit_over_the_limit_is_too_large_with_its_digit_count() {
        let mut pool = ExprPool::new();
        let power = power_of(&mut pool, 2, 262_144);

        assert_eq!(too_large_digits(&mut pool, power), (power, 262_144, 78_914));
    }

    #[test]
    fn power_of_a_square_root_within_the_limit_is_evaluated() {
        let mut pool = ExprPool::new();
        let two = integer(&mut pool, 2);
        let root = apply(&mut pool, Operator::Sqrt, &[two]);
        let exponent = integer(&mut pool, 131_073);
        let power = apply(&mut pool, Operator::Pow, &[root, exponent]);

        let evaluation = evaluate_exact(&mut pool, power);

        assert!(
            matches!(
                evaluation,
                Ok(ExactEvaluation {
                    kind: ResultKind::Algebraic,
                    ..
                })
            ),
            "{evaluation:?}"
        );
    }

    #[test]
    fn power_of_a_square_root_sum_over_the_limit_is_too_large() {
        let mut pool = ExprPool::new();
        let two = integer(&mut pool, 2);
        let three = integer(&mut pool, 3);
        let root_two = apply(&mut pool, Operator::Sqrt, &[two]);
        let root_three = apply(&mut pool, Operator::Sqrt, &[three]);
        let sum = apply(&mut pool, Operator::Add, &[root_two, root_three]);
        let exponent = integer(&mut pool, 200_000);
        let power = apply(&mut pool, Operator::Pow, &[sum, exponent]);

        let (expression, limit_bits, _) = too_large_digits(&mut pool, power);

        assert_eq!((expression, limit_bits), (power, 262_144));
    }

    #[test]
    fn ten_to_four_billion_is_too_large_before_any_digit_is_built() {
        let mut pool = ExprPool::new();
        let power = power_of(&mut pool, 10, 4_000_000_000);

        let (_, _, digits) = too_large_digits(&mut pool, power);

        assert!(
            (4_000_000_001..=4_000_002_000).contains(&digits),
            "{digits}"
        );
    }

    #[test]
    fn product_of_two_allowed_powers_over_the_limit_is_too_large() {
        let mut pool = ExprPool::new();
        let left = power_of(&mut pool, 2, 200_000);
        let right = power_of(&mut pool, 3, 130_000);
        let product = apply(&mut pool, Operator::Mul, &[left, right]);

        let (expression, _, digits) = too_large_digits(&mut pool, product);

        assert_eq!(expression, product);
        assert!((122_232..=122_240).contains(&digits), "{digits}");
    }

    #[test]
    fn factorial_over_the_limit_is_too_large_before_it_is_multiplied_out() {
        let (mut pool, factorial) = unary(Operator::Factorial, |pool| integer(pool, 1_000_000));

        let (expression, limit_bits, _) = too_large_digits(&mut pool, factorial);

        assert_eq!((expression, limit_bits), (factorial, 262_144));
    }

    #[test]
    fn pow_with_exponent_beyond_u32_is_out_of_range() {
        let mut pool = ExprPool::new();
        let base = integer(&mut pool, 2);
        let exponent = integer(&mut pool, 1 << 40);
        let power = apply(&mut pool, Operator::Pow, &[base, exponent]);

        assert_eq!(
            evaluate_exact(&mut pool, power),
            Err(ExactEvaluationError::IntegerArgumentOutOfRange {
                expression: power,
                operator: Operator::Pow,
                argument: Integer::from(1_i64 << 40),
                limit: u64::from(u32::MAX)
            })
        );
    }

    #[test]
    fn sqrt_moves_square_factors_out() {
        let (mut pool, square_root) = unary(Operator::Sqrt, |pool| integer(pool, 12));
        let two = integer(&mut pool, 2);
        let square_root_three = root(&mut pool, 3);
        let expected = apply(&mut pool, Operator::Mul, &[two, square_root_three]);

        expect_expression(&mut pool, square_root, ResultKind::Algebraic, expected);
    }

    #[test]
    fn sqrt_of_fraction_is_rationalized() {
        let (mut pool, square_root) = unary(Operator::Sqrt, |pool| fraction(pool, 1, 3));
        let square_root_three = root(&mut pool, 3);
        let three = integer(&mut pool, 3);
        let expected = apply(&mut pool, Operator::Div, &[square_root_three, three]);

        expect_expression(&mut pool, square_root, ResultKind::Algebraic, expected);
    }

    #[test]
    fn sqrt_of_negative_is_outside_domain() {
        let (mut pool, square_root) = unary(Operator::Sqrt, |pool| integer(pool, -1));

        assert_eq!(
            evaluate_exact(&mut pool, square_root),
            Err(outside_domain(square_root, Operator::Sqrt))
        );
    }

    #[test]
    fn sqrt_with_uncertified_radicand_stays_algebraic() {
        let mut pool = ExprPool::new();
        let radicand = pool
            .number(Number::Integer(
                &Integer::from(4_294_967_311_i64) * &Integer::from(4_294_967_357_i64),
            ))
            .unwrap();
        let square_root = apply(&mut pool, Operator::Sqrt, &[radicand]);

        expect_expression(&mut pool, square_root, ResultKind::Algebraic, square_root);
    }

    #[test]
    fn abs_of_negative_algebraic_value_negates_it() {
        let mut pool = ExprPool::new();
        let one = integer(&mut pool, 1);
        let square_root_two = root(&mut pool, 2);
        let difference = apply(&mut pool, Operator::Sub, &[one, square_root_two]);
        let absolute = apply(&mut pool, Operator::Abs, &[difference]);
        let minus_one = integer(&mut pool, -1);
        let expected = apply(&mut pool, Operator::Add, &[minus_one, square_root_two]);

        expect_expression(&mut pool, absolute, ResultKind::Algebraic, expected);
    }

    #[test]
    fn mul_add_is_exact_product_plus_addend() {
        let mut pool = ExprPool::new();
        let square_root_two = root(&mut pool, 2);
        let one = integer(&mut pool, 1);
        let fused = apply(
            &mut pool,
            Operator::MulAdd,
            &[square_root_two, square_root_two, one],
        );

        expect_rational(&mut pool, fused, 3, 1);
    }

    #[test]
    fn floor_of_scaled_square_root_is_exact() {
        let mut pool = ExprPool::new();
        let factor = integer(&mut pool, 1_000_000);
        let square_root_two = root(&mut pool, 2);
        let scaled = apply(&mut pool, Operator::Mul, &[factor, square_root_two]);
        let floor = apply(&mut pool, Operator::Floor, &[scaled]);

        expect_rational(&mut pool, floor, 1_414_213, 1);
    }

    #[test]
    fn floor_of_negative_fraction_rounds_down() {
        let (mut pool, floor) = unary(Operator::Floor, |pool| fraction(pool, -7, 2));

        expect_rational(&mut pool, floor, -4, 1);
    }

    #[test]
    fn ceil_of_square_root_two_is_two() {
        let (mut pool, ceiling) = unary(Operator::Ceil, |pool| root(pool, 2));

        expect_rational(&mut pool, ceiling, 2, 1);
    }

    #[test]
    fn ceil_of_negative_fraction_rounds_up() {
        let (mut pool, ceiling) = unary(Operator::Ceil, |pool| fraction(pool, -7, 2));

        expect_rational(&mut pool, ceiling, -3, 1);
    }

    #[test]
    fn trunc_of_negative_square_root_rounds_toward_zero() {
        let (mut pool, truncated) = unary(Operator::Trunc, |pool| {
            let square_root_two = root(pool, 2);
            apply(pool, Operator::Neg, &[square_root_two])
        });

        expect_rational(&mut pool, truncated, -1, 1);
    }

    #[test]
    fn round_ties_even_rounds_halves_to_even() {
        let cases = [
            (5, 2, 2),
            (7, 2, 4),
            (-5, 2, -2),
            (-7, 2, -4),
            (1, 3, 0),
            (5, 3, 2),
        ];

        let results: Vec<Option<Number>> = cases
            .iter()
            .map(|(numerator, denominator, _)| {
                let (mut pool, rounded) = unary(Operator::RoundTiesEven, |pool| {
                    fraction(pool, *numerator, *denominator)
                });
                rational_of(&mut pool, rounded)
            })
            .collect();

        let expected: Vec<Option<Number>> = cases
            .iter()
            .map(|(_, _, rounded)| Some(Number::from(*rounded)))
            .collect();
        assert_eq!(results, expected);
    }

    #[test]
    fn round_ties_even_of_irrational_rounds_to_nearest() {
        let (mut pool, rounded) = unary(Operator::RoundTiesEven, |pool| root(pool, 3));

        expect_rational(&mut pool, rounded, 2, 1);
    }

    #[test]
    fn copy_sign_takes_sign_of_second_argument() {
        let mut pool = ExprPool::new();
        let three = integer(&mut pool, 3);
        let negative_half = fraction(&mut pool, -1, 2);
        let copied = apply(&mut pool, Operator::CopySign, &[three, negative_half]);

        expect_rational(&mut pool, copied, -3, 1);
    }

    #[test]
    fn copy_sign_from_exact_zero_is_positive() {
        let mut pool = ExprPool::new();
        let minus_three = integer(&mut pool, -3);
        let zero = integer(&mut pool, 0);
        let copied = apply(&mut pool, Operator::CopySign, &[minus_three, zero]);

        expect_rational(&mut pool, copied, 3, 1);
    }

    #[test]
    fn min_compares_rational_with_square_root_exactly() {
        let mut pool = ExprPool::new();
        let square_root_two = root(&mut pool, 2);
        let close_below = fraction(&mut pool, 141_421, 100_000);
        let minimum = apply(&mut pool, Operator::Min, &[square_root_two, close_below]);

        expect_rational(&mut pool, minimum, 141_421, 100_000);
    }

    #[test]
    fn max_picks_larger_square_root() {
        let mut pool = ExprPool::new();
        let square_root_two = root(&mut pool, 2);
        let square_root_three = root(&mut pool, 3);
        let maximum = apply(
            &mut pool,
            Operator::Max,
            &[square_root_two, square_root_three],
        );

        expect_expression(&mut pool, maximum, ResultKind::Algebraic, square_root_three);
    }

    #[test]
    fn factorial_of_twenty_is_exact() {
        let (mut pool, factorial) = unary(Operator::Factorial, |pool| integer(pool, 20));

        expect_rational(&mut pool, factorial, 2_432_902_008_176_640_000, 1);
    }

    #[test]
    fn factorial_of_zero_is_one() {
        let (mut pool, factorial) = unary(Operator::Factorial, |pool| integer(pool, 0));

        expect_rational(&mut pool, factorial, 1, 1);
    }

    #[test]
    fn factorial_of_fraction_is_outside_domain() {
        let (mut pool, factorial) = unary(Operator::Factorial, |pool| fraction(pool, 1, 2));

        assert_eq!(
            evaluate_exact(&mut pool, factorial),
            Err(outside_domain(factorial, Operator::Factorial))
        );
    }

    #[test]
    fn factorial_of_negative_integer_is_outside_domain() {
        let (mut pool, factorial) = unary(Operator::Factorial, |pool| integer(pool, -1));

        assert_eq!(
            evaluate_exact(&mut pool, factorial),
            Err(outside_domain(factorial, Operator::Factorial))
        );
    }

    #[test]
    fn factorial_beyond_u32_is_out_of_range() {
        let (mut pool, factorial) = unary(Operator::Factorial, |pool| integer(pool, 1 << 40));

        assert_eq!(
            evaluate_exact(&mut pool, factorial),
            Err(ExactEvaluationError::IntegerArgumentOutOfRange {
                expression: factorial,
                operator: Operator::Factorial,
                argument: Integer::from(1_i64 << 40),
                limit: u64::from(u32::MAX)
            })
        );
    }

    #[test]
    fn exp_of_zero_is_one() {
        let (mut pool, exponential) = unary(Operator::Exp, |pool| integer(pool, 0));

        expect_rational(&mut pool, exponential, 1, 1);
    }

    #[test]
    fn exp_of_one_is_euler_number() {
        let (mut pool, exponential) = unary(Operator::Exp, |pool| integer(pool, 1));
        let expected = euler(&mut pool);

        expect_expression(&mut pool, exponential, ResultKind::Symbolic, expected);
    }

    #[test]
    fn exp_of_two_stays_symbolic() {
        let (mut pool, exponential) = unary(Operator::Exp, |pool| integer(pool, 2));

        expect_expression(&mut pool, exponential, ResultKind::Symbolic, exponential);
    }

    #[test]
    fn ln_of_one_is_zero() {
        let (mut pool, logarithm) = unary(Operator::Ln, |pool| integer(pool, 1));

        expect_rational(&mut pool, logarithm, 0, 1);
    }

    #[test]
    fn ln_of_euler_number_is_one() {
        let (mut pool, logarithm) = unary(Operator::Ln, euler);

        expect_rational(&mut pool, logarithm, 1, 1);
    }

    #[test]
    fn ln_of_zero_is_outside_domain() {
        let (mut pool, logarithm) = unary(Operator::Ln, |pool| integer(pool, 0));

        assert_eq!(
            evaluate_exact(&mut pool, logarithm),
            Err(outside_domain(logarithm, Operator::Ln))
        );
    }

    #[test]
    fn ln_of_two_stays_symbolic() {
        let (mut pool, logarithm) = unary(Operator::Ln, |pool| integer(pool, 2));

        expect_expression(&mut pool, logarithm, ResultKind::Symbolic, logarithm);
    }

    #[test]
    fn sin_of_sixth_of_pi_is_one_half() {
        let (mut pool, sine) = unary(Operator::Sin, |pool| pi_over(pool, 6));

        expect_rational(&mut pool, sine, 1, 2);
    }

    #[test]
    fn sin_of_fifth_of_pi_is_algebraic_but_kept() {
        let (mut pool, sine) = unary(Operator::Sin, |pool| pi_over(pool, 5));
        let angle = pi_times(&mut pool, 1, 5);
        let expected = apply(&mut pool, Operator::Sin, &[angle]);

        expect_expression(&mut pool, sine, ResultKind::Algebraic, expected);
    }

    #[test]
    fn sin_of_one_is_symbolic() {
        let (mut pool, sine) = unary(Operator::Sin, |pool| integer(pool, 1));

        expect_expression(&mut pool, sine, ResultKind::Symbolic, sine);
    }

    #[test]
    fn cos_of_two_thirds_of_pi_is_minus_one_half() {
        let (mut pool, cosine) = unary(Operator::Cos, |pool| pi_times(pool, 2, 3));

        expect_rational(&mut pool, cosine, -1, 2);
    }

    #[test]
    fn tan_of_third_of_pi_is_square_root_three() {
        let (mut pool, tangent) = unary(Operator::Tan, |pool| pi_over(pool, 3));
        let expected = root(&mut pool, 3);

        expect_expression(&mut pool, tangent, ResultKind::Algebraic, expected);
    }

    #[test]
    fn tan_of_half_pi_is_outside_domain() {
        let (mut pool, tangent) = unary(Operator::Tan, |pool| pi_over(pool, 2));

        assert_eq!(
            evaluate_exact(&mut pool, tangent),
            Err(outside_domain(tangent, Operator::Tan))
        );
    }

    #[test]
    fn asin_of_one_half_is_sixth_of_pi() {
        let (mut pool, arcsine) = unary(Operator::Asin, |pool| fraction(pool, 1, 2));
        let expected = pi_times(&mut pool, 1, 6);

        expect_expression(&mut pool, arcsine, ResultKind::Symbolic, expected);
    }

    #[test]
    fn asin_of_two_is_outside_domain() {
        let (mut pool, arcsine) = unary(Operator::Asin, |pool| integer(pool, 2));

        assert_eq!(
            evaluate_exact(&mut pool, arcsine),
            Err(outside_domain(arcsine, Operator::Asin))
        );
    }

    #[test]
    fn acos_of_minus_one_is_pi() {
        let (mut pool, arccosine) = unary(Operator::Acos, |pool| integer(pool, -1));
        let expected = pi(&mut pool);

        expect_expression(&mut pool, arccosine, ResultKind::Symbolic, expected);
    }

    #[test]
    fn acos_of_one_is_zero() {
        let (mut pool, arccosine) = unary(Operator::Acos, |pool| integer(pool, 1));

        expect_rational(&mut pool, arccosine, 0, 1);
    }

    #[test]
    fn atan_of_one_is_quarter_pi() {
        let (mut pool, arctangent) = unary(Operator::Atan, |pool| integer(pool, 1));
        let expected = pi_times(&mut pool, 1, 4);

        expect_expression(&mut pool, arctangent, ResultKind::Symbolic, expected);
    }

    #[test]
    fn atan2_in_second_quadrant_adds_pi() {
        let mut pool = ExprPool::new();
        let one = integer(&mut pool, 1);
        let minus_one = integer(&mut pool, -1);
        let angle = apply(&mut pool, Operator::Atan2, &[one, minus_one]);
        let expected = pi_times(&mut pool, 3, 4);

        expect_expression(&mut pool, angle, ResultKind::Symbolic, expected);
    }

    #[test]
    fn atan2_in_third_quadrant_subtracts_pi() {
        let mut pool = ExprPool::new();
        let minus_one = integer(&mut pool, -1);
        let angle = apply(&mut pool, Operator::Atan2, &[minus_one, minus_one]);
        let expected = pi_times(&mut pool, -3, 4);

        expect_expression(&mut pool, angle, ResultKind::Symbolic, expected);
    }

    #[test]
    fn atan2_on_negative_ordinate_axis_is_minus_half_pi() {
        let mut pool = ExprPool::new();
        let minus_one = integer(&mut pool, -1);
        let zero = integer(&mut pool, 0);
        let angle = apply(&mut pool, Operator::Atan2, &[minus_one, zero]);
        let expected = pi_times(&mut pool, -1, 2);

        expect_expression(&mut pool, angle, ResultKind::Symbolic, expected);
    }

    #[test]
    fn atan2_of_origin_is_outside_domain() {
        let mut pool = ExprPool::new();
        let zero = integer(&mut pool, 0);
        let angle = apply(&mut pool, Operator::Atan2, &[zero, zero]);

        assert_eq!(
            evaluate_exact(&mut pool, angle),
            Err(outside_domain(angle, Operator::Atan2))
        );
    }

    #[test]
    fn to_exact_gives_the_rational_a_binary64_value_denotes() {
        let (mut pool, exact) = unary(Operator::ToExact, |pool| {
            pool.number(Number::F64(0.1)).unwrap()
        });

        expect_rational(
            &mut pool,
            exact,
            3_602_879_701_896_397,
            36_028_797_018_963_968,
        );
    }

    #[test]
    fn to_exact_of_infinity_is_not_finite() {
        let (mut pool, exact) = unary(Operator::ToExact, |pool| {
            pool.number(Number::F64(f64::INFINITY)).unwrap()
        });

        assert_eq!(
            evaluate_exact(&mut pool, exact),
            Err(ExactEvaluationError::NotFinite(exact))
        );
    }

    #[test]
    fn to_exact_of_exact_value_keeps_it() {
        let (mut pool, exact) = unary(Operator::ToExact, |pool| fraction(pool, 1, 3));

        expect_rational(&mut pool, exact, 1, 3);
    }

    #[test]
    fn machine_number_operand_is_the_rational_it_is() {
        let mut pool = ExprPool::new();
        let machine = pool.number(Number::F64(0.5)).unwrap();
        let one = integer(&mut pool, 1);
        let sum = apply(&mut pool, Operator::Add, &[machine, one]);

        expect_rational(&mut pool, sum, 3, 2);
    }

    #[test]
    fn a_machine_number_is_not_taken_apart() {
        let mut pool = ExprPool::new();
        let machine = pool.number(Number::F64(1.5)).unwrap();
        let part = apply(&mut pool, Operator::RationalPart, &[machine]);

        assert_eq!(
            evaluate_exact(&mut pool, part),
            Err(ExactEvaluationError::NotInRadicalField {
                expression: part,
                operator: Operator::RationalPart,
            })
        );
    }

    #[test]
    fn a_machine_number_that_is_not_finite_is_still_an_error() {
        let mut pool = ExprPool::new();
        let machine = pool.number(Number::F64(f64::INFINITY)).unwrap();
        let one = integer(&mut pool, 1);
        let sum = apply(&mut pool, Operator::Add, &[machine, one]);

        assert_eq!(
            evaluate_exact(&mut pool, sum),
            Err(ExactEvaluationError::MachineNumber(machine))
        );
    }

    #[test]
    fn comparison_operator_is_unsupported() {
        let mut pool = ExprPool::new();
        let one = integer(&mut pool, 1);
        let two = integer(&mut pool, 2);
        let comparison = apply(&mut pool, Operator::Less, &[one, two]);

        assert_eq!(
            evaluate_exact(&mut pool, comparison),
            Err(ExactEvaluationError::UnsupportedOperator {
                expression: comparison,
                operator: Operator::Less
            })
        );
    }

    #[test]
    fn bound_variable_is_an_unsupported_node() {
        let mut pool = ExprPool::new();
        let bound = pool.bound(0).unwrap();

        assert_eq!(
            evaluate_exact(&mut pool, bound),
            Err(ExactEvaluationError::UnsupportedNode(bound))
        );
    }

    #[test]
    fn infinity_is_an_unsupported_constant() {
        let mut pool = ExprPool::new();
        let infinity = pool.symbol(BuiltinConstant::Infinity.symbol()).unwrap();

        assert_eq!(
            evaluate_exact(&mut pool, infinity),
            Err(ExactEvaluationError::UnsupportedConstant(infinity))
        );
    }

    #[test]
    fn expression_from_another_pool_is_an_access_error() {
        let mut other = ExprPool::new();
        let mut last = integer(&mut other, 0);
        for value in 1..10 {
            last = integer(&mut other, value);
        }
        let mut pool = ExprPool::new();

        assert_eq!(
            evaluate_exact(&mut pool, last),
            Err(ExactEvaluationError::Access(AccessError::UnknownExprId(
                last
            )))
        );
    }

    #[test]
    fn free_variable_keeps_written_shape_and_is_symbolic() {
        let mut pool = ExprPool::new();
        let symbol = pool.intern_symbol("x", SymbolKind::Variable).unwrap();
        let variable = pool.symbol(symbol).unwrap();
        let one = integer(&mut pool, 1);
        let two = integer(&mut pool, 2);
        let constant = apply(&mut pool, Operator::Add, &[one, two]);
        let sum = apply(&mut pool, Operator::Add, &[variable, constant]);
        let three = integer(&mut pool, 3);
        let expected = apply(&mut pool, Operator::Add, &[variable, three]);

        expect_expression(&mut pool, sum, ResultKind::Symbolic, expected);
    }

    #[test]
    fn function_application_is_symbolic() {
        let mut pool = ExprPool::new();
        let function = pool
            .intern_symbol("f", SymbolKind::Function { arity: 1 })
            .unwrap();
        let four = integer(&mut pool, 4);
        let square_root_four = apply(&mut pool, Operator::Sqrt, &[four]);
        let application = pool
            .apply(Head::Function(function), &[square_root_four])
            .unwrap();
        let two = integer(&mut pool, 2);
        let expected = pool.apply(Head::Function(function), &[two]).unwrap();

        expect_expression(&mut pool, application, ResultKind::Symbolic, expected);
    }

    #[test]
    fn pi_minus_pi_is_exact_zero() {
        let mut pool = ExprPool::new();
        let pi = pi(&mut pool);
        let difference = apply(&mut pool, Operator::Sub, &[pi, pi]);

        expect_rational(&mut pool, difference, 0, 1);
    }

    #[test]
    fn multiples_of_pi_combine() {
        let mut pool = ExprPool::new();
        let sixth = pi_over(&mut pool, 6);
        let third = pi_over(&mut pool, 3);
        let sum = apply(&mut pool, Operator::Add, &[sixth, third]);
        let expected = pi_times(&mut pool, 1, 2);

        expect_expression(&mut pool, sum, ResultKind::Symbolic, expected);
    }

    #[test]
    fn sine_of_arcsine_round_trips() {
        let mut pool = ExprPool::new();
        let half_root_three = {
            let half = fraction(&mut pool, 1, 2);
            let square_root_three = root(&mut pool, 3);
            apply(&mut pool, Operator::Mul, &[half, square_root_three])
        };
        let arcsine = apply(&mut pool, Operator::Asin, &[half_root_three]);
        let sine = apply(&mut pool, Operator::Sin, &[arcsine]);
        let expected = evaluate_exact(&mut pool, half_root_three)
            .unwrap()
            .expression();

        expect_expression(&mut pool, sine, ResultKind::Algebraic, expected);
    }

    #[test]
    fn evaluating_an_evaluated_expression_returns_it_unchanged() {
        let mut pool = ExprPool::new();
        let square_root_two = root(&mut pool, 2);
        let square_root_six = root(&mut pool, 6);
        let third = fraction(&mut pool, -1, 3);
        let scaled = apply(&mut pool, Operator::Mul, &[third, square_root_six]);
        let one = integer(&mut pool, 1);
        let partial = apply(&mut pool, Operator::Sub, &[scaled, one]);
        let sum = apply(&mut pool, Operator::Add, &[partial, square_root_two]);
        let first = evaluate_exact(&mut pool, sum).unwrap().expression();

        let second = evaluate_exact(&mut pool, first).unwrap().expression();

        assert_eq!(first, second);
    }

    #[test]
    fn quantity_product_is_exact_in_coherent_units_with_its_named_unit() {
        let mut pool = ExprPool::new();
        let mass_value = fraction(&mut pool, 5, 2);
        let kilogram = pool.units_mut().lookup("kg").unwrap();
        let mass = pool.quantity(mass_value, kilogram).unwrap();
        let acceleration_value = fraction(&mut pool, 981, 100);
        let metre = pool.units_mut().lookup("m").unwrap();
        let second = pool.units_mut().lookup("s").unwrap();
        let squared = pool.units_mut().power(second, 2).unwrap();
        let acceleration_unit = pool.units_mut().divide(metre, squared).unwrap();
        let acceleration = pool
            .quantity(acceleration_value, acceleration_unit)
            .unwrap();
        let force = apply(&mut pool, Operator::Mul, &[mass, acceleration]);
        let newton = pool.units_mut().lookup("N").unwrap();

        let evaluation = evaluate_exact(&mut pool, force).unwrap();

        let expected = Number::fraction(&Integer::from(981_i64), &Integer::from(40_i64)).unwrap();
        assert_eq!(
            (evaluation.rational_value(), evaluation.unit()),
            (Some(&expected), Some(newton))
        );
    }

    #[test]
    fn sine_of_thirty_degrees_is_exactly_one_half() {
        let mut pool = ExprPool::new();
        let thirty = integer(&mut pool, 30);
        let degree = pool.units_mut().lookup("deg").unwrap();
        let angle = pool.quantity(thirty, degree).unwrap();
        let sine = apply(&mut pool, Operator::Sin, &[angle]);

        let evaluation = evaluate_exact(&mut pool, sine).unwrap();

        let half = Number::fraction(&Integer::from(1_i64), &Integer::from(2_i64)).unwrap();
        assert_eq!(
            (evaluation.rational_value(), evaluation.unit()),
            (Some(&half), None)
        );
    }

    #[test]
    fn arcsine_converted_to_degrees_is_exactly_thirty() {
        let mut pool = ExprPool::new();
        let half = fraction(&mut pool, 1, 2);
        let arcsine = apply(&mut pool, Operator::Asin, &[half]);
        let one = integer(&mut pool, 1);
        let degree = pool.units_mut().lookup("deg").unwrap();
        let one_degree = pool.quantity(one, degree).unwrap();
        let conversion = apply(&mut pool, Operator::ConvertUnit, &[arcsine, one_degree]);

        let evaluation = evaluate_exact(&mut pool, conversion).unwrap();

        assert_eq!(
            (evaluation.rational_value(), evaluation.unit()),
            (Some(&Number::from(30_i64)), Some(degree))
        );
    }

    #[test]
    fn inhomogeneous_sum_is_a_quantity_error() {
        let mut pool = ExprPool::new();
        let three = integer(&mut pool, 3);
        let metre = pool.units_mut().lookup("m").unwrap();
        let second = pool.units_mut().lookup("s").unwrap();
        let length = pool.quantity(three, metre).unwrap();
        let time = pool.quantity(three, second).unwrap();
        let sum = apply(&mut pool, Operator::Add, &[length, time]);

        assert!(matches!(
            evaluate_exact(&mut pool, sum),
            Err(ExactEvaluationError::Quantity(
                QuantityError::DimensionMismatch { .. }
            ))
        ));
    }

    #[test]
    fn deeply_nested_sum_does_not_exhaust_the_stack() {
        let mut pool = ExprPool::new();
        let one = integer(&mut pool, 1);
        let mut sum = one;
        for _ in 1..200_000 {
            sum = apply(&mut pool, Operator::Add, &[sum, one]);
        }

        expect_rational(&mut pool, sum, 200_000, 1);
    }

    #[test]
    fn rational_multiple_of_pi_reports_its_coefficient() {
        let mut pool = ExprPool::new();
        let pi = pi(&mut pool);
        let six = integer(&mut pool, 6);
        let quotient = apply(&mut pool, Operator::Div, &[pi, six]);

        let evaluation = evaluate_exact(&mut pool, quotient).unwrap();

        assert_eq!(
            evaluation.pi_power(),
            Some((
                &Number::fraction(&Integer::from(1_i64), &Integer::from(6_i64)).unwrap(),
                1
            ))
        );
    }

    #[test]
    fn square_root_has_no_pi_power() {
        let mut pool = ExprPool::new();
        let two = integer(&mut pool, 2);
        let root = apply(&mut pool, Operator::Sqrt, &[two]);

        let evaluation = evaluate_exact(&mut pool, root).unwrap();

        assert_eq!(evaluation.pi_power(), None);
    }

    fn pi_power_of(pool: &mut ExprPool, expression: ExprId) -> Option<(Number, i32)> {
        evaluate_exact(pool, expression)
            .unwrap()
            .pi_power()
            .map(|(coefficient, exponent)| (coefficient.clone(), exponent))
    }

    fn sixth() -> Number {
        Number::fraction(&Integer::from(1_i64), &Integer::from(6_i64)).unwrap()
    }

    #[test]
    fn square_of_pi_over_six_is_a_second_power_of_pi() {
        let mut pool = ExprPool::new();
        let pi = pi(&mut pool);
        let two = integer(&mut pool, 2);
        let six = integer(&mut pool, 6);
        let square = apply(&mut pool, Operator::Pow, &[pi, two]);
        let quotient = apply(&mut pool, Operator::Div, &[square, six]);

        assert_eq!(pi_power_of(&mut pool, quotient), Some((sixth(), 2)));
    }

    #[test]
    fn reciprocal_of_pi_is_a_negative_power_of_pi() {
        let mut pool = ExprPool::new();
        let one = integer(&mut pool, 1);
        let pi = pi(&mut pool);
        let quotient = apply(&mut pool, Operator::Div, &[one, pi]);

        assert_eq!(
            pi_power_of(&mut pool, quotient),
            Some((Number::from(1_i64), -1))
        );
    }

    #[test]
    fn product_of_pi_multiples_adds_the_powers() {
        let mut pool = ExprPool::new();
        let pi = pi(&mut pool);
        let half = fraction(&mut pool, 1, 2);
        let half_pi = apply(&mut pool, Operator::Mul, &[half, pi]);
        let product = apply(&mut pool, Operator::Mul, &[half_pi, pi]);

        assert_eq!(
            pi_power_of(&mut pool, product),
            Some((
                Number::fraction(&Integer::from(1_i64), &Integer::from(2_i64)).unwrap(),
                2
            ))
        );
    }

    #[test]
    fn sum_of_equal_powers_of_pi_adds_the_coefficients() {
        let mut pool = ExprPool::new();
        let pi = pi(&mut pool);
        let two = integer(&mut pool, 2);
        let square = apply(&mut pool, Operator::Pow, &[pi, two]);
        let sum = apply(&mut pool, Operator::Add, &[square, square]);

        assert_eq!(pi_power_of(&mut pool, sum), Some((Number::from(2_i64), 2)));
    }

    #[test]
    fn quotient_of_equal_powers_of_pi_is_rational() {
        let mut pool = ExprPool::new();
        let pi = pi(&mut pool);
        let two = integer(&mut pool, 2);
        let square = apply(&mut pool, Operator::Pow, &[pi, two]);
        let quotient = apply(&mut pool, Operator::Div, &[square, square]);

        expect_rational(&mut pool, quotient, 1, 1);
    }

    #[test]
    fn sum_of_different_powers_of_pi_is_symbolic() {
        let mut pool = ExprPool::new();
        let pi = pi(&mut pool);
        let two = integer(&mut pool, 2);
        let square = apply(&mut pool, Operator::Pow, &[pi, two]);
        let sum = apply(&mut pool, Operator::Add, &[square, pi]);

        assert_eq!(pi_power_of(&mut pool, sum), None);
    }

    #[test]
    fn logarithm_of_a_negative_power_of_pi_is_outside_the_domain() {
        let mut pool = ExprPool::new();
        let pi = pi(&mut pool);
        let two = integer(&mut pool, 2);
        let square = apply(&mut pool, Operator::Pow, &[pi, two]);
        let negative = apply(&mut pool, Operator::Neg, &[square]);
        let logarithm = apply(&mut pool, Operator::Ln, &[negative]);

        assert!(matches!(
            evaluate_exact(&mut pool, logarithm),
            Err(ExactEvaluationError::OutsideDomain { .. })
        ));
    }
}
