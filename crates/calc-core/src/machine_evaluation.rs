use std::collections::{BTreeMap, HashMap};

use calc_exec::{
    Backend, BackendKind, Batch, BatchError, Domain, Gate, LowerError, LowerSpec, OperationClass,
    Preference, RunError, SelectError, SkippedBackend, ValueForm, lower, select,
};
use calc_expr::{
    AccessError, BinderKind, BuiltinConstant, ExprId, ExprPool, Head, NodeView, Operator,
    ReductionShape, SymbolId,
};
use calc_numbers::{Integer, Interval, Number, Truth};

use crate::computed_result::{
    ComputedResult, ComputedResultError, Diagnostic, Method, PLAN_EVALUATION_METHOD,
    ParameterValue, ResultKind, ResultValue,
};
use crate::exact_evaluation::evaluate_exact;
use crate::quantities::{QuantityError, to_coherent_units};

const DOMAIN_PARAMETER: &str = "domain";
const DOMAIN_F32: &str = "f32";
const DOMAIN_F64: &str = "f64";
const SHAPE_PARAMETER: &str = "reduction_shape";
const LEFT_FOLD: &str = "left_fold";
const HALVING: &str = "halving";
const MACHINE_OPERATIONS_BLOCK: &str = "M-NUM-D-006";
const ROUNDING_DEVIATION_BLOCK: &str = "M-NUM-D-005";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineEvaluation {
    result: ComputedResult,
    backend: BackendKind,
    skipped: Vec<SkippedBackend>,
    has_approximate_operations: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MachineEvaluationError {
    Access(AccessError),
    Lower(LowerError),
    IndexRangeTooLong(ExprId),
    Batch(BatchError),
    Select(SelectError),
    Run(RunError),
    Result(ComputedResultError),
    Quantity(QuantityError),
}

impl MachineEvaluation {
    pub fn result(&self) -> &ComputedResult {
        &self.result
    }

    pub fn backend(&self) -> BackendKind {
        self.backend
    }

    pub fn skipped(&self) -> &[SkippedBackend] {
        &self.skipped
    }

    pub fn has_approximate_operations(&self) -> bool {
        self.has_approximate_operations
    }
}

#[derive(Clone, Copy, Debug)]
enum Enclosure {
    Real(Option<Interval>),
    Boolean(Truth),
    Complex {
        real: Option<Interval>,
        imaginary: Option<Interval>,
    },
}

struct Reduction {
    shape: ReductionShape,
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

fn factorial_as_number(pool: &mut ExprPool, factorial: ExprId) -> ExprId {
    let Ok(evaluation) = evaluate_exact(pool, factorial) else {
        return factorial;
    };
    match (evaluation.unit(), evaluation.rational_value()) {
        (None, Some(Number::Integer(_))) => evaluation.expression(),
        _ => factorial,
    }
}

fn reduced_factorials(pool: &mut ExprPool, expression: ExprId) -> ExprId {
    let Ok(view) = pool.node(expression) else {
        return expression;
    };
    match view {
        NodeView::Number(_) | NodeView::Symbol(_) | NodeView::Bound(_) => expression,
        NodeView::Apply { head, arguments } => {
            let arguments = arguments.to_vec();
            let reduced = reduced_each(pool, &arguments);
            let applied = if reduced == arguments {
                expression
            } else {
                pool.apply(head, &reduced).unwrap_or(expression)
            };
            match head {
                Head::Operator(Operator::Factorial) => factorial_as_number(pool, applied),
                _ => applied,
            }
        }
        NodeView::Bind {
            binder,
            arguments,
            body,
        } => {
            let arguments = arguments.to_vec();
            let reduced = reduced_each(pool, &arguments);
            let reduced_body = reduced_factorials(pool, body);
            if reduced == arguments && reduced_body == body {
                return expression;
            }
            let bound_name = pool.bound_name(expression).map(str::to_string);
            let Ok(rebuilt) = pool.bind(binder, &reduced, reduced_body) else {
                return expression;
            };
            if let Some(name) = bound_name
                && pool.record_bound_name(rebuilt, &name).is_err()
            {
                return expression;
            }
            rebuilt
        }
        NodeView::Quantity { value, unit } => {
            let reduced = reduced_factorials(pool, value);
            if reduced == value {
                expression
            } else {
                pool.quantity(reduced, unit).unwrap_or(expression)
            }
        }
        NodeView::Array { shape, elements } => {
            let shape = shape.to_vec();
            let elements = elements.to_vec();
            let reduced = reduced_each(pool, &elements);
            if reduced == elements {
                expression
            } else {
                pool.array(&shape, &reduced).unwrap_or(expression)
            }
        }
    }
}

fn reduced_each(pool: &mut ExprPool, expressions: &[ExprId]) -> Vec<ExprId> {
    expressions
        .iter()
        .map(|expression| reduced_factorials(pool, *expression))
        .collect()
}

fn without_conversion_to_domain(pool: &ExprPool, root: ExprId, domain: Domain) -> ExprId {
    let conversion = match domain {
        Domain::F32 => Operator::ToF32,
        Domain::F64 => Operator::ToF64,
    };
    let mut inner = root;
    loop {
        match pool.node(inner) {
            Ok(NodeView::Apply {
                head: Head::Operator(operator),
                arguments: [argument],
            }) if operator == conversion => inner = *argument,
            _ => return inner,
        }
    }
}

fn reduction_of(
    pool: &ExprPool,
    root: ExprId,
) -> Result<Option<Reduction>, MachineEvaluationError> {
    let NodeView::Bind {
        binder,
        arguments,
        body,
    } = pool.node(root).map_err(MachineEvaluationError::Access)?
    else {
        return Ok(None);
    };
    let (shape, is_product) = match binder {
        BinderKind::Sum(shape) => (shape, false),
        BinderKind::Product(shape) => (shape, true),
        _ => return Ok(None),
    };
    let [lower_bound, upper_bound] = arguments else {
        return Ok(None);
    };
    let (Some(lower_bound), Some(upper_bound)) = (
        integer_constant(pool, *lower_bound),
        integer_constant(pool, *upper_bound),
    ) else {
        return Ok(None);
    };
    let span = &upper_bound - &lower_bound;
    let count = span
        .to_i64()
        .and_then(|span| span.checked_add(1))
        .and_then(|count| usize::try_from(count).ok())
        .ok_or(MachineEvaluationError::IndexRangeTooLong(root))?;
    Ok(Some(Reduction {
        shape,
        is_product,
        lower_bound,
        count,
        body,
    }))
}

fn index_values(reduction: &Reduction) -> impl Iterator<Item = Integer> + '_ {
    (0..reduction.count).map(|offset| {
        let offset = Integer::from(u64::try_from(offset).unwrap_or(u64::MAX));
        &reduction.lower_bound + &offset
    })
}

pub(crate) fn constant_enclosure(value: f64) -> Option<Interval> {
    let point = Interval::point(value)?;
    let below = Interval::point(value.next_down())?;
    let above = Interval::point(value.next_up())?;
    Some(point.hull(&below).hull(&above))
}

struct Encloser<'pool> {
    pool: &'pool ExprPool,
    bound: Option<Interval>,
    inputs: HashMap<SymbolId, Enclosure>,
    memo: HashMap<ExprId, Enclosure>,
    clips_domains: bool,
    clipped: bool,
}

fn real(enclosure: Enclosure) -> Option<Interval> {
    match enclosure {
        Enclosure::Real(interval) => interval,
        Enclosure::Boolean(_) | Enclosure::Complex { .. } => None,
    }
}

fn truth(enclosure: Enclosure) -> Truth {
    match enclosure {
        Enclosure::Boolean(truth) => truth,
        Enclosure::Real(_) | Enclosure::Complex { .. } => Truth::Unknown,
    }
}

fn complex_parts(enclosure: Enclosure) -> (Option<Interval>, Option<Interval>) {
    match enclosure {
        Enclosure::Real(interval) => (interval, Interval::point(0.0)),
        Enclosure::Complex { real, imaginary } => (real, imaginary),
        Enclosure::Boolean(_) => (None, None),
    }
}

fn binary(
    left: Option<Interval>,
    right: Option<Interval>,
    operation: impl Fn(&Interval, &Interval) -> Option<Interval>,
) -> Option<Interval> {
    operation(&left?, &right?)
}

fn comparison(
    left: Option<Interval>,
    right: Option<Interval>,
    compare: impl Fn(&Interval, &Interval) -> Truth,
) -> Truth {
    match (left, right) {
        (Some(left), Some(right)) => compare(&left, &right),
        _ => Truth::Unknown,
    }
}

impl<'pool> Encloser<'pool> {
    fn enclose(&mut self, expression: ExprId) -> Enclosure {
        if let Some(enclosure) = self.memo.get(&expression) {
            return *enclosure;
        }
        let enclosure = match self.pool.node(expression) {
            Ok(NodeView::Number(number)) => Enclosure::Real(
                self.pool
                    .number_value(number)
                    .ok()
                    .and_then(Interval::from_exact),
            ),
            Ok(NodeView::Symbol(symbol)) => self.symbol(symbol),
            Ok(NodeView::Bound(0)) => Enclosure::Real(self.bound),
            Ok(NodeView::Apply {
                head: Head::Operator(operator),
                arguments,
            }) => self.apply(operator, arguments),
            _ => Enclosure::Real(None),
        };
        self.memo.insert(expression, enclosure);
        enclosure
    }

    fn symbol(&self, symbol: SymbolId) -> Enclosure {
        if let Some(input) = self.inputs.get(&symbol) {
            *input
        } else if symbol == BuiltinConstant::Pi.symbol() {
            Enclosure::Real(constant_enclosure(std::f64::consts::PI))
        } else if symbol == BuiltinConstant::E.symbol() {
            Enclosure::Real(constant_enclosure(std::f64::consts::E))
        } else if symbol == BuiltinConstant::ImaginaryUnit.symbol() {
            Enclosure::Complex {
                real: Interval::point(0.0),
                imaginary: Interval::point(1.0),
            }
        } else {
            Enclosure::Real(None)
        }
    }

