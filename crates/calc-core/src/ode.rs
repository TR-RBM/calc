use std::cmp::Ordering;
use std::collections::HashMap;

use calc_expr::{ExprId, ExprPool, Head, NodeView, Operator, SymbolId};
use calc_numbers::{Interval, Number};

use crate::exact_evaluation::closed_square_root_sum;
use crate::exact_rational::ExactRational;
use crate::machine_evaluation::real_enclosure;

pub const TAYLOR_ORDER: usize = 20;

pub const LARGEST_STEP_COUNT: usize = 10_000;

pub const LARGEST_HALVING_COUNT: u32 = 40;

const ROUGH_ENCLOSURE_TRIES: usize = 4;

const UNIT_ROUNDOFF: f64 = f64::EPSILON / 2.0;

#[derive(Clone, Debug)]
pub struct OdeProblem {
    pub time: SymbolId,
    pub names: Vec<SymbolId>,
    pub derivatives: Vec<ExprId>,
}

#[derive(Clone, Debug)]
pub struct OdePoint {
    pub time: Number,
    pub values: Vec<Interval>,
}

#[derive(Clone, Debug)]
pub struct OdeEnclosure {
    pub points: Vec<OdePoint>,
    pub steps: usize,
    pub largest_step_bounds: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepLimit {
    pub at: Number,
    pub step: Number,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OdeRefusal {
    Unsupported(ExprId),
    DivisionByZero(ExprId),
    SizeMismatch,
    InitialValue(usize),
    MachineTime,
    StopBeforeStart,
    NotEnclosed { at: Number },
    TooManySteps(Box<StepLimit>),
}

enum Failure {
    Refused(OdeRefusal),
    Arithmetic,
}

impl From<OdeRefusal> for Failure {
    fn from(refusal: OdeRefusal) -> Self {
        Failure::Refused(refusal)
    }
}

fn arithmetic<T>(value: Option<T>) -> Result<T, Failure> {
    value.ok_or(Failure::Arithmetic)
}

fn zero() -> Option<Interval> {
    Interval::point(0.0)
}

fn one() -> Option<Interval> {
    Interval::point(1.0)
}

fn span(lower: f64, upper: f64) -> Option<Interval> {
    Some(Interval::point(lower)?.hull(&Interval::point(upper)?))
}

fn magnitude(interval: &Interval) -> f64 {
    interval.lower().abs().max(interval.upper().abs())
}

fn middle(interval: &Interval) -> f64 {
    interval.lower() / 2.0 + interval.upper() / 2.0
}

fn width_bound(interval: &Interval) -> Option<f64> {
    if interval.lower() == interval.upper() {
        return Some(0.0);
    }
    Some(
        Interval::point(interval.upper())?
            .sub(&Interval::point(interval.lower())?)?
            .upper(),
    )
}

fn intersection(left: &Interval, right: &Interval) -> Option<Interval> {
    let lower = left.lower().max(right.lower());
    let upper = left.upper().min(right.upper());
    if lower <= upper {
        span(lower, upper)
    } else {
        None
    }
}

fn counted(value: usize) -> Option<Interval> {
    Interval::point(f64::from(u32::try_from(value).ok()?))
}

fn reciprocal_of_count(value: usize) -> Option<Interval> {
    one()?.div(&counted(value)?)
}

#[derive(Clone, Debug)]
struct Jet {
    value: Interval,
    gradient: Vec<Interval>,
}

impl Jet {
    fn constant(value: Interval, width: usize) -> Option<Jet> {
        Some(Jet {
            value,
            gradient: vec![zero()?; width],
        })
    }

    fn chained(&self, value: Interval, factor: &Interval) -> Option<Jet> {
        Some(Jet {
            value,
            gradient: self
                .gradient
                .iter()
                .map(|entry| entry.mul(factor))
                .collect::<Option<Vec<_>>>()?,
        })
    }

    fn plus(&self, other: &Jet) -> Option<Jet> {
        Some(Jet {
            value: self.value.add(&other.value)?,
            gradient: self
                .gradient
                .iter()
                .zip(&other.gradient)
                .map(|(one, two)| one.add(two))
                .collect::<Option<Vec<_>>>()?,
        })
    }

    fn negated(&self) -> Jet {
        Jet {
            value: self.value.neg(),
            gradient: self.gradient.iter().map(Interval::neg).collect(),
        }
    }

    fn minus(&self, other: &Jet) -> Option<Jet> {
        self.plus(&other.negated())
    }

    fn scaled(&self, factor: &Interval) -> Option<Jet> {
        self.chained(self.value.mul(factor)?, factor)
    }

    fn times(&self, other: &Jet) -> Option<Jet> {
        Some(Jet {
            value: self.value.mul(&other.value)?,
            gradient: self
                .gradient
                .iter()
                .zip(&other.gradient)
                .map(|(one, two)| one.mul(&other.value)?.add(&two.mul(&self.value)?))
                .collect::<Option<Vec<_>>>()?,
        })
    }

    fn over(&self, other: &Jet) -> Option<Jet> {
        let value = self.value.div(&other.value)?;
        Some(Jet {
            value,
            gradient: self
                .gradient
                .iter()
                .zip(&other.gradient)
                .map(|(one, two)| one.sub(&two.mul(&value)?)?.div(&other.value))
                .collect::<Option<Vec<_>>>()?,
        })
    }

    fn exp(&self) -> Option<Jet> {
        let value = self.value.exp();
        self.chained(value, &value)
    }

    fn ln(&self) -> Option<Jet> {
        let factor = one()?.div(&self.value)?;
        self.chained(self.value.ln()?, &factor)
    }

    fn sin(&self) -> Option<Jet> {
        self.chained(self.value.sin()?, &self.value.cos()?)
    }

    fn cos(&self) -> Option<Jet> {
        self.chained(self.value.cos()?, &self.value.sin()?.neg())
    }

    fn sqrt(&self) -> Option<Jet> {
        let value = self.value.sqrt()?;
        let factor = Interval::point(0.5)?.div(&value)?;
        self.chained(value, &factor)
    }

    fn power(&self, exponent: &Interval) -> Option<Jet> {
        let value = Interval::pow(&self.value, exponent)?;
        let lowered = Interval::pow(&self.value, &lowered_exponent(exponent)?)?;
        self.chained(value, &exponent.mul(&lowered)?)
    }
}

type Series = Vec<Jet>;

fn lowered_exponent(exponent: &Interval) -> Option<Interval> {
    let point = exponent.lower();
    let lowered = point - 1.0;
    if exponent.upper() == point && lowered + 1.0 == point && point - lowered == 1.0 {
        Interval::point(lowered)
    } else {
        exponent.sub(&one()?)
    }
}

fn constant_series(value: Interval, length: usize, width: usize) -> Option<Series> {
    (0..length)
        .map(|order| Jet::constant(if order == 0 { value } else { zero()? }, width))
        .collect()
}

fn series_plus(left: &Series, right: &Series) -> Option<Series> {
    left.iter()
        .zip(right)
        .map(|(one, two)| one.plus(two))
        .collect()
}

fn series_minus(left: &Series, right: &Series) -> Option<Series> {
    left.iter()
        .zip(right)
        .map(|(one, two)| one.minus(two))
        .collect()
}

fn series_times(left: &Series, right: &Series) -> Option<Series> {
    (0..left.len().min(right.len()))
        .map(|order| {
            let mut total = left.first()?.times(right.get(order)?)?;
            for part in 1..=order {
                total = total.plus(&left.get(part)?.times(right.get(order - part)?)?)?;
            }
            Some(total)
        })
        .collect()
}

fn series_over(left: &Series, right: &Series) -> Option<Series> {
    let divisor = right.first()?;
    let mut quotient: Series = Vec::new();
    for order in 0..left.len().min(right.len()) {
        let mut numerator = left.get(order)?.clone();
        for (part, earlier) in quotient.iter().enumerate() {
            numerator = numerator.minus(&earlier.times(right.get(order - part)?)?)?;
        }
        quotient.push(numerator.over(divisor)?);
    }
    Some(quotient)
}

fn weighted_sum(first: &Series, second: &Series, order: usize, last: usize) -> Option<Jet> {
    let width = first.first()?.gradient.len();
    let mut total = Jet::constant(zero()?, width)?;
    for part in 1..=last {
        let term = first
            .get(part)?
            .scaled(&counted(part)?)?
            .times(second.get(order - part)?)?;
        total = total.plus(&term)?;
    }
    Some(total)
}

fn series_exp(argument: &Series) -> Option<Series> {
    let mut result: Series = vec![argument.first()?.exp()?];
    for order in 1..argument.len() {
        let next =
            weighted_sum(argument, &result, order, order)?.scaled(&reciprocal_of_count(order)?)?;
        result.push(next);
    }
    Some(result)
}

fn series_ln(argument: &Series) -> Option<Series> {
    let base = argument.first()?;
    let mut result: Series = vec![base.ln()?];
    for order in 1..argument.len() {
        let carried = weighted_sum(&result, argument, order, order - 1)?
            .scaled(&reciprocal_of_count(order)?)?;
        result.push(argument.get(order)?.minus(&carried)?.over(base)?);
    }
    Some(result)
}

fn series_sine_and_cosine(argument: &Series) -> Option<(Series, Series)> {
    let first = argument.first()?;
    let mut sine: Series = vec![first.sin()?];
    let mut cosine: Series = vec![first.cos()?];
    for order in 1..argument.len() {
        let reciprocal = reciprocal_of_count(order)?;
        let next_sine = weighted_sum(argument, &cosine, order, order)?.scaled(&reciprocal)?;
        let next_cosine = weighted_sum(argument, &sine, order, order)?
            .scaled(&reciprocal)?
            .negated();
        sine.push(next_sine);
        cosine.push(next_cosine);
    }
    Some((sine, cosine))
}

fn series_sqrt(argument: &Series) -> Option<Series> {
    let root = argument.first()?.sqrt()?;
    let doubled = root.scaled(&Interval::point(2.0)?)?;
    let mut result: Series = vec![root];
    for order in 1..argument.len() {
        let mut numerator = argument.get(order)?.clone();
        for part in 1..order {
            numerator = numerator.minus(&result.get(part)?.times(result.get(order - part)?)?)?;
        }
        result.push(numerator.over(&doubled)?);
    }
    Some(result)
}

fn series_power(base: &Series, exponent: &Interval) -> Option<Series> {
    let first = base.first()?;
    let width = first.gradient.len();
    let mut result: Series = vec![first.power(exponent)?];
    for order in 1..base.len() {
        let mut total = Jet::constant(zero()?, width)?;
        for (part, earlier) in result.iter().enumerate() {
            let weight = exponent
                .mul(&counted(order - part)?)?
                .sub(&counted(part)?)?;
            total = total.plus(&base.get(order - part)?.times(earlier)?.scaled(&weight)?)?;
        }
        result.push(total.over(&first.scaled(&counted(order)?)?)?);
    }
    Some(result)
}

fn binary_digits(value: f64) -> Option<Vec<bool>> {
    if !(value.is_finite() && value >= 0.0 && value.fract() == 0.0) {
        return None;
    }
    let mut digits = Vec::new();
    let mut rest = value;
    while rest > 0.0 {
        let digit = rest % 2.0;
        digits.push(digit == 1.0);
        rest = (rest - digit) / 2.0;
    }
    Some(digits)
}

fn series_whole_power(base: &Series, digits: &[bool]) -> Option<Series> {
    let width = base.first()?.gradient.len();
    let mut result = constant_series(one()?, base.len(), width)?;
    let mut square = base.clone();
    for (position, digit) in digits.iter().enumerate() {
        if *digit {
            result = series_times(&result, &square)?;
        }
        if position + 1 < digits.len() {
            square = series_times(&square, &square)?;
        }
    }
    Some(result)
}

struct Evaluation<'problem> {
    pool: &'problem ExprPool,
    problem: &'problem OdeProblem,
    time: Series,
    state: &'problem [Series],
    length: usize,
    width: usize,
    memo: HashMap<ExprId, Series>,
}

impl Evaluation<'_> {
    fn mentions_a_variable(&self, expression: ExprId) -> bool {
        mentions(self.pool, self.problem, expression)
    }

