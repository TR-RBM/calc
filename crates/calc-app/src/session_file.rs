use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use calc_concepts::is_identifier;
use calc_core::{
    ComputedResult, Condition, ConditionKind, Convergence, ConvergenceStatus, Diagnostic, Method,
    ParameterValue, RecognitionTruncation, ResultKind, ResultValue, RoundingError, Seed,
    SortRecord, Uncertainty,
};
use calc_exec::{BackendKind, Domain, ExecutionMode, PlanOp, Preference};
use calc_expr::{ExprPool, NodeView};
use calc_numbers::{Integer, Number};
use calc_syntax::{Statement, parse_expression, parse_statement};
use calc_units::{UnitAccessError, UnitId};
use calc_viz::Orbit;

use crate::decimal_text::{
    integer_from_decimal, integer_to_decimal, timestamp_from_text, timestamp_to_text,
};
use crate::json::{self, Json, JsonSyntaxError};
use crate::line_picture::{picture, picture_json, reading, reading_json};
use crate::recognition::RecognitionUnavailable;
use crate::result_record::{
    BackendUse, CalculatorVersion, LineId, OperationModeUse, RecognitionLimitsUsed, Recognized,
    RecognizedBinding, RecognizedConcept, RecordRecognition, ResultRecord, SkipCause,
    SkippedBackendUse, UtcTimestamp,
};
use crate::session::{KindUnit, Line, Outcome, Precision, Settings, UnitOverride};
use crate::solve_answer::{
    self, outcome_status_from_name, outcome_status_name, sorted_identifiers,
};

const FORMAT_NAME: &str = "calc-session";
pub const FORMAT_VERSION: u64 = 22;
const PICTURE_STATUS: &str = "picture";
const DEFINED_STATUS: &str = "defined";
const ORBIT_STATUS: &str = "escape_time_reading";
const ORBIT_CLASSES: [(&str, u8); 3] = [
    ("escaped", calc_viz::ESCAPED_CELL),
    ("inside", calc_viz::INSIDE_CELL),
    ("undecided", calc_viz::UNDECIDED_CELL),
];
const VERSION_WITHOUT_SOURCES: u64 = 1;
const VERSION_WITHOUT_PICTURES: u64 = 2;
const VERSION_WITHOUT_FURTHER_INPUT_CONDITIONS: u64 = 3;
const VERSION_WITHOUT_UNIT_OVERRIDE: u64 = 4;
const VERSION_WITHOUT_AXIS_UNITS: u64 = 5;
const VERSION_WITHOUT_CONDITION_PARAMETERS: u64 = 6;
const VERSION_WITHOUT_ITERATION_LIMIT: u64 = 7;
const VERSION_WITHOUT_OPERATION_MODES: u64 = 8;
const VERSION_WITHOUT_DEFINED_OUTCOME: u64 = 9;
const VERSION_WITH_DERIVED_OPERATION_NAMES: u64 = 11;
const DEFINITION_ERROR_CODE: &str = "function_definition_not_evaluated";
const VERSION_WITHOUT_RECOGNIZED: u64 = 10;
const VERSION_WITHOUT_TRUNCATION: u64 = 12;
const VERSION_WITHOUT_INVERSE_TRIGONOMETRY: u64 = 13;
const VERSION_WITHOUT_PREPARE_FAILED: u64 = 14;
const VERSION_WITH_ONE_BACKEND: u64 = 15;
const VERSION_WITHOUT_SORT: u64 = 16;
const VERSION_WITHOUT_TALLIES: u64 = 17;
const VERSION_WITHOUT_DECREMENTS: u64 = 18;
const VERSION_WITHOUT_DRAWS: u64 = 19;
const VERSION_WITHOUT_FLIPS: u64 = 20;
const VERSION_WITH_POSITIONS_ALWAYS: u64 = 21;
const CONCEPT_SET_VERSION_DIGITS: usize = 16;
const RECOGNITION_NOT_RUN: &str = "not_run";
const RECOGNITION_RAN: &str = "ran";
const RECOGNITION_UNAVAILABLE: &str = "unavailable";
const TRUNCATED_BY_CAP: &str = "cap";
const TRUNCATED_BY_WORK_BUDGET: &str = "work_budget";
const RECOGNITION_CAPS: std::ops::RangeInclusive<u32> = 1..=100;
const RECOGNITION_WORK_BUDGETS: std::ops::RangeInclusive<u64> = 1..=10_000_000;
const LINE_LABEL_PREFIX: &str = "r";
const HEX_PREFIX: &str = "0x";
const F32_HEX_DIGITS: usize = 8;
const F64_HEX_DIGITS: usize = 16;
pub(crate) const MAXIMUM_SAFE_COUNT: u64 = (1 << 53) - 1;
pub(crate) const UNIT_VALUE_PREFIX: &str = "1 ";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathSegment {
    Member(String),
    Index(usize),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JsonPath(pub Vec<PathSegment>);

impl JsonPath {
    pub(crate) fn member(&self, name: &str) -> Self {
        let mut segments = self.0.clone();
        segments.push(PathSegment::Member(name.to_string()));
        Self(segments)
    }

    pub(crate) fn index(&self, position: usize) -> Self {
        let mut segments = self.0.clone();
        segments.push(PathSegment::Index(position));
        Self(segments)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    Syntax(JsonSyntaxError),
    DuplicateMember(JsonPath),
    MissingMember(JsonPath),
    UnknownMember(JsonPath),
    InvalidValue(JsonPath),
    DecimalDoesNotMatchBits(JsonPath),
    UnsupportedVersion { found: u64, supported: u64 },
    DuplicateLineId(JsonPath),
    LineNumberNotBelowNext(JsonPath),
    DuplicateName(JsonPath),
    NameDoesNotMatchInput(JsonPath),
    MissingDependency(JsonPath),
    DependencyCycle(LineId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveError {
    CountTooLarge(LineId),
    UnitNotPrintable(LineId),
}

pub(crate) struct SessionData {
    pub settings: Settings,
    pub unit_override: Option<UnitOverride>,
    pub next_line_number: u64,
    pub lines: Vec<Line>,
}

pub(crate) fn line_label(line: LineId) -> String {
    format!("{LINE_LABEL_PREFIX}{}", line.number())
}

pub(crate) fn line_from_label(text: &str) -> Option<LineId> {
    let digits = text.strip_prefix(LINE_LABEL_PREFIX)?;
    if digits.is_empty()
        || digits.starts_with('0')
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    LineId::from_number(digits.parse().ok()?)
}

fn precision_name(precision: Precision) -> &'static str {
    match precision {
        Precision::Exact => "exact",
        Precision::F32 => "f32",
        Precision::F64 => "f64",
    }
}

fn backend_name(kind: BackendKind) -> &'static str {
    match kind {
        BackendKind::Cpu => "cpu",
        BackendKind::Simd => "simd",
        BackendKind::Gpu => "gpu",
    }
}

fn preference_name(preference: Preference) -> &'static str {
    match preference {
        Preference::Automatic => "automatic",
        Preference::Only(kind) => backend_name(kind),
    }
}

pub(crate) fn kind_name(kind: ResultKind) -> &'static str {
    match kind {
        ResultKind::ExactRational => "exact_rational",
        ResultKind::Algebraic => "algebraic",
        ResultKind::Symbolic => "symbolic",
        ResultKind::MachineFloat => "machine_float",
    }
}

fn count_json(count: usize, line: LineId) -> Result<Json, SaveError> {
    u64::try_from(count)
        .ok()
        .filter(|count| *count <= MAXIMUM_SAFE_COUNT)
        .map(Json::Count)
        .ok_or(SaveError::CountTooLarge(line))
}

fn sort_json(sort: Option<&SortRecord>, line: LineId) -> Result<Json, SaveError> {
    let Some(sort) = sort else {
        return Ok(Json::Null);
    };
    let count = |value: u64| {
        Some(value)
            .filter(|value| *value <= MAXIMUM_SAFE_COUNT)
            .map(Json::Count)
            .ok_or(SaveError::CountTooLarge(line))
    };
    let positions = match &sort.from_positions {
        None => Json::Null,
        Some(positions) => Json::Array(
            positions
                .iter()
                .map(|position| count(*position))
                .collect::<Result<Vec<_>, _>>()?,
        ),
    };
    let optional_count = |value: Option<u64>| match value {
        None => Ok(Json::Null),
        Some(value) => count(value),
    };
    Ok(Json::object(vec![
        ("from_positions", positions),
        ("comparisons", count(sort.comparisons)?),
        ("writes", count(sort.writes)?),
        ("key_evaluations", optional_count(sort.key_evaluations)?),
        ("tallies", optional_count(sort.tallies)?),
        ("key_range", optional_count(sort.key_range)?),
        ("draws", optional_count(sort.draws)?),
        ("flips", optional_count(sort.flips)?),
    ]))
}

fn sort_record(json: &Json, path: &JsonPath) -> Result<SortRecord, LoadError> {
    let found = members(
        json,
        path,
        &[
            "from_positions",
            "comparisons",
            "writes",
            "key_evaluations",
            "tallies",
            "key_range",
            "draws",
            "flips",
        ],
    )?;
    let positions_path = path.member("from_positions");
    let from_positions = match optional(found[0]) {
        None => None,
        Some(positions) => Some(
            array(positions, &positions_path)?
                .iter()
                .enumerate()
                .map(|(index, position)| count(position, &positions_path.index(index)))
                .collect::<Result<Vec<_>, _>>()?,
        ),
    };
    let optional_count = |index: usize, name: &str| match optional(found[index]) {
        None => Ok(None),
        Some(value) => count(value, &path.member(name)).map(Some),
    };
    Ok(SortRecord {
        from_positions,
        comparisons: count(found[1], &path.member("comparisons"))?,
        writes: count(found[2], &path.member("writes"))?,
        key_evaluations: optional_count(3, "key_evaluations")?,
        tallies: optional_count(4, "tallies")?,
        key_range: optional_count(5, "key_range")?,
        draws: optional_count(6, "draws")?,
        flips: optional_count(7, "flips")?,
    })
}

fn f64_decimal(value: f64) -> String {
    if value.is_nan() {
        "nan".to_string()
    } else {
        format!("{value:e}")
    }
}

fn f32_decimal(value: f32) -> String {
    if value.is_nan() {
        "nan".to_string()
    } else {
        format!("{value:e}")
    }
}

pub(crate) fn number_json(number: &Number) -> Json {
    match number {
        Number::Integer(integer) => Json::object(vec![
            ("type", Json::string("integer")),
            ("digits", Json::String(integer_to_decimal(integer))),
        ]),
        Number::Rational(rational) => Json::object(vec![
            ("type", Json::string("rational")),
            (
                "numerator",
                Json::String(integer_to_decimal(rational.numerator())),
            ),
            (
                "denominator",
                Json::String(integer_to_decimal(rational.denominator())),
            ),
        ]),
        Number::F32(value) => Json::object(vec![
            ("type", Json::string("f32")),
            (
                "bits",
                Json::String(format!("{HEX_PREFIX}{:08x}", value.to_bits())),
            ),
            ("decimal", Json::String(f32_decimal(*value))),
        ]),
        Number::F64(value) => Json::object(vec![
            ("type", Json::string("f64")),
            (
                "bits",
                Json::String(format!("{HEX_PREFIX}{:016x}", value.to_bits())),
            ),
            ("decimal", Json::String(f64_decimal(*value))),
        ]),
    }
}

pub(crate) fn value_json(value: &ResultValue, line: LineId) -> Result<Json, SaveError> {
    Ok(match value {
        ResultValue::Number(number) => number_json(number),
        ResultValue::Expression(text) => Json::object(vec![
            ("type", Json::string("expression")),
            ("text", Json::string(text)),
        ]),
        ResultValue::Complex { real, imaginary } => Json::object(vec![
            ("type", Json::string("complex")),
            ("re", number_json(real)),
            ("im", number_json(imaginary)),
        ]),
        ResultValue::Array { shape, elements } => Json::object(vec![
            ("type", Json::string("array")),
            (
                "shape",
                Json::Array(
                    shape
                        .iter()
                        .map(|length| count_json(*length, line))
                        .collect::<Result<_, _>>()?,
                ),
            ),
            (
                "elements",
                Json::Array(
                    elements
                        .iter()
                        .map(|element| value_json(element, line))
                        .collect::<Result<_, _>>()?,
                ),
            ),
        ]),
    })
}

fn parameter_json(parameter: &ParameterValue, line: LineId) -> Result<Json, SaveError> {
    Ok(match parameter {
        ParameterValue::Value(value) => value_json(value, line)?,
        ParameterValue::Identifier(identifier) => Json::string(identifier),
        ParameterValue::Boolean(flag) => Json::Boolean(*flag),
    })
}

fn parameters_json(
    parameters: &BTreeMap<String, ParameterValue>,
    line: LineId,
) -> Result<Json, SaveError> {
    Ok(Json::Object(
        parameters
            .iter()
            .map(|(name, parameter)| Ok((name.clone(), parameter_json(parameter, line)?)))
            .collect::<Result<_, SaveError>>()?,
    ))
}

pub(crate) fn diagnostic_json(diagnostic: &Diagnostic, line: LineId) -> Result<Json, SaveError> {
    Ok(Json::object(vec![
        ("code", Json::string(&diagnostic.code)),
        ("data", parameters_json(&diagnostic.data, line)?),
    ]))
}

pub(crate) fn unit_text(pool: &ExprPool, unit: UnitId) -> Result<Option<String>, UnitAccessError> {
    let table = pool.units();
    let factors = table.factors(unit)?;
    if factors.is_empty() {
        return Ok(None);
    }
    let mut parts = Vec::new();
    for factor in factors {
        let symbol = table.symbol(factor.named_unit())?;
        parts.push(match factor.exponent() {
            1 => symbol.to_string(),
            exponent => format!("{symbol}^{exponent}"),
        });
    }
    Ok(Some(parts.join("*")))
}

fn unit_json(pool: &ExprPool, unit: Option<UnitId>, line: LineId) -> Result<Json, SaveError> {
    let Some(unit) = unit else {
        return Ok(Json::Null);
    };
    let text = unit_text(pool, unit).map_err(|_| SaveError::UnitNotPrintable(line))?;
    Ok(Json::optional(text.map(Json::String)))
}

fn method_json(method: &Method, line: LineId) -> Result<Json, SaveError> {
    let convergence = match &method.convergence {
        None => Json::Null,
        Some(convergence) => Json::object(vec![
            (
                "status",
                Json::string(match convergence.status {
                    ConvergenceStatus::Converged => "converged",
                    ConvergenceStatus::NotConverged => "not_converged",
                    ConvergenceStatus::IterationLimit => "iteration_limit",
                }),
            ),
            (
                "iterations",
                Json::Count(
                    Some(convergence.iterations)
                        .filter(|count| *count <= MAXIMUM_SAFE_COUNT)
                        .ok_or(SaveError::CountTooLarge(line))?,
                ),
            ),
            (
                "error_estimate",
                match &convergence.error_estimate {
                    Some(estimate) => value_json(estimate, line)?,
                    None => Json::Null,
                },
            ),
        ]),
    };
    let condition = match &method.condition {
        None => Json::Null,
        Some(condition) => Json::object(vec![
            ("number", value_json(&condition.number, line)?),
            (
                "kind",
                Json::string(match condition.kind {
                    ConditionKind::Absolute => "absolute",
                    ConditionKind::Relative => "relative",
                }),
            ),
        ]),
    };
    Ok(Json::object(vec![
        ("name", Json::string(&method.name)),
        ("parameters", parameters_json(&method.parameters, line)?),
        ("convergence", convergence),
        ("condition", condition),
        (
            "notes",
            Json::Array(
                method
                    .notes
                    .iter()
                    .map(|note| diagnostic_json(note, line))
                    .collect::<Result<_, _>>()?,
            ),
        ),
    ]))
}

fn backend_list_json(uses: &[BackendUse]) -> Result<Json, SaveError> {
    Ok(Json::Array(
        uses.iter()
            .map(backend_json)
            .collect::<Result<Vec<Json>, SaveError>>()?,
    ))
}

fn domain_name(width: Domain) -> &'static str {
    match width {
        Domain::F32 => "f32",
        Domain::F64 => "f64",
    }
}