    fn within_domain(&mut self, value: Option<Interval>, low: f64, high: f64) -> Option<Interval> {
        let value = value?;
        if !self.clips_domains || (value.lower() >= low && value.upper() <= high) {
            return Some(value);
        }
        let (lower, upper) = (value.lower().max(low), value.upper().min(high));
        if lower > upper {
            return Some(value);
        }
        self.clipped = true;
        Some(Interval::point(lower)?.hull(&Interval::point(upper)?))
    }

    fn apply(&mut self, operator: Operator, arguments: &[ExprId]) -> Enclosure {
        let values: Vec<Enclosure> = arguments
            .iter()
            .map(|argument| self.enclose(*argument))
            .collect();
        let is_complex = values
            .iter()
            .any(|value| matches!(value, Enclosure::Complex { .. }));
        if operator == Operator::Complex {
            return match values.as_slice() {
                [real_part, imaginary_part] => Enclosure::Complex {
                    real: real(*real_part),
                    imaginary: real(*imaginary_part),
                },
                _ => Enclosure::Real(None),
            };
        }
        if is_complex {
            return complex_operation(operator, &values);
        }
        let first = values.first().copied().and_then(real);
        let second = values.get(1).copied().and_then(real);
        let third = values.get(2).copied().and_then(real);
        match operator {
            Operator::Add => Enclosure::Real(binary(first, second, Interval::add)),
            Operator::Sub => Enclosure::Real(binary(first, second, Interval::sub)),
            Operator::Mul => Enclosure::Real(binary(first, second, Interval::mul)),
            Operator::Div => Enclosure::Real(binary(first, second, Interval::div)),
            Operator::Neg => Enclosure::Real(first.map(|value| value.neg())),
            Operator::Abs => Enclosure::Real(first.map(|value| value.abs())),
            Operator::Sqrt => Enclosure::Real(
                self.within_domain(first, 0.0, f64::INFINITY)
                    .and_then(|value| value.sqrt()),
            ),
            Operator::MulAdd => {
                Enclosure::Real(first.and_then(|factor| factor.mul_add(&second?, &third?)))
            }
            Operator::Floor => Enclosure::Real(first.map(|value| value.floor())),
            Operator::Ceil => Enclosure::Real(first.map(|value| value.ceil())),
            Operator::Trunc => Enclosure::Real(first.map(|value| value.trunc())),
            Operator::RoundTiesEven => Enclosure::Real(first.map(|value| value.round_ties_even())),
            Operator::CopySign => Enclosure::Real(binary(first, second, Interval::copy_sign)),
            Operator::Min => {
                Enclosure::Real(binary(first, second, |left, right| Some(left.min(right))))
            }
            Operator::Max => {
                Enclosure::Real(binary(first, second, |left, right| Some(left.max(right))))
            }
            Operator::Exp => Enclosure::Real(first.map(|value| value.exp())),
            Operator::Ln => Enclosure::Real(first.and_then(|value| value.ln())),
            Operator::Sin => Enclosure::Real(first.and_then(|value| value.sin())),
            Operator::Cos => Enclosure::Real(first.and_then(|value| value.cos())),
            Operator::Tan => Enclosure::Real(first.and_then(|value| value.tan())),
            Operator::Asin => Enclosure::Real(
                self.within_domain(first, -1.0, 1.0)
                    .and_then(|value| value.asin()),
            ),
            Operator::Acos => Enclosure::Real(
                self.within_domain(first, -1.0, 1.0)
                    .and_then(|value| value.acos()),
            ),
            Operator::Atan => Enclosure::Real(first.and_then(|value| value.atan())),
            Operator::Atan2 => Enclosure::Real(binary(first, second, Interval::atan2)),
            Operator::Pow => Enclosure::Real(binary(first, second, Interval::pow)),
            Operator::Equal => Enclosure::Boolean(comparison(first, second, Interval::equal)),
            Operator::NotEqual => {
                Enclosure::Boolean(comparison(first, second, Interval::equal).negated())
            }
            Operator::Less => Enclosure::Boolean(comparison(first, second, Interval::less)),
            Operator::LessOrEqual => {
                Enclosure::Boolean(comparison(first, second, Interval::less_or_equal))
            }
            Operator::Greater => Enclosure::Boolean(comparison(second, first, Interval::less)),
            Operator::GreaterOrEqual => {
                Enclosure::Boolean(comparison(second, first, Interval::less_or_equal))
            }
            Operator::And => enclosure_of_truths(&values, Truth::and),
            Operator::Or => enclosure_of_truths(&values, Truth::or),
            Operator::Not => Enclosure::Boolean(
                values
                    .first()
                    .copied()
                    .map_or(Truth::Unknown, truth)
                    .negated(),
            ),
            Operator::Select => select_enclosure(&values),
            Operator::ToF64 | Operator::ToF32 => {
                values.first().copied().unwrap_or(Enclosure::Real(None))
            }
            _ => Enclosure::Real(None),
        }
    }
}

fn enclosure_of_truths(values: &[Enclosure], combine: fn(Truth, Truth) -> Truth) -> Enclosure {
    match values {
        [left, right] => Enclosure::Boolean(combine(truth(*left), truth(*right))),
        _ => Enclosure::Boolean(Truth::Unknown),
    }
}

fn select_enclosure(values: &[Enclosure]) -> Enclosure {
    let [condition, when_true, when_false] = values else {
        return Enclosure::Real(None);
    };
    match truth(*condition) {
        Truth::True => *when_true,
        Truth::False => *when_false,
        Truth::Unknown => Enclosure::Real(binary(
            real(*when_true),
            real(*when_false),
            |left, right| Some(left.hull(right)),
        )),
    }
}

fn complex_operation(operator: Operator, values: &[Enclosure]) -> Enclosure {
    let (left_real, left_imaginary) = values.first().copied().map_or((None, None), complex_parts);
    let (right_real, right_imaginary) = values.get(1).copied().map_or((None, None), complex_parts);
    match operator {
        Operator::Neg => Enclosure::Complex {
            real: left_real.map(|value| value.neg()),
            imaginary: left_imaginary.map(|value| value.neg()),
        },
        Operator::Add => Enclosure::Complex {
            real: binary(left_real, right_real, Interval::add),
            imaginary: binary(left_imaginary, right_imaginary, Interval::add),
        },
        Operator::Sub => Enclosure::Complex {
            real: binary(left_real, right_real, Interval::sub),
            imaginary: binary(left_imaginary, right_imaginary, Interval::sub),
        },
        Operator::Mul => {
            let real_real = binary(left_real, right_real, Interval::mul);
            let imaginary_imaginary = binary(left_imaginary, right_imaginary, Interval::mul);
            let real_imaginary = binary(left_real, right_imaginary, Interval::mul);
            let imaginary_real = binary(left_imaginary, right_real, Interval::mul);
            Enclosure::Complex {
                real: binary(real_real, imaginary_imaginary, Interval::sub),
                imaginary: binary(real_imaginary, imaginary_real, Interval::add),
            }
        }
        _ => Enclosure::Complex {
            real: None,
            imaginary: None,
        },
    }
}

pub(crate) fn real_enclosure(pool: &ExprPool, expression: ExprId) -> Option<Interval> {
    match enclose_expression(pool, expression, None) {
        Enclosure::Real(enclosure) => enclosure,
        Enclosure::Boolean(_) | Enclosure::Complex { .. } => None,
    }
}

pub(crate) fn is_provably_not_real(pool: &ExprPool, expression: ExprId) -> bool {
    matches!(
        enclose_expression(pool, expression, None),
        Enclosure::Complex {
            imaginary: Some(imaginary),
            ..
        } if !imaginary.contains_zero()
    )
}