    fn series(&mut self, expression: ExprId) -> Result<Series, Failure> {
        if let Some(known) = self.memo.get(&expression) {
            return Ok(known.clone());
        }
        let unsupported = OdeRefusal::Unsupported(expression);
        let result = if !self.mentions_a_variable(expression) {
            let value = real_enclosure(self.pool, expression).ok_or(unsupported)?;
            arithmetic(constant_series(value, self.length, self.width))?
        } else {
            match self.pool.node(expression) {
                Ok(NodeView::Symbol(symbol)) if symbol == self.problem.time => self.time.clone(),
                Ok(NodeView::Symbol(symbol)) => {
                    let known = self
                        .problem
                        .names
                        .iter()
                        .position(|name| *name == symbol)
                        .and_then(|index| self.state.get(index))
                        .and_then(|series| series.get(..self.length))
                        .ok_or(unsupported)?;
                    known.to_vec()
                }
                Ok(NodeView::Apply {
                    head: Head::Operator(operator),
                    arguments,
                }) => {
                    let arguments = arguments.to_vec();
                    self.applied(expression, operator, &arguments)?
                }
                _ => return Err(unsupported.into()),
            }
        };
        self.memo.insert(expression, result.clone());
        Ok(result)
    }

    fn applied(
        &mut self,
        expression: ExprId,
        operator: Operator,
        arguments: &[ExprId],
    ) -> Result<Series, Failure> {
        let result = match (operator, arguments) {
            (Operator::Add, [left, right]) => {
                series_plus(&self.series(*left)?, &self.series(*right)?)
            }
            (Operator::Sub, [left, right]) => {
                series_minus(&self.series(*left)?, &self.series(*right)?)
            }
            (Operator::Mul, [left, right]) => {
                series_times(&self.series(*left)?, &self.series(*right)?)
            }
            (Operator::Div, [left, right]) => {
                series_over(&self.series(*left)?, &self.series(*right)?)
            }
            (Operator::Neg, [argument]) => {
                Some(self.series(*argument)?.iter().map(Jet::negated).collect())
            }
            (Operator::Exp, [argument]) => series_exp(&self.series(*argument)?),
            (Operator::Ln, [argument]) => series_ln(&self.series(*argument)?),
            (Operator::Sqrt, [argument]) => series_sqrt(&self.series(*argument)?),
            (Operator::Sin, [argument]) => {
                series_sine_and_cosine(&self.series(*argument)?).map(|(sine, _)| sine)
            }
            (Operator::Cos, [argument]) => {
                series_sine_and_cosine(&self.series(*argument)?).map(|(_, cosine)| cosine)
            }
            (Operator::Pow, [base, exponent]) if !self.mentions_a_variable(*exponent) => {
                let base = self.series(*base)?;
                let power = real_enclosure(self.pool, *exponent)
                    .ok_or(OdeRefusal::Unsupported(expression))?;
                let whole = power.lower() == power.upper();
                let digits = whole.then(|| binary_digits(power.lower().abs())).flatten();
                match digits {
                    Some(digits) if power.lower() < 0.0 => {
                        let width = base.first().map_or(0, |jet| jet.gradient.len());
                        let unit = arithmetic(
                            one().and_then(|one| constant_series(one, base.len(), width)),
                        )?;
                        series_whole_power(&base, &digits)
                            .and_then(|powered| series_over(&unit, &powered))
                    }
                    Some(digits) => series_whole_power(&base, &digits),
                    None => series_power(&base, &power),
                }
            }
            _ => return Err(OdeRefusal::Unsupported(expression).into()),
        };
        arithmetic(result)
    }
}

