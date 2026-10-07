use std::collections::HashMap;

use calc_core::{
    LARGEST_HALVING_COUNT, LARGEST_STEP_COUNT, OdeProblem, OdeRefusal, TAYLOR_ORDER,
    enclose_decimal, enclose_ode, to_coherent_units,
};
use calc_expr::{
    BinderKind, ExprId, ExprPool, Head, NodeView, Operator, SymbolId, SymbolKind,
    substitute_symbols,
};
use calc_numbers::{DecimalEnclosure, Interval, Number, enclose_between};
use calc_syntax::{ParseError, PrintMode, parse_expression, print_expression};
use calc_units::UnitId;

use crate::json::{Json, write_canonical};
use crate::session_file::{number_json, unit_text};

const PART_SEPARATOR: char = ';';
const LIST_SEPARATOR: char = ',';
const INITIAL_DIGITS: u32 = 20;
const LARGEST_SHOWN_DIGITS: u32 = 17;
const STEP_BOUND_DIGITS: u32 = 2;
const METHOD_NAME: &str = "lohner_interval_taylor";
const UNIT_ONE: &str = "1";
const UNIT_PER: char = '/';

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OdeComponent {
    pub name: String,
    pub unit: Option<String>,
    pub step_bound: Number,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OdeTimePoint {
    pub time: Number,
    pub written: Option<String>,
    pub values: Vec<DecimalEnclosure>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OdeReport {
    pub time_name: String,
    pub time_unit: Option<String>,
    pub components: Vec<OdeComponent>,
    pub points: Vec<OdeTimePoint>,
    pub steps: usize,
    pub order: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OdeTime {
    pub name: String,
    pub unit: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OdeError {
    Unreadable(Box<OdeUnreadable>),
    NotAnAssignment {
        part: String,
    },
    NotADerivative {
        part: String,
    },
    TimeNamesDiffer {
        first: String,
        second: String,
    },
    ComponentTwice {
        name: String,
    },
    NoInitialValue {
        name: String,
    },
    NotInTheSystem {
        name: String,
    },
    NoTimes,
    UnknownName {
        name: String,
        part: String,
    },
    UnitMismatch {
        name: String,
        time: String,
        found: String,
        wanted: String,
    },
    UnitsDoNotCombine {
        name: String,
    },
    TimeUnitMismatch {
        part: String,
    },
    Inconsistent,
    UnitWithOffset {
        name: String,
    },
    NotARealNumber {
        part: String,
    },
    TimeNotRational {
        part: String,
    },
    Unsupported {
        part: String,
    },
    DivisionByZero {
        part: String,
    },
    StopBeforeStart,
    NotEnclosed(Box<OdeHalted>),
    TooManySteps(Box<OdeStepLimit>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OdeUnreadable {
    pub part: String,
    pub error: ParseError,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OdeHalted {
    pub time: OdeTime,
    pub at: Number,
    pub halvings: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OdeStepLimit {
    pub time: OdeTime,
    pub at: Number,
    pub step: Number,
    pub stop: Number,
    pub steps: usize,
}

struct Equation {
    name: SymbolId,
    time: SymbolId,
    right: ExprId,
    text: String,
}

struct Assignment {
    name: SymbolId,
    value: ExprId,
    text: String,
}

fn parts(text: &str) -> impl Iterator<Item = &str> {
    text.split(PART_SEPARATOR)
        .map(str::trim)
        .filter(|part| !part.is_empty())
}

fn parsed(pool: &mut ExprPool, part: &str) -> Result<ExprId, OdeError> {
    parse_expression(pool, part).map_err(|error| {
        OdeError::Unreadable(Box::new(OdeUnreadable {
            part: part.to_owned(),
            error,
        }))
    })
}

fn name_of(pool: &ExprPool, symbol: SymbolId) -> String {
    pool.symbol_name(symbol).unwrap_or_default().to_owned()
}

fn printed(pool: &ExprPool, expression: ExprId) -> String {
    print_expression(pool, expression, PrintMode::Ascii).unwrap_or_default()
}

fn equation_sides(pool: &ExprPool, expression: ExprId) -> Option<(ExprId, ExprId)> {
    match pool.node(expression) {
        Ok(NodeView::Apply {
            head: Head::Operator(Operator::Equal),
            arguments: [left, right],
        }) => Some((*left, *right)),
        _ => None,
    }
}

fn symbol_of(pool: &ExprPool, expression: ExprId) -> Option<SymbolId> {
    match pool.node(expression) {
        Ok(NodeView::Symbol(symbol)) => Some(symbol),
        _ => None,
    }
}

fn equation(pool: &mut ExprPool, part: &str) -> Result<Equation, OdeError> {
    let expression = parsed(pool, part)?;
    let not_a_derivative = || OdeError::NotADerivative {
        part: part.to_owned(),
    };
    let (left, right) = equation_sides(pool, expression).ok_or_else(not_a_derivative)?;
    let Ok(NodeView::Bind {
        binder: BinderKind::Derivative,
        arguments: [point],
        body,
    }) = pool.node(left)
    else {
        return Err(not_a_derivative());
    };
    let time = symbol_of(pool, *point).ok_or_else(not_a_derivative)?;
    let name = symbol_of(pool, body).ok_or_else(not_a_derivative)?;
    Ok(Equation {
        name,
        time,
        right,
        text: part.to_owned(),
    })
}

fn assignment(pool: &mut ExprPool, part: &str) -> Result<Assignment, OdeError> {
    let expression = parsed(pool, part)?;
    let (left, value) =
        equation_sides(pool, expression).ok_or_else(|| OdeError::NotAnAssignment {
            part: part.to_owned(),
        })?;
    let name = symbol_of(pool, left).ok_or_else(|| OdeError::NotAnAssignment {
        part: part.to_owned(),
    })?;
    Ok(Assignment {
        name,
        value,
        text: part.to_owned(),
    })
}

fn free_symbols(pool: &ExprPool, expression: ExprId) -> Vec<SymbolId> {
    let mut found = Vec::new();
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        match pool.node(node) {
            Ok(NodeView::Symbol(symbol)) if !found.contains(&symbol) => found.push(symbol),
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
    found
}

fn written_unit(pool: &mut ExprPool, value: ExprId) -> Option<UnitId> {
    calc_core::expression_unit(pool, value).ok().flatten()
}

fn one_of(pool: &mut ExprPool, unit: UnitId) -> Option<ExprId> {
    let one = pool.number(Number::from(1_i64)).ok()?;
    pool.quantity(one, unit).ok()
}

fn operated(pool: &mut ExprPool, operator: Operator, arguments: &[ExprId]) -> Option<ExprId> {
    pool.apply(Head::Operator(operator), arguments).ok()
}

fn in_unit(pool: &mut ExprPool, value: ExprId, unit: Option<UnitId>) -> Option<ExprId> {
    let divided = match unit {
        Some(unit) => {
            let one = one_of(pool, unit)?;
            operated(pool, Operator::Div, &[value, one])?
        }
        None => value,
    };
    let coherent = to_coherent_units(pool, divided).ok()?;
    coherent
        .dimension
        .is_dimensionless()
        .then_some(coherent.expression)
}

fn has_offset(pool: &mut ExprPool, unit: UnitId) -> bool {
    let offset = pool
        .number(Number::from(0_i64))
        .ok()
        .and_then(|zero| pool.quantity(zero, unit).ok())
        .and_then(|zero| to_coherent_units(pool, zero).ok())
        .and_then(|coherent| calc_core::evaluate_exact(pool, coherent.expression).ok())
        .and_then(|value| value.rational_value().cloned());
    !offset.is_some_and(|offset| offset == Number::from(0_i64))
}

fn exact_rational(pool: &mut ExprPool, expression: ExprId) -> Option<Number> {
    calc_core::evaluate_exact(pool, expression)
        .ok()?
        .rational_value()
        .cloned()
}

fn enclosed(pool: &mut ExprPool, expression: ExprId) -> Option<Interval> {
    let decimal = enclose_decimal(pool, expression, INITIAL_DIGITS, &|| false).ok()?;
    let lower = Interval::from_exact(&decimal.lower)?;
    let upper = Interval::from_exact(&decimal.upper)?;
    Some(lower.hull(&upper))
}

fn shown_digits(value: &Interval) -> u32 {
    let width = value.upper() - value.lower();
    let size = value.lower().abs().max(value.upper().abs());
    if !(width > 0.0 && size > 0.0) {
        return LARGEST_SHOWN_DIGITS;
    }
    let digits = (size / width).log10().floor() + 2.0;
    (1..=LARGEST_SHOWN_DIGITS)
        .rev()
        .find(|shown| f64::from(*shown) <= digits)
        .unwrap_or(1)
}

fn decimal_of(value: &Interval) -> Option<DecimalEnclosure> {
    enclose_between(
        &Number::F64(value.lower()),
        &Number::F64(value.upper()),
        shown_digits(value),
    )
    .ok()
}

fn bound_of(bound: f64) -> Number {
    enclose_between(&Number::F64(bound), &Number::F64(bound), STEP_BOUND_DIGITS)
        .map_or(Number::F64(bound), |decimal| decimal.upper)
}

struct Units {
    time: Option<UnitId>,
    components: Vec<Option<UnitId>>,
}

fn right_sides(
    pool: &mut ExprPool,
    equations: &[Equation],
    units: &Units,
) -> Result<Vec<ExprId>, OdeError> {
    let time = equations
        .first()
        .map(|equation| equation.time)
        .ok_or(OdeError::NoTimes)?;
    let mut replacements = HashMap::new();
    for (equation, unit) in equations.iter().zip(&units.components) {
        if let Some(unit) = unit {
            let symbol = pool
                .symbol(equation.name)
                .map_err(|_| OdeError::Inconsistent)?;
            let quantity = pool
                .quantity(symbol, *unit)
                .map_err(|_| OdeError::Inconsistent)?;
            replacements.insert(equation.name, quantity);
        }
    }
    if let Some(unit) = units.time {
        let symbol = pool.symbol(time).map_err(|_| OdeError::Inconsistent)?;
        let quantity = pool
            .quantity(symbol, unit)
            .map_err(|_| OdeError::Inconsistent)?;
        replacements.insert(time, quantity);
    }
    let names: Vec<SymbolId> = equations.iter().map(|equation| equation.name).collect();
    let mut sides = Vec::with_capacity(equations.len());
    for (equation, unit) in equations.iter().zip(&units.components) {
        for symbol in free_symbols(pool, equation.right) {
            let known = symbol == time
                || names.contains(&symbol)
                || matches!(pool.symbol_kind(symbol), Ok(SymbolKind::Constant));
            if !known {
                return Err(OdeError::UnknownName {
                    name: name_of(pool, symbol),
                    part: equation.text.clone(),
                });
            }
        }
        let component_name = name_of(pool, equation.name);
        let substituted = substitute_symbols(pool, equation.right, &replacements)
            .map_err(|_| OdeError::Inconsistent)?;
        let Ok(found) = to_coherent_units(pool, substituted) else {
            return Err(OdeError::UnitsDoNotCombine {
                name: component_name,
            });
        };
        let found = unit_or_one(pool, found.unit);
        let wanted = match (unit, units.time) {
            (component, Some(time)) => format!(
                "{}{UNIT_PER}{}",
                unit_or_one(pool, *component),
                unit_or_one(pool, Some(time))
            ),
            (component, None) => unit_or_one(pool, *component),
        };
        let time_name = name_of(pool, time);
        let mismatch = || OdeError::UnitMismatch {
            name: component_name.clone(),
            time: time_name.clone(),
            found: found.clone(),
            wanted: wanted.clone(),
        };
        let per_time = match units.time {
            Some(unit) => {
                let one = one_of(pool, unit).ok_or_else(mismatch)?;
                operated(pool, Operator::Mul, &[substituted, one]).ok_or_else(mismatch)?
            }
            None => substituted,
        };
        let side = in_unit(pool, per_time, *unit).ok_or_else(mismatch)?;
        sides.push(side);
    }
    Ok(sides)
}

fn unit_or_one(pool: &ExprPool, unit: Option<UnitId>) -> String {
    unit.and_then(|unit| unit_text(pool, unit).ok().flatten())
        .unwrap_or_else(|| UNIT_ONE.to_owned())
}

fn system(pool: &mut ExprPool, text: &str) -> Result<Vec<Equation>, OdeError> {
    let equations = parts(text)
        .map(|part| equation(pool, part))
        .collect::<Result<Vec<_>, _>>()?;
    let Some(first) = equations.first() else {
        return Err(OdeError::NotADerivative {
            part: text.to_owned(),
        });
    };
    for (index, equation) in equations.iter().enumerate() {
        if equation.time != first.time {
            return Err(OdeError::TimeNamesDiffer {
                first: name_of(pool, first.time),
                second: name_of(pool, equation.time),
            });
        }
        if equation.name == equation.time {
            return Err(OdeError::NotADerivative {
                part: equation.text.clone(),
            });
        }
        if equations[..index]
            .iter()
            .any(|earlier| earlier.name == equation.name)
        {
            return Err(OdeError::ComponentTwice {
                name: name_of(pool, equation.name),
            });
        }
    }
    Ok(equations)
}

fn time_value(
    pool: &mut ExprPool,
    value: ExprId,
    unit: Option<UnitId>,
    part: &str,
) -> Result<Number, OdeError> {
    let in_time_unit = in_unit(pool, value, unit).ok_or_else(|| OdeError::TimeUnitMismatch {
        part: part.to_owned(),
    })?;
    exact_rational(pool, in_time_unit).ok_or_else(|| OdeError::TimeNotRational {
        part: part.to_owned(),
    })
}

fn stops(
    pool: &mut ExprPool,
    text: &str,
    time: SymbolId,
    unit: Option<UnitId>,
) -> Result<Vec<(Number, String)>, OdeError> {
    let time_name = name_of(pool, time);
    let mut found = Vec::new();
    for part in parts(text) {
        let (name, values) = part
            .split_once('=')
            .ok_or_else(|| OdeError::NotAnAssignment {
                part: part.to_owned(),
            })?;
        let name = name.trim();
        if name != time_name {
            return Err(OdeError::NotInTheSystem {
                name: name.to_owned(),
            });
        }
        for value in values.split(LIST_SEPARATOR).map(str::trim) {
            let expression = parsed(pool, value)?;
            found.push((time_value(pool, expression, unit, value)?, value.to_owned()));
        }
    }
    if found.is_empty() {
        return Err(OdeError::NoTimes);
    }
    Ok(found)
}

fn refusal(pool: &ExprPool, refusal: OdeRefusal, time: OdeTime, last_stop: &Number) -> OdeError {
    match refusal {
        OdeRefusal::Unsupported(expression) => OdeError::Unsupported {
            part: printed(pool, expression),
        },
        OdeRefusal::DivisionByZero(expression) => OdeError::DivisionByZero {
            part: printed(pool, expression),
        },
        OdeRefusal::StopBeforeStart => OdeError::StopBeforeStart,
        OdeRefusal::NotEnclosed { at } => OdeError::NotEnclosed(Box::new(OdeHalted {
            time,
            at,
            halvings: LARGEST_HALVING_COUNT,
        })),
        OdeRefusal::TooManySteps(limit) => OdeError::TooManySteps(Box::new(OdeStepLimit {
            time,
            at: limit.at,
            step: limit.step,
            stop: last_stop.clone(),
            steps: LARGEST_STEP_COUNT,
        })),
        OdeRefusal::SizeMismatch | OdeRefusal::MachineTime | OdeRefusal::InitialValue(_) => {
            OdeError::Inconsistent
        }
    }
}

pub fn solve_ode(
    system_text: &str,
    initial_text: &str,
    at_text: &str,
) -> Result<OdeReport, OdeError> {
    let mut pool = ExprPool::new();
    let equations = system(&mut pool, system_text)?;
    let time = equations
        .first()
        .map(|equation| equation.time)
        .ok_or(OdeError::NoTimes)?;
    let assignments = parts(initial_text)
        .map(|part| assignment(&mut pool, part))
        .collect::<Result<Vec<_>, _>>()?;
    for given in &assignments {
        if given.name != time && !equations.iter().any(|equation| equation.name == given.name) {
            return Err(OdeError::NotInTheSystem {
                name: name_of(&pool, given.name),
            });
        }
    }
    let unit_of = |pool: &mut ExprPool, given: &Assignment| -> Result<Option<UnitId>, OdeError> {
        let unit = written_unit(pool, given.value);
        if let Some(unit) = unit
            && has_offset(pool, unit)
        {
            return Err(OdeError::UnitWithOffset {
                name: name_of(pool, given.name),
            });
        }
        Ok(unit)
    };
    let time_name = name_of(&pool, time);
    let start_given = assignments
        .iter()
        .find(|given| given.name == time)
        .ok_or_else(|| OdeError::NoInitialValue {
            name: time_name.clone(),
        })?;
    let time_unit = unit_of(&mut pool, start_given)?;
    let start = time_value(&mut pool, start_given.value, time_unit, &start_given.text)?;
    let mut component_units = Vec::with_capacity(equations.len());
    let mut initial = Vec::with_capacity(equations.len());
    for equation in &equations {
        let given = assignments
            .iter()
            .find(|given| given.name == equation.name)
            .ok_or_else(|| OdeError::NoInitialValue {
                name: name_of(&pool, equation.name),
            })?;
        let unit = unit_of(&mut pool, given)?;
        let not_a_number = || OdeError::NotARealNumber {
            part: given.text.clone(),
        };
        let value = in_unit(&mut pool, given.value, unit).ok_or_else(not_a_number)?;
        initial.push(enclosed(&mut pool, value).ok_or_else(not_a_number)?);
        component_units.push(unit);
    }
    let units = Units {
        time: time_unit,
        components: component_units,
    };
    let derivatives = right_sides(&mut pool, &equations, &units)?;
    let written_stops = stops(&mut pool, at_text, time, time_unit)?;
    let stops: Vec<Number> = written_stops.iter().map(|(time, _)| time.clone()).collect();
    let last_stop = stops
        .iter()
        .max_by(|one, two| {
            Interval::from_exact(one)
                .map(|one| one.lower())
                .unwrap_or(0.0)
                .total_cmp(
                    &Interval::from_exact(two)
                        .map(|two| two.lower())
                        .unwrap_or(0.0),
                )
        })
        .cloned()
        .unwrap_or_else(|| start.clone());
    let problem = OdeProblem {
        time,
        names: equations.iter().map(|equation| equation.name).collect(),
        derivatives,
    };
    let time_unit_text = time_unit.and_then(|unit| unit_text(&pool, unit).ok().flatten());
    let enclosure =
        enclose_ode(&mut pool, &problem, &start, &initial, &stops).map_err(|refused| {
            let time = OdeTime {
                name: time_name.clone(),
                unit: time_unit_text.clone(),
            };
            refusal(&pool, refused, time, &last_stop)
        })?;
    let unit_texts: Vec<Option<String>> = units
        .components
        .iter()
        .map(|unit| unit.and_then(|unit| unit_text(&pool, unit).ok().flatten()))
        .collect();
    let components = equations
        .iter()
        .zip(unit_texts)
        .zip(&enclosure.largest_step_bounds)
        .map(|((equation, unit), bound)| OdeComponent {
            name: name_of(&pool, equation.name),
            unit,
            step_bound: bound_of(*bound),
        })
        .collect();
    let mut unused = vec![true; written_stops.len()];
    let points = enclosure
        .points
        .iter()
        .map(|point| {
            let written = written_stops
                .iter()
                .zip(unused.iter_mut())
                .find(|((time, _), free)| **free && *time == point.time)
                .map(|((_, written), free)| {
                    *free = false;
                    written.clone()
                });
            Some(OdeTimePoint {
                time: point.time.clone(),
                written,
                values: point
                    .values
                    .iter()
                    .map(decimal_of)
                    .collect::<Option<Vec<_>>>()?,
            })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(OdeError::Inconsistent)?;
    Ok(OdeReport {
        time_name,
        time_unit: time_unit_text,
        components,
        points,
        steps: enclosure.steps,
        order: TAYLOR_ORDER,
    })
}

fn report_value(system_text: &str, report: &OdeReport) -> Json {
    let unit = |unit: &Option<String>| Json::optional(unit.as_deref().map(Json::string));
    let count = |value: usize| Json::Count(u64::try_from(value).unwrap_or(u64::MAX));
    let components = report
        .components
        .iter()
        .map(|component| {
            Json::object(vec![
                ("name", Json::string(&component.name)),
                ("unit", unit(&component.unit)),
                ("step_bound", number_json(&component.step_bound)),
            ])
        })
        .collect();
    let points = report
        .points
        .iter()
        .map(|point| {
            let values = report
                .components
                .iter()
                .zip(&point.values)
                .map(|(component, value)| {
                    Json::object(vec![
                        ("name", Json::string(&component.name)),
                        ("lower", number_json(&value.lower)),
                        ("upper", number_json(&value.upper)),
                        ("interval_bound", number_json(&value.width)),
                    ])
                })
                .collect();
            Json::object(vec![
                ("time", number_json(&point.time)),
                ("values", Json::Array(values)),
            ])
        })
        .collect();
    Json::object(vec![
        ("ode", Json::string(system_text)),
        ("method", Json::string(METHOD_NAME)),
        ("order", count(report.order)),
        ("steps", count(report.steps)),
        (
            "time",
            Json::object(vec![
                ("name", Json::string(&report.time_name)),
                ("unit", unit(&report.time_unit)),
            ]),
        ),
        ("components", Json::Array(components)),
        ("points", Json::Array(points)),
    ])
}

pub fn ode_report_json(system_text: &str, report: &OdeReport) -> Vec<u8> {
    write_canonical(&report_value(system_text, report)).into_bytes()
}