fn enclose_expression(pool: &ExprPool, root: ExprId, reduction: Option<&Reduction>) -> Enclosure {
    let Some(reduction) = reduction else {
        let mut encloser = Encloser {
            pool,
            bound: None,
            inputs: HashMap::new(),
            memo: HashMap::new(),
            clips_domains: false,
            clipped: false,
        };
        return encloser.enclose(root);
    };
    let mut total: Option<Interval> = None;
    for index in index_values(reduction) {
        let mut encloser = Encloser {
            pool,
            bound: Interval::from_exact(&Number::Integer(index)),
            inputs: HashMap::new(),
            memo: HashMap::new(),
            clips_domains: false,
            clipped: false,
        };
        let term = real(encloser.enclose(reduction.body));
        total = match total {
            None => term,
            Some(partial) => {
                let combine = if reduction.is_product {
                    Interval::mul
                } else {
                    Interval::add
                };
                binary(Some(partial), term, combine)
            }
        };
        if total.is_none() {
            return Enclosure::Real(None);
        }
    }
    Enclosure::Real(total)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SamplePosition {
    Real(Number),
    Complex { real: Number, imaginary: Number },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleInput {
    pub symbol: SymbolId,
    pub position: SamplePosition,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SampleEnclosure {
    Real(Option<Interval>),
    Complex {
        real: Option<Interval>,
        imaginary: Option<Interval>,
    },
}

fn bound_or_unknown(enclosure: Option<Interval>, sampled: f64) -> f64 {
    enclosure
        .and_then(|interval| interval.distance_bound(sampled))
        .unwrap_or(f64::NAN)
}

impl SampleEnclosure {
    pub fn rounding_bound(&self, sampled: f64) -> f64 {
        match self {
            SampleEnclosure::Real(interval) => bound_or_unknown(*interval, sampled),
            SampleEnclosure::Complex { .. } => f64::NAN,
        }
    }

    pub fn complex_rounding_bounds(&self, sampled_real: f64, sampled_imaginary: f64) -> (f64, f64) {
        let (real, imaginary) = match self {
            SampleEnclosure::Real(interval) => (*interval, Interval::point(0.0)),
            SampleEnclosure::Complex { real, imaginary } => (*real, *imaginary),
        };
        (
            bound_or_unknown(real, sampled_real),
            bound_or_unknown(imaginary, sampled_imaginary),
        )
    }
}

fn position_enclosure(position: &SamplePosition) -> Enclosure {
    match position {
        SamplePosition::Real(value) => Enclosure::Real(Interval::from_exact(value)),
        SamplePosition::Complex { real, imaginary } => Enclosure::Complex {
            real: Interval::from_exact(real),
            imaginary: Interval::from_exact(imaginary),
        },
    }
}

pub fn enclose_sample(
    pool: &ExprPool,
    expression: ExprId,
    inputs: &[SampleInput],
) -> SampleEnclosure {
    let mut encloser = Encloser {
        pool,
        bound: None,
        inputs: inputs
            .iter()
            .map(|input| (input.symbol, position_enclosure(&input.position)))
            .collect(),
        memo: HashMap::new(),
        clips_domains: false,
        clipped: false,
    };
    match encloser.enclose(expression) {
        Enclosure::Real(interval) => SampleEnclosure::Real(interval),
        Enclosure::Complex { real, imaginary } => SampleEnclosure::Complex { real, imaginary },
        Enclosure::Boolean(_) => SampleEnclosure::Real(None),
    }
}

pub fn enclose_over(
    pool: &ExprPool,
    expression: ExprId,
    symbol: SymbolId,
    low: &Number,
    high: &Number,
) -> Option<(f64, f64, bool)> {
    let column = Interval::from_exact(low)?.hull(&Interval::from_exact(high)?);
    let mut encloser = Encloser {
        pool,
        bound: None,
        inputs: [(symbol, Enclosure::Real(Some(column)))]
            .into_iter()
            .collect(),
        memo: HashMap::new(),
        clips_domains: true,
        clipped: false,
    };
    match encloser.enclose(expression) {
        Enclosure::Real(Some(interval)) => {
            Some((interval.lower(), interval.upper(), encloser.clipped))
        }
        _ => None,
    }
}

fn with_bound(
    pool: &mut ExprPool,
    expression: ExprId,
    variable: ExprId,
    bound: ExprId,
) -> Option<ExprId> {
    if expression == variable {
        return Some(bound);
    }
    match pool.node(expression).ok()? {
        NodeView::Apply { head, arguments } => {
            let arguments = arguments.to_vec();
            let mut changed = Vec::with_capacity(arguments.len());
            for argument in arguments {
                changed.push(with_bound(pool, argument, variable, bound)?);
            }
            pool.apply(head, &changed).ok()
        }
        NodeView::Symbol(_) | NodeView::Number(_) => Some(expression),
        _ => None,
    }
}

pub fn slope_of(pool: &mut ExprPool, expression: ExprId, symbol: SymbolId) -> Option<ExprId> {
    let variable = pool.symbol(symbol).ok()?;
    let bound = pool.bound(0).ok()?;
    let body = with_bound(pool, expression, variable, bound)?;
    crate::derivative::derivative(pool, body, 0)
}

pub fn enclose_slope_over(
    pool: &ExprPool,
    slope: ExprId,
    low: &Number,
    high: &Number,
) -> Option<(f64, f64)> {
    let column = Interval::from_exact(low)?.hull(&Interval::from_exact(high)?);
    let mut encloser = Encloser {
        pool,
        bound: Some(column),
        inputs: HashMap::new(),
        memo: HashMap::new(),
        clips_domains: false,
        clipped: false,
    };
    match encloser.enclose(slope) {
        Enclosure::Real(Some(interval)) => Some((interval.lower(), interval.upper())),
        _ => None,
    }
}

enum Attained {
    Constant,
    Between(Interval),
}

fn mentions(pool: &ExprPool, expression: ExprId, symbol: SymbolId) -> bool {
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

impl Encloser<'_> {
    fn attained(
        &mut self,
        expression: ExprId,
        symbol: SymbolId,
        column: Interval,
    ) -> Option<Attained> {
        match self.pool.node(expression).ok()? {
            NodeView::Symbol(found) if found == symbol => Some(Attained::Between(column)),
            NodeView::Number(_) | NodeView::Symbol(_) => Some(Attained::Constant),
            NodeView::Apply {
                head: Head::Operator(operator),
                arguments,
            } if is_continuous_operator(operator) => {
                let mut varying = arguments
                    .iter()
                    .filter(|argument| mentions(self.pool, **argument, symbol));
                let Some(argument) = varying.next().copied() else {
                    return Some(Attained::Constant);
                };
                if varying.next().is_some() {
                    return None;
                }
                let Attained::Between(inner) = self.attained(argument, symbol, column)? else {
                    return None;
                };
                let attained = match operator {
                    Operator::Sin => inner.sin_attained()?,
                    Operator::Cos => inner.cos_attained()?,
                    _ => {
                        let mut points = vec![inner.lower(), inner.upper()];
                        if matches!(operator, Operator::Abs | Operator::Pow)
                            && inner.lower() < 0.0
                            && inner.upper() > 0.0
                        {
                            points.push(0.0);
                        }
                        let mut values = Vec::with_capacity(points.len());
                        for point in points {
                            self.memo
                                .insert(argument, Enclosure::Real(Interval::point(point)));
                            values.push(real(self.apply(operator, arguments))?);
                        }
                        Interval::attained_between(&values)?
                    }
                };
                Some(Attained::Between(attained))
            }
            _ => None,
        }
    }
}

pub fn attained_over(
    pool: &ExprPool,
    expression: ExprId,
    symbol: SymbolId,
    low: &Number,
    high: &Number,
) -> Option<(f64, f64)> {
    let column =
        Interval::attained_between(&[Interval::from_exact(low)?, Interval::from_exact(high)?])?;
    let mut encloser = Encloser {
        pool,
        bound: None,
        inputs: HashMap::new(),
        memo: HashMap::new(),
        clips_domains: false,
        clipped: false,
    };
    match encloser.attained(expression, symbol, column)? {
        Attained::Between(interval) => Some((interval.lower(), interval.upper())),
        Attained::Constant => None,
    }
}

fn is_continuous_operator(operator: Operator) -> bool {
    matches!(
        operator,
        Operator::Add
            | Operator::Sub
            | Operator::Mul
            | Operator::Div
            | Operator::Neg
            | Operator::Abs
            | Operator::Sqrt
            | Operator::MulAdd
            | Operator::Min
            | Operator::Max
            | Operator::Exp
            | Operator::Ln
            | Operator::Sin
            | Operator::Cos
            | Operator::Tan
            | Operator::Asin
            | Operator::Acos
            | Operator::Atan
            | Operator::Atan2
            | Operator::Pow
    )
}

pub fn is_continuous_where_enclosed(pool: &ExprPool, expression: ExprId) -> bool {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        match pool.node(node) {
            Ok(NodeView::Number(_) | NodeView::Symbol(_) | NodeView::Bound(0)) => {}
            Ok(NodeView::Apply {
                head: Head::Operator(operator),
                arguments,
            }) if is_continuous_operator(operator) => pending.extend(arguments.iter().copied()),
            _ => return false,
        }
    }
    true
}

fn rounding_bound(enclosure: Option<Interval>, value: f64) -> Option<Number> {
    enclosure
        .and_then(|interval| interval.distance_bound(value))
        .map(Number::F64)
}

fn shape_name(shape: ReductionShape) -> &'static str {
    match shape {
        ReductionShape::LeftFold => LEFT_FOLD,
        ReductionShape::Halving => HALVING,
    }
}

fn lower_in(
    pool: &ExprPool,
    root: ExprId,
    domain: Domain,
) -> Result<calc_exec::Plan, MachineEvaluationError> {
    let spec = |output| LowerSpec {
        domain,
        inputs: Vec::new(),
        output,
    };
    match lower(pool, root, &spec(ValueForm::Real)) {
        Err(LowerError::OutputFormMismatch { .. }) => {
            lower(pool, root, &spec(ValueForm::Complex)).map_err(MachineEvaluationError::Lower)
        }
        other => other.map_err(MachineEvaluationError::Lower),
    }
}

fn domain_name(domain: Domain) -> &'static str {
    match domain {
        Domain::F32 => DOMAIN_F32,
        Domain::F64 => DOMAIN_F64,
    }
}

fn index_batch(
    reduction: Option<&Reduction>,
    domain: Domain,
) -> Result<(usize, Batch), MachineEvaluationError> {
    let Some(reduction) = reduction else {
        let empty = match domain {
            Domain::F32 => Batch::from_f32_columns(1, Vec::new()),
            Domain::F64 => Batch::from_f64_columns(1, Vec::new()),
        };
        return empty
            .map(|batch| (1, batch))
            .map_err(MachineEvaluationError::Batch);
    };
    let length = reduction.count;
    let batch = match domain {
        Domain::F32 => Batch::from_f32_columns(
            length,
            vec![
                index_values(reduction)
                    .map(|index| Number::Integer(index).round_to_f32_ties_even())
                    .collect(),
            ],
        ),
        Domain::F64 => Batch::from_f64_columns(
            length,
            vec![
                index_values(reduction)
                    .map(|index| Number::Integer(index).round_to_f64_ties_even())
                    .collect(),
            ],
        ),
    };
    batch
        .map(|batch| (length, batch))
        .map_err(MachineEvaluationError::Batch)
}

fn first_output(outputs: &Batch, domain: Domain, channel: usize) -> Number {
    match domain {
        Domain::F32 => Number::F32(
            outputs
                .f32_channel(channel)
                .and_then(|column| column.first().copied())
                .unwrap_or(f32::NAN),
        ),
        Domain::F64 => Number::F64(
            outputs
                .f64_channel(channel)
                .and_then(|column| column.first().copied())
                .unwrap_or(f64::NAN),
        ),
    }
}

fn widened(value: &Number) -> f64 {
    match value {
        Number::F32(single) => f64::from(*single),
        Number::F64(double) => *double,
        Number::Integer(_) | Number::Rational(_) => f64::NAN,
    }
}

pub fn evaluate_f64(
    pool: &mut ExprPool,
    expression: ExprId,
    backends: &[&dyn Backend],
    preference: Preference,
) -> Result<MachineEvaluation, MachineEvaluationError> {
    evaluate_in(pool, expression, backends, preference, Domain::F64)
}

pub fn evaluate_f32(
    pool: &mut ExprPool,
    expression: ExprId,
    backends: &[&dyn Backend],
    preference: Preference,
) -> Result<MachineEvaluation, MachineEvaluationError> {
    evaluate_in(pool, expression, backends, preference, Domain::F32)
}

const OVERFLOWED_NOTE: &str = "overflowed_to_infinity";
const NOT_A_NUMBER_NOTE: &str = "not_a_number_from_finite_operands";