fn time_series(start: Interval, length: usize, width: usize) -> Option<Series> {
    let mut series = constant_series(start, length, width)?;
    if let Some(slope) = series.get_mut(1) {
        slope.value = one()?;
    }
    Some(series)
}

fn taylor_coefficients(
    pool: &ExprPool,
    problem: &OdeProblem,
    time: Interval,
    state: &[Jet],
    order: usize,
) -> Result<Vec<Series>, Failure> {
    let width = state.first().map_or(0, |jet| jet.gradient.len());
    let mut coefficients: Vec<Series> = state.iter().map(|jet| vec![jet.clone()]).collect();
    for current in 0..order {
        let length = current + 1;
        let mut evaluation = Evaluation {
            pool,
            problem,
            time: arithmetic(time_series(time, length, width))?,
            state: &coefficients,
            length,
            width,
            memo: HashMap::new(),
        };
        let reciprocal = arithmetic(reciprocal_of_count(current + 1))?;
        let mut next = Vec::with_capacity(problem.derivatives.len());
        for derivative in &problem.derivatives {
            let series = evaluation.series(*derivative)?;
            next.push(arithmetic(
                series.get(current).and_then(|jet| jet.scaled(&reciprocal)),
            )?);
        }
        for (series, coefficient) in coefficients.iter_mut().zip(next) {
            series.push(coefficient);
        }
    }
    Ok(coefficients)
}

fn plain(values: &[Interval]) -> Option<Vec<Jet>> {
    values
        .iter()
        .map(|value| Jet::constant(*value, 0))
        .collect()
}

fn seeded(values: &[Interval]) -> Option<Vec<Jet>> {
    let width = values.len();
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let mut jet = Jet::constant(*value, width)?;
            *jet.gradient.get_mut(index)? = one()?;
            Some(jet)
        })
        .collect()
}

fn powers(step: &Interval, order: usize) -> Option<Vec<Interval>> {
    let mut result = vec![one()?];
    for _ in 0..order {
        let next = result.last()?.mul(step)?;
        result.push(next);
    }
    Some(result)
}

fn polynomial(coefficients: &Series, powers: &[Interval], order: usize) -> Option<Jet> {
    let mut total = coefficients.first()?.clone();
    for index in 1..=order {
        total = total.plus(&coefficients.get(index)?.scaled(powers.get(index)?)?)?;
    }
    Some(total)
}

fn tail(coefficients: &Series, powers: &[Interval], order: usize) -> Option<Interval> {
    coefficients.get(order)?.value.mul(powers.get(order)?)
}

type Matrix = Vec<Vec<f64>>;
type IntervalMatrix = Vec<Vec<Interval>>;

fn identity(size: usize) -> Matrix {
    (0..size)
        .map(|row| {
            (0..size)
                .map(|column| if row == column { 1.0 } else { 0.0 })
                .collect()
        })
        .collect()
}

fn as_intervals(matrix: &Matrix) -> Option<IntervalMatrix> {
    matrix
        .iter()
        .map(|row| row.iter().map(|entry| Interval::point(*entry)).collect())
        .collect()
}

fn transposed(matrix: &Matrix) -> Matrix {
    (0..matrix.len())
        .map(|row| {
            matrix
                .iter()
                .filter_map(|entries| entries.get(row).copied())
                .collect()
        })
        .collect()
}

fn applied_to(matrix: &IntervalMatrix, vector: &[Interval]) -> Option<Vec<Interval>> {
    matrix
        .iter()
        .map(|row| {
            let mut total = zero()?;
            for (entry, value) in row.iter().zip(vector) {
                total = total.add(&entry.mul(value)?)?;
            }
            Some(total)
        })
        .collect()
}

fn product(left: &IntervalMatrix, right: &IntervalMatrix) -> Option<IntervalMatrix> {
    let columns = right.first().map_or(0, Vec::len);
    left.iter()
        .map(|row| {
            (0..columns)
                .map(|column| {
                    let mut total = zero()?;
                    for (entry, right_row) in row.iter().zip(right) {
                        total = total.add(&entry.mul(right_row.get(column)?)?)?;
                    }
                    Some(total)
                })
                .collect()
        })
        .collect()
}