fn backend_json(backend: &BackendUse) -> Result<Json, SaveError> {
    let skipped = backend
        .skipped
        .iter()
        .map(|skipped| {
            let reason = match skipped.cause {
                SkipCause::UnsupportedDomain => "unsupported_domain",
                SkipCause::UnsupportedOperation => "unsupported_operation",
                SkipCause::BatchTooLong => "batch_too_long",
                SkipCause::PrepareFailed => "prepare_failed",
            };
            Ok(Json::object(vec![
                ("backend", Json::string(backend_name(skipped.backend))),
                ("reason", Json::string(reason)),
            ]))
        })
        .collect::<Result<_, _>>()?;
    Ok(Json::object(vec![
        (
            "preference",
            Json::string(preference_name(backend.preference)),
        ),
        ("width", Json::string(domain_name(backend.width))),
        (
            "selected",
            Json::string(backend.selected.map_or("none", backend_name)),
        ),
        ("skipped", Json::Array(skipped)),
        (
            "approximate_operations",
            Json::Boolean(backend.approximate_operations),
        ),
        (
            "modes",
            Json::Array(
                backend
                    .modes
                    .iter()
                    .map(|entry| {
                        Json::object(vec![
                            ("operation", Json::string(operation_name(entry.operation))),
                            ("mode", Json::string(execution_mode_name(entry.mode))),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]))
}

pub(crate) fn record_json(
    pool: &ExprPool,
    record: &ResultRecord,
    line: LineId,
) -> Result<Json, SaveError> {
    let computed = record.computed();
    let rounding_error = match computed.rounding_error() {
        RoundingError::None => Json::object(vec![("status", Json::string("none"))]),
        RoundingError::Unknown => Json::object(vec![("status", Json::string("unknown"))]),
        RoundingError::Bound(bound) => Json::object(vec![
            ("status", Json::string("bound")),
            ("bound", value_json(bound, line)?),
        ]),
    };
    let uncertainty = match computed.uncertainty() {
        None => Json::Null,
        Some(uncertainty) => Json::object(vec![
            ("standard", value_json(uncertainty.standard(), line)?),
            (
                "coverage_factor",
                number_json(uncertainty.coverage_factor()),
            ),
        ]),
    };
    let seed = match computed.seed() {
        None => Json::Null,
        Some(seed) => Json::object(vec![
            ("value", Json::String(seed.value.to_string())),
            ("generator", Json::string(&seed.generator)),
        ]),
    };
    let duration = u64::try_from(record.duration().as_nanos())
        .ok()
        .filter(|nanoseconds| *nanoseconds <= MAXIMUM_SAFE_COUNT)
        .ok_or(SaveError::CountTooLarge(line))?;
    let version = record.produced_by();
    Ok(Json::object(vec![
        ("kind", Json::string(kind_name(computed.kind()))),
        ("value", value_json(computed.value(), line)?),
        ("unit", unit_json(pool, computed.unit(), line)?),
        ("rounding_error", rounding_error),
        ("uncertainty", uncertainty),
        ("method", method_json(computed.method(), line)?),
        (
            "corpus_references",
            Json::Array(computed.corpus_references().map(Json::string).collect()),
        ),
        (
            "sources",
            Json::Array(record.sources().map(Json::string).collect()),
        ),
        ("recognized", recognized_json(record.recognition())),
        ("seed", seed),
        ("sort", sort_json(computed.sort(), line)?),
        ("backend", backend_list_json(record.backend())?),
        (
            "dependencies",
            Json::Array(
                record
                    .dependencies()
                    .iter()
                    .map(|dependency| Json::String(line_label(*dependency)))
                    .collect(),
            ),
        ),
        (
            "computed_at",
            Json::String(timestamp_to_text(
                record.computed_at().milliseconds_since_unix_epoch(),
            )),
        ),
        ("duration_ns", Json::Count(duration)),
        (
            "produced_by",
            Json::String(format!(
                "{}.{}.{}",
                version.major, version.minor, version.patch
            )),
        ),
    ]))
}

fn recognized_json(recognition: &RecordRecognition) -> Json {
    let recognized = match recognition {
        RecordRecognition::NotRun => {
            return Json::object(vec![("state", Json::string(RECOGNITION_NOT_RUN))]);
        }
        RecordRecognition::Unavailable(reason) => {
            return Json::object(vec![
                ("state", Json::string(RECOGNITION_UNAVAILABLE)),
                ("reason", Json::string(unavailable_name(*reason))),
            ]);
        }
        RecordRecognition::Ran(recognized) => recognized,
    };
    Json::object(vec![
        ("state", Json::string(RECOGNITION_RAN)),
        (
            "concept_set_version",
            Json::string(&recognized.concept_set_version),
        ),
        (
            "limits",
            match recognized.limits {
                None => Json::Null,
                Some(limits) => Json::object(vec![
                    ("cap", Json::Count(u64::from(limits.cap))),
                    ("work_budget", Json::Count(limits.work_budget)),
                ]),
            },
        ),
        (
            "truncated",
            match recognized.truncated {
                None => Json::Null,
                Some(truncated) => Json::Boolean(truncated),
            },
        ),
        (
            "truncated_by",
            match recognized.truncated_by {
                None => Json::Null,
                Some(RecognitionTruncation::Cap) => Json::string(TRUNCATED_BY_CAP),
                Some(RecognitionTruncation::WorkBudget) => Json::string(TRUNCATED_BY_WORK_BUDGET),
            },
        ),
        (
            "matches",
            Json::Array(
                recognized
                    .matches
                    .iter()
                    .map(|found| {
                        Json::object(vec![
                            ("concept", Json::string(&found.concept)),
                            ("pattern", Json::string(&found.pattern)),
                            ("coverage", number_json(&found.coverage)),
                            ("site", Json::string(&found.site)),
                            (
                                "bindings",
                                Json::Array(
                                    found
                                        .bindings
                                        .iter()
                                        .map(|binding| {
                                            Json::object(vec![
                                                ("variable", Json::string(&binding.variable)),
                                                ("expression", Json::string(&binding.expression)),
                                            ])
                                        })
                                        .collect(),
                                ),
                            ),
                            (
                                "holds",
                                Json::Array(
                                    found
                                        .holds
                                        .iter()
                                        .map(|holds| match holds {
                                            Some(holds) => Json::Boolean(*holds),
                                            None => Json::Null,
                                        })
                                        .collect(),
                                ),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

const fn unavailable_name(reason: RecognitionUnavailable) -> &'static str {
    match reason {
        RecognitionUnavailable::ConceptSetNotLoaded => "concept_set_not_loaded",
        RecognitionUnavailable::PatternsNotBuilt => "patterns_not_built",
        RecognitionUnavailable::MatcherFailed => "matcher_failed",
    }
}

fn orbit_json(orbit: Orbit) -> Json {
    let class = ORBIT_CLASSES
        .iter()
        .find(|(_, class)| *class == orbit.class)
        .map_or("undecided", |(name, _)| *name);
    Json::object(vec![
        ("class", Json::string(class)),
        (
            "count",
            Json::optional(orbit.count.map(|count| Json::Count(u64::from(count)))),
        ),
        ("limit", Json::Count(u64::from(orbit.limit))),
    ])
}

fn orbit(json: &Json, path: &JsonPath) -> Result<Orbit, LoadError> {
    let found = members(json, path, &["class", "count", "limit"])?;
    let class_path = path.member("class");
    let class = one_of(found[0], &class_path, &ORBIT_CLASSES)?;
    let count_path = path.member("count");
    let escapes = optional(found[1])
        .map(|escapes| {
            u32::try_from(count(escapes, &count_path)?)
                .map_err(|_| LoadError::InvalidValue(count_path.clone()))
        })
        .transpose()?;
    if (class == calc_viz::ESCAPED_CELL) != escapes.is_some() {
        return Err(LoadError::InvalidValue(count_path));
    }
    let limit_path = path.member("limit");
    let limit = u32::try_from(count(found[2], &limit_path)?)
        .ok()
        .filter(|limit| *limit > 0)
        .ok_or(LoadError::InvalidValue(limit_path))?;
    Ok(Orbit {
        class,
        count: escapes,
        limit,
    })
}

fn line_json(pool: &ExprPool, line: &Line) -> Result<Json, SaveError> {
    let outcome = match line.outcome() {
        Outcome::Result(record) => Json::object(vec![
            ("status", Json::string("result")),
            ("record", record_json(pool, record, line.id())?),
        ]),
        Outcome::Error(error) => Json::object(vec![
            ("status", Json::string("error")),
            ("error", diagnostic_json(error, line.id())?),
        ]),
        Outcome::NotEvaluated | Outcome::Reachable(_) => {
            Json::object(vec![("status", Json::string("not_evaluated"))])
        }
        Outcome::Defined => Json::object(vec![("status", Json::string(DEFINED_STATUS))]),
        Outcome::Picture => Json::object(vec![("status", Json::string(PICTURE_STATUS))]),
        Outcome::EscapeTimeReading(orbit) => Json::object(vec![
            ("status", Json::string(ORBIT_STATUS)),
            ("orbit", orbit_json(*orbit)),
        ]),
        Outcome::Answer(answer) => Json::object(vec![
            (
                "status",
                Json::string(outcome_status_name(answer.outcome_status())),
            ),
            (
                "answer",
                solve_answer::answer_json(pool, answer, line.id())?,
            ),
        ]),
    };
    Ok(Json::object(vec![
        ("id", Json::String(line_label(line.id()))),
        ("name", Json::optional(line.name().map(Json::string))),
        ("input", Json::string(line.input())),
        ("outcome", outcome),
        ("picture", picture_json(line.picture())),
        ("reading", reading_json(line.reading())),
    ]))
}

pub(crate) fn encode_record(
    pool: &ExprPool,
    record: &ResultRecord,
    line: LineId,
) -> Result<Vec<u8>, SaveError> {
    Ok(json::write_canonical(&record_json(pool, record, line)?).into_bytes())
}

pub(crate) fn encode_orbit(orbit: Orbit) -> Vec<u8> {
    json::write_canonical(&orbit_json(orbit)).into_bytes()
}

pub(crate) fn encode_line(pool: &ExprPool, line: &Line) -> Result<Vec<u8>, SaveError> {
    Ok(json::write_canonical(&line_json(pool, line)?).into_bytes())
}

pub(crate) fn encode_line_compact(pool: &ExprPool, line: &Line) -> Result<Vec<u8>, SaveError> {
    Ok(json::write_one_line(&line_json(pool, line)?).into_bytes())
}

pub(crate) fn encode(
    pool: &ExprPool,
    settings: &Settings,
    unit_override: Option<&UnitOverride>,
    next_line_number: u64,
    lines: &[Line],
) -> Result<Vec<u8>, SaveError> {
    let lines_json = lines
        .iter()
        .map(|line| line_json(pool, line))
        .collect::<Result<_, _>>()?;
    let session = Json::object(vec![
        ("format", Json::string(FORMAT_NAME)),
        ("version", Json::Count(FORMAT_VERSION)),
        (
            "settings",
            Json::object(vec![
                (
                    "precision",
                    Json::string(precision_name(settings.precision)),
                ),
                ("backend", Json::string(preference_name(settings.backend))),
                ("units", unit_override_json(unit_override)),
            ]),
        ),
        ("next_line_number", Json::Count(next_line_number)),
        ("lines", Json::Array(lines_json)),
    ]);
    Ok(json::write_canonical(&session).into_bytes())
}

pub(crate) fn members<'json>(
    value: &'json Json,
    path: &JsonPath,
    names: &[&str],
) -> Result<Vec<&'json Json>, LoadError> {
    let Json::Object(object) = value else {
        return Err(LoadError::InvalidValue(path.clone()));
    };
    let mut found: HashMap<&str, &Json> = HashMap::new();
    for (name, member) in object {
        if !names.contains(&name.as_str()) {
            return Err(LoadError::UnknownMember(path.member(name)));
        }
        if found.insert(name.as_str(), member).is_some() {
            return Err(LoadError::DuplicateMember(path.member(name)));
        }
    }
    names
        .iter()
        .map(|name| {
            found
                .get(name)
                .copied()
                .ok_or_else(|| LoadError::MissingMember(path.member(name)))
        })
        .collect()
}

pub(crate) fn string<'json>(value: &'json Json, path: &JsonPath) -> Result<&'json str, LoadError> {
    match value {
        Json::String(text) => Ok(text),
        _ => Err(LoadError::InvalidValue(path.clone())),
    }
}

pub(crate) fn identifier<'json>(
    value: &'json Json,
    path: &JsonPath,
) -> Result<&'json str, LoadError> {
    string(value, path).and_then(|text| {
        if text.is_empty() {
            Err(LoadError::InvalidValue(path.clone()))
        } else {
            Ok(text)
        }
    })
}

pub(crate) fn count(value: &Json, path: &JsonPath) -> Result<u64, LoadError> {
    match value {
        Json::Count(count) => Ok(*count),
        _ => Err(LoadError::InvalidValue(path.clone())),
    }
}

pub(crate) fn boolean(value: &Json, path: &JsonPath) -> Result<bool, LoadError> {
    match value {
        Json::Boolean(flag) => Ok(*flag),
        _ => Err(LoadError::InvalidValue(path.clone())),
    }
}

pub(crate) fn array<'json>(
    value: &'json Json,
    path: &JsonPath,
) -> Result<&'json [Json], LoadError> {
    match value {
        Json::Array(elements) => Ok(elements),
        _ => Err(LoadError::InvalidValue(path.clone())),
    }
}

pub(crate) fn one_of<T: Copy>(
    value: &Json,
    path: &JsonPath,
    choices: &[(&str, T)],
) -> Result<T, LoadError> {
    let text = string(value, path)?;
    choices
        .iter()
        .find(|(name, _)| *name == text)
        .map(|(_, choice)| *choice)
        .ok_or_else(|| LoadError::InvalidValue(path.clone()))
}

fn backend_kind(value: &Json, path: &JsonPath) -> Result<BackendKind, LoadError> {
    one_of(
        value,
        path,
        &[
            ("cpu", BackendKind::Cpu),
            ("simd", BackendKind::Simd),
            ("gpu", BackendKind::Gpu),
        ],
    )
}

fn preference(value: &Json, path: &JsonPath) -> Result<Preference, LoadError> {
    one_of(
        value,
        path,
        &[
            ("automatic", Preference::Automatic),
            ("cpu", Preference::Only(BackendKind::Cpu)),
            ("simd", Preference::Only(BackendKind::Simd)),
            ("gpu", Preference::Only(BackendKind::Gpu)),
        ],
    )
}

fn decimal_integer(value: &Json, path: &JsonPath) -> Result<Integer, LoadError> {
    integer_from_decimal(string(value, path)?).ok_or_else(|| LoadError::InvalidValue(path.clone()))
}

fn bits(value: &Json, path: &JsonPath, hex_digits: usize) -> Result<u64, LoadError> {
    let text = string(value, path)?;
    text.strip_prefix(HEX_PREFIX)
        .filter(|digits| {
            digits.len() == hex_digits
                && digits
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .and_then(|digits| u64::from_str_radix(digits, 16).ok())
        .ok_or_else(|| LoadError::InvalidValue(path.clone()))
}

pub(crate) fn number(value: &Json, path: &JsonPath) -> Result<Number, LoadError> {
    let Json::Object(object) = value else {
        return Err(LoadError::InvalidValue(path.clone()));
    };
    let type_path = path.member("type");
    let type_name = object
        .iter()
        .find(|(name, _)| name == "type")
        .map(|(_, member)| string(member, &type_path))
        .transpose()?
        .ok_or_else(|| LoadError::MissingMember(type_path.clone()))?;
    match type_name {
        "integer" => {
            let found = members(value, path, &["type", "digits"])?;
            Ok(Number::Integer(decimal_integer(
                found[1],
                &path.member("digits"),
            )?))
        }
        "rational" => {
            let found = members(value, path, &["type", "numerator", "denominator"])?;
            let numerator = decimal_integer(found[1], &path.member("numerator"))?;
            let denominator = decimal_integer(found[2], &path.member("denominator"))?;
            match Number::fraction(&numerator, &denominator) {
                Ok(Number::Rational(rational))
                    if rational.numerator() == &numerator
                        && rational.denominator() == &denominator =>
                {
                    Ok(Number::Rational(rational))
                }
                _ => Err(LoadError::InvalidValue(path.member("denominator"))),
            }
        }
        "f32" => {
            let found = members(value, path, &["type", "bits", "decimal"])?;
            let bits = u32::try_from(bits(found[1], &path.member("bits"), F32_HEX_DIGITS)?)
                .map_err(|_| LoadError::InvalidValue(path.member("bits")))?;
            let machine = f32::from_bits(bits);
            if string(found[2], &path.member("decimal"))? != f32_decimal(machine) {
                return Err(LoadError::DecimalDoesNotMatchBits(path.member("decimal")));
            }
            Ok(Number::F32(machine))
        }
        "f64" => {
            let found = members(value, path, &["type", "bits", "decimal"])?;
            let machine = f64::from_bits(bits(found[1], &path.member("bits"), F64_HEX_DIGITS)?);
            if string(found[2], &path.member("decimal"))? != f64_decimal(machine) {
                return Err(LoadError::DecimalDoesNotMatchBits(path.member("decimal")));
            }
            Ok(Number::F64(machine))
        }
        _ => Err(LoadError::InvalidValue(type_path)),
    }
}

fn is_same_numeric_type(left: &Number, right: &Number) -> bool {
    match (left, right) {
        (Number::F32(_), Number::F32(_)) | (Number::F64(_), Number::F64(_)) => true,
        (Number::F32(_) | Number::F64(_), _) | (_, Number::F32(_) | Number::F64(_)) => false,
        _ => true,
    }
}

pub(crate) fn value(json: &Json, path: &JsonPath) -> Result<ResultValue, LoadError> {
    let type_path = path.member("type");
    let type_name = match json {
        Json::Object(object) => object
            .iter()
            .find(|(name, _)| name == "type")
            .map(|(_, member)| string(member, &type_path))
            .transpose()?,
        _ => return Err(LoadError::InvalidValue(path.clone())),
    };
    match type_name {
        Some("complex") => {
            let found = members(json, path, &["type", "re", "im"])?;
            let real = number(found[1], &path.member("re"))?;
            let imaginary = number(found[2], &path.member("im"))?;
            if !is_same_numeric_type(&real, &imaginary) {
                return Err(LoadError::InvalidValue(path.member("im")));
            }
            Ok(ResultValue::Complex { real, imaginary })
        }
        Some("array") => {
            let found = members(json, path, &["type", "shape", "elements"])?;
            let shape_path = path.member("shape");
            let shape = array(found[1], &shape_path)?
                .iter()
                .enumerate()
                .map(|(position, length)| {
                    let length_path = shape_path.index(position);
                    usize::try_from(count(length, &length_path)?)
                        .map_err(|_| LoadError::InvalidValue(length_path))
                })
                .collect::<Result<_, _>>()?;
            let elements_path = path.member("elements");
            let elements = array(found[2], &elements_path)?
                .iter()
                .enumerate()
                .map(|(position, element)| value(element, &elements_path.index(position)))
                .collect::<Result<_, _>>()?;
            Ok(ResultValue::Array { shape, elements })
        }
        Some("expression") => {
            let found = members(json, path, &["type", "text"])?;
            let text_path = path.member("text");
            let text = string(found[1], &text_path)?;
            if parse_expression(&mut ExprPool::new(), text).is_err() {
                return Err(LoadError::InvalidValue(text_path));
            }
            Ok(ResultValue::Expression(text.to_owned()))
        }
        _ => number(json, path).map(ResultValue::Number),
    }
}

fn parameter_map(
    json: &Json,
    path: &JsonPath,
    allows_boolean: bool,
) -> Result<BTreeMap<String, ParameterValue>, LoadError> {
    let Json::Object(object) = json else {
        return Err(LoadError::InvalidValue(path.clone()));
    };
    let mut parameters = BTreeMap::new();
    for (name, member) in object {
        let member_path = path.member(name);
        let parameter = match member {
            Json::String(_) => {
                ParameterValue::Identifier(identifier(member, &member_path)?.to_string())
            }
            Json::Boolean(flag) if allows_boolean => ParameterValue::Boolean(*flag),
            Json::Object(_) => ParameterValue::Value(value(member, &member_path)?),
            _ => return Err(LoadError::InvalidValue(member_path)),
        };
        if parameters.insert(name.clone(), parameter).is_some() {
            return Err(LoadError::DuplicateMember(member_path));
        }
    }
    Ok(parameters)
}

fn diagnostic(json: &Json, path: &JsonPath) -> Result<Diagnostic, LoadError> {
    let found = members(json, path, &["code", "data"])?;
    Ok(Diagnostic {
        code: identifier(found[0], &path.member("code"))?.to_string(),
        data: parameter_map(found[1], &path.member("data"), false)?,
    })
}

pub(crate) fn optional(json: &Json) -> Option<&Json> {
    match json {
        Json::Null => None,
        other => Some(other),
    }
}

fn method(json: &Json, path: &JsonPath) -> Result<Method, LoadError> {
    let found = members(
        json,
        path,
        &["name", "parameters", "convergence", "condition", "notes"],
    )?;
    let convergence = optional(found[2])
        .map(|convergence| {
            let convergence_path = path.member("convergence");
            let parts = members(
                convergence,
                &convergence_path,
                &["status", "iterations", "error_estimate"],
            )?;
            Ok::<_, LoadError>(Convergence {
                status: one_of(
                    parts[0],
                    &convergence_path.member("status"),
                    &[
                        ("converged", ConvergenceStatus::Converged),
                        ("not_converged", ConvergenceStatus::NotConverged),
                        ("iteration_limit", ConvergenceStatus::IterationLimit),
                    ],
                )?,
                iterations: count(parts[1], &convergence_path.member("iterations"))?,
                error_estimate: optional(parts[2])
                    .map(|estimate| value(estimate, &convergence_path.member("error_estimate")))
                    .transpose()?,
            })
        })
        .transpose()?;
    let condition = optional(found[3])
        .map(|condition| {
            let condition_path = path.member("condition");
            let parts = members(condition, &condition_path, &["number", "kind"])?;
            Ok::<_, LoadError>(Condition {
                number: value(parts[0], &condition_path.member("number"))?,
                kind: one_of(
                    parts[1],
                    &condition_path.member("kind"),
                    &[
                        ("absolute", ConditionKind::Absolute),
                        ("relative", ConditionKind::Relative),
                    ],
                )?,
            })
        })
        .transpose()?;
    let notes_path = path.member("notes");
    let notes = array(found[4], &notes_path)?
        .iter()
        .enumerate()
        .map(|(position, note)| diagnostic(note, &notes_path.index(position)))
        .collect::<Result<_, _>>()?;
    let mut method = Method::named(identifier(found[0], &path.member("name"))?);
    method.parameters = parameter_map(found[1], &path.member("parameters"), true)?;
    method.convergence = convergence;
    method.condition = condition;
    method.notes = notes;
    Ok(method)
}

pub(crate) fn unit(
    pool: &mut ExprPool,
    json: &Json,
    path: &JsonPath,
) -> Result<Option<UnitId>, LoadError> {
    let Some(text) = optional(json) else {
        return Ok(None);
    };
    let text = string(text, path)?;
    let invalid = || LoadError::InvalidValue(path.clone());
    if text.is_empty() || text.contains(char::is_whitespace) {
        return Err(invalid());
    }
    let expression =
        parse_expression(pool, &format!("{UNIT_VALUE_PREFIX}{text}")).map_err(|_| invalid())?;
    match pool.node(expression).map_err(|_| invalid())? {
        NodeView::Quantity { unit, .. } => Ok(Some(unit)),
        _ => Err(invalid()),
    }
}

fn backend_use(json: &Json, path: &JsonPath) -> Result<BackendUse, LoadError> {
    let found = members(
        json,
        path,
        &[
            "preference",
            "width",
            "selected",
            "skipped",
            "approximate_operations",
            "modes",
        ],
    )?;
    let width = one_of(
        found[1],
        &path.member("width"),
        &[("f32", Domain::F32), ("f64", Domain::F64)],
    )?;
    let selected_path = path.member("selected");
    let selected = match string(found[2], &selected_path)? {
        "none" => None,
        _ => Some(backend_kind(found[2], &selected_path)?),
    };
    let skipped_path = path.member("skipped");
    let skipped = array(found[3], &skipped_path)?
        .iter()
        .enumerate()
        .map(|(position, entry)| {
            let entry_path = skipped_path.index(position);
            let parts = members(entry, &entry_path, &["backend", "reason"])?;
            Ok(SkippedBackendUse {
                backend: backend_kind(parts[0], &entry_path.member("backend"))?,
                cause: one_of(
                    parts[1],
                    &entry_path.member("reason"),
                    &[
                        ("unsupported_domain", SkipCause::UnsupportedDomain),
                        ("unsupported_operation", SkipCause::UnsupportedOperation),
                        ("batch_too_long", SkipCause::BatchTooLong),
                        ("prepare_failed", SkipCause::PrepareFailed),
                    ],
                )?,
            })
        })
        .collect::<Result<_, LoadError>>()?;
    let modes_path = path.member("modes");
    let modes = array(found[5], &modes_path)?
        .iter()
        .enumerate()
        .map(|(position, entry)| {
            let entry_path = modes_path.index(position);
            let parts = members(entry, &entry_path, &["operation", "mode"])?;
            Ok(OperationModeUse {
                operation: operation(parts[0], &entry_path.member("operation"))?,
                mode: one_of(
                    parts[1],
                    &entry_path.member("mode"),
                    &[
                        ("native", ExecutionMode::Native),
                        ("integer_exact", ExecutionMode::IntegerExact),
                        ("exact_in_both_forms", ExecutionMode::ExactInBothForms),
                    ],
                )?,
            })
        })
        .collect::<Result<Vec<OperationModeUse>, LoadError>>()?;
    let mut last: Option<usize> = None;
    for (position, entry) in modes.iter().enumerate() {
        let place = PlanOp::ALL
            .iter()
            .position(|listed| *listed == entry.operation)
            .unwrap_or(usize::MAX);
        if last.is_some_and(|previous| place <= previous) {
            return Err(LoadError::InvalidValue(
                modes_path.index(position).member("operation"),
            ));
        }
        last = Some(place);
    }
    Ok(BackendUse {
        preference: preference(found[0], &path.member("preference"))?,
        width,
        selected,
        skipped,
        approximate_operations: boolean(found[4], &path.member("approximate_operations"))?,
        modes,
    })
}

fn version(json: &Json, path: &JsonPath) -> Result<CalculatorVersion, LoadError> {
    let invalid = || LoadError::InvalidValue(path.clone());
    let parts: Vec<u32> = string(json, path)?
        .split('.')
        .map(|part| {
            let is_canonical = !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part == "0" || !part.starts_with('0'));
            is_canonical
                .then(|| part.parse().ok())
                .flatten()
                .ok_or_else(invalid)
        })
        .collect::<Result<_, _>>()?;
    match parts.as_slice() {
        [major, minor, patch] => Ok(CalculatorVersion {
            major: *major,
            minor: *minor,
            patch: *patch,
        }),
        _ => Err(invalid()),
    }
}

fn recognized(json: &Json, path: &JsonPath) -> Result<RecordRecognition, LoadError> {
    let state_path = path.member("state");
    let state = match json {
        Json::Object(members) => members
            .iter()
            .find(|(name, _)| name == "state")
            .map(|(_, value)| string(value, &state_path))
            .transpose()?
            .ok_or_else(|| LoadError::MissingMember(state_path.clone()))?,
        _ => return Err(LoadError::InvalidValue(path.clone())),
    };
    match state {
        RECOGNITION_NOT_RUN => {
            members(json, path, &["state"])?;
            Ok(RecordRecognition::NotRun)
        }
        RECOGNITION_UNAVAILABLE => {
            let found = members(json, path, &["state", "reason"])?;
            let reason = one_of(
                found[1],
                &path.member("reason"),
                &[
                    (
                        "concept_set_not_loaded",
                        RecognitionUnavailable::ConceptSetNotLoaded,
                    ),
                    (
                        "patterns_not_built",
                        RecognitionUnavailable::PatternsNotBuilt,
                    ),
                    ("matcher_failed", RecognitionUnavailable::MatcherFailed),
                ],
            )?;
            Ok(RecordRecognition::Unavailable(reason))
        }
        RECOGNITION_RAN => Ok(RecordRecognition::Ran(recognition_answer(json, path)?)),
        _ => Err(LoadError::InvalidValue(state_path)),
    }
}

fn recognition_answer(json: &Json, path: &JsonPath) -> Result<Recognized, LoadError> {
    let found = members(
        json,
        path,
        &[
            "state",
            "concept_set_version",
            "limits",
            "truncated",
            "truncated_by",
            "matches",
        ],
    )?;
    let version_path = path.member("concept_set_version");
    let concept_set_version = string(found[1], &version_path)?;
    let is_hexadecimal = concept_set_version.len() == CONCEPT_SET_VERSION_DIGITS
        && concept_set_version
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if !is_hexadecimal {
        return Err(LoadError::InvalidValue(version_path));
    }
    let limits = recognition_limits(found[2], &path.member("limits"))?;
    let truncated_path = path.member("truncated");
    let truncated = match optional(found[3]) {
        None => None,
        Some(value) => Some(boolean(value, &truncated_path)?),
    };
    let truncated_by_path = path.member("truncated_by");
    let truncated_by = match optional(found[4]) {
        None => None,
        Some(name) => Some(one_of(
            name,
            &truncated_by_path,
            &[
                (TRUNCATED_BY_CAP, RecognitionTruncation::Cap),
                (TRUNCATED_BY_WORK_BUDGET, RecognitionTruncation::WorkBudget),
            ],
        )?),
    };
    if truncated_by.is_some() != (truncated == Some(true)) {
        return Err(LoadError::InvalidValue(truncated_by_path));
    }
    let matches_path = path.member("matches");
    let mut matches = Vec::new();
    for (position, found_match) in array(found[5], &matches_path)?.iter().enumerate() {
        matches.push(recognized_concept(
            found_match,
            &matches_path.index(position),
        )?);
    }
    Ok(Recognized {
        concept_set_version: concept_set_version.to_owned(),
        limits,
        truncated,
        truncated_by,
        matches,
    })
}

fn is_coverage(coverage: &Number) -> bool {
    let one = Number::Integer(Integer::one());
    let zero = Number::Integer(Integer::zero());
    let is_above_zero = matches!(
        coverage.sub_exact(&zero),
        Ok(Number::Integer(difference)) if !difference.is_negative() && !difference.is_zero()
    ) || matches!(
        coverage.sub_exact(&zero),
        Ok(Number::Rational(difference)) if !difference.numerator().is_negative()
    );
    let is_at_most_one = matches!(
        one.sub_exact(coverage),
        Ok(Number::Integer(difference)) if !difference.is_negative()
    ) || matches!(
        one.sub_exact(coverage),
        Ok(Number::Rational(difference)) if !difference.numerator().is_negative()
    );
    is_above_zero && is_at_most_one
}

fn recognition_limits(
    json: &Json,
    path: &JsonPath,
) -> Result<Option<RecognitionLimitsUsed>, LoadError> {
    let Some(json) = optional(json) else {
        return Ok(None);
    };
    let found = members(json, path, &["cap", "work_budget"])?;
    let cap_path = path.member("cap");
    let Some(cap) = u32::try_from(count(found[0], &cap_path)?)
        .ok()
        .filter(|cap| RECOGNITION_CAPS.contains(cap))
    else {
        return Err(LoadError::InvalidValue(cap_path));
    };
    let budget_path = path.member("work_budget");
    let work_budget = count(found[1], &budget_path)?;
    if !RECOGNITION_WORK_BUDGETS.contains(&work_budget) {
        return Err(LoadError::InvalidValue(budget_path));
    }
    Ok(Some(RecognitionLimitsUsed { cap, work_budget }))
}

fn recognized_concept(json: &Json, path: &JsonPath) -> Result<RecognizedConcept, LoadError> {
    let found = members(
        json,
        path,
        &[
            "concept", "pattern", "coverage", "site", "bindings", "holds",
        ],
    )?;
    let bindings_path = path.member("bindings");
    let mut bindings = Vec::new();
    for (position, binding) in array(found[4], &bindings_path)?.iter().enumerate() {
        let binding_path = bindings_path.index(position);
        let parts = members(binding, &binding_path, &["variable", "expression"])?;
        bindings.push(RecognizedBinding {
            variable: identifier(parts[0], &binding_path.member("variable"))?.to_owned(),
            expression: string(parts[1], &binding_path.member("expression"))?.to_owned(),
        });
    }
    let holds_path = path.member("holds");
    let mut holds = Vec::new();
    for (position, held) in array(found[5], &holds_path)?.iter().enumerate() {
        let held_path = holds_path.index(position);
        holds.push(match optional(held) {
            None => None,
            Some(value) => Some(boolean(value, &held_path)?),
        });
    }
    let coverage_path = path.member("coverage");
    let coverage = number(found[2], &coverage_path)?;
    if !is_coverage(&coverage) {
        return Err(LoadError::InvalidValue(coverage_path));
    }
    Ok(RecognizedConcept {
        concept: identifier(found[0], &path.member("concept"))?.to_owned(),
        pattern: identifier(found[1], &path.member("pattern"))?.to_owned(),
        coverage,
        site: string(found[3], &path.member("site"))?.to_owned(),
        bindings,
        holds,
    })
}

pub(crate) fn record(
    pool: &mut ExprPool,
    json: &Json,
    path: &JsonPath,
) -> Result<ResultRecord, LoadError> {
    let found = members(
        json,
        path,
        &[
            "kind",
            "value",
            "unit",
            "rounding_error",
            "uncertainty",
            "method",
            "corpus_references",
            "sources",
            "recognized",
            "seed",
            "sort",
            "backend",
            "dependencies",
            "computed_at",
            "duration_ns",
            "produced_by",
        ],
    )?;
    let kind = one_of(
        found[0],
        &path.member("kind"),
        &[
            ("exact_rational", ResultKind::ExactRational),
            ("algebraic", ResultKind::Algebraic),
            ("symbolic", ResultKind::Symbolic),
            ("machine_float", ResultKind::MachineFloat),
        ],
    )?;
    let result_value = value(found[1], &path.member("value"))?;
    let result_unit = unit(pool, found[2], &path.member("unit"))?;
    let result_method = method(found[5], &path.member("method"))?;
    let mut computed = ComputedResult::new(kind, result_value, result_unit, result_method)
        .map_err(|_| LoadError::InvalidValue(path.member("value")))?;

    let rounding_path = path.member("rounding_error");
    let rounding_status = found[3];
    let status_path = rounding_path.member("status");
    let status_name = match rounding_status {
        Json::Object(object) => object
            .iter()
            .find(|(name, _)| name == "status")
            .map(|(_, member)| string(member, &status_path))
            .transpose()?
            .ok_or_else(|| LoadError::MissingMember(status_path.clone()))?,
        _ => return Err(LoadError::InvalidValue(rounding_path)),
    };
    computed = match status_name {
        "bound" => {
            let parts = members(rounding_status, &rounding_path, &["status", "bound"])?;
            let bound_path = rounding_path.member("bound");
            computed
                .with_rounding_bound(value(parts[1], &bound_path)?)
                .map_err(|_| LoadError::InvalidValue(bound_path))?
        }
        "none" | "unknown" => {
            members(rounding_status, &rounding_path, &["status"])?;
            let expected = if status_name == "none" {
                RoundingError::None
            } else {
                RoundingError::Unknown
            };
            if computed.rounding_error() != &expected {
                return Err(LoadError::InvalidValue(status_path));
            }
            computed
        }
        _ => return Err(LoadError::InvalidValue(status_path)),
    };

    if let Some(uncertainty) = optional(found[4]) {
        let uncertainty_path = path.member("uncertainty");
        let parts = members(
            uncertainty,
            &uncertainty_path,
            &["standard", "coverage_factor"],
        )?;
        let standard = value(parts[0], &uncertainty_path.member("standard"))?;
        let coverage_factor = number(parts[1], &uncertainty_path.member("coverage_factor"))?;
        let stated = Uncertainty::new(standard, coverage_factor)
            .map_err(|_| LoadError::InvalidValue(uncertainty_path))?;
        computed = computed.with_uncertainty(stated);
    }

    let references_path = path.member("corpus_references");
    let references = array(found[6], &references_path)?;
    let mut previous: Option<&str> = None;
    for (position, reference) in references.iter().enumerate() {
        let reference_path = references_path.index(position);
        let text = identifier(reference, &reference_path)?;
        if previous.is_some_and(|earlier| earlier >= text) {
            return Err(LoadError::InvalidValue(reference_path));
        }
        previous = Some(text);
        computed = computed.with_corpus_reference(text);
    }

    let sources = sorted_identifiers(found[7], &path.member("sources"))?;
    let recognition = recognized(found[8], &path.member("recognized"))?;

    if let Some(seed) = optional(found[9]) {
        let seed_path = path.member("seed");
        let parts = members(seed, &seed_path, &["value", "generator"])?;
        let value_path = seed_path.member("value");
        let seed_value = integer_from_decimal(string(parts[0], &value_path)?)
            .and_then(|integer| integer.to_i128())
            .and_then(|integer| u64::try_from(integer).ok())
            .ok_or_else(|| LoadError::InvalidValue(value_path.clone()))?;
        computed = computed.with_seed(Seed {
            value: seed_value,
            generator: identifier(parts[1], &seed_path.member("generator"))?.to_string(),
        });
    }
    if let Some(sort) = optional(found[10]) {
        computed = computed.with_sort(sort_record(sort, &path.member("sort"))?);
    }

    let backend_path = path.member("backend");
    let backend = array(found[11], &backend_path)?
        .iter()
        .enumerate()
        .map(|(position, entry)| backend_use(entry, &backend_path.index(position)))
        .collect::<Result<Vec<BackendUse>, LoadError>>()?;

    let dependencies_path = path.member("dependencies");
    let mut dependencies = Vec::new();
    for (position, dependency) in array(found[12], &dependencies_path)?.iter().enumerate() {
        let dependency_path = dependencies_path.index(position);
        let line = line_from_label(string(dependency, &dependency_path)?)
            .ok_or_else(|| LoadError::InvalidValue(dependency_path.clone()))?;
        if dependencies.contains(&line) {
            return Err(LoadError::InvalidValue(dependency_path));
        }
        dependencies.push(line);
    }

    let time_path = path.member("computed_at");
    let computed_at = timestamp_from_text(string(found[13], &time_path)?)
        .map(UtcTimestamp::from_milliseconds_since_unix_epoch)
        .ok_or(LoadError::InvalidValue(time_path))?;
    let duration = Duration::from_nanos(count(found[14], &path.member("duration_ns"))?);
    let produced_by = version(found[15], &path.member("produced_by"))?;

    let record = ResultRecord::new(
        computed,
        backend,
        dependencies,
        computed_at,
        duration,
        produced_by,
    )
    .with_sources(sources)
    .with_recognition(recognition);
    Ok(record)
}

fn outcome(pool: &mut ExprPool, json: &Json, path: &JsonPath) -> Result<Outcome, LoadError> {
    let status_path = path.member("status");
    let status = match json {
        Json::Object(object) => object
            .iter()
            .find(|(name, _)| name == "status")
            .map(|(_, member)| string(member, &status_path))
            .transpose()?
            .ok_or_else(|| LoadError::MissingMember(status_path.clone()))?,
        _ => return Err(LoadError::InvalidValue(path.clone())),
    };
    match status {
        "result" => {
            let found = members(json, path, &["status", "record"])?;
            record(pool, found[1], &path.member("record"))
                .map(|record| Outcome::Result(Box::new(record)))
        }
        "error" => {
            let found = members(json, path, &["status", "error"])?;
            diagnostic(found[1], &path.member("error")).map(Outcome::Error)
        }
        "not_evaluated" => {
            members(json, path, &["status"])?;
            Ok(Outcome::NotEvaluated)
        }
        PICTURE_STATUS => {
            members(json, path, &["status"])?;
            Ok(Outcome::Picture)
        }
        DEFINED_STATUS => {
            members(json, path, &["status"])?;
            Ok(Outcome::Defined)
        }
        ORBIT_STATUS => {
            let found = members(json, path, &["status", "orbit"])?;
            orbit(found[1], &path.member("orbit")).map(Outcome::EscapeTimeReading)
        }
        other => {
            let expected = outcome_status_from_name(other)
                .ok_or_else(|| LoadError::InvalidValue(status_path.clone()))?;
            let found = members(json, path, &["status", "answer"])?;
            let stored = solve_answer::answer(pool, found[1], &path.member("answer"))?;
            if stored.outcome_status() != expected {
                return Err(LoadError::InvalidValue(status_path));
            }
            Ok(Outcome::Answer(Box::new(stored)))
        }
    }
}

fn is_valid_name(name: &str) -> bool {
    let mut pool = ExprPool::new();
    matches!(
        parse_statement(&mut pool, &format!("{name} = 0")),
        Ok(Statement::Naming { name: symbol, .. })
            if pool.symbol_name(symbol).is_ok_and(|parsed| parsed == name)
    )
}

fn check_dependency_graph(lines: &[Line], lines_path: &JsonPath) -> Result<(), LoadError> {
    let known: HashSet<LineId> = lines.iter().map(Line::id).collect();
    let mut dependencies: HashMap<LineId, Vec<LineId>> = HashMap::new();
    for (position, line) in lines.iter().enumerate() {
        let outcome_path = lines_path.index(position).member("outcome");
        let records: Vec<(JsonPath, &ResultRecord)> = match line.outcome() {
            Outcome::Result(record) => vec![(outcome_path.member("record"), record.as_ref())],
            Outcome::Defined => Vec::new(),
            Outcome::Answer(answer) => answer
                .records()
                .into_iter()
                .map(|(inner, record)| {
                    let mut record_path = outcome_path.member("answer");
                    record_path.0.extend(inner.0);
                    (record_path, record)
                })
                .collect(),
            Outcome::Error(_)
            | Outcome::EscapeTimeReading(_)
            | Outcome::NotEvaluated
            | Outcome::Picture
            | Outcome::Reachable(_) => Vec::new(),
        };
        let line_dependencies = dependencies.entry(line.id()).or_default();
        if let Some(reading) = line.reading() {
            if !known.contains(&reading.source) {
                return Err(LoadError::MissingDependency(
                    lines_path
                        .index(position)
                        .member("reading")
                        .member("source"),
                ));
            }
            line_dependencies.push(reading.source);
        }
        for (record_path, record) in records {
            for (dependency_position, dependency) in record.dependencies().iter().enumerate() {
                if !known.contains(dependency) {
                    return Err(LoadError::MissingDependency(
                        record_path
                            .member("dependencies")
                            .index(dependency_position),
                    ));
                }
                if !line_dependencies.contains(dependency) {
                    line_dependencies.push(*dependency);
                }
            }
        }
    }
    let mut finished: HashSet<LineId> = HashSet::new();
    for line in lines {
        if finished.contains(&line.id()) {
            continue;
        }
        let mut on_path: HashSet<LineId> = HashSet::new();
        let mut pending = vec![(line.id(), false)];
        while let Some((current, children_are_done)) = pending.pop() {
            if children_are_done {
                on_path.remove(&current);
                finished.insert(current);
                continue;
            }
            if finished.contains(&current) {
                continue;
            }
            if !on_path.insert(current) {
                return Err(LoadError::DependencyCycle(current));
            }
            pending.push((current, true));
            for dependency in dependencies.get(&current).into_iter().flatten() {
                if !finished.contains(dependency) {
                    pending.push((*dependency, false));
                }
            }
        }
    }
    Ok(())
}

fn version_of(document: &Json) -> Option<u64> {
    let Json::Object(object) = document else {
        return None;
    };
    object
        .iter()
        .find_map(|(name, member)| match (name.as_str(), member) {
            ("version", Json::Count(version)) => Some(*version),
            _ => None,
        })
}

fn object_member_mut<'json>(json: &'json mut Json, wanted: &str) -> Option<&'json mut Json> {
    let Json::Object(object) = json else {
        return None;
    };
    object
        .iter_mut()
        .find(|(name, _)| name == wanted)
        .map(|(_, member)| member)
}

pub(crate) fn convert_version_1(mut document: Json) -> Result<Json, LoadError> {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(VERSION_WITHOUT_PICTURES);
    }
    let lines_path = JsonPath::default().member("lines");
    let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") else {
        return Ok(document);
    };
    for (position, line) in lines.iter_mut().enumerate() {
        let status_path = lines_path
            .index(position)
            .member("outcome")
            .member("status");
        let Some(outcome) = object_member_mut(line, "outcome") else {
            continue;
        };
        match object_member_mut(outcome, "status") {
            Some(Json::String(status)) if status == "result" => {}
            Some(Json::String(status)) if outcome_status_from_name(status).is_some() => {
                return Err(LoadError::InvalidValue(status_path));
            }
            _ => continue,
        }
        let Some(Json::Object(record)) = object_member_mut(outcome, "record") else {
            continue;
        };
        if let Some(position) = record
            .iter()
            .position(|(name, _)| name == "corpus_references")
        {
            record.insert(
                position + 1,
                ("sources".to_string(), Json::Array(Vec::new())),
            );
        }
    }
    Ok(document)
}

pub(crate) fn convert_version_2(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(VERSION_WITHOUT_FURTHER_INPUT_CONDITIONS);
    }
    if let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") {
        for line in lines {
            if let Json::Object(members) = line {
                members.push(("picture".to_string(), Json::Null));
                members.push(("reading".to_string(), Json::Null));
            }
        }
    }
    document
}

fn insert_empty_array_after(members: &mut Vec<(String, Json)>, after: &str, name: &str) {
    let position = members
        .iter()
        .position(|(member, _)| member == after)
        .map_or(members.len(), |position| position + 1);
    members.insert(position, (name.to_string(), Json::Array(Vec::new())));
}

fn insert_null_after(members: &mut Vec<(String, Json)>, after: &str, name: &str) {
    let position = members
        .iter()
        .position(|(member, _)| member == after)
        .map_or(members.len(), |position| position + 1);
    members.insert(position, (name.to_string(), Json::Null));
}

pub(crate) fn convert_version_3(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(VERSION_WITHOUT_UNIT_OVERRIDE);
    }
    let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") else {
        return document;
    };
    for line in lines {
        let Some(answer) = object_member_mut(line, "outcome")
            .and_then(|outcome| object_member_mut(outcome, "answer"))
        else {
            continue;
        };
        if let Some(Json::Array(entries)) = object_member_mut(answer, "missing_any_of") {
            for entry in entries {
                if let Json::Object(members) = entry {
                    insert_null_after(members, "rule", "derivation");
                    insert_null_after(members, "between", "conditions");
                }
            }
        }
        if let Some(Json::Object(bounds)) = object_member_mut(answer, "bounds") {
            insert_null_after(bounds, "sources", "from");
        }
    }
    document
}