fn not_finite_note(pool: &ExprPool, root: ExprId, value: &ResultValue) -> Option<Diagnostic> {
    let code = match not_finite_kind(value)? {
        NotFinite::Infinite => OVERFLOWED_NOTE,
        NotFinite::NotANumber => NOT_A_NUMBER_NOTE,
    };
    holds_only_finite_operands(pool, root).then(|| Diagnostic {
        code: code.to_owned(),
        data: BTreeMap::new(),
    })
}

enum NotFinite {
    Infinite,
    NotANumber,
}

fn not_finite_kind(value: &ResultValue) -> Option<NotFinite> {
    let numbers: Vec<&Number> = match value {
        ResultValue::Number(number) => vec![number],
        ResultValue::Complex { real, imaginary } => vec![real, imaginary],
        _ => Vec::new(),
    };
    let mut infinite = None;
    for number in numbers {
        match number {
            Number::F64(value) if value.is_nan() => return Some(NotFinite::NotANumber),
            Number::F32(value) if value.is_nan() => return Some(NotFinite::NotANumber),
            Number::F64(value) if value.is_infinite() => infinite = Some(NotFinite::Infinite),
            Number::F32(value) if value.is_infinite() => infinite = Some(NotFinite::Infinite),
            _ => {}
        }
    }
    infinite
}

fn holds_only_finite_operands(pool: &ExprPool, root: ExprId) -> bool {
    let mut pending = vec![root];
    let mut seen: Vec<ExprId> = Vec::new();
    while let Some(current) = pending.pop() {
        if seen.contains(&current) {
            continue;
        }
        seen.push(current);
        match pool.node(current) {
            Ok(NodeView::Number(number)) => {
                let finite = match pool.number_value(number) {
                    Ok(Number::F64(value)) => value.is_finite(),
                    Ok(Number::F32(value)) => value.is_finite(),
                    Ok(_) => true,
                    Err(_) => false,
                };
                if !finite {
                    return false;
                }
            }
            Ok(NodeView::Symbol(symbol)) => {
                if symbol == BuiltinConstant::Infinity.symbol() {
                    return false;
                }
            }
            Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().copied()),
            Ok(NodeView::Quantity { value, .. }) => pending.push(value),
            Ok(NodeView::Array { elements, .. }) => pending.extend(elements.iter().copied()),
            Ok(NodeView::Bind {
                arguments, body, ..
            }) => {
                pending.extend(arguments.iter().copied());
                pending.push(body);
            }
            _ => {}
        }
    }
    true
}