fn orthonormal_columns(matrix: &Matrix) -> Option<Matrix> {
    let mut columns = transposed(matrix);
    for current in 0..columns.len() {
        for earlier in 0..current {
            let reference = columns.get(earlier)?.clone();
            let column = columns.get_mut(current)?;
            let projection: f64 = column
                .iter()
                .zip(&reference)
                .map(|(one, two)| one * two)
                .sum();
            for (entry, direction) in column.iter_mut().zip(&reference) {
                *entry -= projection * direction;
            }
        }
        let column = columns.get_mut(current)?;
        let norm = column.iter().map(|entry| entry * entry).sum::<f64>().sqrt();
        if !(norm.is_finite() && norm > 0.0) {
            return None;
        }
        for entry in column.iter_mut() {
            *entry /= norm;
        }
    }
    Some(transposed(&columns))
}

fn row_sum_bound(matrix: &IntervalMatrix) -> Option<Interval> {
    let mut largest = zero()?;
    for row in matrix {
        let mut total = zero()?;
        for entry in row {
            total = total.add(&Interval::point(magnitude(entry))?)?;
        }
        largest = largest.max(&total);
    }
    Some(largest)
}

fn enclosed_inverse(basis: &Matrix) -> Option<IntervalMatrix> {
    let approximate = as_intervals(&transposed(basis))?;
    let near_identity = product(&approximate, &as_intervals(basis)?)?;
    let residual = as_intervals(&identity(basis.len()))?
        .iter()
        .zip(&near_identity)
        .map(|(ones, entries)| {
            ones.iter()
                .zip(entries)
                .map(|(one, entry)| one.sub(entry))
                .collect()
        })
        .collect::<Option<IntervalMatrix>>()?;
    let defect = row_sum_bound(&residual)?;
    if defect.upper() >= 0.5 {
        return None;
    }
    let spread = defect
        .mul(&row_sum_bound(&approximate)?)?
        .div(&one()?.sub(&defect)?)?
        .upper();
    let widening = span(-spread, spread)?;
    approximate
        .iter()
        .map(|row| row.iter().map(|entry| entry.add(&widening)).collect())
        .collect()
}

struct State {
    center: Vec<f64>,
    basis: Matrix,
    offsets: Vec<Interval>,
    enclosure: Vec<Interval>,
}

fn centered(values: &[Interval], center: &[f64]) -> Option<Vec<Interval>> {
    values
        .iter()
        .zip(center)
        .map(|(value, center)| value.sub(&Interval::point(*center)?))
        .collect()
}

fn points(values: &[f64]) -> Option<Vec<Interval>> {
    values.iter().map(|value| Interval::point(*value)).collect()
}

impl State {
    fn from_values(values: Vec<Interval>) -> Option<State> {
        let center: Vec<f64> = values.iter().map(middle).collect();
        Some(State {
            basis: identity(values.len()),
            offsets: centered(&values, &center)?,
            center,
            enclosure: values,
        })
    }

    fn mean_value_box(&self) -> Option<Vec<Interval>> {
        self.enclosure
            .iter()
            .zip(&self.center)
            .map(|(value, center)| Some(value.hull(&Interval::point(*center)?)))
            .collect()
    }
}

fn rough_enclosure(
    pool: &ExprPool,
    problem: &OdeProblem,
    start: Interval,
    time_range: Interval,
    values: &[Interval],
    reach: &Interval,
) -> Result<Vec<Interval>, Failure> {
    let order = TAYLOR_ORDER;
    let reach_powers = arithmetic(powers(reach, order + 1))?;
    let over_box =
        taylor_coefficients(pool, problem, start, &arithmetic(plain(values))?, order + 1)?;
    let mut guess = Vec::with_capacity(values.len());
    let mut margins = Vec::with_capacity(values.len());
    for series in &over_box {
        guess.push(arithmetic(polynomial(series, &reach_powers, order))?.value);
        let last = arithmetic(tail(series, &reach_powers, order + 1))?;
        margins.push((2.0 * magnitude(&last)).max(f64::MIN_POSITIVE).next_up());
    }
    for _ in 0..ROUGH_ENCLOSURE_TRIES {
        let candidate = arithmetic(
            guess
                .iter()
                .zip(&margins)
                .map(|(value, margin): (&Interval, &f64)| value.add(&span(-*margin, *margin)?))
                .collect::<Option<Vec<_>>>(),
        )?;
        let over_candidate = taylor_coefficients(
            pool,
            problem,
            time_range,
            &arithmetic(plain(&candidate))?,
            order + 1,
        )?;
        let mut holds = true;
        let mut needed = Vec::with_capacity(margins.len());
        for (series, margin) in over_candidate.iter().zip(&margins) {
            let last = arithmetic(tail(series, &reach_powers, order + 1))?;
            holds &= last.lower() > -*margin && last.upper() < *margin;
            needed.push(magnitude(&last));
        }
        if holds {
            return Ok(candidate);
        }
        margins = margins
            .iter()
            .zip(&needed)
            .map(|(margin, need)| (2.0 * margin.max(*need)).next_up())
            .collect();
        if !margins.iter().all(|margin| margin.is_finite()) {
            break;
        }
    }
    Err(Failure::Arithmetic)
}