pub(crate) fn convert_version_4(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(VERSION_WITHOUT_AXIS_UNITS);
    }
    if let Some(Json::Object(settings)) = object_member_mut(&mut document, "settings") {
        settings.push(("units".to_string(), Json::Null));
    }
    document
}

fn add_axis_units(view: &mut Json) {
    if let Some(Json::Array(axes)) = object_member_mut(view, "axes") {
        for axis in axes {
            if let Json::Object(members) = axis {
                insert_null_after(members, "upper", "unit");
            }
        }
    }
}

pub(crate) fn convert_version_5(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(VERSION_WITHOUT_CONDITION_PARAMETERS);
    }
    let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") else {
        return document;
    };
    for line in lines {
        if let Some(Json::Array(views)) = object_member_mut(line, "picture")
            .and_then(|picture| object_member_mut(picture, "views"))
        {
            views.iter_mut().for_each(add_axis_units);
        }
        if let Some(view) = object_member_mut(line, "reading")
            .and_then(|reading| object_member_mut(reading, "view"))
        {
            add_axis_units(view);
        }
    }
    document
}

fn add_condition_parameters(conditions: Option<&mut Json>) {
    if let Some(Json::Array(conditions)) = conditions {
        for condition in conditions {
            if let Json::Object(members) = condition {
                insert_null_after(members, "text", "parameters");
            }
        }
    }
}