fn evaluate_in(
    pool: &mut ExprPool,
    expression: ExprId,
    backends: &[&dyn Backend],
    preference: Preference,
    domain: Domain,
) -> Result<MachineEvaluation, MachineEvaluationError> {
    let coherent = to_coherent_units(pool, expression).map_err(|error| match error {
        QuantityError::Access(access) => MachineEvaluationError::Access(access),
        other => MachineEvaluationError::Quantity(other),
    })?;
    let root = reduced_factorials(pool, coherent.expression);
    let pool: &ExprPool = pool;
    let root = without_conversion_to_domain(pool, root, domain);
    let plan = lower_in(pool, root, domain)?;
    let reduction = reduction_of(pool, root)?;
    let (length, inputs) = index_batch(reduction.as_ref(), domain)?;
    let mut outputs = Batch::zeroed(
        domain,
        plan.output_channel_count(),
        plan.output_length(length),
    );
    let mut selection =
        select(backends, &plan, length, preference).map_err(MachineEvaluationError::Select)?;
    selection
        .prepared
        .run(&inputs, &mut outputs)
        .map_err(MachineEvaluationError::Run)?;

    let enclosure = enclose_expression(pool, root, reduction.as_ref());
    let (value, bound) = match (plan.output_channel_count(), enclosure) {
        (2, enclosure) => {
            let (real_part, imaginary_part) = complex_parts(enclosure);
            let real_value = first_output(&outputs, domain, 0);
            let imaginary_value = first_output(&outputs, domain, 1);
            let bound = rounding_bound(real_part, widened(&real_value))
                .zip(rounding_bound(imaginary_part, widened(&imaginary_value)))
                .map(|(real_bound, imaginary_bound)| ResultValue::Complex {
                    real: real_bound,
                    imaginary: imaginary_bound,
                });
            (
                ResultValue::Complex {
                    real: real_value,
                    imaginary: imaginary_value,
                },
                bound,
            )
        }
        (_, enclosure) => {
            let value = first_output(&outputs, domain, 0);
            let bound = rounding_bound(real(enclosure), widened(&value)).map(ResultValue::Number);
            (ResultValue::Number(value), bound)
        }
    };

    let mut method = Method::named(PLAN_EVALUATION_METHOD);
    method.parameters.insert(
        DOMAIN_PARAMETER.to_string(),
        ParameterValue::Identifier(domain_name(domain).to_string()),
    );
    if let Some(note) = not_finite_note(pool, root, &value) {
        method.notes.push(note);
    }
    if let Some(reduction) = &reduction {
        method.parameters.insert(
            SHAPE_PARAMETER.to_string(),
            ParameterValue::Identifier(shape_name(reduction.shape).to_string()),
        );
    }
    let mut result = ComputedResult::new(ResultKind::MachineFloat, value, coherent.unit, method)
        .map_err(MachineEvaluationError::Result)?
        .with_corpus_reference(MACHINE_OPERATIONS_BLOCK)
        .with_corpus_reference(ROUNDING_DEVIATION_BLOCK);
    if let Some(bound) = bound {
        result = result
            .with_rounding_bound(bound)
            .map_err(MachineEvaluationError::Result)?;
    }
    Ok(MachineEvaluation {
        result,
        backend: selection.kind,
        skipped: selection.skipped,
        has_approximate_operations: plan.gates().iter().any(|gate| {
            matches!(gate, Gate::Operation { operation, .. } if operation.class() == OperationClass::Approximate)
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computed_result::RoundingError;
    use calc_exec_cpu::CpuBackend;
    use calc_expr::SymbolKind;

    const TIGHT_BOUND: f64 = 1.0e-12;

    struct Fixture {
        pool: ExprPool,
    }

    impl Fixture {
        fn new() -> Fixture {
            Fixture {
                pool: ExprPool::new(),
            }
        }

        fn exact(&mut self, numerator: i64, denominator: i64) -> ExprId {
            let value =
                Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap();
            self.pool.number(value).unwrap()
        }

        fn machine(&mut self, value: f64) -> ExprId {
            self.pool.number(Number::F64(value)).unwrap()
        }

        fn apply(&mut self, operator: Operator, arguments: &[ExprId]) -> ExprId {
            self.pool
                .apply(Head::Operator(operator), arguments)
                .unwrap()
        }

        fn constant(&mut self, constant: BuiltinConstant) -> ExprId {
            self.pool.symbol(constant.symbol()).unwrap()
        }

        fn select_on(&mut self, condition: ExprId) -> ExprId {
            let when_true = self.machine(1.0);
            let when_false = self.machine(2.0);
            self.apply(Operator::Select, &[condition, when_true, when_false])
        }

        fn evaluate(&mut self, root: ExprId) -> MachineEvaluation {
            let backend = CpuBackend::new();
            evaluate_f64(&mut self.pool, root, &[&backend], Preference::Automatic).unwrap()
        }
    }

    fn scalar(evaluation: &MachineEvaluation) -> f64 {
        match evaluation.result().value() {
            ResultValue::Number(Number::F64(value)) => *value,
            other => panic!("{other:?}"),
        }
    }

    fn bound(evaluation: &MachineEvaluation) -> Number {
        match evaluation.result().rounding_error() {
            RoundingError::Bound(ResultValue::Number(bound)) => bound.clone(),
            other => panic!("{other:?}"),
        }
    }

    fn fraction(numerator: i128, denominator: i128) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    fn is_at_most(left: &Number, right: &Number) -> bool {
        match right.sub_exact(left).unwrap() {
            Number::Integer(difference) => !difference.is_negative(),
            Number::Rational(difference) => !difference.numerator().is_negative(),
            _ => false,
        }
    }

    fn magnitude(value: Number) -> Number {
        if is_at_most(&value, &Number::from(0)) {
            value.negate_exact().unwrap()
        } else {
            value
        }
    }

    fn assert_bound_covers(evaluation: &MachineEvaluation, exact: &Number, tolerance: &Number) {
        let value = Number::F64(scalar(evaluation)).to_exact().unwrap();
        let error = magnitude(value.sub_exact(exact).unwrap());
        let bound = bound(evaluation).to_exact().unwrap();
        assert!(is_at_most(&error, &bound.add_exact(tolerance).unwrap()));
        assert!(is_at_most(
            &bound,
            &Number::F64(TIGHT_BOUND).to_exact().unwrap()
        ));
    }

    fn assert_exact_result(evaluation: &MachineEvaluation, exact: &Number) {
        assert_bound_covers(evaluation, exact, &Number::from(0));
    }

    fn reference(text: &str) -> (Number, Number) {
        let (whole, fraction_digits) = text.split_once('.').unwrap();
        let digits: i128 = format!("{whole}{fraction_digits}").parse().unwrap();
        let scale = u32::try_from(fraction_digits.len()).unwrap();
        let denominator = Integer::from(10i64).pow(scale);
        let value = Number::fraction(&Integer::from(digits), &denominator).unwrap();
        let tolerance = Number::fraction(&Integer::one(), &denominator).unwrap();
        (value, tolerance)
    }

    #[test]
    fn add_of_rounded_tenths_is_bounded() {
        let mut fixture = Fixture::new();
        let tenth = fixture.exact(1, 10);
        let fifth = fixture.exact(2, 10);
        let root = fixture.apply(Operator::Add, &[tenth, fifth]);

        let evaluation = fixture.evaluate(root);

        assert_eq!(scalar(&evaluation).to_bits(), (0.1f64 + 0.2).to_bits());
        assert_exact_result(&evaluation, &fraction(3, 10));
    }

    #[test]
    fn quantity_is_evaluated_in_coherent_units_with_unit_on_value_and_bound() {
        let mut fixture = Fixture::new();
        let tenth = fixture.exact(1, 10);
        let kilometre = fixture.pool.units_mut().lookup("km").unwrap();
        let length = fixture.pool.quantity(tenth, kilometre).unwrap();
        let three = fixture.exact(3, 1);
        let root = fixture.apply(Operator::Mul, &[length, three]);
        let metre = fixture.pool.units_mut().lookup("m").unwrap();

        let evaluation = fixture.evaluate(root);

        assert_eq!(evaluation.result().unit(), Some(metre));
        assert_exact_result(&evaluation, &fraction(300, 1));
    }

    #[test]
    fn arithmetic_has_no_approximate_operations() {
        let mut fixture = Fixture::new();
        let tenth = fixture.exact(1, 10);
        let fifth = fixture.exact(2, 10);
        let root = fixture.apply(Operator::Add, &[tenth, fifth]);

        assert!(!fixture.evaluate(root).has_approximate_operations());
    }

    #[test]
    fn elementary_function_is_an_approximate_operation() {
        let mut fixture = Fixture::new();
        let one = fixture.exact(1, 1);
        let root = fixture.apply(Operator::Exp, &[one]);

        assert!(fixture.evaluate(root).has_approximate_operations());
    }

    #[test]
    fn sub_is_bounded() {
        let mut fixture = Fixture::new();
        let one = fixture.exact(1, 1);
        let third = fixture.exact(1, 3);
        let root = fixture.apply(Operator::Sub, &[one, third]);

        assert_exact_result(&fixture.evaluate(root), &fraction(2, 3));
    }

    #[test]
    fn mul_is_bounded() {
        let mut fixture = Fixture::new();
        let tenth = fixture.exact(1, 10);
        let three = fixture.exact(3, 1);
        let root = fixture.apply(Operator::Mul, &[tenth, three]);

        assert_exact_result(&fixture.evaluate(root), &fraction(3, 10));
    }

    #[test]
    fn div_is_bounded() {
        let mut fixture = Fixture::new();
        let one = fixture.exact(1, 1);
        let seven = fixture.exact(7, 1);
        let root = fixture.apply(Operator::Div, &[one, seven]);

        assert_exact_result(&fixture.evaluate(root), &fraction(1, 7));
    }

    #[test]
    fn div_by_zero_has_unknown_rounding_error() {
        let mut fixture = Fixture::new();
        let one = fixture.exact(1, 1);
        let zero = fixture.exact(0, 1);
        let root = fixture.apply(Operator::Div, &[one, zero]);

        let evaluation = fixture.evaluate(root);

        assert_eq!(scalar(&evaluation), f64::INFINITY);
        assert_eq!(
            evaluation.result().rounding_error(),
            &RoundingError::Unknown
        );
    }

    #[test]
    fn neg_is_exact() {
        let mut fixture = Fixture::new();
        let quarter = fixture.exact(1, 4);
        let root = fixture.apply(Operator::Neg, &[quarter]);

        assert_exact_result(&fixture.evaluate(root), &fraction(-1, 4));
    }

    #[test]
    fn pow_is_bounded() {
        let mut fixture = Fixture::new();
        let tenth = fixture.exact(1, 10);
        let three = fixture.exact(3, 1);
        let root = fixture.apply(Operator::Pow, &[tenth, three]);

        assert_exact_result(&fixture.evaluate(root), &fraction(1, 1000));
    }

    #[test]
    fn sqrt_is_bounded() {
        let mut fixture = Fixture::new();
        let two = fixture.exact(2, 1);
        let root = fixture.apply(Operator::Sqrt, &[two]);
        let (exact, tolerance) = reference("1.4142135623730950488016887242");

        assert_bound_covers(&fixture.evaluate(root), &exact, &tolerance);
    }

    #[test]
    fn a_factorial_under_a_root_is_reduced_before_the_root() {
        let mut fixture = Fixture::new();
        let three = fixture.exact(3, 1);
        let factorial = fixture.apply(Operator::Factorial, &[three]);
        let root = fixture.apply(Operator::Sqrt, &[factorial]);
        let six = fixture.exact(6, 1);
        let over_six = fixture.apply(Operator::Sqrt, &[six]);

        let evaluation = fixture.evaluate(root);

        assert_eq!(scalar(&evaluation), scalar(&fixture.evaluate(over_six)));
    }

    #[test]
    fn a_root_over_a_factorial_is_bounded() {
        let mut fixture = Fixture::new();
        let three = fixture.exact(3, 1);
        let factorial = fixture.apply(Operator::Factorial, &[three]);
        let root = fixture.apply(Operator::Sqrt, &[factorial]);
        let (exact, tolerance) = reference("2.4494897427831780981972840747");

        assert_bound_covers(&fixture.evaluate(root), &exact, &tolerance);
    }

    #[test]
    fn an_overflow_from_finite_operands_is_a_note() {
        let mut fixture = Fixture::new();
        let large = fixture.exact(10, 1);
        let power = fixture.exact(400, 1);
        let root = fixture.apply(Operator::Pow, &[large, power]);

        let evaluation = fixture.evaluate(root);

        assert_eq!(
            evaluation
                .result()
                .method()
                .notes
                .iter()
                .map(|note| note.code.as_str())
                .collect::<Vec<&str>>(),
            vec!["overflowed_to_infinity"]
        );
    }

    #[test]
    fn a_finite_result_carries_no_note() {
        let mut fixture = Fixture::new();
        let third = fixture.exact(1, 3);

        let evaluation = fixture.evaluate(third);

        assert!(evaluation.result().method().notes.is_empty());
    }

    #[test]
    fn an_infinity_that_was_written_is_not_a_note() {
        let mut fixture = Fixture::new();
        let infinity = fixture
            .pool
            .symbol(BuiltinConstant::Infinity.symbol())
            .unwrap();
        let root = fixture.apply(Operator::Sub, &[infinity, infinity]);

        let evaluation = fixture.evaluate(root);

        assert!(evaluation.result().method().notes.is_empty());
    }

    #[test]
    fn abs_is_exact() {
        let mut fixture = Fixture::new();
        let value = fixture.machine(-2.5);
        let root = fixture.apply(Operator::Abs, &[value]);

        assert_exact_result(&fixture.evaluate(root), &fraction(5, 2));
    }

    #[test]
    fn mul_add_is_bounded() {
        let mut fixture = Fixture::new();
        let tenth = fixture.exact(1, 10);
        let ten = fixture.exact(10, 1);
        let minus_one = fixture.exact(-1, 1);
        let root = fixture.apply(Operator::MulAdd, &[tenth, ten, minus_one]);

        assert_exact_result(&fixture.evaluate(root), &fraction(0, 1));
    }

    #[test]
    fn floor_is_exact() {
        let mut fixture = Fixture::new();
        let value = fixture.exact(-3, 2);
        let root = fixture.apply(Operator::Floor, &[value]);

        assert_exact_result(&fixture.evaluate(root), &fraction(-2, 1));
    }

    #[test]
    fn ceil_is_exact() {
        let mut fixture = Fixture::new();
        let value = fixture.exact(-3, 2);
        let root = fixture.apply(Operator::Ceil, &[value]);

        assert_exact_result(&fixture.evaluate(root), &fraction(-1, 1));
    }

    #[test]
    fn trunc_is_exact() {
        let mut fixture = Fixture::new();
        let value = fixture.exact(7, 2);
        let root = fixture.apply(Operator::Trunc, &[value]);

        assert_exact_result(&fixture.evaluate(root), &fraction(3, 1));
    }

    #[test]
    fn round_ties_even_is_exact() {
        let mut fixture = Fixture::new();
        let value = fixture.exact(5, 2);
        let root = fixture.apply(Operator::RoundTiesEven, &[value]);

        assert_exact_result(&fixture.evaluate(root), &fraction(2, 1));
    }

    #[test]
    fn copy_sign_is_exact() {
        let mut fixture = Fixture::new();
        let magnitude_value = fixture.exact(3, 1);
        let sign = fixture.exact(-1, 2);
        let root = fixture.apply(Operator::CopySign, &[magnitude_value, sign]);

        assert_exact_result(&fixture.evaluate(root), &fraction(-3, 1));
    }

    #[test]
    fn min_is_bounded() {
        let mut fixture = Fixture::new();
        let third = fixture.exact(1, 3);
        let half = fixture.exact(1, 2);
        let root = fixture.apply(Operator::Min, &[third, half]);

        assert_exact_result(&fixture.evaluate(root), &fraction(1, 3));
    }

    #[test]
    fn max_is_bounded() {
        let mut fixture = Fixture::new();
        let third = fixture.exact(1, 3);
        let half = fixture.exact(1, 2);
        let root = fixture.apply(Operator::Max, &[third, half]);

        assert_exact_result(&fixture.evaluate(root), &fraction(1, 2));
    }

    #[test]
    fn exp_is_bounded() {
        let mut fixture = Fixture::new();
        let one = fixture.exact(1, 1);
        let root = fixture.apply(Operator::Exp, &[one]);
        let (exact, tolerance) = reference("2.71828182845904523536028747135");

        let evaluation = fixture.evaluate(root);

        assert_eq!(scalar(&evaluation).to_bits(), std::f64::consts::E.to_bits());
        assert_bound_covers(&evaluation, &exact, &tolerance);
    }

    #[test]
    fn ln_is_bounded() {
        let mut fixture = Fixture::new();
        let two = fixture.exact(2, 1);
        let root = fixture.apply(Operator::Ln, &[two]);
        let (exact, tolerance) = reference("0.693147180559945309417232121458");

        assert_bound_covers(&fixture.evaluate(root), &exact, &tolerance);
    }

    #[test]
    fn sin_is_bounded() {
        let mut fixture = Fixture::new();
        let one = fixture.exact(1, 1);
        let root = fixture.apply(Operator::Sin, &[one]);
        let (exact, tolerance) = reference("0.841470984807896506652502321630");

        assert_bound_covers(&fixture.evaluate(root), &exact, &tolerance);
    }

    #[test]
    fn cos_is_bounded() {
        let mut fixture = Fixture::new();
        let one = fixture.exact(1, 1);
        let root = fixture.apply(Operator::Cos, &[one]);
        let (exact, tolerance) = reference("0.540302305868139717400936607442");

        assert_bound_covers(&fixture.evaluate(root), &exact, &tolerance);
    }

    #[test]
    fn tan_is_bounded() {
        let mut fixture = Fixture::new();
        let one = fixture.exact(1, 1);
        let root = fixture.apply(Operator::Tan, &[one]);
        let (exact, tolerance) = reference("1.557407724654902230506974807458");

        assert_bound_covers(&fixture.evaluate(root), &exact, &tolerance);
    }

    #[test]
    fn atan2_is_bounded() {
        let mut fixture = Fixture::new();
        let one = fixture.exact(1, 1);
        let two = fixture.exact(2, 1);
        let root = fixture.apply(Operator::Atan2, &[one, two]);
        let (exact, tolerance) = reference("0.463647609000806116214256231461");

        assert_bound_covers(&fixture.evaluate(root), &exact, &tolerance);
    }

    #[test]
    fn equal_selects_true_branch_for_equal_values() {
        let mut fixture = Fixture::new();
        let first = fixture.exact(1, 2);
        let second = fixture.exact(2, 4);
        let condition = fixture.apply(Operator::Equal, &[first, second]);
        let root = fixture.select_on(condition);

        assert_exact_result(&fixture.evaluate(root), &fraction(1, 1));
    }

    #[test]
    fn not_equal_selects_false_branch_for_equal_values() {
        let mut fixture = Fixture::new();
        let first = fixture.exact(1, 2);
        let second = fixture.exact(1, 2);
        let condition = fixture.apply(Operator::NotEqual, &[first, second]);
        let root = fixture.select_on(condition);

        assert_exact_result(&fixture.evaluate(root), &fraction(2, 1));
    }

    #[test]
    fn less_selects_true_branch() {
        let mut fixture = Fixture::new();
        let first = fixture.exact(1, 3);
        let second = fixture.exact(1, 2);
        let condition = fixture.apply(Operator::Less, &[first, second]);
        let root = fixture.select_on(condition);

        assert_exact_result(&fixture.evaluate(root), &fraction(1, 1));
    }

    #[test]
    fn less_or_equal_selects_false_branch() {
        let mut fixture = Fixture::new();
        let first = fixture.exact(1, 2);
        let second = fixture.exact(1, 3);
        let condition = fixture.apply(Operator::LessOrEqual, &[first, second]);
        let root = fixture.select_on(condition);

        assert_exact_result(&fixture.evaluate(root), &fraction(2, 1));
    }

    #[test]
    fn greater_selects_true_branch() {
        let mut fixture = Fixture::new();
        let first = fixture.exact(1, 2);
        let second = fixture.exact(1, 3);
        let condition = fixture.apply(Operator::Greater, &[first, second]);
        let root = fixture.select_on(condition);

        assert_exact_result(&fixture.evaluate(root), &fraction(1, 1));
    }

    #[test]
    fn greater_or_equal_selects_false_branch() {
        let mut fixture = Fixture::new();
        let first = fixture.exact(1, 3);
        let second = fixture.exact(1, 2);
        let condition = fixture.apply(Operator::GreaterOrEqual, &[first, second]);
        let root = fixture.select_on(condition);

        assert_exact_result(&fixture.evaluate(root), &fraction(2, 1));
    }

    #[test]
    fn and_selects_false_branch_when_one_side_is_false() {
        let mut fixture = Fixture::new();
        let small = fixture.exact(1, 3);
        let large = fixture.exact(1, 2);
        let holds = fixture.apply(Operator::Less, &[small, large]);
        let fails = fixture.apply(Operator::Less, &[large, small]);
        let condition = fixture.apply(Operator::And, &[holds, fails]);
        let root = fixture.select_on(condition);

        assert_exact_result(&fixture.evaluate(root), &fraction(2, 1));
    }

    #[test]
    fn or_selects_true_branch_when_one_side_is_true() {
        let mut fixture = Fixture::new();
        let small = fixture.exact(1, 3);
        let large = fixture.exact(1, 2);
        let holds = fixture.apply(Operator::Less, &[small, large]);
        let fails = fixture.apply(Operator::Less, &[large, small]);
        let condition = fixture.apply(Operator::Or, &[fails, holds]);
        let root = fixture.select_on(condition);

        assert_exact_result(&fixture.evaluate(root), &fraction(1, 1));
    }

    #[test]
    fn not_inverts_the_condition() {
        let mut fixture = Fixture::new();
        let small = fixture.exact(1, 3);
        let large = fixture.exact(1, 2);
        let holds = fixture.apply(Operator::Less, &[small, large]);
        let condition = fixture.apply(Operator::Not, &[holds]);
        let root = fixture.select_on(condition);

        assert_exact_result(&fixture.evaluate(root), &fraction(2, 1));
    }

    #[test]
    fn select_with_undecided_condition_bounds_both_branches() {
        let mut fixture = Fixture::new();
        let tenth = fixture.exact(1, 10);
        let rounded_tenth = fixture.machine(0.1);
        let condition = fixture.apply(Operator::Equal, &[tenth, rounded_tenth]);
        let root = fixture.select_on(condition);

        let evaluation = fixture.evaluate(root);

        assert_eq!(scalar(&evaluation), 1.0);
        assert!(is_at_most(
            &Number::from(1),
            &bound(&evaluation).to_exact().unwrap()
        ));
    }

    #[test]
    fn complex_product_bounds_both_parts() {
        let mut fixture = Fixture::new();
        let real_part = fixture.exact(1, 10);
        let imaginary_part = fixture.exact(1, 1);
        let complex = fixture.apply(Operator::Complex, &[real_part, imaginary_part]);
        let root = fixture.apply(Operator::Mul, &[complex, complex]);

        let evaluation = fixture.evaluate(root);

        match (
            evaluation.result().value(),
            evaluation.result().rounding_error(),
        ) {
            (
                ResultValue::Complex {
                    real: Number::F64(real_value),
                    imaginary: Number::F64(imaginary_value),
                },
                RoundingError::Bound(ResultValue::Complex {
                    real: real_bound,
                    imaginary: imaginary_bound,
                }),
            ) => {
                let real_error = magnitude(
                    Number::F64(*real_value)
                        .to_exact()
                        .unwrap()
                        .sub_exact(&fraction(-99, 100))
                        .unwrap(),
                );
                let imaginary_error = magnitude(
                    Number::F64(*imaginary_value)
                        .to_exact()
                        .unwrap()
                        .sub_exact(&fraction(1, 5))
                        .unwrap(),
                );
                assert!(is_at_most(&real_error, &real_bound.to_exact().unwrap()));
                assert!(is_at_most(
                    &imaginary_error,
                    &imaginary_bound.to_exact().unwrap()
                ));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn to_f64_is_bounded_like_its_argument() {
        let mut fixture = Fixture::new();
        let tenth = fixture.exact(1, 10);
        let root = fixture.apply(Operator::ToF64, &[tenth]);

        assert_exact_result(&fixture.evaluate(root), &fraction(1, 10));
    }

    #[test]
    fn pi_constant_is_bounded() {
        let mut fixture = Fixture::new();
        let root = fixture.constant(BuiltinConstant::Pi);
        let (exact, tolerance) = reference("3.141592653589793238462643383279");

        assert_bound_covers(&fixture.evaluate(root), &exact, &tolerance);
    }

    #[test]
    fn a_sum_asked_for_in_the_domain_it_runs_in_keeps_its_shape() {
        let mut fixture = Fixture::new();
        let tenth = fixture.exact(1, 10);
        let index = fixture.pool.bound(0).unwrap();
        let body = fixture.apply(Operator::Mul, &[tenth, index]);
        let lower_bound = fixture.exact(1, 1);
        let upper_bound = fixture.exact(10, 1);
        let sum = fixture
            .pool
            .bind(
                BinderKind::Sum(ReductionShape::Halving),
                &[lower_bound, upper_bound],
                body,
            )
            .unwrap();
        let root = fixture.apply(Operator::ToF64, &[sum]);

        let evaluation = fixture.evaluate(root);

        assert_exact_result(&evaluation, &fraction(11, 2));
        assert_eq!(
            evaluation.result().method().parameters.get(SHAPE_PARAMETER),
            Some(&ParameterValue::Identifier(HALVING.to_string()))
        );
    }

    #[test]
    fn halving_sum_records_shape_and_bound() {
        let mut fixture = Fixture::new();
        let tenth = fixture.exact(1, 10);
        let index = fixture.pool.bound(0).unwrap();
        let body = fixture.apply(Operator::Mul, &[tenth, index]);
        let lower_bound = fixture.exact(1, 1);
        let upper_bound = fixture.exact(10, 1);
        let root = fixture
            .pool
            .bind(
                BinderKind::Sum(ReductionShape::Halving),
                &[lower_bound, upper_bound],
                body,
            )
            .unwrap();

        let evaluation = fixture.evaluate(root);

        assert_exact_result(&evaluation, &fraction(11, 2));
        assert_eq!(
            evaluation.result().method().parameters.get(SHAPE_PARAMETER),
            Some(&ParameterValue::Identifier(HALVING.to_string()))
        );
    }

    #[test]
    fn evaluation_reports_selected_backend() {
        let mut fixture = Fixture::new();
        let root = fixture.exact(1, 3);

        let evaluation = fixture.evaluate(root);

        assert_eq!(evaluation.backend(), BackendKind::Cpu);
        assert!(evaluation.skipped().is_empty());
    }

    #[test]
    fn result_kind_is_machine_float_with_f64_value() {
        let mut fixture = Fixture::new();
        let root = fixture.exact(1, 3);

        let evaluation = fixture.evaluate(root);

        assert_eq!(evaluation.result().kind(), ResultKind::MachineFloat);
        assert!(matches!(
            evaluation.result().value(),
            ResultValue::Number(Number::F64(_))
        ));
    }

    #[test]
    fn operator_without_plan_operation_is_a_lowering_error() {
        let mut fixture = Fixture::new();
        let half = fixture.exact(1, 2);
        let root = fixture.apply(Operator::Factorial, &[half]);
        let backend = CpuBackend::new();

        let result = evaluate_f64(&mut fixture.pool, root, &[&backend], Preference::Automatic);

        assert_eq!(
            result.err(),
            Some(MachineEvaluationError::Lower(
                LowerError::UnsupportedOperator(Operator::Factorial)
            ))
        );
    }

    #[test]
    fn arcsine_answers_with_the_reference_value() {
        let mut fixture = Fixture::new();
        let argument = fixture.exact(3, 10);
        let root = fixture.apply(Operator::Asin, &[argument]);
        let backend = CpuBackend::new();

        let evaluation =
            evaluate_f64(&mut fixture.pool, root, &[&backend], Preference::Automatic).unwrap();

        assert_eq!(
            evaluation.result().value(),
            &ResultValue::Number(Number::F64(calc_numbers::asin_f64(0.3)))
        );
    }

    #[test]
    fn unregistered_backend_preference_is_a_selection_error() {
        let mut fixture = Fixture::new();
        let root = fixture.exact(1, 2);
        let backend = CpuBackend::new();

        let result = evaluate_f64(
            &mut fixture.pool,
            root,
            &[&backend],
            Preference::Only(BackendKind::Gpu),
        );

        assert_eq!(
            result.err(),
            Some(MachineEvaluationError::Select(SelectError::NotRegistered(
                BackendKind::Gpu
            )))
        );
    }

    #[test]
    fn free_variable_is_a_lowering_error() {
        let mut fixture = Fixture::new();
        let symbol = fixture
            .pool
            .intern_symbol("x", SymbolKind::Variable)
            .unwrap();
        let root = fixture.pool.symbol(symbol).unwrap();
        let backend = CpuBackend::new();

        let result = evaluate_f64(&mut fixture.pool, root, &[&backend], Preference::Automatic);

        assert_eq!(
            result.err(),
            Some(MachineEvaluationError::Lower(LowerError::UnboundSymbol(
                symbol
            )))
        );
    }

    const RESOLVABLE_STEP: f64 = 2.0e-4;

    fn variable(fixture: &mut Fixture, name: &str) -> (SymbolId, ExprId) {
        let symbol = fixture
            .pool
            .intern_symbol(name, SymbolKind::Variable)
            .unwrap();
        (symbol, fixture.pool.symbol(symbol).unwrap())
    }

    fn real_input(symbol: SymbolId, position: Number) -> Vec<SampleInput> {
        vec![SampleInput {
            symbol,
            position: SamplePosition::Real(position),
        }]
    }

    fn cancellation_quotient(fixture: &mut Fixture, x: ExprId) -> ExprId {
        let one = fixture.exact(1, 1);
        let two = fixture.exact(2, 1);
        let cosine = fixture.apply(Operator::Cos, &[x]);
        let numerator = fixture.apply(Operator::Sub, &[one, cosine]);
        let square = fixture.apply(Operator::Pow, &[x, two]);
        fixture.apply(Operator::Div, &[numerator, square])
    }

    fn inverse_power_of_ten(exponent: u32) -> Number {
        Number::fraction(&Integer::one(), &Integer::from(10i64).pow(exponent)).unwrap()
    }

    fn sampled_cancellation_quotient(position: &Number) -> f64 {
        let x = position.round_to_f64_ties_even();
        (1.0 - calc_numbers::cos_f64(x)) / (x * x)
    }

    fn cancellation_bound_at(exponent: u32) -> f64 {
        cancellation_bound_at_position(inverse_power_of_ten(exponent))
    }

    fn cancellation_bound_at_position(position: Number) -> f64 {
        let mut fixture = Fixture::new();
        let (symbol, x) = variable(&mut fixture, "x");
        let root = cancellation_quotient(&mut fixture, x);
        let enclosure = enclose_sample(&fixture.pool, root, &real_input(symbol, position.clone()));
        enclosure.rounding_bound(sampled_cancellation_quotient(&position))
    }

    fn real_interval(enclosure: SampleEnclosure) -> Interval {
        match enclosure {
            SampleEnclosure::Real(Some(interval)) => interval,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn sample_at_machine_number_position_has_bound_of_at_most_one_subnormal() {
        let mut fixture = Fixture::new();
        let (symbol, x) = variable(&mut fixture, "x");
        let enclosure = enclose_sample(&fixture.pool, x, &real_input(symbol, fraction(1, 2)));

        assert!(enclosure.rounding_bound(0.5) <= f64::from_bits(1));
    }

    #[test]
    fn sample_position_between_machine_numbers_encloses_both_neighbours() {
        let mut fixture = Fixture::new();
        let (symbol, x) = variable(&mut fixture, "x");
        let enclosure = enclose_sample(&fixture.pool, x, &real_input(symbol, fraction(1, 10)));

        let interval = real_interval(enclosure);
        let above = 0.1f64;
        let below = above.next_down();
        assert!(is_at_most(
            &Number::F64(below).to_exact().unwrap(),
            &fraction(1, 10)
        ));
        assert!(interval.lower() <= below && above <= interval.upper());
    }

    #[test]
    fn cancellation_quotient_at_one_hundred_thousandth_is_resolved() {
        assert!(2.0 * cancellation_bound_at(5) < RESOLVABLE_STEP);
    }

    #[test]
    fn cancellation_quotient_at_ten_millionth_is_unresolved() {
        assert!(2.0 * cancellation_bound_at(7) >= RESOLVABLE_STEP);
    }

    #[test]
    fn cancellation_quotient_enclosure_contains_one_half_near_zero() {
        let mut fixture = Fixture::new();
        let (symbol, x) = variable(&mut fixture, "x");
        let root = cancellation_quotient(&mut fixture, x);
        let enclosure = enclose_sample(
            &fixture.pool,
            root,
            &real_input(symbol, inverse_power_of_ten(5)),
        );

        let interval = real_interval(enclosure);
        assert!(interval.lower() <= 0.5 && 0.5 <= interval.upper());
    }

    #[test]
    fn sample_bound_is_taken_for_the_f32_value() {
        let mut fixture = Fixture::new();
        let (symbol, x) = variable(&mut fixture, "x");
        let enclosure = enclose_sample(&fixture.pool, x, &real_input(symbol, fraction(1, 10)));

        let bound = enclosure.rounding_bound(f64::from(0.1f32));
        assert!(bound >= (f64::from(0.1f32) - 0.1).abs());
    }

    #[test]
    fn symbol_without_sample_input_has_unknown_bound() {
        let mut fixture = Fixture::new();
        let (symbol, x) = variable(&mut fixture, "x");
        let (_, y) = variable(&mut fixture, "y");
        let root = fixture.apply(Operator::Add, &[x, y]);
        let enclosure = enclose_sample(&fixture.pool, root, &real_input(symbol, fraction(1, 2)));

        assert!(enclosure.rounding_bound(0.5).is_nan());
    }

    #[test]
    fn division_by_zero_position_has_unknown_bound() {
        let mut fixture = Fixture::new();
        let (symbol, x) = variable(&mut fixture, "x");
        let one = fixture.exact(1, 1);
        let root = fixture.apply(Operator::Div, &[one, x]);
        let enclosure = enclose_sample(&fixture.pool, root, &real_input(symbol, Number::from(0)));

        assert!(enclosure.rounding_bound(f64::INFINITY).is_nan());
    }

    #[test]
    fn complex_sample_position_encloses_both_parts_of_the_square() {
        let mut fixture = Fixture::new();
        let (symbol, z) = variable(&mut fixture, "z");
        let root = fixture.apply(Operator::Mul, &[z, z]);
        let inputs = vec![SampleInput {
            symbol,
            position: SamplePosition::Complex {
                real: fraction(1, 10),
                imaginary: fraction(1, 10),
            },
        }];
        let enclosure = enclose_sample(&fixture.pool, root, &inputs);

        let (real_bound, imaginary_bound) = enclosure.complex_rounding_bounds(0.0, 0.02);
        assert!(real_bound < TIGHT_BOUND);
        assert!(imaginary_bound < TIGHT_BOUND);
    }

    const SINGLE_TIGHT_BOUND: f64 = 1.0e-6;

    fn evaluate_single(fixture: &mut Fixture, root: ExprId) -> MachineEvaluation {
        let backend = CpuBackend::new();
        evaluate_f32(&mut fixture.pool, root, &[&backend], Preference::Automatic).unwrap()
    }

    fn single(evaluation: &MachineEvaluation) -> f32 {
        match evaluation.result().value() {
            ResultValue::Number(Number::F32(value)) => *value,
            other => panic!("{other:?}"),
        }
    }

    fn assert_single_bound_covers(evaluation: &MachineEvaluation, exact: &Number) {
        let value = Number::F32(single(evaluation)).to_exact().unwrap();
        let error = magnitude(value.sub_exact(exact).unwrap());
        let bound = bound(evaluation).to_exact().unwrap();
        assert!(is_at_most(&error, &bound));
        assert!(is_at_most(
            &bound,
            &Number::F64(SINGLE_TIGHT_BOUND).to_exact().unwrap()
        ));
    }

    #[test]
    fn single_add_of_rounded_tenths_is_bounded() {
        let mut fixture = Fixture::new();
        let tenth = fixture.exact(1, 10);
        let fifth = fixture.exact(2, 10);
        let root = fixture.apply(Operator::Add, &[tenth, fifth]);

        let evaluation = evaluate_single(&mut fixture, root);

        assert_eq!(single(&evaluation).to_bits(), (0.1f32 + 0.2f32).to_bits());
        assert_single_bound_covers(&evaluation, &fraction(3, 10));
    }

    #[test]
    fn single_square_root_of_two_bound_encloses_the_root() {
        let mut fixture = Fixture::new();
        let two = fixture.exact(2, 1);
        let root = fixture.apply(Operator::Sqrt, &[two]);

        let evaluation = evaluate_single(&mut fixture, root);

        let value = Number::F32(single(&evaluation)).to_exact().unwrap();
        let bound = bound(&evaluation).to_exact().unwrap();
        let below = value.sub_exact(&bound).unwrap();
        let above = value.add_exact(&bound).unwrap();
        assert!(is_at_most(
            &below.mul_exact(&below).unwrap(),
            &fraction(2, 1)
        ));
        assert!(is_at_most(
            &fraction(2, 1),
            &above.mul_exact(&above).unwrap()
        ));
    }

    #[test]
    fn single_floor_is_exact() {
        let mut fixture = Fixture::new();
        let value = fixture.exact(7, 2);
        let root = fixture.apply(Operator::Floor, &[value]);

        let evaluation = evaluate_single(&mut fixture, root);

        assert_single_bound_covers(&evaluation, &fraction(3, 1));
    }

    #[test]
    fn single_less_selects_true_branch() {
        let mut fixture = Fixture::new();
        let first = fixture.exact(1, 3);
        let second = fixture.exact(1, 2);
        let condition = fixture.apply(Operator::Less, &[first, second]);
        let when_true = fixture.exact(1, 1);
        let when_false = fixture.exact(2, 1);
        let root = fixture.apply(Operator::Select, &[condition, when_true, when_false]);

        let evaluation = evaluate_single(&mut fixture, root);

        assert_single_bound_covers(&evaluation, &fraction(1, 1));
    }

    #[test]
    fn single_sine_uses_the_f32_reference_and_is_bounded() {
        let mut fixture = Fixture::new();
        let half = fixture.exact(1, 2);
        let root = fixture.apply(Operator::Sin, &[half]);

        let evaluation = evaluate_single(&mut fixture, root);

        assert_eq!(
            single(&evaluation).to_bits(),
            calc_numbers::sin_f32(0.5).to_bits()
        );
        let (reference, tolerance) = reference("0.479425538604203000273287935216");
        let value = Number::F32(single(&evaluation)).to_exact().unwrap();
        let error = magnitude(value.sub_exact(&reference).unwrap());
        let bound = bound(&evaluation).to_exact().unwrap();
        assert!(is_at_most(&error, &bound.add_exact(&tolerance).unwrap()));
    }

    #[test]
    fn single_power_uses_the_f32_reference_and_is_bounded() {
        let mut fixture = Fixture::new();
        let base = fixture.exact(3, 1);
        let exponent = fixture.exact(1, 2);
        let root = fixture.apply(Operator::Pow, &[base, exponent]);

        let evaluation = evaluate_single(&mut fixture, root);

        assert_eq!(
            single(&evaluation).to_bits(),
            calc_numbers::pow_f32(3.0, 0.5).to_bits()
        );
        let (reference, tolerance) = reference("1.732050807568877293527446341505");
        let value = Number::F32(single(&evaluation)).to_exact().unwrap();
        let error = magnitude(value.sub_exact(&reference).unwrap());
        let bound = bound(&evaluation).to_exact().unwrap();
        assert!(is_at_most(&error, &bound.add_exact(&tolerance).unwrap()));
    }

    #[test]
    fn single_complex_product_has_single_parts() {
        let mut fixture = Fixture::new();
        let real_part = fixture.exact(1, 10);
        let imaginary_part = fixture.exact(1, 1);
        let complex = fixture.apply(Operator::Complex, &[real_part, imaginary_part]);
        let root = fixture.apply(Operator::Mul, &[complex, complex]);

        let evaluation = evaluate_single(&mut fixture, root);

        assert!(matches!(
            (
                evaluation.result().value(),
                evaluation.result().rounding_error()
            ),
            (
                ResultValue::Complex {
                    real: Number::F32(_),
                    imaginary: Number::F32(_),
                },
                RoundingError::Bound(ResultValue::Complex { .. })
            )
        ));
    }

    #[test]
    fn single_halving_sum_is_bounded() {
        let mut fixture = Fixture::new();
        let tenth = fixture.exact(1, 10);
        let index = fixture.pool.bound(0).unwrap();
        let body = fixture.apply(Operator::Mul, &[tenth, index]);
        let lower_bound = fixture.exact(1, 1);
        let upper_bound = fixture.exact(10, 1);
        let root = fixture
            .pool
            .bind(
                BinderKind::Sum(ReductionShape::Halving),
                &[lower_bound, upper_bound],
                body,
            )
            .unwrap();

        let evaluation = evaluate_single(&mut fixture, root);

        assert_single_bound_covers(&evaluation, &fraction(11, 2));
    }

    #[test]
    fn single_evaluation_records_the_f32_domain() {
        let mut fixture = Fixture::new();
        let root = fixture.exact(1, 3);

        let evaluation = evaluate_single(&mut fixture, root);

        assert_eq!(
            evaluation
                .result()
                .method()
                .parameters
                .get(DOMAIN_PARAMETER),
            Some(&ParameterValue::Identifier(DOMAIN_F32.to_string()))
        );
    }

    #[test]
    fn single_evaluation_with_a_double_operand_is_a_lowering_error() {
        let mut fixture = Fixture::new();
        let double = fixture.machine(0.5);
        let backend = CpuBackend::new();

        let result = evaluate_f32(
            &mut fixture.pool,
            double,
            &[&backend],
            Preference::Automatic,
        );

        assert!(matches!(result, Err(MachineEvaluationError::Lower(_))));
    }

    fn square_enclosure_at(position: Number) -> Interval {
        let mut fixture = Fixture::new();
        let (symbol, x) = variable(&mut fixture, "x");
        let two = fixture.exact(2, 1);
        let root = fixture.apply(Operator::Pow, &[x, two]);
        real_interval(enclose_sample(
            &fixture.pool,
            root,
            &real_input(symbol, position),
        ))
    }

    #[test]
    fn square_enclosure_is_symmetric_in_the_sample_position() {
        let positive = inverse_power_of_ten(5);
        let negative = positive.negate_exact().unwrap();

        assert_eq!(square_enclosure_at(negative), square_enclosure_at(positive));
    }

    #[test]
    fn cancellation_quotient_at_minus_one_hundred_thousandth_is_resolved() {
        let position = inverse_power_of_ten(5).negate_exact().unwrap();

        assert!(2.0 * cancellation_bound_at_position(position) < RESOLVABLE_STEP);
    }

    #[test]
    fn cancellation_quotient_at_minus_ten_millionth_is_unresolved() {
        let position = inverse_power_of_ten(7).negate_exact().unwrap();

        assert!(2.0 * cancellation_bound_at_position(position) >= RESOLVABLE_STEP);
    }

    #[test]
    fn cancellation_quotient_bound_is_the_same_on_both_sides_of_zero() {
        let positive = inverse_power_of_ten(6);
        let negative = positive.negate_exact().unwrap();

        assert_eq!(
            cancellation_bound_at_position(negative).to_bits(),
            cancellation_bound_at_position(positive).to_bits()
        );
    }
}

#[cfg(test)]
mod column_tests {
    use super::*;
    use calc_expr::SymbolKind;

    fn parsed(text: &str) -> (ExprPool, ExprId, SymbolId) {
        let mut pool = ExprPool::new();
        let expression = calc_syntax::parse_expression(&mut pool, text).unwrap();
        let symbol = pool.intern_symbol("x", SymbolKind::Variable).unwrap();
        (pool, expression, symbol)
    }

    #[test]
    fn a_column_of_a_fast_sine_is_enclosed_by_the_whole_range() {
        let (pool, expression, symbol) = parsed("sin(1000*x)");

        let (low, high, clipped) = enclose_over(
            &pool,
            expression,
            symbol,
            &Number::from(0_i64),
            &Number::from(1_i64),
        )
        .unwrap();

        assert_eq!((low, high, clipped), (-1.0, 1.0, false));
    }

    #[test]
    fn a_root_whose_argument_reaches_below_zero_by_rounding_is_clipped() {
        let (pool, expression, symbol) = parsed("sqrt(1 - x^2)");
        let low = Number::fraction(&Integer::from(3999_i64), &Integer::from(4000_i64)).unwrap();

        let (lower, upper, clipped) =
            enclose_over(&pool, expression, symbol, &low, &Number::from(1_i64)).unwrap();

        assert_eq!((clipped, lower), (true, 0.0));
        assert!((0.0223..0.0225).contains(&upper), "{upper}");
    }

    #[test]
    fn a_floor_is_not_continuous() {
        let (pool, expression, _) = parsed("floor(x) + 1");

        assert!(!is_continuous_where_enclosed(&pool, expression));
    }

    #[test]
    fn a_quotient_of_sines_is_continuous_where_enclosed() {
        let (pool, expression, _) = parsed("sin(x) / x");

        assert!(is_continuous_where_enclosed(&pool, expression));
    }

    fn attained(text: &str, low: Number, high: Number) -> Option<(f64, f64)> {
        let (pool, expression, symbol) = parsed(text);
        attained_over(&pool, expression, symbol, &low, &high)
    }

    fn seventh() -> Number {
        Number::fraction(&Integer::from(1_i64), &Integer::from(7_i64)).unwrap()
    }

    #[test]
    fn a_column_of_a_fast_sine_attains_the_whole_range() {
        assert_eq!(
            attained("sin(1000*x)", Number::from(0_i64), seventh()),
            Some((-1.0, 1.0))
        );
    }

    #[test]
    fn a_scaled_and_shifted_fast_sine_attains_nearly_its_whole_range() {
        let (low, high) = attained("3 * sin(1000*x) + 1", Number::from(0_i64), seventh()).unwrap();

        assert!((-2.0..-1.999_999).contains(&low), "{low}");
        assert!(high > 3.999_999 && high < 4.0, "{high}");
    }

    #[test]
    fn a_square_over_zero_attains_down_to_zero() {
        let (low, high) = attained("x^2", Number::from(-1_i64), Number::from(2_i64)).unwrap();

        assert!((0.0..1.0e-300).contains(&low), "{low}");
        assert!(high > 3.999_999 && high <= 4.0, "{high}");
    }

    #[test]
    fn a_name_that_occurs_twice_attains_nothing_by_this_route() {
        assert_eq!(
            attained("x * sin(1000*x)", Number::from(0_i64), seventh()),
            None
        );
    }

    #[test]
    fn a_constant_attains_nothing_by_this_route() {
        assert_eq!(attained("2", Number::from(0_i64), seventh()), None);
    }

    #[test]
    fn the_slope_of_a_square_is_enclosed_over_its_column() {
        let (mut pool, expression, symbol) = parsed("x^2");
        let slope = slope_of(&mut pool, expression, symbol).unwrap();

        let (low, high) =
            enclose_slope_over(&pool, slope, &Number::from(1_i64), &Number::from(2_i64)).unwrap();

        assert!((1.99..=2.0).contains(&low) && (4.0..4.01).contains(&high));
    }
}