fn step(
    pool: &ExprPool,
    problem: &OdeProblem,
    time: &ExactRational,
    length: &ExactRational,
    state: &State,
) -> Result<(State, Vec<f64>), Failure> {
    let order = TAYLOR_ORDER;
    let start = arithmetic(Interval::from_exact(&time.to_number()))?;
    let reach = arithmetic(Interval::from_exact(&length.to_number()))?;
    let zero_to_reach = arithmetic(zero())?.hull(&reach);
    let time_range = arithmetic(start.add(&zero_to_reach))?;
    let rough = rough_enclosure(
        pool,
        problem,
        start,
        time_range,
        &state.enclosure,
        &zero_to_reach,
    )?;
    let step_powers = arithmetic(powers(&reach, order + 1))?;
    let over_rough = taylor_coefficients(
        pool,
        problem,
        time_range,
        &arithmetic(plain(&rough))?,
        order + 1,
    )?;
    let tails = arithmetic(
        over_rough
            .iter()
            .map(|series| tail(series, &step_powers, order + 1))
            .collect::<Option<Vec<_>>>(),
    )?;
    let center_values = arithmetic(points(&state.center))?;
    let at_center = taylor_coefficients(
        pool,
        problem,
        start,
        &arithmetic(plain(&center_values))?,
        order,
    )?;
    let mean_value_box = arithmetic(state.mean_value_box())?;
    let over_box = taylor_coefficients(
        pool,
        problem,
        start,
        &arithmetic(seeded(&mean_value_box))?,
        order,
    )?;
    let size = state.center.len();
    let mut moved = Vec::with_capacity(size);
    let mut direct = Vec::with_capacity(size);
    let mut jacobian: IntervalMatrix = Vec::with_capacity(size);
    for ((center_series, box_series), last) in at_center.iter().zip(&over_box).zip(&tails) {
        let center_part = arithmetic(polynomial(center_series, &step_powers, order))?;
        let box_part = arithmetic(polynomial(box_series, &step_powers, order))?;
        moved.push(arithmetic(center_part.value.add(last))?);
        direct.push(arithmetic(box_part.value.add(last))?);
        jacobian.push(box_part.gradient);
    }
    let carried = arithmetic(product(&jacobian, &arithmetic(as_intervals(&state.basis))?))?;
    let center: Vec<f64> = moved.iter().map(middle).collect();
    let carried_middle: Matrix = carried
        .iter()
        .map(|row| row.iter().map(middle).collect())
        .collect();
    let chosen = orthonormal_columns(&carried_middle)
        .and_then(|basis| Some((enclosed_inverse(&basis)?, basis)));
    let (inverse, basis) = match chosen {
        Some(chosen) => chosen,
        None => (arithmetic(as_intervals(&identity(size)))?, identity(size)),
    };
    let transfer = arithmetic(product(&inverse, &carried))?;
    let spread = arithmetic(applied_to(&transfer, &state.offsets))?;
    let shift = arithmetic(applied_to(
        &inverse,
        &arithmetic(centered(&moved, &center))?,
    ))?;
    let offsets = arithmetic(
        spread
            .iter()
            .zip(&shift)
            .map(|(one, two)| one.add(two))
            .collect::<Option<Vec<_>>>(),
    )?;
    let image = arithmetic(applied_to(&arithmetic(as_intervals(&basis))?, &offsets))?;
    let enclosure = arithmetic(
        image
            .iter()
            .zip(&center)
            .zip(&direct)
            .map(|((offset, center), whole)| {
                intersection(&offset.add(&Interval::point(*center)?)?, whole)
            })
            .collect::<Option<Vec<_>>>(),
    )?;
    if !enclosure.iter().all(Interval::is_bounded) {
        return Err(Failure::Arithmetic);
    }
    let bounds = arithmetic(tails.iter().map(width_bound).collect::<Option<Vec<_>>>())?;
    Ok((
        State {
            center,
            basis,
            offsets,
            enclosure,
        },
        bounds,
    ))
}

fn coefficient_sizes(
    pool: &ExprPool,
    problem: &OdeProblem,
    time: &ExactRational,
    state: &State,
) -> Result<Vec<Vec<f64>>, Failure> {
    let start = arithmetic(Interval::from_exact(&time.to_number()))?;
    let center_values = arithmetic(points(&state.center))?;
    let at_center = taylor_coefficients(
        pool,
        problem,
        start,
        &arithmetic(plain(&center_values))?,
        TAYLOR_ORDER,
    )?;
    Ok(at_center
        .iter()
        .map(|series| {
            series
                .iter()
                .map(|coefficient| magnitude(&coefficient.value))
                .collect()
        })
        .collect())
}

fn root_of(value: f64, index: usize) -> Option<f64> {
    Some(value.powf(1.0 / f64::from(u32::try_from(index).ok()?)))
}

fn component_radius(sizes: &[f64]) -> Option<f64> {
    let order = sizes.len().checked_sub(1)?;
    let (reference, offset) = match (sizes.get(1), sizes.first()) {
        (Some(slope), _) if *slope > 0.0 && slope.is_finite() => (*slope, 1),
        (_, Some(value)) if *value > 0.0 && value.is_finite() => (*value, 0),
        _ => return Some(f64::INFINITY),
    };
    let mut radius = f64::INFINITY;
    for index in [order - 1, order] {
        let size = *sizes.get(index)?;
        if size > 0.0 && size.is_finite() {
            radius = radius.min(root_of(reference / size, index - offset)?);
        }
    }
    Some(radius)
}

fn suggested_length(sizes: &[Vec<f64>]) -> Option<f64> {
    let mut radius = f64::INFINITY;
    for component in sizes {
        radius = radius.min(component_radius(component)?);
    }
    Some(radius / (std::f64::consts::E * std::f64::consts::E))
}

fn step_tolerances(sizes: &[Vec<f64>], length: f64) -> Vec<f64> {
    sizes
        .iter()
        .map(|component| {
            let mut power = 1.0_f64;
            let mut reach = 0.0_f64;
            for size in component {
                reach += size * power;
                power *= length;
            }
            (UNIT_ROUNDOFF * reach).max(f64::MIN_POSITIVE)
        })
        .collect()
}

fn within(bounds: &[f64], tolerances: &[f64]) -> bool {
    bounds
        .iter()
        .zip(tolerances)
        .all(|(bound, tolerance)| bound <= tolerance)
}

fn dyadic_at_most(value: f64) -> Option<ExactRational> {
    if !(value.is_finite() && value > 0.0) {
        return None;
    }
    let mut dyadic = 1.0_f64;
    while dyadic > value {
        dyadic /= 2.0;
        if dyadic == 0.0 {
            return None;
        }
    }
    while (dyadic * 2.0).is_finite() && dyadic * 2.0 <= value {
        dyadic *= 2.0;
    }
    ExactRational::from_number(&Number::F64(dyadic).to_exact().ok()?)
}

fn constant_zero_divisor(
    pool: &mut ExprPool,
    problem: &OdeProblem,
    expression: ExprId,
) -> Option<ExprId> {
    let mut pending = vec![expression];
    let mut divisions = Vec::new();
    while let Some(node) = pending.pop() {
        if let Ok(NodeView::Apply { head, arguments }) = pool.node(node) {
            if let (Head::Operator(Operator::Div), [_, divisor]) = (head, arguments)
                && !mentions(pool, problem, *divisor)
            {
                divisions.push((node, *divisor));
            }
            pending.extend(arguments.iter().copied());
        }
    }
    divisions.into_iter().find_map(|(division, divisor)| {
        closed_square_root_sum(pool, divisor)
            .is_some_and(|value| value.is_zero())
            .then_some(division)
    })
}

fn mentions(pool: &ExprPool, problem: &OdeProblem, expression: ExprId) -> bool {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        match pool.node(node) {
            Ok(NodeView::Symbol(symbol))
                if symbol == problem.time || problem.names.contains(&symbol) =>
            {
                return true;
            }
            Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().copied()),
            _ => {}
        }
    }
    false
}