const ANSWER_WAY_LISTS: [&str; 5] = ["ways", "front", "dominated", "entries", "missing_any_of"];

pub(crate) fn convert_version_6(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(VERSION_WITHOUT_ITERATION_LIMIT);
    }
    let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") else {
        return document;
    };
    for line in lines {
        let Some(answer) = object_member_mut(line, "outcome")
            .and_then(|outcome| object_member_mut(outcome, "answer"))
        else {
            continue;
        };
        for list in ANSWER_WAY_LISTS {
            if let Some(Json::Array(items)) = object_member_mut(answer, list) {
                for item in items {
                    add_condition_parameters(object_member_mut(item, "conditions"));
                }
            }
        }
    }
    document
}

pub(crate) fn convert_version_7(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(FORMAT_VERSION);
    }
    let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") else {
        return document;
    };
    for line in lines {
        if let Some(Json::Object(picture)) = object_member_mut(line, "picture") {
            insert_null_after(picture, "parameters", "iteration_limit");
        }
    }
    document
}

fn insert_modes(record: &mut Json) {
    if let Some(Json::Object(backend)) = object_member_mut(record, "backend") {
        insert_empty_array_after(backend, "approximate_operations", "modes");
    }
}

pub(crate) fn convert_version_8(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(FORMAT_VERSION);
    }
    let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") else {
        return document;
    };
    for line in lines {
        let Some(outcome) = object_member_mut(line, "outcome") else {
            continue;
        };
        if let Some(record) = object_member_mut(outcome, "record") {
            insert_modes(record);
        }
        let Some(answer) = object_member_mut(outcome, "answer") else {
            continue;
        };
        for group in ["ways", "front", "dominated"] {
            if let Some(Json::Array(ways)) = object_member_mut(answer, group) {
                for way in ways {
                    if let Some(record) = object_member_mut(way, "record") {
                        insert_modes(record);
                    }
                }
            }
        }
    }
    document
}

const RENAMED_OPERATIONS: [(&str, &str, &str); 2] = [
    ("operator", "copy_sign", "copysign"),
    ("operator", "to_exact", "exact"),
];

const RENAMED_MODE_OPERATIONS: [(&str, &str, &str); 1] = [("operation", "copy_sign", "copysign")];

fn rename_operation_names(json: &mut Json) {
    match json {
        Json::Array(elements) => {
            for element in elements {
                rename_operation_names(element);
            }
        }
        Json::Object(members) => {
            for (name, value) in members {
                if let Json::String(text) = value
                    && let Some((_, _, renamed)) = RENAMED_OPERATIONS
                        .iter()
                        .chain(RENAMED_MODE_OPERATIONS.iter())
                        .find(|(member, old, _)| member == name && old == text)
                {
                    *text = (*renamed).to_string();
                }
                rename_operation_names(value);
            }
        }
        _ => {}
    }
}

pub(crate) fn convert_version_11(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(FORMAT_VERSION);
    }
    rename_operation_names(&mut document);
    document
}

pub(crate) fn convert_version_9(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(FORMAT_VERSION);
    }
    let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") else {
        return document;
    };
    for line in lines {
        if let Some(outcome) = object_member_mut(line, "outcome")
            && is_definition_error(outcome)
        {
            *outcome = Json::object(vec![("status", Json::string(DEFINED_STATUS))]);
        }
    }
    document
}

fn is_definition_error(outcome: &Json) -> bool {
    let Json::Object(members) = outcome else {
        return false;
    };
    let member = |wanted: &str| {
        members
            .iter()
            .find(|(name, _)| name == wanted)
            .map(|(_, value)| value)
    };
    let is_error = matches!(member("status"), Some(Json::String(status)) if status == "error");
    let Some(Json::Object(error)) = member("error") else {
        return false;
    };
    let has_code = matches!(
        error.iter().find(|(name, _)| name == "code"),
        Some((_, Json::String(code))) if code == DEFINITION_ERROR_CODE
    );
    let has_no_data = matches!(
        error.iter().find(|(name, _)| name == "data"),
        Some((_, Json::Object(data))) if data.is_empty()
    );
    is_error && has_code && has_no_data
}

pub(crate) fn convert_version_10(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(FORMAT_VERSION);
    }
    let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") else {
        return document;
    };
    for line in lines {
        let Some(outcome) = object_member_mut(line, "outcome") else {
            continue;
        };
        if let Some(Json::Object(record)) = object_member_mut(outcome, "record") {
            insert_null_after(record, "sources", "recognized");
        }
        let Some(answer) = object_member_mut(outcome, "answer") else {
            continue;
        };
        for list in ANSWER_WAY_LISTS {
            if let Some(Json::Array(items)) = object_member_mut(answer, list) {
                for item in items {
                    if let Some(Json::Object(record)) = object_member_mut(item, "record") {
                        insert_null_after(record, "sources", "recognized");
                    }
                }
            }
        }
    }
    document
}

pub(crate) fn convert_version_15(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(FORMAT_VERSION);
    }
    let width = stored_width(&document);
    let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") else {
        return document;
    };
    for line in lines {
        let Some(outcome) = object_member_mut(line, "outcome") else {
            continue;
        };
        if let Some(record) = object_member_mut(outcome, "record") {
            listed_backend(record, width);
        }
        let Some(answer) = object_member_mut(outcome, "answer") else {
            continue;
        };
        for list in ANSWER_WAY_LISTS {
            if let Some(Json::Array(items)) = object_member_mut(answer, list) {
                for item in items {
                    if let Some(record) = object_member_mut(item, "record") {
                        listed_backend(record, width);
                    }
                }
            }
        }
    }
    document
}

pub(crate) fn convert_version_16(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(FORMAT_VERSION);
    }
    let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") else {
        return document;
    };
    for line in lines {
        let Some(outcome) = object_member_mut(line, "outcome") else {
            continue;
        };
        if let Some(record) = object_member_mut(outcome, "record") {
            without_sort(record);
        }
        let Some(answer) = object_member_mut(outcome, "answer") else {
            continue;
        };
        for list in ANSWER_WAY_LISTS {
            if let Some(Json::Array(items)) = object_member_mut(answer, list) {
                for item in items {
                    if let Some(record) = object_member_mut(item, "record") {
                        without_sort(record);
                    }
                }
            }
        }
    }
    document
}

pub(crate) fn convert_version_17(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(FORMAT_VERSION);
    }
    let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") else {
        return document;
    };
    for line in lines {
        let Some(outcome) = object_member_mut(line, "outcome") else {
            continue;
        };
        if let Some(record) = object_member_mut(outcome, "record") {
            without_tallies(record);
        }
        let Some(answer) = object_member_mut(outcome, "answer") else {
            continue;
        };
        for list in ANSWER_WAY_LISTS {
            if let Some(Json::Array(items)) = object_member_mut(answer, list) {
                for item in items {
                    if let Some(record) = object_member_mut(item, "record") {
                        without_tallies(record);
                    }
                }
            }
        }
    }
    document
}

pub(crate) fn convert_version_18(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(FORMAT_VERSION);
    }
    let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") else {
        return document;
    };
    for line in lines {
        let Some(outcome) = object_member_mut(line, "outcome") else {
            continue;
        };
        if let Some(record) = object_member_mut(outcome, "record") {
            with_decrements(record);
        }
        let Some(answer) = object_member_mut(outcome, "answer") else {
            continue;
        };
        for list in ANSWER_WAY_LISTS {
            if let Some(Json::Array(items)) = object_member_mut(answer, list) {
                for item in items {
                    if let Some(record) = object_member_mut(item, "record") {
                        with_decrements(record);
                    }
                }
            }
        }
    }
    document
}

pub(crate) fn convert_version_19(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(FORMAT_VERSION);
    }
    let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") else {
        return document;
    };
    for line in lines {
        let Some(outcome) = object_member_mut(line, "outcome") else {
            continue;
        };
        if let Some(record) = object_member_mut(outcome, "record") {
            without_draws(record);
        }
        let Some(answer) = object_member_mut(outcome, "answer") else {
            continue;
        };
        for list in ANSWER_WAY_LISTS {
            if let Some(Json::Array(items)) = object_member_mut(answer, list) {
                for item in items {
                    if let Some(record) = object_member_mut(item, "record") {
                        without_draws(record);
                    }
                }
            }
        }
    }
    document
}

pub(crate) fn convert_version_21(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(FORMAT_VERSION);
    }
    document
}

pub(crate) fn convert_version_20(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(FORMAT_VERSION);
    }
    let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") else {
        return document;
    };
    for line in lines {
        let Some(outcome) = object_member_mut(line, "outcome") else {
            continue;
        };
        if let Some(record) = object_member_mut(outcome, "record") {
            without_flips(record);
        }
        let Some(answer) = object_member_mut(outcome, "answer") else {
            continue;
        };
        for list in ANSWER_WAY_LISTS {
            if let Some(Json::Array(items)) = object_member_mut(answer, list) {
                for item in items {
                    if let Some(record) = object_member_mut(item, "record") {
                        without_flips(record);
                    }
                }
            }
        }
    }
    document
}

fn without_flips(record: &mut Json) {
    let Some(Json::Object(members)) = object_member_mut(record, "sort") else {
        return;
    };
    if members.iter().all(|(member, _)| member != "flips") {
        members.push(("flips".to_string(), Json::Null));
    }
}

fn without_draws(record: &mut Json) {
    let Some(Json::Object(members)) = object_member_mut(record, "sort") else {
        return;
    };
    if members.iter().all(|(member, _)| member != "draws") {
        members.push(("draws".to_string(), Json::Null));
    }
}

fn with_decrements(record: &mut Json) {
    let Some(sort) = object_member_mut(record, "sort") else {
        return;
    };
    let placed = match object_member_mut(sort, "from_positions") {
        Some(Json::Array(positions)) => u64::try_from(positions.len()).unwrap_or(u64::MAX),
        _ => return,
    };
    if let Some(Json::Count(tallies)) = object_member_mut(sort, "tallies") {
        *tallies = tallies.saturating_add(placed);
    }
}

fn without_tallies(record: &mut Json) {
    let Some(Json::Object(members)) = object_member_mut(record, "sort") else {
        return;
    };
    for name in ["tallies", "key_range"] {
        if members.iter().all(|(member, _)| member != name) {
            members.push((name.to_string(), Json::Null));
        }
    }
}

fn without_sort(record: &mut Json) {
    let Json::Object(members) = record else {
        return;
    };
    if members.iter().all(|(name, _)| name != "sort") {
        let place = members
            .iter()
            .position(|(name, _)| name == "seed")
            .map_or(members.len(), |seed| seed + 1);
        members.insert(place, ("sort".to_string(), Json::Null));
    }
}

fn stored_width(document: &Json) -> &'static str {
    let Json::Object(members) = document else {
        return "f64";
    };
    let Some((_, settings)) = members.iter().find(|(name, _)| name == "settings") else {
        return "f64";
    };
    let Json::Object(settings) = settings else {
        return "f64";
    };
    match settings.iter().find(|(name, _)| name == "precision") {
        Some((_, Json::String(precision))) if precision == "f32" => "f32",
        _ => "f64",
    }
}

fn listed_backend(record: &mut Json, width: &'static str) {
    let Some(backend) = object_member_mut(record, "backend") else {
        return;
    };
    let Json::Object(members) = backend else {
        return;
    };
    let ran = !matches!(
        members.iter().find(|(name, _)| name == "selected"),
        Some((_, Json::String(selected))) if selected == "none"
    );
    if !ran {
        *backend = Json::Array(Vec::new());
        return;
    }
    let mut used = members.clone();
    let place = used
        .iter()
        .position(|(name, _)| name == "selected")
        .unwrap_or(0);
    used.insert(place, ("width".to_string(), Json::string(width)));
    *backend = Json::Array(vec![Json::Object(used)]);
}

pub(crate) fn convert_version_14(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(FORMAT_VERSION);
    }
    document
}

pub(crate) fn convert_version_12(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(FORMAT_VERSION);
    }
    let Some(Json::Array(lines)) = object_member_mut(&mut document, "lines") else {
        return document;
    };
    for line in lines {
        let Some(outcome) = object_member_mut(line, "outcome") else {
            continue;
        };
        if let Some(record) = object_member_mut(outcome, "record") {
            add_recognition_state(record);
        }
        let Some(answer) = object_member_mut(outcome, "answer") else {
            continue;
        };
        for list in ANSWER_WAY_LISTS {
            if let Some(Json::Array(items)) = object_member_mut(answer, list) {
                for item in items {
                    if let Some(record) = object_member_mut(item, "record") {
                        add_recognition_state(record);
                    }
                }
            }
        }
    }
    document
}

pub(crate) fn convert_version_13(mut document: Json) -> Json {
    if let Some(version) = object_member_mut(&mut document, "version") {
        *version = Json::Count(FORMAT_VERSION);
    }
    document
}

fn add_recognition_state(record: &mut Json) {
    let Some(recognized) = object_member_mut(record, "recognized") else {
        return;
    };
    let Json::Object(members) = recognized else {
        *recognized = Json::object(vec![("state", Json::string(RECOGNITION_NOT_RUN))]);
        return;
    };
    insert_null_after(members, "concept_set_version", "truncated_by");
    insert_null_after(members, "concept_set_version", "truncated");
    insert_null_after(members, "concept_set_version", "limits");
    members.insert(0, ("state".to_string(), Json::string(RECOGNITION_RAN)));
}

const OPERATION_NAMES: [(&str, PlanOp); 35] = [
    ("add", PlanOp::Add),
    ("sub", PlanOp::Sub),
    ("mul", PlanOp::Mul),
    ("div", PlanOp::Div),
    ("neg", PlanOp::Neg),
    ("abs", PlanOp::Abs),
    ("sqrt", PlanOp::Sqrt),
    ("mul_add", PlanOp::MulAdd),
    ("min", PlanOp::Min),
    ("max", PlanOp::Max),
    ("floor", PlanOp::Floor),
    ("ceil", PlanOp::Ceil),
    ("trunc", PlanOp::Trunc),
    ("round_ties_even", PlanOp::RoundTiesEven),
    ("copysign", PlanOp::CopySign),
    ("less", PlanOp::Less),
    ("less_or_equal", PlanOp::LessOrEqual),
    ("greater", PlanOp::Greater),
    ("greater_or_equal", PlanOp::GreaterOrEqual),
    ("equal", PlanOp::Equal),
    ("not_equal", PlanOp::NotEqual),
    ("and", PlanOp::And),
    ("or", PlanOp::Or),
    ("not", PlanOp::Not),
    ("select", PlanOp::Select),
    ("exp", PlanOp::Exp),
    ("ln", PlanOp::Ln),
    ("sin", PlanOp::Sin),
    ("cos", PlanOp::Cos),
    ("tan", PlanOp::Tan),
    ("asin", PlanOp::Asin),
    ("acos", PlanOp::Acos),
    ("atan", PlanOp::Atan),
    ("atan2", PlanOp::Atan2),
    ("pow", PlanOp::Pow),
];

pub fn operation_name(operation: PlanOp) -> &'static str {
    OPERATION_NAMES
        .iter()
        .find(|(_, listed)| *listed == operation)
        .map_or("", |(text, _)| *text)
}

fn operation(json: &Json, path: &JsonPath) -> Result<PlanOp, LoadError> {
    one_of(json, path, &OPERATION_NAMES)
}

fn execution_mode_name(mode: ExecutionMode) -> &'static str {
    match mode {
        ExecutionMode::Native => "native",
        ExecutionMode::IntegerExact => "integer_exact",
        ExecutionMode::ExactInBothForms => "exact_in_both_forms",
    }
}