fn refused_or(failure: Failure, time: &ExactRational) -> OdeRefusal {
    match failure {
        Failure::Refused(refusal) => refusal,
        Failure::Arithmetic => OdeRefusal::NotEnclosed {
            at: time.to_number(),
        },
    }
}

pub fn enclose_ode(
    pool: &mut ExprPool,
    problem: &OdeProblem,
    start: &Number,
    initial: &[Interval],
    stops: &[Number],
) -> Result<OdeEnclosure, OdeRefusal> {
    enclose_within(pool, problem, start, initial, stops, LARGEST_STEP_COUNT)
}

fn enclose_within(
    pool: &mut ExprPool,
    problem: &OdeProblem,
    start: &Number,
    initial: &[Interval],
    stops: &[Number],
    largest_step_count: usize,
) -> Result<OdeEnclosure, OdeRefusal> {
    let start = ExactRational::from_number(start).ok_or(OdeRefusal::MachineTime)?;
    let mut ordered = stops
        .iter()
        .map(|stop| ExactRational::from_number(stop).ok_or(OdeRefusal::MachineTime))
        .collect::<Result<Vec<_>, _>>()?;
    if problem.names.len() != problem.derivatives.len() || problem.names.len() != initial.len() {
        return Err(OdeRefusal::SizeMismatch);
    }
    for derivative in &problem.derivatives {
        if let Some(divisor) = constant_zero_divisor(pool, problem, *derivative) {
            return Err(OdeRefusal::DivisionByZero(divisor));
        }
    }
    let pool: &ExprPool = pool;
    if let Some(index) = initial.iter().position(|value| !value.is_bounded()) {
        return Err(OdeRefusal::InitialValue(index));
    }
    let mut state = State::from_values(initial.to_vec()).ok_or(OdeRefusal::InitialValue(0))?;
    ordered.sort_by(ExactRational::compare);
    if ordered
        .first()
        .is_some_and(|first| first.compare(&start) == Ordering::Less)
    {
        return Err(OdeRefusal::StopBeforeStart);
    }
    let mut time = start;
    let mut points = Vec::with_capacity(ordered.len());
    let mut steps = 0;
    let mut largest_step_bounds = vec![0.0_f64; initial.len()];
    let mut last_length = ExactRational::zero();
    for stop in ordered {
        while time.compare(&stop) == Ordering::Less {
            if steps >= largest_step_count {
                return Err(OdeRefusal::TooManySteps(Box::new(StepLimit {
                    at: time.to_number(),
                    step: last_length.to_number(),
                })));
            }
            let remaining = stop.subtract(&time);
            let sizes = coefficient_sizes(pool, problem, &time, &state)
                .map_err(|failure| refused_or(failure, &time))?;
            let suggested = suggested_length(&sizes).unwrap_or(f64::INFINITY);
            let mut length = match dyadic_at_most(suggested) {
                Some(dyadic) if dyadic.compare(&remaining) == Ordering::Less => dyadic,
                _ => remaining,
            };
            let mut halvings = 0;
            let mut wide: Option<(State, Vec<f64>, ExactRational)> = None;
            let (next, bounds, length) = loop {
                let reach =
                    Interval::from_exact(&length.to_number()).map_or(0.0, |reach| reach.upper());
                let tolerances = step_tolerances(&sizes, reach);
                match step(pool, problem, &time, &length, &state) {
                    Ok((next, bounds))
                        if within(&bounds, &tolerances) || halvings >= LARGEST_HALVING_COUNT =>
                    {
                        break (next, bounds, length);
                    }
                    Ok((next, bounds)) => wide = Some((next, bounds, length.clone())),
                    Err(Failure::Arithmetic) => {
                        if let Some(taken) = wide.take() {
                            break taken;
                        }
                        if halvings >= LARGEST_HALVING_COUNT {
                            return Err(refused_or(Failure::Arithmetic, &time));
                        }
                    }
                    Err(failure) => return Err(refused_or(failure, &time)),
                }
                halvings += 1;
                length = length.multiply(&ExactRational::one_half());
            };
            state = next;
            time = time.plus(&length);
            last_length = length;
            for (largest_bound, bound) in largest_step_bounds.iter_mut().zip(bounds) {
                *largest_bound = largest_bound.max(bound);
            }
            steps += 1;
        }
        points.push(OdePoint {
            time: stop.to_number(),
            values: state.enclosure.clone(),
        });
    }
    Ok(OdeEnclosure {
        points,
        steps,
        largest_step_bounds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_expr::SymbolKind;
    use calc_syntax::parse_expression;

    fn problem(pool: &mut ExprPool, system: &[(&str, &str)]) -> OdeProblem {
        let time = pool
            .intern_symbol("t", SymbolKind::Variable)
            .expect("the name interns");
        let names = system
            .iter()
            .map(|(name, _)| {
                pool.intern_symbol(name, SymbolKind::Variable)
                    .expect("the name interns")
            })
            .collect();
        let derivatives = system
            .iter()
            .map(|(_, text)| parse_expression(pool, text).expect("the input parses"))
            .collect();
        OdeProblem {
            time,
            names,
            derivatives,
        }
    }

    fn rational(numerator: i64, denominator: i64) -> ExactRational {
        ExactRational::from_i64(numerator)
            .divide(&ExactRational::from_i64(denominator))
            .expect("a nonzero denominator")
    }

    fn number(numerator: i64, denominator: i64) -> Number {
        rational(numerator, denominator).to_number()
    }

    fn solved(
        system: &[(&str, &str)],
        initial: &[Number],
        stops: &[Number],
    ) -> (ExprPool, Result<OdeEnclosure, OdeRefusal>) {
        let mut pool = ExprPool::new();
        let problem = problem(&mut pool, system);
        let initial: Vec<Interval> = initial
            .iter()
            .map(|value| Interval::from_exact(value).expect("an exact initial value encloses"))
            .collect();
        let result = enclose_ode(&mut pool, &problem, &number(0, 1), &initial, stops);
        (pool, result)
    }

    fn truth(text: &str) -> Interval {
        let mut pool = ExprPool::new();
        let expression = parse_expression(&mut pool, text).expect("the input parses");
        let decimal =
            crate::decimal_enclosure::enclose_decimal(&mut pool, expression, 30, &|| false)
                .expect("the closed form encloses");
        let lower = Interval::from_exact(&decimal.lower).expect("the lower end is finite");
        let upper = Interval::from_exact(&decimal.upper).expect("the upper end is finite");
        lower.hull(&upper)
    }

    fn assert_encloses(enclosure: &Interval, closed_form: &str, largest_width: f64) {
        let exact = truth(closed_form);
        assert!(
            enclosure.lower() <= exact.lower() && exact.upper() <= enclosure.upper(),
            "{closed_form} lies in [{}, {}], outside [{}, {}]",
            exact.lower(),
            exact.upper(),
            enclosure.lower(),
            enclosure.upper()
        );
        assert!(
            enclosure.upper() - enclosure.lower() <= largest_width,
            "the enclosure of {closed_form} is {} wide",
            enclosure.upper() - enclosure.lower()
        );
    }

    fn values_at(result: &Result<OdeEnclosure, OdeRefusal>, index: usize) -> Vec<Interval> {
        result.as_ref().expect("the problem is enclosed").points[index]
            .values
            .clone()
    }

    #[test]
    fn decay_at_one_encloses_the_reciprocal_of_e() {
        let (_, result) = solved(&[("y", "-y")], &[number(1, 1)], &[number(1, 1)]);
        assert_encloses(&values_at(&result, 0)[0], "exp(-1)", 1e-12);
    }

    #[test]
    fn every_stop_is_enclosed_in_increasing_order() {
        let (_, result) = solved(
            &[("y", "-y")],
            &[number(1, 1)],
            &[number(1, 1), number(1, 3)],
        );
        assert_encloses(&values_at(&result, 0)[0], "exp(-1/3)", 1e-12);
        assert_encloses(&values_at(&result, 1)[0], "exp(-1)", 1e-12);
    }

    #[test]
    fn harmonic_oscillator_at_ten_encloses_cosine_and_sine() {
        let (_, result) = solved(
            &[("x", "v"), ("v", "-x")],
            &[number(1, 1), number(0, 1)],
            &[number(10, 1)],
        );
        let values = values_at(&result, 0);
        assert_encloses(&values[0], "cos(10)", 1e-10);
        assert_encloses(&values[1], "-sin(10)", 1e-10);
    }

    #[test]
    fn projectile_with_linear_drag_encloses_its_closed_form() {
        let (_, result) = solved(
            &[
                ("x", "u"),
                ("u", "-(1/2)*u"),
                ("y", "w"),
                ("w", "-981/100 - (1/2)*w"),
            ],
            &[number(0, 1), number(10, 1), number(0, 1), number(10, 1)],
            &[number(2, 1)],
        );
        let values = values_at(&result, 0);
        assert_encloses(&values[0], "20*(1 - exp(-1))", 1e-10);
        assert_encloses(&values[1], "10*exp(-1)", 1e-10);
        assert_encloses(&values[2], "2*(10 + 981/50)*(1 - exp(-1)) - 981/25", 1e-10);
        assert_encloses(&values[3], "(10 + 981/50)*exp(-1) - 981/50", 1e-10);
    }

    #[test]
    fn a_right_side_in_time_alone_is_integrated_exactly_enough() {
        let (_, result) = solved(&[("y", "t")], &[number(0, 1)], &[number(2, 1)]);
        assert_encloses(&values_at(&result, 0)[0], "2", 1e-12);
    }

    #[test]
    fn a_whole_power_and_a_quotient_are_enclosed() {
        let (_, result) = solved(&[("y", "y^2 / (1 + t)")], &[number(1, 2)], &[number(1, 1)]);
        assert_encloses(&values_at(&result, 0)[0], "1/(2 - ln(2))", 1e-10);
    }

    #[test]
    fn a_solution_that_blows_up_is_refused() {
        let (_, result) = solved(&[("y", "y^2")], &[number(1, 1)], &[number(2, 1)]);
        assert!(matches!(
            result,
            Err(OdeRefusal::NotEnclosed { .. } | OdeRefusal::TooManySteps(_))
        ));
    }

    #[test]
    fn a_function_without_a_series_is_refused_by_name() {
        let (_, result) = solved(&[("y", "floor(y)")], &[number(1, 1)], &[number(1, 1)]);
        assert!(matches!(result, Err(OdeRefusal::Unsupported(_))));
    }

    #[test]
    fn a_polynomial_solution_is_enclosed_as_narrowly_as_any_other() {
        let (_, result) = solved(&[("y", "sqrt(y)")], &[number(1, 1)], &[number(2, 1)]);
        assert_encloses(&values_at(&result, 0)[0], "4", 1e-10);
    }

    #[test]
    fn a_linear_solution_with_time_in_the_right_side_is_enclosed_narrowly() {
        let (_, result) = solved(&[("y", "y / (1 + t)")], &[number(1, 1)], &[number(2, 1)]);
        assert_encloses(&values_at(&result, 0)[0], "3", 1e-10);
    }

    #[test]
    fn a_negative_whole_power_of_a_negative_value_is_enclosed() {
        let (_, result) = solved(&[("y", "y^(-1)")], &[number(-1, 1)], &[number(1, 1)]);
        assert_encloses(&values_at(&result, 0)[0], "-sqrt(3)", 1e-10);
    }

    #[test]
    fn a_division_by_a_constant_zero_is_refused_as_a_division_by_zero() {
        let (_, result) = solved(&[("y", "y / (1 - 1)")], &[number(1, 1)], &[number(1, 1)]);
        assert!(matches!(result, Err(OdeRefusal::DivisionByZero(_))));
    }

    #[test]
    fn an_initial_value_known_only_as_an_enclosure_is_carried() {
        let mut pool = ExprPool::new();
        let problem = problem(&mut pool, &[("y", "-y")]);
        let result = enclose_ode(
            &mut pool,
            &problem,
            &number(0, 1),
            &[truth("pi")],
            &[number(1, 1)],
        );
        assert_encloses(&values_at(&result, 0)[0], "pi*exp(-1)", 1e-12);
    }

    #[test]
    fn the_step_limit_names_the_last_step_length() {
        let mut pool = ExprPool::new();
        let problem = problem(&mut pool, &[("x", "v"), ("v", "-1000000*x")]);
        let initial = [truth("1"), truth("0")];
        let result = enclose_within(
            &mut pool,
            &problem,
            &number(0, 1),
            &initial,
            &[number(10, 1)],
            50,
        );
        let Err(OdeRefusal::TooManySteps(limit)) = result else {
            panic!("the fast oscillator reaches the step limit");
        };
        let step = Interval::from_exact(&limit.step).expect("the step is exact");
        assert!(step.upper() < 0.01 && step.lower() > 0.0);
    }

    #[test]
    fn a_small_start_is_enclosed_as_tightly_relative_to_its_size() {
        let (_, result) = solved(&[("y", "-y")], &[number(1, 100_000)], &[number(50, 1)]);
        let value = values_at(&result, 0)[0];
        let exact = truth("exp(-50)/100000");
        assert!(value.lower() <= exact.lower() && exact.upper() <= value.upper());
        assert!((value.upper() - value.lower()) / exact.lower() < 1e-12);
    }

    #[test]
    fn scaling_a_linear_problem_by_a_power_of_two_scales_its_enclosure_exactly() {
        let scale = f64::from_bits((1023 - 40) << 52);
        let (_, unscaled) = solved(
            &[("x", "v"), ("v", "-x")],
            &[number(1, 1), number(1, 2)],
            &[number(10, 1)],
        );
        let mut pool = ExprPool::new();
        let problem = problem(&mut pool, &[("x", "v"), ("v", "-x")]);
        let initial = [
            Interval::point(scale).unwrap(),
            Interval::point(scale / 2.0).unwrap(),
        ];
        let scaled = enclose_ode(
            &mut pool,
            &problem,
            &number(0, 1),
            &initial,
            &[number(10, 1)],
        );
        let (unscaled, scaled) = (unscaled.unwrap(), scaled.unwrap());
        assert_eq!(unscaled.steps, scaled.steps);
        for (one, two) in unscaled.points[0]
            .values
            .iter()
            .zip(&scaled.points[0].values)
        {
            assert_eq!((one.lower() * scale).to_bits(), two.lower().to_bits());
            assert_eq!((one.upper() * scale).to_bits(), two.upper().to_bits());
        }
    }

    fn largest(values: &[f64]) -> f64 {
        values
            .iter()
            .fold(0.0_f64, |largest, value| largest.max(*value))
    }

    #[test]
    fn a_state_that_stays_exactly_zero_is_enclosed_at_once() {
        let (_, result) = solved(&[("y", "y")], &[number(0, 1)], &[number(1, 1)]);
        let enclosure = result.expect("the problem is enclosed");
        let value = enclosure.points[0].values[0];
        assert!(value.lower() <= 0.0 && 0.0 <= value.upper());
        assert!(value.upper() - value.lower() < f64::MIN_POSITIVE);
        assert!(enclosure.steps < 10);
    }

    #[test]
    fn a_small_component_beside_a_large_one_keeps_its_own_relative_width() {
        let (_, result) = solved(
            &[("a", "-a"), ("b", "-10*b")],
            &[number(10_000_000_000, 1), number(1, 10_000_000_000)],
            &[number(2, 1)],
        );
        let values = values_at(&result, 0);
        let exact = truth("exp(-20)/10000000000");
        assert!(values[1].lower() <= exact.lower() && exact.upper() <= values[1].upper());
        assert!((values[1].upper() - values[1].lower()) / exact.lower() < 1e-10);
    }

    #[test]
    fn a_stop_before_the_start_is_refused() {
        let (_, result) = solved(&[("y", "-y")], &[number(1, 1)], &[number(-1, 1)]);
        assert_eq!(result.err(), Some(OdeRefusal::StopBeforeStart));
    }

    #[test]
    fn the_per_step_bound_is_reported_beside_the_steps() {
        let (_, result) = solved(&[("y", "-y")], &[number(1, 1)], &[number(1, 1)]);
        let enclosure = result.expect("the problem is enclosed");
        assert!(enclosure.steps >= 1);
        let bound = largest(&enclosure.largest_step_bounds);
        assert!(bound > 0.0 && bound < 1e-12);
    }

    type MeasuredCase = (
        &'static str,
        &'static [(&'static str, &'static str)],
        Vec<Number>,
        Number,
    );

    #[test]
    #[ignore = "a measurement, run by hand"]
    fn measurement() {
        let cases: Vec<MeasuredCase> = vec![
            (
                "large decay to 1",
                &[("y", "-y")],
                vec![number(1_000_000, 1)],
                number(1, 1),
            ),
            (
                "decay to 1",
                &[("y", "-y")],
                vec![number(1, 1)],
                number(1, 1),
            ),
            (
                "oscillator to 10",
                &[("x", "v"), ("v", "-x")],
                vec![number(1, 1), number(0, 1)],
                number(10, 1),
            ),
            (
                "oscillator to 100",
                &[("x", "v"), ("v", "-x")],
                vec![number(1, 1), number(0, 1)],
                number(100, 1),
            ),
            (
                "lotka-volterra to 10",
                &[("x", "x - x*y"), ("y", "x*y - y")],
                vec![number(2, 1), number(1, 1)],
                number(10, 1),
            ),
            (
                "square root to 2",
                &[("y", "sqrt(y)")],
                vec![number(1, 1)],
                number(2, 1),
            ),
            (
                "reciprocal from -1 to 1",
                &[("y", "y^(-1)")],
                vec![number(-1, 1)],
                number(1, 1),
            ),
            (
                "pendulum to 20",
                &[("x", "v"), ("v", "-sin(x)")],
                vec![number(1, 1), number(0, 1)],
                number(20, 1),
            ),
            (
                "fast oscillator to 10",
                &[("x", "v"), ("v", "-1000000*x")],
                vec![number(1, 1), number(0, 1)],
                number(10, 1),
            ),
            (
                "blow-up to 2",
                &[("y", "y^2")],
                vec![number(1, 1)],
                number(2, 1),
            ),
        ];
        for (name, system, initial, stop) in cases {
            let begun = std::time::Instant::now();
            let (_, result) = solved(system, &initial, &[stop]);
            let elapsed = begun.elapsed();
            match result {
                Ok(enclosure) => {
                    let widths: Vec<f64> = enclosure.points[0]
                        .values
                        .iter()
                        .map(|value| value.upper() - value.lower())
                        .collect();
                    println!(
                        "MEASURE order {TAYLOR_ORDER} | {name} | steps {} | per-step {:.2e} | widths {:?} | {:?}",
                        enclosure.steps,
                        largest(&enclosure.largest_step_bounds),
                        widths,
                        elapsed
                    );
                }
                Err(refusal) => println!(
                    "MEASURE order {TAYLOR_ORDER} | {name} | refused {} | {elapsed:?}",
                    refused_text(&refusal)
                ),
            }
        }
    }

    fn refused_text(refusal: &OdeRefusal) -> String {
        match refusal {
            OdeRefusal::NotEnclosed { at } => format!(
                "not enclosed at {}",
                Interval::from_exact(at).map_or(f64::NAN, |at| at.lower())
            ),
            OdeRefusal::TooManySteps(limit) => format!(
                "too many steps at {}",
                Interval::from_exact(&limit.at).map_or(f64::NAN, |at| at.lower())
            ),
            other => format!("{other:?}"),
        }
    }
}