fn unit_override_json(unit_override: Option<&UnitOverride>) -> Json {
    Json::optional(unit_override.map(|unit_override| {
        Json::object(vec![
            (
                "system",
                Json::optional(unit_override.system.as_deref().map(Json::string)),
            ),
            (
                "overrides",
                Json::Array(
                    unit_override
                        .overrides
                        .iter()
                        .map(|entry| {
                            Json::object(vec![
                                ("kind", Json::string(&entry.kind)),
                                ("unit", Json::string(&entry.unit)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }))
}

fn content_identifier(json: &Json, path: &JsonPath) -> Result<String, LoadError> {
    let text = string(json, path)?;
    if is_identifier(text) {
        Ok(text.to_owned())
    } else {
        Err(LoadError::InvalidValue(path.clone()))
    }
}

fn non_empty_string(json: &Json, path: &JsonPath) -> Result<String, LoadError> {
    let text = string(json, path)?;
    if text.is_empty() {
        Err(LoadError::InvalidValue(path.clone()))
    } else {
        Ok(text.to_owned())
    }
}

fn unit_override(json: &Json, path: &JsonPath) -> Result<Option<UnitOverride>, LoadError> {
    let Some(json) = optional(json) else {
        return Ok(None);
    };
    let found = members(json, path, &["system", "overrides"])?;
    let system_path = path.member("system");
    let system = optional(found[0])
        .map(|system| content_identifier(system, &system_path))
        .transpose()?;
    let overrides_path = path.member("overrides");
    let mut overrides: Vec<KindUnit> = Vec::new();
    for (position, entry) in array(found[1], &overrides_path)?.iter().enumerate() {
        let entry_path = overrides_path.index(position);
        let parts = members(entry, &entry_path, &["kind", "unit"])?;
        let kind_path = entry_path.member("kind");
        let kind = content_identifier(parts[0], &kind_path)?;
        if overrides
            .last()
            .is_some_and(|previous| previous.kind >= kind)
        {
            return Err(LoadError::InvalidValue(kind_path));
        }
        overrides.push(KindUnit {
            kind,
            unit: non_empty_string(parts[1], &entry_path.member("unit"))?,
        });
    }
    if system.is_none() && overrides.is_empty() {
        return Err(LoadError::InvalidValue(path.clone()));
    }
    Ok(Some(UnitOverride { system, overrides }))
}

pub(crate) fn decode(pool: &mut ExprPool, bytes: &[u8]) -> Result<SessionData, LoadError> {
    let parsed = json::parse(bytes).map_err(LoadError::Syntax)?;
    let root = JsonPath::default();
    let from = version_of(&parsed).filter(|version| (1..FORMAT_VERSION).contains(version));
    let converts_from = |version: u64| from.is_some_and(|from| from <= version);
    let mut document = parsed;
    if converts_from(VERSION_WITHOUT_SOURCES) {
        document = convert_version_1(document)?;
    }
    if converts_from(VERSION_WITHOUT_PICTURES) {
        document = convert_version_2(document);
    }
    if converts_from(VERSION_WITHOUT_FURTHER_INPUT_CONDITIONS) {
        document = convert_version_3(document);
    }
    if converts_from(VERSION_WITHOUT_UNIT_OVERRIDE) {
        document = convert_version_4(document);
    }
    if converts_from(VERSION_WITHOUT_AXIS_UNITS) {
        document = convert_version_5(document);
    }
    if converts_from(VERSION_WITHOUT_CONDITION_PARAMETERS) {
        document = convert_version_6(document);
    }
    if converts_from(VERSION_WITHOUT_ITERATION_LIMIT) {
        document = convert_version_7(document);
    }
    if converts_from(VERSION_WITHOUT_OPERATION_MODES) {
        document = convert_version_8(document);
    }
    if converts_from(VERSION_WITHOUT_DEFINED_OUTCOME) {
        document = convert_version_9(document);
    }
    if converts_from(VERSION_WITHOUT_RECOGNIZED) {
        document = convert_version_10(document);
    }
    if converts_from(VERSION_WITH_DERIVED_OPERATION_NAMES) {
        document = convert_version_11(document);
    }
    if converts_from(VERSION_WITHOUT_TRUNCATION) {
        document = convert_version_12(document);
    }
    if converts_from(VERSION_WITHOUT_INVERSE_TRIGONOMETRY) {
        document = convert_version_13(document);
    }
    if converts_from(VERSION_WITHOUT_PREPARE_FAILED) {
        document = convert_version_14(document);
    }
    if converts_from(VERSION_WITH_ONE_BACKEND) {
        document = convert_version_15(document);
    }
    if converts_from(VERSION_WITHOUT_SORT) {
        document = convert_version_16(document);
    }
    if converts_from(VERSION_WITHOUT_TALLIES) {
        document = convert_version_17(document);
    }
    if converts_from(VERSION_WITHOUT_DECREMENTS) {
        document = convert_version_18(document);
    }
    if converts_from(VERSION_WITHOUT_DRAWS) {
        document = convert_version_19(document);
    }
    if converts_from(VERSION_WITHOUT_FLIPS) {
        document = convert_version_20(document);
    }
    if converts_from(VERSION_WITH_POSITIONS_ALWAYS) {
        document = convert_version_21(document);
    }
    let found = members(
        &document,
        &root,
        &["format", "version", "settings", "next_line_number", "lines"],
    )?;
    if string(found[0], &root.member("format"))? != FORMAT_NAME {
        return Err(LoadError::InvalidValue(root.member("format")));
    }
    let found_version = count(found[1], &root.member("version"))?;
    if found_version != FORMAT_VERSION {
        return Err(LoadError::UnsupportedVersion {
            found: found_version,
            supported: FORMAT_VERSION,
        });
    }
    let settings_path = root.member("settings");
    let settings_members = members(found[2], &settings_path, &["precision", "backend", "units"])?;
    let settings = Settings {
        precision: one_of(
            settings_members[0],
            &settings_path.member("precision"),
            &[
                ("exact", Precision::Exact),
                ("f32", Precision::F32),
                ("f64", Precision::F64),
            ],
        )?,
        backend: preference(settings_members[1], &settings_path.member("backend"))?,
    };
    let unit_override = unit_override(settings_members[2], &settings_path.member("units"))?;
    let next_line_number = count(found[3], &root.member("next_line_number"))?;
    if next_line_number == 0 {
        return Err(LoadError::InvalidValue(root.member("next_line_number")));
    }

    let lines_path = root.member("lines");
    let mut lines = Vec::new();
    let mut seen_ids = HashSet::new();
    let mut seen_names = HashSet::new();
    for (position, line_json) in array(found[4], &lines_path)?.iter().enumerate() {
        let line_path = lines_path.index(position);
        let parts = members(
            line_json,
            &line_path,
            &["id", "name", "input", "outcome", "picture", "reading"],
        )?;
        let id_path = line_path.member("id");
        let id = line_from_label(string(parts[0], &id_path)?)
            .ok_or_else(|| LoadError::InvalidValue(id_path.clone()))?;
        if !seen_ids.insert(id) {
            return Err(LoadError::DuplicateLineId(id_path));
        }
        if id.number() >= next_line_number {
            return Err(LoadError::LineNumberNotBelowNext(id_path));
        }
        let name_path = line_path.member("name");
        let name = optional(parts[1])
            .map(|name| string(name, &name_path).map(str::to_string))
            .transpose()?;
        if let Some(name) = &name {
            if !is_valid_name(name) {
                return Err(LoadError::InvalidValue(name_path));
            }
            if !seen_names.insert(name.clone()) {
                return Err(LoadError::DuplicateName(name_path));
            }
        }
        let input = string(parts[2], &line_path.member("input"))?.to_string();
        let line_outcome = outcome(pool, parts[3], &line_path.member("outcome"))?;
        let line_picture = picture(
            parts[4],
            &line_path.member("picture"),
            crate::escape_time_line::is_escape_time_text(&input),
        )?;
        let line_reading = reading(parts[5], &line_path.member("reading"))?;
        lines.push(
            Line::restored(id, name, input, line_outcome)
                .with_picture_and_reading(line_picture, line_reading),
        );
    }
    let known_lines: HashSet<LineId> = lines.iter().map(Line::id).collect();
    for (position, line) in lines.iter().enumerate() {
        if let Some(stated) = line.reading()
            && !known_lines.contains(&stated.source)
        {
            return Err(LoadError::MissingDependency(
                lines_path
                    .index(position)
                    .member("reading")
                    .member("source"),
            ));
        }
    }
    check_dependency_graph(&lines, &lines_path)?;
    Ok(SessionData {
        settings,
        unit_override,
        next_line_number,
        lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line_picture::{AxisDisplayUnit, CameraProjection};
    use crate::session::Session;
    use crate::session::tests::{fixed_clock, session};
    use calc_exec_cpu::CpuBackend;
    use calc_units::TemperatureScale;

    const EXAMPLE_LINE: &str = r#"{
  "id": "r3",
  "name": "F",
  "input": "F = m*a",
  "outcome": {
    "status": "result",
    "record": {
      "kind": "machine_float",
      "value": {
        "type": "f64",
        "bits": "0x4038866666666667",
        "decimal": "2.4525000000000002e1"
      },
      "unit": "N",
      "rounding_error": {
        "status": "bound",
        "bound": {
          "type": "f64",
          "bits": "0x3cf0000000000000",
          "decimal": "3.552713678800501e-15"
        }
      },
      "uncertainty": {
        "standard": {
          "type": "f64",
          "bits": "0x3fbc2ffd6203d5e4",
          "decimal": "1.1010726588195713e-1"
        },
        "coverage_factor": {
          "type": "integer",
          "digits": "1"
        }
      },
      "method": {
        "name": "first_order_propagation",
        "parameters": {
          "correlation": "uncorrelated"
        },
        "convergence": null,
        "condition": null,
        "notes": []
      },
      "corpus_references": [
        "P-GRD-D-013",
        "P-GRD-D-038"
      ],
      "seed": null,
      "backend": {
        "preference": "automatic",
        "selected": "cpu",
        "skipped": [],
        "approximate_operations": false
      },
      "dependencies": [
        "r1",
        "r2"
      ],
      "computed_at": "2026-09-15T12:41:07.312Z",
      "duration_ns": 300,
      "produced_by": "0.1.0"
    }
  }
}"#;

    fn indented(text: &str, spaces: usize) -> String {
        let prefix = " ".repeat(spaces);
        text.lines()
            .map(|line| format!("{prefix}{line}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn example_session() -> String {
        format!(
            "{{\n  \"format\": \"calc-session\",\n  \"version\": 1,\n  \"settings\": {{\n    \"precision\": \"f64\",\n    \"backend\": \"automatic\"\n  }},\n  \"next_line_number\": 4,\n  \"lines\": [\n    {{\n      \"id\": \"r1\",\n      \"name\": \"m\",\n      \"input\": \"m = 2.5 +- 0.01\",\n      \"outcome\": {{\n        \"status\": \"not_evaluated\"\n      }}\n    }},\n    {{\n      \"id\": \"r2\",\n      \"name\": \"a\",\n      \"input\": \"a = 9.81 +- 0.02\",\n      \"outcome\": {{\n        \"status\": \"not_evaluated\"\n      }}\n    }},\n{}\n  ]\n}}\n",
            indented(EXAMPLE_LINE, 4)
        )
    }

    fn open(text: &str) -> Result<Session, LoadError> {
        Session::open_from_bytes(
            text.as_bytes(),
            fixed_clock(),
            vec![Box::new(CpuBackend::new())],
        )
    }

    fn path(segments: &[&str]) -> JsonPath {
        JsonPath(
            segments
                .iter()
                .map(|segment| match segment.parse::<usize>() {
                    Ok(position) => PathSegment::Index(position),
                    Err(_) => PathSegment::Member((*segment).to_string()),
                })
                .collect(),
        )
    }

    fn with_replaced(original: &str, replacement: &str) -> String {
        let text = example_session();
        assert!(text.contains(original), "{original}");
        text.replacen(original, replacement, 1)
    }

    fn example_session_version_2() -> String {
        example_session()
            .replacen("\"version\": 1", "\"version\": 2", 1)
            .replacen(
                "\"P-GRD-D-038\"\n          ],\n",
                "\"P-GRD-D-038\"\n          ],\n          \"sources\": [],\n",
                1,
            )
    }

    fn example_session_version_3() -> String {
        example_session_version_2()
            .replacen("\"version\": 2", "\"version\": 3", 1)
            .replace(
                "\n      }\n    }",
                "\n      },\n      \"picture\": null,\n      \"reading\": null\n    }",
            )
    }

    fn example_session_version_4() -> String {
        example_session_version_3().replacen("\"version\": 3", "\"version\": 4", 1)
    }

    const FRONT_REQUEST: &str = r#"{"phase": "ways", "object": "circle", "wanted": "circumference", "criterion": {"by": ["fewest_measurements", "gate_count"], "combine": "front"}}"#;

    fn without_condition_parameters(value: &mut Json) {
        match value {
            Json::Object(members) => {
                for (name, member) in members.iter_mut() {
                    if name == "conditions"
                        && let Json::Array(conditions) = member
                    {
                        for condition in conditions {
                            if let Json::Object(condition) = condition {
                                condition.retain(|(name, _)| name != "parameters");
                            }
                        }
                    }
                    without_condition_parameters(member);
                }
            }
            Json::Array(elements) => {
                for element in elements {
                    without_condition_parameters(element);
                }
            }
            _ => {}
        }
    }

    fn version_6_with_a_front_answer() -> String {
        let mut solving = crate::session::tests::session();
        let request = crate::solve_request::parse_request(FRONT_REQUEST.as_bytes()).unwrap();
        solving.enter_solve(&request).unwrap();
        let saved = solving.save_to_bytes().unwrap();
        let mut document = json::parse(&saved).unwrap();
        without_condition_parameters(&mut document);
        if let Some(version) = object_member_mut(&mut document, "version") {
            *version = Json::Count(VERSION_WITHOUT_CONDITION_PARAMETERS);
        }
        json::write_canonical(&document)
    }

    fn answer_list<'json>(document: &'json Json, list: &str) -> &'json [Json] {
        let Json::Object(members) = document else {
            panic!("an object");
        };
        let lines = members
            .iter()
            .find(|(name, _)| name == "lines")
            .map(|(_, value)| value)
            .expect("lines");
        let Json::Array(lines) = lines else {
            panic!("an array of lines")
        };
        let mut found = None;
        for line in lines {
            let Json::Object(line) = line else { continue };
            let Some((_, outcome)) = line.iter().find(|(name, _)| name == "outcome") else {
                continue;
            };
            let Json::Object(outcome) = outcome else {
                continue;
            };
            let Some((_, answer)) = outcome.iter().find(|(name, _)| name == "answer") else {
                continue;
            };
            let Json::Object(answer) = answer else {
                continue;
            };
            if let Some((_, Json::Array(items))) = answer.iter().find(|(name, _)| name == list) {
                found = Some(items.as_slice());
            }
        }
        found.unwrap_or(&[])
    }

    #[test]
    fn a_version_6_front_answer_opens() {
        let text = version_6_with_a_front_answer();

        assert!(open(&text).is_ok(), "{text}");
    }

    #[test]
    fn a_version_6_front_answer_keeps_its_ways_after_saving() {
        let text = version_6_with_a_front_answer();

        let saved = open(&text).unwrap().save_to_bytes().unwrap();

        let document = json::parse(&saved).unwrap();
        assert!(!answer_list(&document, "front").is_empty());
    }

    #[test]
    fn a_version_6_answer_holds_dominated_ways_to_convert() {
        let text = version_6_with_a_front_answer();

        let saved = open(&text).unwrap().save_to_bytes().unwrap();

        let document = json::parse(&saved).unwrap();
        assert!(!answer_list(&document, "dominated").is_empty());
    }

    fn condition_members(way: &Json) -> Vec<&str> {
        let Json::Object(way) = way else {
            panic!("a way")
        };
        let Some((_, Json::Array(conditions))) = way.iter().find(|(name, _)| name == "conditions")
        else {
            panic!("conditions")
        };
        let Some(Json::Object(condition)) = conditions.first() else {
            panic!("a condition")
        };
        condition.iter().map(|(name, _)| name.as_str()).collect()
    }

    #[test]
    fn a_condition_of_a_front_way_gains_its_parameters() {
        let text = version_6_with_a_front_answer();

        let saved = open(&text).unwrap().save_to_bytes().unwrap();

        let document = json::parse(&saved).unwrap();
        let front = answer_list(&document, "front");
        assert!(condition_members(&front[0]).contains(&"parameters"));
    }

    #[test]
    fn a_condition_of_a_dominated_way_gains_its_parameters() {
        let text = version_6_with_a_front_answer();

        let saved = open(&text).unwrap().save_to_bytes().unwrap();

        let document = json::parse(&saved).unwrap();
        let dominated = answer_list(&document, "dominated");
        assert!(condition_members(&dominated[0]).contains(&"parameters"));
    }

    #[test]
    fn the_conversion_gives_a_front_way_its_parameters() {
        let mut document = json::parse(version_6_with_a_front_answer().as_bytes()).unwrap();
        if let Some(version) = object_member_mut(&mut document, "version") {
            *version = Json::Count(VERSION_WITHOUT_CONDITION_PARAMETERS);
        }
        let converted = convert_version_6(document);

        let raised = json::write_canonical(&converted);

        assert!(raised.contains("\"parameters\": null"), "{raised}");
    }

    #[test]
    fn the_conversion_walks_every_list_an_answer_writes() {
        assert_eq!(
            ANSWER_WAY_LISTS,
            ["ways", "front", "dominated", "entries", "missing_any_of"]
        );
    }

    fn example_session_version_5() -> String {
        example_session_version_4()
            .replacen("\"version\": 4", "\"version\": 5", 1)
            .replacen(
                "\"backend\": \"automatic\"\n  },",
                "\"backend\": \"automatic\",\n    \"units\": null\n  },",
                1,
            )
    }

    fn example_session_version_6() -> String {
        example_session_version_5().replacen("\"version\": 5", "\"version\": 6", 1)
    }

    fn example_session_version_7() -> String {
        example_session_version_6().replacen("\"version\": 6", "\"version\": 7", 1)
    }

    fn example_session_version_9() -> String {
        example_session_version_7()
            .replacen("\"version\": 7", "\"version\": 9", 1)
            .replacen(
                "\"approximate_operations\": false\n          },",
                "\"approximate_operations\": false,\n            \"modes\": []\n          },",
                1,
            )
    }

    fn example_session_version_10() -> String {
        example_session_version_9().replacen("\"version\": 9", "\"version\": 10", 1)
    }

    fn example_session_version_11() -> String {
        example_session_version_10()
            .replacen("\"version\": 10", "\"version\": 11", 1)
            .replacen(
                "\"sources\": [],\n",
                "\"sources\": [],\n          \"recognized\": null,\n",
                1,
            )
    }

    fn example_session_version_12() -> String {
        example_session_version_11().replacen("\"version\": 11", "\"version\": 12", 1)
    }

    fn example_session_version_13() -> String {
        example_session_version_12()
            .replacen("\"version\": 12", "\"version\": 13", 1)
            .replacen(
                "\"recognized\": null,",
                "\"recognized\": {\n            \"state\": \"not_run\"\n          },",
                1,
            )
    }

    fn example_session_version_14() -> String {
        example_session_version_13().replacen("\"version\": 13", "\"version\": 14", 1)
    }

    fn example_session_version_15() -> String {
        example_session_version_14().replacen("\"version\": 14", "\"version\": 15", 1)
    }

    fn example_session_version_16() -> String {
        example_session_version_15()
            .replacen("\"version\": 15", "\"version\": 16", 1)
            .replacen(
                "\"backend\": {\n            \"preference\": \"automatic\",\n            \"selected\": \"cpu\",\n            \"skipped\": [],\n            \"approximate_operations\": false,\n            \"modes\": []\n          },",
                "\"backend\": [\n            {\n              \"preference\": \"automatic\",\n              \"width\": \"f64\",\n              \"selected\": \"cpu\",\n              \"skipped\": [],\n              \"approximate_operations\": false,\n              \"modes\": []\n            }\n          ],",
                1,
            )
    }

    fn example_session_version_17() -> String {
        example_session_version_16()
            .replacen("\"version\": 16", "\"version\": 17", 1)
            .replacen(
                "\"seed\": null,\n",
                "\"seed\": null,\n          \"sort\": null,\n",
                1,
            )
    }

    fn example_session_version_18() -> String {
        example_session_version_17().replacen("\"version\": 17", "\"version\": 18", 1)
    }

    fn example_session_version_19() -> String {
        example_session_version_18().replacen("\"version\": 18", "\"version\": 19", 1)
    }

    fn example_session_version_20() -> String {
        example_session_version_19().replacen("\"version\": 19", "\"version\": 20", 1)
    }

    fn example_session_version_21() -> String {
        example_session_version_20().replacen("\"version\": 20", "\"version\": 21", 1)
    }

    fn example_session_version_22() -> String {
        example_session_version_21().replacen("\"version\": 21", "\"version\": 22", 1)
    }

    const PICTURE: &str = "\"picture\": {\"views\": [{\"axes\": [{\"lower\": {\"type\": \"integer\", \"digits\": \"-2\"}, \"upper\": {\"type\": \"rational\", \"numerator\": \"5\", \"denominator\": \"2\"}}], \"camera\": null}, {\"axes\": [{\"lower\": {\"type\": \"integer\", \"digits\": \"0\"}, \"upper\": {\"type\": \"integer\", \"digits\": \"1\"}}, {\"lower\": {\"type\": \"integer\", \"digits\": \"0\"}, \"upper\": {\"type\": \"integer\", \"digits\": \"1\"}}], \"camera\": {\"azimuth_degrees\": {\"type\": \"integer\", \"digits\": \"30\"}, \"elevation_degrees\": {\"type\": \"integer\", \"digits\": \"20\"}, \"projection\": {\"perspective\": {\"type\": \"integer\", \"digits\": \"45\"}}}}], \"parameters\": [{\"name\": \"k\", \"value\": {\"type\": \"rational\", \"numerator\": \"1\", \"denominator\": \"3\"}}]},";
    const READING: &str = "\"reading\": {\"source\": \"r1\", \"view\": {\"axes\": [{\"lower\": {\"type\": \"integer\", \"digits\": \"0\"}, \"upper\": {\"type\": \"integer\", \"digits\": \"4\"}}], \"camera\": null}, \"divisions\": [640], \"at\": [{\"type\": \"rational\", \"numerator\": \"3\", \"denominator\": \"4\"}], \"snapped\": true}";

    fn with_picture_and_reading() -> String {
        let text = example_session_version_3();
        let first_picture = "\"picture\": null,";
        assert!(text.contains(first_picture));
        let with_picture = text.replacen(first_picture, PICTURE, 1);
        let second_line_reading = "\"reading\": null\n    },\n    {\n      \"id\": \"r3\"";
        assert!(with_picture.contains(second_line_reading));
        with_picture.replacen(
            second_line_reading,
            &format!("{READING}\n    }},\n    {{\n      \"id\": \"r3\""),
            1,
        )
    }

    fn with_replaced_in_picture(original: &str, replacement: &str) -> String {
        let text = with_picture_and_reading();
        assert!(text.contains(original), "{original}");
        text.replacen(original, replacement, 1)
    }

    #[test]
    fn example_of_version_1_converts_to_version_12_with_empty_sources_and_no_pictures() {
        let saved = open(&example_session()).unwrap().save_to_bytes().unwrap();

        assert_eq!(
            String::from_utf8(saved).unwrap(),
            example_session_version_22()
        );
    }

    #[test]
    fn example_of_version_2_converts_to_version_12_with_no_pictures() {
        let saved = open(&example_session_version_2())
            .unwrap()
            .save_to_bytes()
            .unwrap();

        assert_eq!(
            String::from_utf8(saved).unwrap(),
            example_session_version_22()
        );
    }

    #[test]
    fn example_of_version_3_converts_to_version_12() {
        let saved = open(&example_session_version_3())
            .unwrap()
            .save_to_bytes()
            .unwrap();

        assert_eq!(
            String::from_utf8(saved).unwrap(),
            example_session_version_22()
        );
    }

    fn definition_line_text(version: u64, outcome: &str) -> String {
        format!(
            "{{\n  \"format\": \"calc-session\",\n  \"version\": {version},\n  \"settings\": {{\"precision\": \"f64\", \"backend\": \"automatic\", \"units\": null}},\n  \"next_line_number\": 2,\n  \"lines\": [\n    {{\n      \"id\": \"r1\",\n      \"name\": \"f\",\n      \"input\": \"f(x) = x^2\",\n      \"outcome\": {outcome},\n      \"picture\": null,\n      \"reading\": null\n    }}\n  ]\n}}\n"
        )
    }

    const DEFINITION_ERROR: &str = "{\"status\": \"error\", \"error\": {\"code\": \"function_definition_not_evaluated\", \"data\": {}}}";
    const DEFINITION_ERROR_WITH_DATA: &str = "{\"status\": \"error\", \"error\": {\"code\": \"function_definition_not_evaluated\", \"data\": {\"line\": \"r1\"}}}";

    #[test]
    fn a_version_9_definition_error_converts_to_the_defined_outcome() {
        let session = open(&definition_line_text(9, DEFINITION_ERROR)).unwrap();

        assert_eq!(
            session.lines().first().map(Line::outcome),
            Some(&Outcome::Defined)
        );
    }

    #[test]
    fn a_converted_definition_line_saves_as_the_current_version() {
        let session = open(&definition_line_text(9, DEFINITION_ERROR)).unwrap();

        let saved = String::from_utf8(session.save_to_bytes().unwrap()).unwrap();

        assert!(
            saved.contains(&format!("\"version\": {FORMAT_VERSION},"))
                && saved.contains("\"status\": \"defined\"")
        );
    }

    #[test]
    fn a_definition_error_carrying_data_stays_an_error() {
        let session = open(&definition_line_text(9, DEFINITION_ERROR_WITH_DATA)).unwrap();

        assert!(matches!(
            session.lines().first().map(Line::outcome),
            Some(Outcome::Error(error)) if error.code == "function_definition_not_evaluated"
        ));
    }

    #[test]
    fn a_defined_outcome_reads_back_as_it_was_written() {
        let text = definition_line_text(10, "{\"status\": \"defined\"}");

        let session = open(&text).unwrap();

        assert_eq!(
            session.lines().first().map(Line::outcome),
            Some(&Outcome::Defined)
        );
    }

    #[test]
    fn example_of_version_4_converts_to_version_12_without_an_override() {
        let session = open(&example_session_version_4()).unwrap();

        assert_eq!(
            (
                String::from_utf8(session.save_to_bytes().unwrap()).unwrap(),
                session.unit_override()
            ),
            (example_session_version_22(), None)
        );
    }

    const UNITS: &str = "\"units\": {\"system\": \"us-customary\", \"overrides\": [{\"kind\": \"speed\", \"unit\": \"km/h\"}, {\"kind\": \"temperature\", \"unit\": \"celsius\"}]}";

    fn with_units(units: &str) -> String {
        let text = example_session_version_5();
        assert!(text.contains("\"units\": null"));
        text.replacen("\"units\": null", units, 1)
    }

    #[test]
    fn unit_override_names_its_system() {
        let session = open(&with_units(UNITS)).unwrap();

        assert_eq!(
            session.unit_override().unwrap().system.as_deref(),
            Some("us-customary")
        );
    }

    #[test]
    fn unit_override_lists_its_kind_and_unit_pairs() {
        let session = open(&with_units(UNITS)).unwrap();

        assert_eq!(
            session.unit_override().unwrap().overrides,
            vec![
                KindUnit {
                    kind: "speed".to_owned(),
                    unit: "km/h".to_owned()
                },
                KindUnit {
                    kind: "temperature".to_owned(),
                    unit: "celsius".to_owned()
                }
            ]
        );
    }

    #[test]
    fn unit_override_without_a_system_is_read() {
        let text = with_units(&UNITS.replacen("\"us-customary\"", "null", 1));

        assert_eq!(open(&text).unwrap().unit_override().unwrap().system, None);
    }

    #[test]
    fn unit_override_survives_saving_and_opening() {
        let session = open(&with_units(UNITS)).unwrap();
        let reopened =
            open(std::str::from_utf8(&session.save_to_bytes().unwrap()).unwrap()).unwrap();

        assert_eq!(reopened.unit_override(), session.unit_override());
    }

    #[test]
    fn unresolved_system_and_unit_survive_saving_unchanged() {
        let unknown = UNITS.replacen("us-customary", "imperial-1824", 1).replacen(
            "km/h",
            "furlong/fortnight",
            1,
        );
        let session = open(&with_units(&unknown)).unwrap();
        let saved = String::from_utf8(session.save_to_bytes().unwrap()).unwrap();

        assert!(
            saved.contains("\"system\": \"imperial-1824\",")
                && saved.contains("\"unit\": \"furlong/fortnight\"")
        );
    }

    #[test]
    fn empty_unit_override_is_rejected() {
        let text = with_units("\"units\": {\"system\": null, \"overrides\": []}");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&["settings", "units"])))
        );
    }

    #[test]
    fn unit_overrides_out_of_kind_order_are_rejected() {
        let swapped = UNITS.replacen("\"speed\"", "\"zzz\"", 1);
        let text = with_units(&swapped);

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "settings",
                "units",
                "overrides",
                "1",
                "kind"
            ])))
        );
    }

    #[test]
    fn two_overrides_of_one_kind_are_rejected() {
        let twice = UNITS.replacen("\"temperature\"", "\"speed\"", 1);

        assert_eq!(
            open(&with_units(&twice)).err(),
            Some(LoadError::InvalidValue(path(&[
                "settings",
                "units",
                "overrides",
                "1",
                "kind"
            ])))
        );
    }

    #[test]
    fn unit_override_system_that_is_not_an_identifier_is_rejected() {
        let text = with_units(&UNITS.replacen("\"us-customary\"", "\"\"", 1));

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "settings", "units", "system"
            ])))
        );
    }

    #[test]
    fn unit_override_system_with_spaces_and_capitals_is_rejected() {
        let text = with_units(&UNITS.replacen("\"us-customary\"", "\"US customary\"", 1));

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "settings", "units", "system"
            ])))
        );
    }

    #[test]
    fn unit_override_kind_with_a_capital_is_rejected() {
        let text = with_units(&UNITS.replacen("\"speed\"", "\"Speed\"", 1));

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "settings",
                "units",
                "overrides",
                "0",
                "kind"
            ])))
        );
    }

    #[test]
    fn unit_override_kind_written_as_a_unit_is_rejected() {
        let text = with_units(&UNITS.replacen("\"speed\"", "\"km/h\"", 1));

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "settings",
                "units",
                "overrides",
                "0",
                "kind"
            ])))
        );
    }

    #[test]
    fn empty_unit_of_an_override_is_rejected() {
        let text = with_units(&UNITS.replacen("\"km/h\"", "\"\"", 1));

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "settings",
                "units",
                "overrides",
                "0",
                "unit"
            ])))
        );
    }

    #[test]
    fn new_session_has_no_unit_override() {
        let session = Session::new(fixed_clock(), Vec::new());

        assert_eq!(session.unit_override(), None);
    }

    #[test]
    fn version_7_picture_converts_with_a_null_iteration_limit() {
        let text = example_session_version_7().replacen(
            "\"picture\": null,",
            "\"picture\": {\"views\": [], \"parameters\": []},",
            1,
        );

        let opened = open(&text).unwrap();

        assert_eq!(opened.lines()[0].picture().unwrap().iteration_limit, None);
    }

    #[test]
    fn iteration_limit_of_a_line_that_is_not_an_escape_time_line_is_rejected() {
        let text = example_session_version_15().replacen(
            "\"picture\": null,",
            "\"picture\": {\"views\": [], \"parameters\": [], \"iteration_limit\": {\"rule\": \"fixed\", \"iterations\": 32}},",
            1,
        );

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "0",
                "picture",
                "iteration_limit"
            ])))
        );
    }

    #[test]
    fn escape_time_line_keeps_its_iteration_limit_through_a_round_trip() {
        let mut session = Session::new(fixed_clock(), vec![Box::new(CpuBackend::new())]);
        let line = session
            .enter(r#"escape_time {"form": "quadratic_parameter"}"#)
            .unwrap();
        session.set_line_picture_for_tests_with_limit(
            line,
            Some(calc_viz::IterationLimitRule::Fixed { iterations: 512 }),
        );
        let saved = String::from_utf8(session.save_to_bytes().unwrap()).unwrap();

        let opened = open(&saved).unwrap();

        assert_eq!(
            opened.lines()[0].picture().unwrap().iteration_limit,
            Some(calc_viz::IterationLimitRule::Fixed { iterations: 512 })
        );
    }

    #[test]
    fn iterations_beyond_the_largest_limit_are_rejected() {
        let text = example_session_version_15().replacen(
            "\"input\": \"2 + 2\"",
            "\"input\": \"escape_time {\\\"form\\\": \\\"quadratic_parameter\\\"}\"",
            1,
        ).replacen(
            "\"picture\": null,",
            "\"picture\": {\"views\": [], \"parameters\": [], \"iteration_limit\": {\"rule\": \"fixed\", \"iterations\": 65537}},",
            1,
        );

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "0",
                "picture",
                "iteration_limit",
                "iterations"
            ])))
        );
    }

    #[test]
    fn opened_escape_time_line_is_still_an_escape_time_line() {
        let mut session = Session::new(fixed_clock(), vec![Box::new(CpuBackend::new())]);
        let line = session
            .enter(r#"escape_time {"form": "quadratic_parameter"}"#)
            .unwrap();
        let saved = String::from_utf8(session.save_to_bytes().unwrap()).unwrap();

        let opened = open(&saved).unwrap();

        assert!(opened.escape_time_line_of(line).is_some());
    }

    #[test]
    fn escape_time_reading_line_keeps_its_orbit_through_a_round_trip() {
        let mut session = Session::new(fixed_clock(), vec![Box::new(CpuBackend::new())]);
        session
            .enter(
                r#"escape_time_reading {"form": "quadratic_parameter", "at": {"real": {"type": "integer", "digits": "2"}, "imaginary": {"type": "integer", "digits": "2"}}, "limit": 32}"#,
            )
            .unwrap();
        let saved = String::from_utf8(session.save_to_bytes().unwrap()).unwrap();

        let opened = open(&saved).unwrap();

        assert_eq!(
            opened.lines()[0].outcome(),
            &crate::session::Outcome::EscapeTimeReading(calc_viz::Orbit {
                class: calc_viz::ESCAPED_CELL,
                count: Some(2),
                limit: 32,
            })
        );
    }

    #[test]
    fn an_orbit_that_did_not_escape_with_a_count_is_rejected() {
        let text = example_session_version_15().replacen(
            "\"outcome\": {\n        \"status\": \"not_evaluated\"\n      }",
            "\"outcome\": {\n        \"status\": \"escape_time_reading\",\n        \"orbit\": {\"class\": \"inside\", \"count\": 3, \"limit\": 32}\n      }",
            1,
        );

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "0", "outcome", "orbit", "count"
            ])))
        );
    }

    #[test]
    fn converted_example_saves_to_the_same_bytes_again() {
        let text = example_session_version_22();

        let saved = open(&text).unwrap().save_to_bytes().unwrap();

        assert_eq!(String::from_utf8(saved).unwrap(), text);
    }

    #[test]
    fn expression_value_that_does_not_parse_is_rejected() {
        let json = crate::json::parse(b"{\"type\": \"expression\", \"text\": \"1 +\"}").unwrap();

        assert_eq!(
            value(&json, &path(&[])).err(),
            Some(LoadError::InvalidValue(path(&["text"])))
        );
    }

    #[test]
    fn version_3_line_without_its_picture_member_is_rejected() {
        let text = example_session_version_3().replacen("\"picture\": null,", "", 1);

        assert_eq!(
            open(&text).err(),
            Some(LoadError::MissingMember(path(&["lines", "0", "picture"])))
        );
    }

    fn version_6_with_picture_and_reading() -> String {
        String::from_utf8(
            open(&with_picture_and_reading())
                .unwrap()
                .save_to_bytes()
                .unwrap(),
        )
        .unwrap()
    }

    fn with_first_axis_unit(unit: &str) -> String {
        let text = version_6_with_picture_and_reading();
        assert!(text.contains("\"unit\": null"));
        text.replacen("\"unit\": null", &format!("\"unit\": {unit}"), 1)
    }

    fn first_axis_unit(text: &str) -> AxisDisplayUnit {
        open(text).unwrap().lines()[0].picture().unwrap().views[0].axes[0]
            .unit
            .clone()
    }

    #[test]
    fn version_5_picture_axis_converts_to_the_coherent_unit() {
        assert_eq!(
            first_axis_unit(&with_picture_and_reading()),
            AxisDisplayUnit::Coherent
        );
    }

    #[test]
    fn version_5_reading_view_axis_converts_to_the_coherent_unit() {
        let session = open(&with_picture_and_reading()).unwrap();

        assert_eq!(
            session.lines()[1].reading().unwrap().view.axes[0].unit,
            AxisDisplayUnit::Coherent
        );
    }

    #[test]
    fn coherent_axis_is_written_as_null() {
        assert!(version_6_with_picture_and_reading().contains("\"unit\": null\n"));
    }

    #[test]
    fn axis_in_a_unit_is_read_with_its_unit_text() {
        assert_eq!(
            first_axis_unit(&with_first_axis_unit("{\"unit\": \"in\"}")),
            AxisDisplayUnit::Unit("in".to_owned())
        );
    }

    #[test]
    fn axis_on_the_celsius_scale_is_read() {
        assert_eq!(
            first_axis_unit(&with_first_axis_unit("{\"scale\": \"celsius\"}")),
            AxisDisplayUnit::TemperatureScale(TemperatureScale::Celsius)
        );
    }

    #[test]
    fn axis_on_the_fahrenheit_scale_is_read() {
        assert_eq!(
            first_axis_unit(&with_first_axis_unit("{\"scale\": \"fahrenheit\"}")),
            AxisDisplayUnit::TemperatureScale(TemperatureScale::Fahrenheit)
        );
    }

    #[test]
    fn axis_unit_survives_saving_and_opening() {
        let session = open(&with_first_axis_unit("{\"unit\": \"deg\"}")).unwrap();
        let reopened =
            open(std::str::from_utf8(&session.save_to_bytes().unwrap()).unwrap()).unwrap();

        assert_eq!(reopened.lines(), session.lines());
    }

    #[test]
    fn unresolved_axis_unit_is_kept_unchanged() {
        assert_eq!(
            first_axis_unit(&with_first_axis_unit("{\"unit\": \"cubit\"}")),
            AxisDisplayUnit::Unit("cubit".to_owned())
        );
    }

    #[test]
    fn axis_on_an_unknown_scale_is_rejected() {
        assert_eq!(
            open(&with_first_axis_unit("{\"scale\": \"rankine\"}")).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "0", "picture", "views", "0", "axes", "0", "unit", "scale"
            ])))
        );
    }

    #[test]
    fn axis_on_the_kelvin_scale_is_rejected() {
        assert_eq!(
            open(&with_first_axis_unit("{\"scale\": \"kelvin\"}")).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "0", "picture", "views", "0", "axes", "0", "unit", "scale"
            ])))
        );
    }

    #[test]
    fn axis_with_an_empty_unit_is_rejected() {
        assert_eq!(
            open(&with_first_axis_unit("{\"unit\": \"\"}")).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "0", "picture", "views", "0", "axes", "0", "unit", "unit"
            ])))
        );
    }

    #[test]
    fn axis_unit_that_is_neither_null_nor_an_object_is_rejected() {
        assert_eq!(
            open(&with_first_axis_unit("\"in\"")).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "0", "picture", "views", "0", "axes", "0", "unit"
            ])))
        );
    }

    #[test]
    fn axis_unit_with_both_unit_and_scale_is_rejected() {
        assert!(
            open(&with_first_axis_unit(
                "{\"unit\": \"in\", \"scale\": \"celsius\"}"
            ))
            .is_err()
        );
    }

    #[test]
    fn picture_with_views_camera_and_parameters_round_trips() {
        let session = open(&with_picture_and_reading()).unwrap();
        let picture = session.lines()[0].picture().unwrap();
        let resaved =
            open(std::str::from_utf8(&session.save_to_bytes().unwrap()).unwrap()).unwrap();

        assert_eq!(picture.views.len(), 2);
        assert_eq!(
            picture.views[0].axes[0].upper,
            Number::fraction(&Integer::from(5_i64), &Integer::from(2_i64)).unwrap()
        );
        assert_eq!(
            picture.views[1].camera.as_ref().unwrap().projection,
            CameraProjection::Perspective {
                field_of_view_degrees: Number::from(45_i64)
            }
        );
        assert_eq!(picture.parameters[0].name, "k");
        assert_eq!(resaved.lines(), session.lines());
    }

    #[test]
    fn orthographic_camera_round_trips() {
        let text = with_replaced_in_picture(
            "\"projection\": {\"perspective\": {\"type\": \"integer\", \"digits\": \"45\"}}",
            "\"projection\": \"orthographic\"",
        );
        let session = open(&text).unwrap();

        assert_eq!(
            session.lines()[0].picture().unwrap().views[1]
                .camera
                .as_ref()
                .unwrap()
                .projection,
            CameraProjection::Orthographic
        );
    }

    #[test]
    fn reading_round_trips_with_its_source_view_divisions_and_coordinates() {
        let session = open(&with_picture_and_reading()).unwrap();
        let reading = session.lines()[1].reading().unwrap();
        let saved = String::from_utf8(session.save_to_bytes().unwrap()).unwrap();

        assert_eq!(
            (
                reading.source.number(),
                reading.divisions.clone(),
                reading.at.len(),
                reading.snapped
            ),
            (1, vec![640], 1, true)
        );
        assert_eq!(open(&saved).unwrap().lines(), session.lines());
    }

    #[test]
    fn reading_from_a_missing_line_is_rejected() {
        let text = with_replaced_in_picture("\"source\": \"r1\"", "\"source\": \"r9\"");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::MissingDependency(path(&[
                "lines", "1", "reading", "source"
            ])))
        );
    }

    #[test]
    fn reading_with_divisions_for_another_axis_count_is_rejected() {
        let text = with_replaced_in_picture("\"divisions\": [640]", "\"divisions\": [640, 480]");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "1",
                "reading",
                "divisions"
            ])))
        );
    }

    #[test]
    fn picture_axis_whose_lower_end_is_not_below_its_upper_end_is_rejected() {
        let text = with_replaced_in_picture(
            "\"upper\": {\"type\": \"rational\", \"numerator\": \"5\", \"denominator\": \"2\"}",
            "\"upper\": {\"type\": \"integer\", \"digits\": \"-2\"}",
        );

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "0", "picture", "views", "0", "axes", "0", "upper"
            ])))
        );
    }

    #[test]
    fn reading_coordinate_outside_its_view_axis_is_rejected() {
        let text = with_replaced_in_picture(
            "\"at\": [{\"type\": \"rational\", \"numerator\": \"3\", \"denominator\": \"4\"}]",
            "\"at\": [{\"type\": \"integer\", \"digits\": \"5\"}]",
        );

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "1", "reading", "at", "0"
            ])))
        );
    }

    #[test]
    fn reading_with_divisions_beyond_u32_is_rejected() {
        let text = with_replaced_in_picture("\"divisions\": [640]", "\"divisions\": [4294967296]");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "1",
                "reading",
                "divisions",
                "0"
            ])))
        );
    }

    #[test]
    fn line_reading_itself_is_a_dependency_cycle() {
        let text = with_replaced_in_picture("\"source\": \"r1\"", "\"source\": \"r2\"");

        assert!(matches!(
            open(&text).err(),
            Some(LoadError::DependencyCycle(_))
        ));
    }

    #[test]
    fn picture_value_that_is_a_machine_number_is_rejected() {
        let text = with_replaced_in_picture(
            "\"lower\": {\"type\": \"integer\", \"digits\": \"-2\"}",
            "\"lower\": {\"type\": \"f64\", \"bits\": \"0xc000000000000000\", \"decimal\": \"-2e0\"}",
        );

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "0", "picture", "views", "0", "axes", "0", "lower"
            ])))
        );
    }

    #[test]
    fn picture_with_a_repeated_parameter_name_is_rejected() {
        let text = with_replaced_in_picture(
            "{\"name\": \"k\", \"value\": {\"type\": \"rational\", \"numerator\": \"1\", \"denominator\": \"3\"}}",
            "{\"name\": \"k\", \"value\": {\"type\": \"integer\", \"digits\": \"1\"}}, {\"name\": \"k\", \"value\": {\"type\": \"integer\", \"digits\": \"2\"}}",
        );

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "0",
                "picture",
                "parameters",
                "1",
                "name"
            ])))
        );
    }

    #[test]
    fn version_1_record_that_already_has_sources_is_rejected() {
        let text = with_replaced("\"seed\": null,", "\"sources\": [],\n\"seed\": null,");

        assert!(matches!(
            open(&text).err(),
            Some(LoadError::DuplicateMember(_))
        ));
    }

    #[test]
    fn version_1_file_with_a_solve_outcome_is_rejected() {
        let text = with_replaced("\"status\": \"not_evaluated\"", "\"status\": \"ways\"");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "0", "outcome", "status"
            ])))
        );
    }

    #[test]
    fn unsorted_record_sources_are_rejected() {
        let text = example_session_version_2().replacen(
            "\"sources\": [],",
            "\"sources\": [\"b\", \"a\"],",
            1,
        );

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "2", "outcome", "record", "sources", "1"
            ])))
        );
    }

    #[test]
    fn example_record_keeps_value_bits_unit_and_uncertainty() {
        let session = open(&example_session()).unwrap();

        let Outcome::Result(record) = session.lines()[2].outcome() else {
            panic!("expected a result");
        };
        let computed = record.computed();
        assert_eq!(
            computed.value(),
            &ResultValue::Number(Number::F64(f64::from_bits(0x4038_8666_6666_6667)))
        );
        assert!(computed.unit().is_some());
        assert_eq!(
            computed.uncertainty().unwrap().coverage_factor(),
            &Number::from(1_i64)
        );
    }

    #[test]
    fn load_after_save_gives_equal_lines_and_settings() {
        let mut original = session();
        original.enter("mass = 2.5").unwrap();
        original.enter("mass * 9.81").unwrap();
        original.enter("undefined + 1").unwrap();
        original.enter("f(x) = x + 1").unwrap();
        original.set_precision(Precision::Exact);
        original.enter("exact_value = 1/3 + r2").unwrap();

        let bytes = original.save_to_bytes().unwrap();
        let loaded = open(std::str::from_utf8(&bytes).unwrap()).unwrap();

        assert_eq!(
            (loaded.lines(), loaded.settings()),
            (original.lines(), original.settings())
        );
    }

    #[test]
    fn a_sort_line_keeps_its_counts_through_save_and_open() {
        let mut original = session();
        original.enter("insertion_sort([3, 1, 2])").unwrap();

        let bytes = original.save_to_bytes().unwrap();
        let loaded = open(std::str::from_utf8(&bytes).unwrap()).unwrap();

        let sort = |session: &Session| match session.lines()[0].outcome() {
            Outcome::Result(record) => record.computed().sort().cloned(),
            _ => None,
        };
        assert_eq!(
            sort(&loaded),
            Some(SortRecord {
                from_positions: Some(vec![2, 3, 1]),
                comparisons: 3,
                writes: 4,
                key_evaluations: None,
                tallies: None,
                key_range: None,
                draws: None,
                flips: None,
            })
        );
    }

    #[test]
    fn a_version_20_sort_record_opens_without_flips() {
        let mut original = session();
        original.enter("pancake_sort([3, 1, 2])").unwrap();
        let saved = String::from_utf8(original.save_to_bytes().unwrap()).unwrap();
        let version_20 = saved
            .replacen("\"version\": 22", "\"version\": 20", 1)
            .replace(",\n            \"flips\": 2", "");
        assert!(!version_20.contains("flips"), "{version_20}");

        let opened = open(&version_20).expect("a version 20 file opens");

        let flips = match opened.lines()[0].outcome() {
            Outcome::Result(record) => record.computed().sort().map(|sort| sort.flips),
            _ => None,
        };
        assert_eq!(flips, Some(None));
    }

    #[test]
    fn a_bead_sort_record_saves_no_positions_and_opens_again() {
        let mut original = session();
        original.enter("bead_sort([3, 1, 2])").unwrap();
        let saved = String::from_utf8(original.save_to_bytes().unwrap()).unwrap();
        assert!(saved.contains("\"from_positions\": null"), "{saved}");

        let opened = open(&saved).expect("a bead sort record opens");

        let positions = match opened.lines()[0].outcome() {
            Outcome::Result(record) => record
                .computed()
                .sort()
                .map(|sort| sort.from_positions.clone()),
            _ => None,
        };
        assert_eq!(positions, Some(None));
    }

    #[test]
    fn a_version_21_sort_record_opens_with_its_positions() {
        let mut original = session();
        original.enter("insertion_sort([3, 1, 2])").unwrap();
        let saved = String::from_utf8(original.save_to_bytes().unwrap()).unwrap();
        let version_21 = saved.replacen("\"version\": 22", "\"version\": 21", 1);

        let opened = open(&version_21).expect("a version 21 file opens");

        let positions = match opened.lines()[0].outcome() {
            Outcome::Result(record) => record
                .computed()
                .sort()
                .map(|sort| sort.from_positions.clone()),
            _ => None,
        };
        assert_eq!(positions, Some(Some(vec![2, 3, 1])));
    }

    #[test]
    fn a_version_19_sort_record_opens_without_draws() {
        let mut original = session();
        original
            .enter("quick_sort([3, 1, 2], partition=lomuto, pivot=last)")
            .unwrap();
        let saved = String::from_utf8(original.save_to_bytes().unwrap()).unwrap();
        let version_19 = saved
            .replacen("\"version\": 22", "\"version\": 19", 1)
            .replace(",\n            \"draws\": null", "");
        assert!(!version_19.contains("draws"), "{version_19}");

        let opened = open(&version_19).expect("a version 19 file opens");

        let draws = match opened.lines()[0].outcome() {
            Outcome::Result(record) => record.computed().sort().map(|sort| sort.draws),
            _ => None,
        };
        assert_eq!(draws, Some(None));
    }

    #[test]
    fn a_version_18_counting_record_gains_one_decrement_per_entry() {
        let mut original = session();
        original.enter("counting_sort([3, 1, 2, 1])").unwrap();
        let saved = String::from_utf8(original.save_to_bytes().unwrap()).unwrap();
        let version_18 = saved
            .replacen("\"version\": 22", "\"version\": 18", 1)
            .replacen("\"tallies\": 10", "\"tallies\": 6", 1);
        assert!(version_18.contains("\"tallies\": 6"), "{version_18}");

        let opened = open(&version_18).expect("a version 18 file opens");

        let tallies = match opened.lines()[0].outcome() {
            Outcome::Result(record) => record.computed().sort().and_then(|sort| sort.tallies),
            _ => None,
        };
        assert_eq!(tallies, Some(10));
    }

    #[test]
    fn a_version_17_sort_record_opens_without_tallies() {
        let mut original = session();
        original.enter("insertion_sort([3, 1, 2])").unwrap();
        let saved = String::from_utf8(original.save_to_bytes().unwrap()).unwrap();
        let version_17 = saved
            .replacen("\"version\": 22", "\"version\": 17", 1)
            .replace(
                ",\n            \"tallies\": null,\n            \"key_range\": null",
                "",
            );
        assert!(!version_17.contains("tallies"), "{version_17}");

        let opened = open(&version_17).expect("a version 17 file opens");

        let sort = match opened.lines()[0].outcome() {
            Outcome::Result(record) => record.computed().sort().cloned(),
            _ => None,
        };
        assert_eq!(
            sort.map(|sort| (sort.comparisons, sort.tallies, sort.key_range)),
            Some((3, None, None))
        );
    }

    #[test]
    fn a_version_16_record_opens_without_a_sort() {
        let opened = open(&example_session_version_16()).expect("a version 16 file opens");

        let has_sort = opened.lines().iter().any(|line| {
            matches!(line.outcome(), Outcome::Result(record) if record.computed().sort().is_some())
        });
        assert!(!has_sort);
    }

    #[test]
    fn quantity_line_round_trips_with_its_unit() {
        let mut original = session();
        original.enter("force = 2.50 kg * 9.81 m/s^2").unwrap();
        original.enter("speed = 3 km / (2 s)").unwrap();
        let bytes = original.save_to_bytes().unwrap();

        let text = String::from_utf8(bytes.clone()).unwrap();
        let resaved = open(&text).unwrap().save_to_bytes().unwrap();

        assert!(text.contains("\"unit\": \"N\",\n"));
        assert!(text.contains("\"unit\": \"m*s^-1\",\n"));
        assert_eq!(resaved, bytes);
    }

    #[test]
    fn saving_a_loaded_session_gives_the_same_bytes() {
        let mut original = session();
        original.enter("x = 0.1").unwrap();
        original.enter("sin(x) + x^2").unwrap();
        let bytes = original.save_to_bytes().unwrap();

        let resaved = open(std::str::from_utf8(&bytes).unwrap())
            .unwrap()
            .save_to_bytes()
            .unwrap();

        assert_eq!(resaved, bytes);
    }

    #[test]
    fn loaded_session_recomputes_dependents_of_an_edited_line() {
        let mut original = session();
        let a = original.enter("a = 1").unwrap();
        let b = original.enter("b = a + 1").unwrap();
        let bytes = original.save_to_bytes().unwrap();
        let mut loaded = open(std::str::from_utf8(&bytes).unwrap()).unwrap();

        let recomputed = loaded.edit(a, "a = 5").unwrap();

        let Outcome::Result(record) = loaded.line(b).unwrap().outcome() else {
            panic!("expected a result");
        };
        assert_eq!(
            (recomputed, record.computed().value()),
            (vec![a, b], &ResultValue::Number(Number::from(6_i64)))
        );
    }

    #[test]
    fn nan_payload_and_negative_zero_keep_their_bits() {
        let payload = f64::from_bits(0x7ff8_0000_0000_0001);
        let value = Json::Array(vec![
            number_json(&Number::F64(payload)),
            number_json(&Number::F64(-0.0)),
        ]);
        let text = json::write_canonical(&value);

        let parsed = json::parse(text.as_bytes()).unwrap();
        let Json::Array(elements) = parsed else {
            panic!("expected an array");
        };
        let numbers: Vec<u64> = elements
            .iter()
            .map(
                |element| match number(element, &JsonPath::default()).unwrap() {
                    Number::F64(machine) => machine.to_bits(),
                    other => panic!("expected f64, found {other:?}"),
                },
            )
            .collect();

        assert_eq!(numbers, vec![0x7ff8_0000_0000_0001, (-0.0_f64).to_bits()]);
    }

    #[test]
    fn f32_value_round_trips() {
        let json = number_json(&Number::F32(3.5e-40));

        assert_eq!(
            number(&json, &JsonPath::default()),
            Ok(Number::F32(3.5e-40))
        );
    }

    #[test]
    fn large_integer_and_rational_round_trip() {
        let big = Integer::from(10_u64).pow(40);
        let fraction = Number::fraction(&big.negated(), &Integer::from(3_i64)).unwrap();
        let values = [Number::Integer(big), fraction];

        let decoded: Vec<Number> = values
            .iter()
            .map(|value| number(&number_json(value), &JsonPath::default()).unwrap())
            .collect();

        assert_eq!(decoded, values);
    }

    #[test]
    fn unknown_member_is_rejected_with_its_path() {
        let text = with_replaced(
            "\"approximate_operations\": false",
            "\"approximate_operations\": false,\n        \"colour\": \"red\"",
        );

        assert_eq!(
            open(&text).err(),
            Some(LoadError::UnknownMember(path(&[
                "lines", "2", "outcome", "record", "backend", "0", "colour"
            ])))
        );
    }

    #[test]
    fn missing_member_is_rejected_with_its_path() {
        let text = with_replaced("      \"seed\": null,\n", "");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::MissingMember(path(&[
                "lines", "2", "outcome", "record", "seed"
            ])))
        );
    }

    #[test]
    fn duplicate_member_is_rejected_with_its_path() {
        let text = with_replaced("\"seed\": null,", "\"seed\": null,\n      \"seed\": null,");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::DuplicateMember(path(&[
                "lines", "2", "outcome", "record", "seed"
            ])))
        );
    }

    fn example_session_version_11_with_derived_names() -> String {
        example_session_version_12()
            .replacen("\"version\": 12", "\"version\": 11", 1)
            .replacen(
                "\"modes\": []",
                "\"modes\": [{\"operation\": \"copy_sign\", \"mode\": \"native\"}]",
                1,
            )
            .replacen(
                "\"status\": \"not_evaluated\"",
                "\"status\": \"error\", \"error\": {\"code\": \"unsupported_operator\", \"data\": {\"operator\": \"copy_sign\"}}",
                1,
            )
            .replacen(
                "\"status\": \"not_evaluated\"",
                "\"status\": \"error\", \"error\": {\"code\": \"unsupported_operator\", \"data\": {\"operator\": \"to_exact\"}}",
                1,
            )
    }

    #[test]
    fn version_11_names_of_operations_are_renamed_to_their_call_names() {
        let text = example_session_version_11_with_derived_names();

        let saved = String::from_utf8(open(&text).unwrap().save_to_bytes().unwrap()).unwrap();

        assert!(saved.contains(&format!("\"version\": {FORMAT_VERSION},")));
        assert!(saved.contains("\"operator\": \"copysign\""));
        assert!(saved.contains("\"operator\": \"exact\""));
        assert!(saved.contains("\"operation\": \"copysign\""));
        assert!(!saved.contains("copy_sign"));
        assert!(!saved.contains("to_exact"));
    }

    fn version_8_example() -> String {
        example_session_version_7().replacen("\"version\": 7", "\"version\": 8", 1)
    }

    #[test]
    fn a_version_8_file_opens_and_gains_an_empty_mode_list() {
        let text = version_8_example();
        assert!(!text.contains("modes"));

        let opened = open(&text).expect("a version 8 file opens");

        let saved = String::from_utf8(opened.save_to_bytes().unwrap()).unwrap();
        assert_eq!(saved, example_session_version_22());
    }

    #[test]
    fn a_version_8_answer_way_record_gains_an_empty_mode_list() {
        let converted =
            convert_version_8(json::parse(version_8_answer_example().as_bytes()).expect("parse"));

        let text = json::write_one_line(&converted);
        assert_eq!(text.matches("\"modes\"").count(), 4, "{text}");
    }

    #[test]
    fn a_stored_record_that_ran_no_plan_converts_to_no_backend_at_all() {
        let exact = example_session_version_15().replacen(
            "\"selected\": \"cpu\"",
            "\"selected\": \"none\"",
            1,
        );

        let opened = open(&exact).expect("a version 15 file with an exact line opens");

        let records: Vec<&ResultRecord> = opened
            .lines()
            .iter()
            .filter_map(|line| match line.outcome() {
                Outcome::Result(record) => Some(record.as_ref()),
                _ => None,
            })
            .collect();
        assert_eq!(records.len(), 1);
        assert!(
            records[0].backend().is_empty(),
            "{:?}",
            records[0].backend()
        );
    }

    #[test]
    fn a_stored_record_that_ran_a_plan_converts_to_one_backend_with_its_width() {
        let opened = open(&example_session_version_15()).expect("a version 15 file opens");

        let Some(Outcome::Result(record)) = opened
            .lines()
            .iter()
            .map(|line| line.outcome())
            .find(|outcome| matches!(outcome, Outcome::Result(_)))
        else {
            panic!("expected a result line");
        };
        assert_eq!(
            record
                .backend()
                .iter()
                .map(|used| used.width)
                .collect::<Vec<Domain>>(),
            vec![Domain::F64]
        );
    }

    fn version_8_answer_example() -> String {
        String::from(
            r#"{"format": "calc-session", "version": 8, "lines": [{"outcome": {"status": "result", "record": {"backend": {"preference": "automatic", "selected": "cpu", "skipped": [], "approximate_operations": false}}}}, {"outcome": {"status": "ways", "answer": {"ways": [{"record": {"backend": {"preference": "automatic", "selected": "cpu", "skipped": [], "approximate_operations": false}}}, {"record": null}]}}}, {"outcome": {"status": "front", "answer": {"front": [{"record": {"backend": {"preference": "automatic", "selected": "cpu", "skipped": [], "approximate_operations": false}}}], "dominated": [{"record": {"backend": {"preference": "automatic", "selected": "cpu", "skipped": [], "approximate_operations": false}}}]}}}]}"#,
        )
    }

    #[test]
    fn modes_out_of_the_operation_order_are_rejected() {
        let text = example_session_version_15().replacen(
            "\"modes\": []",
            "\"modes\": [{\"operation\": \"ceil\", \"mode\": \"native\"}, {\"operation\": \"floor\", \"mode\": \"native\"}]",
            1,
        );

        let found = open(&text).err();

        assert!(matches!(found, Some(LoadError::InvalidValue(_))));
    }

    #[test]
    fn a_repeated_operation_in_modes_is_rejected() {
        let text = example_session_version_15().replacen(
            "\"modes\": []",
            "\"modes\": [{\"operation\": \"floor\", \"mode\": \"native\"}, {\"operation\": \"floor\", \"mode\": \"native\"}]",
            1,
        );

        let found = open(&text).err();

        assert!(matches!(found, Some(LoadError::InvalidValue(_))));
    }

    #[test]
    fn modes_in_the_operation_order_are_read() {
        let text = example_session_version_15().replacen(
            "\"modes\": []",
            "\"modes\": [{\"operation\": \"floor\", \"mode\": \"native\"}, {\"operation\": \"ceil\", \"mode\": \"integer_exact\"}]",
            1,
        );

        let saved = String::from_utf8(open(&text).unwrap().save_to_bytes().unwrap()).unwrap();

        let floor = saved.find("\"floor\"");
        let ceil = saved.find("\"ceil\"");
        assert!(floor < ceil && floor.is_some());
        assert_eq!(
            String::from_utf8(open(&saved).unwrap().save_to_bytes().unwrap()).unwrap(),
            saved
        );
    }

    const RECOGNIZED: &str = "{\"state\": \"ran\", \"concept_set_version\": \"0123456789abcdef\", \"limits\": {\"cap\": 5, \"work_budget\": 100000}, \"truncated\": false, \"truncated_by\": null, \"matches\": [{\"concept\": \"pythagoras\", \"pattern\": \"hypotenuse\", \"coverage\": {\"type\": \"rational\", \"numerator\": \"3\", \"denominator\": \"4\"}, \"site\": \"sqrt(x^2 + y^2)\", \"bindings\": [{\"variable\": \"x\", \"expression\": \"3\"}, {\"variable\": \"y\", \"expression\": \"4\"}], \"holds\": [true, null]}]}";

    fn with_recognized(text: &str) -> String {
        let original = "\"recognized\": {\n            \"state\": \"not_run\"\n          },";
        assert!(example_session_version_15().contains(original));
        example_session_version_15().replacen(original, &format!("\"recognized\": {text},"), 1)
    }

    #[test]
    fn a_version_10_record_gains_a_null_recognized_member() {
        let saved = open(&example_session_version_10())
            .unwrap()
            .save_to_bytes()
            .unwrap();

        assert_eq!(
            String::from_utf8(saved).unwrap(),
            example_session_version_22()
        );
    }

    #[test]
    fn a_recognized_member_reads_back_its_matches() {
        let session = open(&with_recognized(RECOGNIZED)).unwrap();

        let record = match session.lines()[2].outcome() {
            Outcome::Result(record) => record.clone(),
            other => panic!("expected a result, found {other:?}"),
        };

        assert_eq!(
            record.recognized().map(|recognized| (
                recognized.concept_set_version.clone(),
                recognized.matches.len(),
                recognized.matches[0].concept.clone(),
                recognized.matches[0].site.clone(),
                recognized.matches[0].holds.clone()
            )),
            Some((
                "0123456789abcdef".to_owned(),
                1,
                "pythagoras".to_owned(),
                "sqrt(x^2 + y^2)".to_owned(),
                vec![Some(true), None]
            ))
        );
    }

    #[test]
    fn a_recognized_member_saves_as_it_was_read() {
        let text = with_recognized(RECOGNIZED);

        let saved = open(&text).unwrap().save_to_bytes().unwrap();

        assert!(
            String::from_utf8(saved)
                .unwrap()
                .contains("\"concept_set_version\": \"0123456789abcdef\"")
        );
    }

    const VERSION_11_RECOGNIZED: &str =
        include_str!("../tests/sessions/version-11-recognized.calc");

    const VERSION_13_SKIPPED: &str = include_str!("../tests/sessions/version-13-skipped.calc");

    #[test]
    fn a_version_13_file_opens_and_saves_as_the_current_version() {
        let opened = open(VERSION_13_SKIPPED).expect("a version 13 file opens");

        let saved = String::from_utf8(opened.save_to_bytes().expect("saves")).unwrap();

        assert!(saved.contains(&format!("\"version\": {FORMAT_VERSION}")));
    }

    #[test]
    fn a_backend_skipped_because_its_device_refused_is_saved_and_read_again() {
        let text = VERSION_13_SKIPPED.replacen(
            "\"skipped\": [],",
            "\"skipped\": [{\"backend\": \"gpu\", \"reason\": \"prepare_failed\"}],",
            1,
        );

        let saved = String::from_utf8(
            open(&text.replacen("\"version\": 13", "\"version\": 14", 1))
                .expect("a version 14 file with the new reason opens")
                .save_to_bytes()
                .expect("saves"),
        )
        .unwrap();

        assert!(saved.contains("\"reason\": \"prepare_failed\""));
    }

    #[test]
    fn a_record_that_was_not_recognized_says_so() {
        let saved = String::from_utf8(
            open(&example_session_version_12())
                .unwrap()
                .save_to_bytes()
                .unwrap(),
        )
        .unwrap();

        assert!(saved.contains("\"state\": \"not_run\""));
    }

    #[test]
    fn a_recognition_that_could_not_run_names_its_reason() {
        let text =
            with_recognized("{\"state\": \"unavailable\", \"reason\": \"concept_set_not_loaded\"}");

        let session = open(&text).unwrap();
        let record = match session.lines()[2].outcome() {
            Outcome::Result(record) => record.clone(),
            other => panic!("expected a result, found {other:?}"),
        };

        assert_eq!(
            record.recognition(),
            &RecordRecognition::Unavailable(RecognitionUnavailable::ConceptSetNotLoaded)
        );
    }

    #[test]
    fn a_recognition_that_found_nothing_is_not_a_recognition_that_did_not_run() {
        let empty = with_recognized(
            "{\"state\": \"ran\", \"concept_set_version\": \"0123456789abcdef\", \"limits\": null, \"truncated\": null, \"truncated_by\": null, \"matches\": []}",
        );

        let session = open(&empty).unwrap();
        let record = match session.lines()[2].outcome() {
            Outcome::Result(record) => record.clone(),
            other => panic!("expected a result, found {other:?}"),
        };

        assert!(matches!(
            record.recognition(),
            RecordRecognition::Ran(recognized) if recognized.matches.is_empty()
        ));
    }

    #[test]
    fn an_unknown_recognition_state_is_rejected() {
        let text = with_recognized("{\"state\": \"maybe\"}");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "2",
                "outcome",
                "record",
                "recognized",
                "state"
            ])))
        );
    }

    #[test]
    fn a_version_11_recognition_gains_the_truncation_members() {
        let session = open(VERSION_11_RECOGNIZED).unwrap();

        let record = match session.lines()[0].outcome() {
            Outcome::Result(record) => record.clone(),
            other => panic!("expected a result, found {other:?}"),
        };

        assert_eq!(
            record.recognized().map(|recognized| (
                recognized.limits,
                recognized.truncated,
                recognized.truncated_by
            )),
            Some((None, None, None))
        );
    }

    #[test]
    fn a_version_11_recognition_keeps_its_matches() {
        let session = open(VERSION_11_RECOGNIZED).unwrap();

        let record = match session.lines()[0].outcome() {
            Outcome::Result(record) => record.clone(),
            other => panic!("expected a result, found {other:?}"),
        };

        assert_eq!(
            record
                .recognized()
                .map(|recognized| recognized.matches.len()),
            Some(1)
        );
    }

    #[test]
    fn a_converted_recognition_saves_with_its_truncation_members() {
        let saved = String::from_utf8(
            open(VERSION_11_RECOGNIZED)
                .unwrap()
                .save_to_bytes()
                .unwrap(),
        )
        .unwrap();

        assert!(
            saved.contains(&format!("\"version\": {FORMAT_VERSION},"))
                && saved.contains("\"limits\": null,")
                && saved.contains("\"truncated\": null,")
                && saved.contains("\"truncated_by\": null,")
        );
    }

    #[test]
    fn a_truncated_by_without_truncated_is_rejected() {
        let text = with_recognized(
            &RECOGNIZED.replace("\"truncated_by\": null", "\"truncated_by\": \"cap\""),
        );

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "2",
                "outcome",
                "record",
                "recognized",
                "truncated_by"
            ])))
        );
    }

    #[test]
    fn a_truncation_without_its_cause_is_rejected() {
        let text = with_recognized(&RECOGNIZED.replace(
            "\"truncated\": false, \"truncated_by\": null",
            "\"truncated\": true, \"truncated_by\": null",
        ));

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "2",
                "outcome",
                "record",
                "recognized",
                "truncated_by"
            ])))
        );
    }

    #[test]
    fn a_truncated_by_that_is_neither_limit_is_rejected() {
        let text = with_recognized(&RECOGNIZED.replace(
            "\"truncated\": false, \"truncated_by\": null",
            "\"truncated\": true, \"truncated_by\": \"time\"",
        ));

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "2",
                "outcome",
                "record",
                "recognized",
                "truncated_by"
            ])))
        );
    }

    #[test]
    fn a_cap_outside_its_range_is_rejected() {
        let text = with_recognized(&RECOGNIZED.replace("\"cap\": 5", "\"cap\": 101"));

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "2",
                "outcome",
                "record",
                "recognized",
                "limits",
                "cap"
            ])))
        );
    }

    #[test]
    fn a_work_budget_outside_its_range_is_rejected() {
        let text =
            with_recognized(&RECOGNIZED.replace("\"work_budget\": 100000", "\"work_budget\": 0"));

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "2",
                "outcome",
                "record",
                "recognized",
                "limits",
                "work_budget"
            ])))
        );
    }

    #[test]
    fn a_recognition_cut_short_by_the_cap_reads_back() {
        let text = with_recognized(&RECOGNIZED.replace(
            "\"truncated\": false, \"truncated_by\": null",
            "\"truncated\": true, \"truncated_by\": \"cap\"",
        ));

        let session = open(&text).unwrap();
        let record = match session.lines()[2].outcome() {
            Outcome::Result(record) => record.clone(),
            other => panic!("expected a result, found {other:?}"),
        };

        assert_eq!(
            record
                .recognized()
                .map(|recognized| (recognized.truncated, recognized.truncated_by)),
            Some((Some(true), Some(RecognitionTruncation::Cap)))
        );
    }

    #[test]
    fn a_coverage_above_one_is_rejected() {
        let text = with_recognized(&RECOGNIZED.replace(
            "\"numerator\": \"3\", \"denominator\": \"4\"",
            "\"numerator\": \"5\", \"denominator\": \"4\"",
        ));

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "2",
                "outcome",
                "record",
                "recognized",
                "matches",
                "0",
                "coverage"
            ])))
        );
    }

    #[test]
    fn a_coverage_of_zero_is_rejected() {
        let text = with_recognized(&RECOGNIZED.replace(
            "{\"type\": \"rational\", \"numerator\": \"3\", \"denominator\": \"4\"}",
            "{\"type\": \"integer\", \"digits\": \"0\"}",
        ));

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "2",
                "outcome",
                "record",
                "recognized",
                "matches",
                "0",
                "coverage"
            ])))
        );
    }

    #[test]
    fn a_coverage_of_one_is_read() {
        let text = with_recognized(&RECOGNIZED.replace(
            "{\"type\": \"rational\", \"numerator\": \"3\", \"denominator\": \"4\"}",
            "{\"type\": \"integer\", \"digits\": \"1\"}",
        ));

        assert!(open(&text).is_ok());
    }

    #[test]
    fn a_concept_set_version_that_is_not_sixteen_hex_digits_is_rejected() {
        let text = with_recognized(&RECOGNIZED.replace("0123456789abcdef", "0123"));

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "2",
                "outcome",
                "record",
                "recognized",
                "concept_set_version"
            ])))
        );
    }

    #[test]
    fn newer_version_is_rejected_naming_both_versions() {
        let text = with_replaced("\"version\": 1", "\"version\": 23");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::UnsupportedVersion {
                found: FORMAT_VERSION + 1,
                supported: FORMAT_VERSION
            })
        );
    }

    #[test]
    fn decimal_that_does_not_read_back_to_the_bits_is_rejected() {
        let text = with_replaced("\"2.4525000000000002e1\"", "\"2.4525e1\"");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::DecimalDoesNotMatchBits(path(&[
                "lines", "2", "outcome", "record", "value", "decimal"
            ])))
        );
    }

    #[test]
    fn value_outside_the_allowed_set_is_rejected() {
        let text = with_replaced("\"selected\": \"cpu\"", "\"selected\": \"quantum\"");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "2", "outcome", "record", "backend", "0", "selected"
            ])))
        );
    }

    #[test]
    fn dependency_on_a_missing_line_is_rejected() {
        let text = with_replaced("\"r2\"\n", "\"r2\",\n        \"r9\"\n");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::MissingDependency(path(&[
                "lines",
                "2",
                "outcome",
                "record",
                "dependencies",
                "2"
            ])))
        );
    }

    #[test]
    fn dependency_cycle_is_rejected() {
        let mut first = session();
        first.enter("a = 1").unwrap();
        first.enter("b = a").unwrap();
        let text = String::from_utf8(first.save_to_bytes().unwrap()).unwrap();
        let first_line_start = text.find("\"id\": \"r1\"").unwrap();
        let dependencies_at = first_line_start
            + text[first_line_start..]
                .find("\"dependencies\": []")
                .unwrap();
        let mut cyclic = text.clone();
        cyclic.replace_range(
            dependencies_at..dependencies_at + "\"dependencies\": []".len(),
            "\"dependencies\": [\"r2\"]",
        );

        assert!(matches!(
            open(&cyclic).err(),
            Some(LoadError::DependencyCycle(_))
        ));
    }

    #[test]
    fn duplicate_name_is_rejected() {
        let text = with_replaced("\"name\": \"a\"", "\"name\": \"m\"");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::DuplicateName(path(&["lines", "1", "name"])))
        );
    }

    #[test]
    fn name_that_differs_from_the_naming_statement_is_rejected() {
        let text = with_replaced("\"name\": \"F\"", "\"name\": \"G\"");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::NameDoesNotMatchInput(path(&[
                "lines", "2", "name"
            ])))
        );
    }

    #[test]
    fn line_number_not_below_next_line_number_is_rejected() {
        let text = with_replaced("\"next_line_number\": 4", "\"next_line_number\": 3");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::LineNumberNotBelowNext(path(&[
                "lines", "2", "id"
            ])))
        );
    }

    #[test]
    fn duplicate_line_id_is_rejected() {
        let text = with_replaced("\"id\": \"r2\"", "\"id\": \"r1\"");

        assert_eq!(
            open(&text).err(),
            Some(LoadError::DuplicateLineId(path(&["lines", "1", "id"])))
        );
    }

    #[test]
    fn rational_that_is_not_reduced_is_rejected() {
        let json = Json::object(vec![
            ("type", Json::string("rational")),
            ("numerator", Json::string("2")),
            ("denominator", Json::string("4")),
        ]);

        assert_eq!(
            number(&json, &JsonPath::default()),
            Err(LoadError::InvalidValue(path(&["denominator"])))
        );
    }

    #[test]
    fn machine_value_with_rounding_status_none_is_rejected() {
        let text = example_session();
        let start = text.find("\"rounding_error\": {").unwrap();
        let end = start + text[start..].find("\"uncertainty\"").unwrap();
        let mut changed = text.clone();
        changed.replace_range(start..end, "\"rounding_error\": {\"status\": \"none\"}, ");

        assert_eq!(
            open(&changed).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "2",
                "outcome",
                "record",
                "rounding_error",
                "status"
            ])))
        );
    }

    #[test]
    fn unsorted_corpus_references_are_rejected() {
        let text = example_session()
            .replacen("\"P-GRD-D-013\"", "\"placeholder\"", 1)
            .replacen("\"P-GRD-D-038\"", "\"P-GRD-D-013\"", 1)
            .replacen("\"placeholder\"", "\"P-GRD-D-038\"", 1);

        assert_eq!(
            open(&text).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "2",
                "outcome",
                "record",
                "corpus_references",
                "1"
            ])))
        );
    }

    #[test]
    fn syntax_error_is_reported() {
        assert_eq!(
            open("{").err(),
            Some(LoadError::Syntax(JsonSyntaxError::UnexpectedEnd))
        );
    }

    #[test]
    fn a_backend_skipped_by_a_failed_prepare_is_saved_as_prepare_failed() {
        let backend = BackendUse {
            preference: Preference::Automatic,
            width: Domain::F64,
            selected: Some(BackendKind::Cpu),
            skipped: vec![SkippedBackendUse {
                backend: BackendKind::Gpu,
                cause: SkipCause::PrepareFailed,
            }],
            approximate_operations: false,
            modes: Vec::new(),
        };

        let written = backend_json(&backend).expect("a skipped backend is written");

        assert!(json::write_one_line(&written).contains("\"prepare_failed\""));
    }

    #[test]
    fn every_optional_record_member_round_trips() {
        let mut pool = ExprPool::new();
        let newton = pool.units_mut().lookup("kN").unwrap();
        let mut method = Method::named("gauss_kronrod_adaptive");
        method.parameters.insert(
            "tolerance".to_string(),
            ParameterValue::Value(ResultValue::Number(Number::F64(1e-10))),
        );
        method
            .parameters
            .insert("adaptive".to_string(), ParameterValue::Boolean(true));
        method.convergence = Some(Convergence {
            status: ConvergenceStatus::IterationLimit,
            iterations: 7,
            error_estimate: Some(ResultValue::Number(Number::F64(2e-10))),
        });
        method.condition = Some(Condition {
            number: ResultValue::Number(Number::from(3_i64)),
            kind: ConditionKind::Relative,
        });
        method.notes.push(Diagnostic {
            code: "subnormal_intermediate".to_string(),
            data: BTreeMap::from([(
                "operator".to_string(),
                ParameterValue::Identifier("mul".to_string()),
            )]),
        });
        let value = ResultValue::Array {
            shape: vec![2],
            elements: vec![
                ResultValue::Number(Number::F64(1.5)),
                ResultValue::Number(Number::F64(-2.5)),
            ],
        };
        let computed = ComputedResult::new(ResultKind::MachineFloat, value, Some(newton), method)
            .unwrap()
            .with_rounding_bound(ResultValue::Number(Number::F64(1e-16)))
            .unwrap()
            .with_uncertainty(
                Uncertainty::new(
                    ResultValue::Number(Number::F64(0.25)),
                    Number::fraction(&Integer::from(5_i64), &Integer::from(2_i64)).unwrap(),
                )
                .unwrap(),
            )
            .with_corpus_reference("M-NUM-D-005")
            .with_seed(Seed {
                value: u64::MAX,
                generator: "xoshiro256starstar_1".to_string(),
            });
        let original = ResultRecord::new(
            computed,
            vec![BackendUse {
                preference: Preference::Only(BackendKind::Simd),
                width: Domain::F64,
                selected: None,
                skipped: vec![SkippedBackendUse {
                    backend: BackendKind::Simd,
                    cause: SkipCause::UnsupportedOperation,
                }],
                approximate_operations: true,
                modes: Vec::new(),
            }],
            vec![LineId::from_number(4).unwrap()],
            UtcTimestamp::from_milliseconds_since_unix_epoch(1_000),
            Duration::from_nanos(MAXIMUM_SAFE_COUNT),
            CalculatorVersion {
                major: 10,
                minor: 0,
                patch: 3,
            },
        );
        let line = LineId::from_number(5).unwrap();
        let json = record_json(&pool, &original, line).unwrap();
        let reparsed = json::parse(json::write_canonical(&json).as_bytes()).unwrap();

        let decoded = record(&mut pool, &reparsed, &JsonPath::default()).unwrap();

        assert_eq!(decoded, original);
    }

    #[test]
    fn line_label_needs_canonical_positive_number() {
        let labels = ["r1", "r01", "r0", "r", "x1", "r-1"];

        let parsed: Vec<Option<u64>> = labels
            .iter()
            .map(|label| line_from_label(label).map(LineId::number))
            .collect();

        assert_eq!(parsed, vec![Some(1), None, None, None, None, None]);
    }
}
