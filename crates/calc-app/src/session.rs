use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::picture::PictureOutline;
use calc_concepts::QuantityKind;
use calc_core::{
    ComputedResult, Diagnostic, EXACT_AFTER_CONVERSION_METHOD, EXACT_EVALUATION_METHOD,
    ExactEvaluationError, MachineEvaluation, MachineEvaluationError, Method, ParameterValue,
    QuantityError, RationalForm, ResultKind, ResultValue, RoundingError, UncertaintyError,
    central_expression, constant_expression, constant_of_bare_symbol, estimated_digits_of,
    evaluate_exact, evaluate_f32, evaluate_f64, has_uncertain_inputs, physical_constant,
    rational_form, to_coherent_units, with_propagated_uncertainty,
};
use calc_exec::{Backend, BackendKind, Domain, Preference};
use calc_expr::{
    BuiltinConstant, ExprId, ExprPool, Head, NodeView, Operator, SymbolError, SymbolId, SymbolKind,
    apply_lambda, bound_by_name, open_lambda_chain, substitute_symbols,
};
use calc_numbers::{Integer, Number};
use calc_syntax::{
    ParseError, ParseErrorKind, PrintMode, Statement, UnitEndedAtSpace, parse_expression,
    parse_statement_with_unit_endings, print_expression,
};
use calc_units::{BaseDimension, Dimension as UnitDimension, UnitId};
use calc_viz::{
    AxisBounds, AxisUnit, ColourLegend, Dimension, EscapeTimeRequest, IterationLimitRule,
    KindColour, Orbit, OrbitRequest, Reading, ResultId, SampleAxis, SampleError, SampleRequest,
    SampledShape,
};

use crate::claim::ClaimOutcome;
use crate::decimal_text::{integer_to_decimal, same_day};
use crate::display_units::{
    CurriculumUnits, DisplayDecision, DisplayQuestion, DisplaySources, UnitSystem, UnitsChoice,
    decide_display_unit,
};
use crate::displayed_value::pi_power_text;
use crate::escape_time_line::{
    EscapeTimeLine, EscapeTimeReadingLine, escape_time_from_line_text,
    escape_time_reading_from_line_text,
};
use crate::line_picture::{AxisDisplayUnit, LineReading, Picture, PictureCamera, PictureParameter};
use crate::messages::parse_error_kind_name;
use crate::picture::{
    AxisState, AxisTitle, CompletedScene, PictureAddress, PictureDefaults, PictureEvent,
    PictureShape, PictureSlot, PlotAxis, PlotError, PlotJob, PlotPlan, PlotRequest, PlotWork,
    SceneCompletion, SettleRequest, ViewState, default_views, outline_for, outline_legend,
    sample_request, stored_views,
};
use crate::platform::Clock;
use crate::platform::{Job, JobState};
use crate::readable::is_zero;
use crate::reading::{
    ReadCoordinate, ReadError, ReadEvent, ReadJob, ReadRequest, snapped_coordinate,
};
use crate::recognition::{
    RecognitionEvent, RecognitionJob, RecognitionState, RecognitionUnavailable, RecognitionWork,
    matcher_patterns,
};
use crate::replay::{ReplayReport, ReplayRun, compare_outcomes};
use crate::result_record::{
    BackendUse, CalculatorVersion, LineId, RecordRecognition, ResultRecord,
};
use crate::session_file::UNIT_VALUE_PREFIX;

const COHERENT_TEMPERATURE: &str = "K";
use crate::session_file::{
    self, JsonPath, LoadError, PathSegment, SaveError, kind_name, line_from_label, line_label,
};
use crate::solve::{
    EvaluationContext, SolveFailure, answer_reachable, answer_request, concept_set,
};
use crate::solve_answer::{ReachableAnswer, SolveAnswer};
use crate::solve_answer::{SolveCriterion, concept_set_version_text};
use crate::solve_request::{
    GivenInput, Phase, RequestError, RequestErrorCode, SolveRequest, request_from_line_text,
};
use crate::summary::ReplayState;
use crate::unit_display::display_text;
use crate::unit_override::{
    AppliedOverride, UnitOverrideError, apply_override, check_structure, display_systems,
};
use crate::views::ViewJobs;
use calc_core::RecognitionLimits;

type PlotVariables = Vec<(SymbolId, String)>;

const SOLVE_FAILED_CODE: &str = "solve_failed";
const ESCAPE_TIME_AXES: usize = 2;
const ORBIT_NOT_READ_CODE: &str = "orbit_not_read";
const ESCAPE_TIME_VALUE_DIVISIONS: u32 = 256;
const ESCAPE_TIME_SAMPLE_LIMIT: u64 = 64_000_000;
const UNCERTAINTY_NOT_PROPAGATED_CODE: &str = "uncertainty_not_propagated";
const NEGATIVE_UNCERTAINTY_CODE: &str = "negative_uncertainty";
const NESTED_UNCERTAINTY_CODE: &str = "nested_uncertainty";
const COVERAGE_FACTOR_BELOW_ONE_CODE: &str = "coverage_factor_below_one";
const LARGEST_WORK_BUDGET: u64 = 10_000_000;
const LARGEST_CAP: u32 = 1000;
type MachineEvaluator = fn(
    &mut ExprPool,
    ExprId,
    &[&dyn Backend],
    Preference,
) -> Result<MachineEvaluation, MachineEvaluationError>;

const NAME_DATA: &str = "name";

const UNDEFINED_NAME_CODE: &str = "undefined_name";

const REFERENCE_CYCLE_CODE: &str = "reference_cycle";
const CONSTANT_DATA: &str = "constant";
const LINE_DATA: &str = "line";
const FUNCTION_DATA: &str = "function";
const WRITTEN_DATA: &str = "written";
const FIRST_DATA: &str = "first";
const SORT_NOT_COMPARABLE: &str = "sort_not_comparable";
const SORT_RANGE_TOO_WIDE: &str = "sort_range_too_wide";
const RANGE_DATA: &str = "range";
const SEED_DATA: &str = "seed";
const STREAM_PARAMETER: &str = "stream";
const INDEX_PARAMETER: &str = "index";
const COMPARISONS_DATA: &str = "comparisons";
const WRITES_DATA: &str = "writes";
const SECOND_DATA: &str = "second";
const NARROW_DATA: &str = "narrow";
const WIDE_DATA: &str = "wide";
const AMBIGUOUS_APPLICATION_CODE: &str = "ambiguous_application";
const AMBIGUOUS_TEMPERATURE_SIGN_CODE: &str = "ambiguous_temperature_sign";
const SIGN_DATA: &str = "sign";
const SIDE_DATA: &str = "side";
const OPERATION_DATA: &str = "operation";
const METHOD_DATA: &str = "method";
const BEADS_DATA: &str = "beads";
const LENGTH_DATA: &str = "length";
const BELOW_DATA: &str = "below";
const ABOVE_DATA: &str = "above";
const DRAWS_DATA: &str = "draws";
const FIRST_POSITION_DATA: &str = "first_position";
const SECOND_POSITION_DATA: &str = "second_position";
const DIFFERENCE_DATA: &str = "difference";
const PARSE_ERROR_CODE: &str = "parse_error";
const ATTEMPT_DATA: &str = "attempt";
const REPLACEMENT_DATA: &str = "replacement";
const CORRECTED_DATA: &str = "corrected";
const COLUMN_DATA: &str = "column";
const RECOGNISED_ATTEMPT_CODE: &str = "recognised_attempt";
const UNIT_ENDED_AT_SPACE_ATTEMPT: &str = "unit_ended_at_space";
const KIND_DATA: &str = "kind";
const MAGNITUDE_DATA: &str = "magnitude";
const DIGITS_DATA: &str = "digits";

const COUNT_DATA: &str = "count";
const CONVERSION_DATA: &str = "conversion";
const TEMPERATURE_DIFFERENCE_NOTE: &str = "temperature_difference";
const TEMPERATURE_DIFFERENCE_ENTRY_NOTE: &str = "temperature_difference_entry";
const TEMPERATURE_DIFFERENCE_ENTRIES_NOTE: &str = "temperature_difference_entries";
const POSITION_DATA: &str = "position";
const TEMPERATURE_READING_NOTE: &str = "temperature_reading";
const SPACED_DIVISION: &str = " / ";
const WRITTEN_DIVISION: &str = "/";
const ENTRY_SEPARATOR: &str = ", ";
const ROW_SEPARATOR: &str = "; ";

pub fn note_describes_the_quantity(note: &Diagnostic) -> bool {
    matches!(
        note.code.as_str(),
        TEMPERATURE_DIFFERENCE_NOTE | TEMPERATURE_READING_NOTE
    )
}

pub(crate) const ZERO_TO_THE_ZERO_NOTE: &str = "zero_to_the_zero";
const VALID_WHERE_CODE: &str = "valid_where";
const DIVISION_BY_ZERO_CODE: &str = "division_by_zero";
pub(crate) const RELATION_IS_A_CLAIM_CODE: &str = "relation_is_a_claim";
const ORDER_NOT_REAL_CODE: &str = "order_not_real";
const EXCLUDING_DATA: &str = "excluding";
const KIND_MISMATCH_CODE: &str = "kind_mismatch";
const LEFT_KIND_DATA: &str = "left_kind";
const RIGHT_KIND_DATA: &str = "right_kind";
const DIFFERENCE_AS_READING_CODE: &str = "temperature_difference_as_reading";
const BELOW_ABSOLUTE_ZERO_CODE: &str = "below_absolute_zero";
const LIMIT_TEXT_DATA: &str = "limit";
const NUMBER_DATA: &str = "number";
const SCALE_DATA: &str = "scale";
const MACHINE_CONVERSION_NOTE: &str = "machine_conversion";
const CELSIUS_DIFFERENCE_SYMBOL: &str = "degC";
const FAHRENHEIT_DIFFERENCE_SYMBOL: &str = "degF";
const UNIT_SYMBOL_DATA: &str = "unit";
const OPERATOR_DATA_NAME: &str = "operator";
const READING_TEXT_DATA: &str = "reading";
const READING_SIGNIFICANT_DIGITS: u32 = 4;
const SPACE_VIEW_AXES: usize = 3;
const OPERATOR_DATA: &str = "operator";
const READING_DATA: &str = "reading";
const ARGUMENT_DATA: &str = "argument";
const LIMIT_DATA: &str = "limit";
const LIMIT_BITS_DATA: &str = "limit_bits";
const ESTIMATED_DIGITS_DATA: &str = "estimated_digits";
const RESULT_TOO_LARGE_CODE: &str = "result_too_large";
const NUMBER_TOO_LARGE_CODE: &str = "number_too_large";
const LIMIT_DIGITS_DATA: &str = "limit_digits";
const OPERAND_NAMES: &[&str] = &["operand", "exponent"];
const ROLE_DATA: &str = "role";
const EXPONENT_ROLE: &str = "exponent";
const ROOT_DEGREE_ROLE: &str = "root_degree";
const SCIENTIFIC_LEADING_ZEROS: usize = 5;
const INTERMEDIATE_STEP_TOO_LARGE_CODE: &str = "intermediate_step_too_large";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precision {
    Exact,
    F32,
    F64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub precision: Precision,
    pub backend: Preference,
}

struct PreparedReading {
    text: String,
    call: Option<String>,
    reading: LineReading,
}

type FurtherBackends = Box<dyn FnOnce() -> Vec<Box<dyn Backend>> + Send>;

#[derive(Clone, Debug, PartialEq)]
struct PicturePlanRecord {
    generation: u64,
    units: Vec<Vec<AxisDisplayUnit>>,
    cameras: Vec<Option<PictureCamera>>,
    parameters: Vec<PictureParameter>,
    iteration_limit: Option<IterationLimitRule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitOverride {
    pub system: Option<String>,
    pub overrides: Vec<KindUnit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KindUnit {
    pub kind: String,
    pub unit: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            precision: Precision::F64,
            backend: Preference::Automatic,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Result(Box<ResultRecord>),
    EscapeTimeReading(Orbit),
    Error(Diagnostic),
    NotEvaluated,
    Answer(Box<SolveAnswer>),
    Reachable(Box<ReachableAnswer>),
    Picture,
    Defined,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    id: LineId,
    name: Option<String>,
    input: String,
    outcome: Outcome,
    picture: Option<Box<Picture>>,
    reading: Option<Box<LineReading>>,
}

impl Line {
    pub(crate) fn restored(
        id: LineId,
        name: Option<String>,
        input: String,
        outcome: Outcome,
    ) -> Self {
        Self {
            id,
            name,
            input,
            outcome,
            picture: None,
            reading: None,
        }
    }

    pub(crate) fn with_picture_and_reading(
        mut self,
        picture: Option<Picture>,
        reading: Option<LineReading>,
    ) -> Self {
        self.picture = picture.map(Box::new);
        self.reading = reading.map(Box::new);
        self
    }

    pub fn picture(&self) -> Option<&Picture> {
        self.picture.as_deref()
    }

    pub fn reading(&self) -> Option<&LineReading> {
        self.reading.as_deref()
    }

    pub fn id(&self) -> LineId {
        self.id
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn outcome(&self) -> &Outcome {
        &self.outcome
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionError {
    Parse(ParseError),
    Symbol(SymbolError),
    NameExists(String),
    NameInUse {
        line: LineId,
        dependents: Vec<LineId>,
    },
    ReferenceCycle(LineId),
    UnknownLine(LineId),
    LineNumbersExhausted,
    InvalidRequest(RequestError),
    NotASolveLine(LineId),
    MachineLineNotApplicable(LineId),
    HoldsFreeNames {
        line: LineId,
        names: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoleMark {
    Obtainable,
    NotObtainable,
    Unmarked,
}

pub(crate) enum ExactOutcome {
    Rational(Box<(ComputedResult, Vec<BackendUse>)>),
    NeedsMachine(Diagnostic),
    Failed(Diagnostic),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatementKind {
    Expression,
    Naming,
    FunctionNaming,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnresolvedNames {
    Undefined,
    Free,
}

#[derive(Clone, Debug)]
struct Definition {
    kind: StatementKind,
    value: ExprId,
    references: Vec<String>,
    unit_endings: Vec<UnitEndedAtSpace>,
}

pub struct Session {
    settings: Settings,
    unit_override: Option<UnitOverride>,
    applied_override: Option<AppliedOverride>,
    unit_systems: Vec<UnitSystem>,
    next_line_number: u64,
    lines: Vec<Line>,
    rational_forms: HashMap<LineId, RationalForm>,
    view_jobs: ViewJobs,
    definitions: HashMap<LineId, Definition>,
    referencing_lines: Vec<LineId>,
    solve_requests: HashMap<LineId, SolveRequest>,
    escape_time_lines: HashMap<LineId, EscapeTimeLine>,
    escape_time_readings: HashMap<LineId, EscapeTimeReadingLine>,
    pool: ExprPool,
    clock: Arc<dyn Clock>,
    replay: Option<ReplayRun>,
    backends: Arc<Vec<Arc<dyn Backend>>>,
    further_backends: Option<FurtherBackends>,
    picture_cancellations: HashMap<(LineId, PictureSlot), Arc<AtomicBool>>,
    pending_pictures: HashMap<(LineId, PictureSlot), (PicturePlanRecord, SceneCompletion)>,
    completed_pictures: HashMap<(LineId, PictureSlot), (PicturePlanRecord, CompletedScene)>,
    reading_cancellations: HashMap<(LineId, PictureSlot), Arc<AtomicBool>>,
    latest_picture_generations: HashMap<(LineId, PictureSlot), u64>,
    next_picture_generation: u64,
    recognitions: HashMap<LineId, RecognitionEntry>,
    pending_recognition: Vec<LineId>,
    pending_recognition_set: HashSet<LineId>,
    next_recognition_generation: u64,
}

struct RecognitionEntry {
    generation: u64,
    state: RecognitionState,
    cancellation: Arc<AtomicBool>,
}

fn unexpected_member(member: &str) -> SessionError {
    SessionError::InvalidRequest(RequestError::new(
        RequestErrorCode::UnexpectedMember,
        JsonPath::default().member(member),
    ))
}

fn diagnostic(code: &str, data: Vec<(&str, ParameterValue)>) -> Diagnostic {
    Diagnostic {
        code: code.to_string(),
        data: data
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn uncertainty_diagnostic(pool: &ExprPool, error: &UncertaintyError) -> Diagnostic {
    let (code, expression) = match error {
        UncertaintyError::NegativeUncertainty(expression) => {
            (NEGATIVE_UNCERTAINTY_CODE, expression)
        }
        UncertaintyError::NestedUncertainty(expression) => (NESTED_UNCERTAINTY_CODE, expression),
        UncertaintyError::CoverageFactorBelowOne(expression) => {
            (COVERAGE_FACTOR_BELOW_ONE_CODE, expression)
        }
        UncertaintyError::Quantity(error) => return quantity_error_diagnostic(pool, *error),
        UncertaintyError::Access(_)
        | UncertaintyError::Build(_)
        | UncertaintyError::UnboundSymbol(_)
        | UncertaintyError::UnsupportedNode(_)
        | UncertaintyError::UnsupportedOperator(_)
        | UncertaintyError::NotFinite(_)
        | UncertaintyError::Result(_) => {
            return diagnostic(UNCERTAINTY_NOT_PROPAGATED_CODE, vec![]);
        }
    };
    read_diagnostic(pool, code, *expression, vec![])
}

fn unit_ended_at_space(
    input: &str,
    ending: &UnitEndedAtSpace,
    right_unit: &calc_syntax::RightUnit,
) -> Option<Diagnostic> {
    let replacement = [
        input.get(ending.unit.clone())?,
        input.get(ending.operator_span.clone())?,
        input.get(right_unit.unit.clone())?,
    ]
    .concat();
    let corrected = [
        input.get(..ending.unit.start)?,
        &replacement,
        input.get(right_unit.unit.end..)?,
    ]
    .concat();
    let column = input.get(..ending.unit.end)?.chars().count() + 1;
    let mut data = vec![
        (ATTEMPT_DATA, identifier(UNIT_ENDED_AT_SPACE_ATTEMPT)),
        (NAME_DATA, identifier(input.get(right_unit.name.clone())?)),
        (REPLACEMENT_DATA, identifier(&replacement)),
        (COLUMN_DATA, identifier(&column.to_string())),
    ];
    if parse_statement_with_unit_endings(&mut ExprPool::new(), &corrected).is_ok() {
        data.push((CORRECTED_DATA, identifier(&corrected)));
    }
    Some(diagnostic(RECOGNISED_ATTEMPT_CODE, data))
}

fn input_not_parsed(error: &ParseError, input: &str) -> Diagnostic {
    let column = input
        .get(..error.span.start)
        .map_or(1, |before| before.chars().count() + 1)
        .to_string();
    match error.kind {
        ParseErrorKind::AmbiguousTemperatureSign(sign) => diagnostic(
            AMBIGUOUS_TEMPERATURE_SIGN_CODE,
            vec![
                (SIGN_DATA, identifier(sign.sign())),
                (WRITTEN_DATA, identifier(&sign.written(input))),
                (COLUMN_DATA, identifier(&column)),
                (DIFFERENCE_DATA, identifier(&sign.difference(input))),
                (READING_DATA, identifier(&sign.reading(input))),
            ],
        ),
        ParseErrorKind::AmbiguousApplication(application) => diagnostic(
            AMBIGUOUS_APPLICATION_CODE,
            vec![
                (FUNCTION_DATA, identifier(&application.function(input))),
                (WRITTEN_DATA, identifier(&application.written(input))),
                (COLUMN_DATA, identifier(&column)),
                (NARROW_DATA, identifier(&application.narrow(input))),
                (WIDE_DATA, identifier(&application.wide(input))),
            ],
        ),
        ParseErrorKind::NestedTooDeeply { limit }
        | ParseErrorKind::ChainTooLong { limit }
        | ParseErrorKind::ExpressionTooDeep { limit } => diagnostic(
            PARSE_ERROR_CODE,
            vec![
                (KIND_DATA, identifier(parse_error_kind_name(&error.kind))),
                (COLUMN_DATA, identifier(&column)),
                (LIMIT_DATA, count_value(limit as u64)),
            ],
        ),
        _ => diagnostic(
            PARSE_ERROR_CODE,
            vec![
                (KIND_DATA, identifier(parse_error_kind_name(&error.kind))),
                (COLUMN_DATA, identifier(&column)),
            ],
        ),
    }
}

fn integer_form_data(problem: calc_core::IntegerProblem) -> Vec<(&'static str, ParameterValue)> {
    use calc_core::IntegerProblem;
    let type_name = |bits: u32, signed: bool| format!("{}{bits}", if signed { "i" } else { "u" });
    match problem {
        IntegerProblem::NotWhole => vec![("problem", identifier("not_whole"))],
        IntegerProblem::Negative => vec![("problem", identifier("negative"))],
        IntegerProblem::ShiftTooLarge => vec![
            ("problem", identifier("shift_too_large")),
            (
                "limit",
                identifier(&calc_expr::LARGEST_INTEGER_WIDTH.to_string()),
            ),
        ],
        IntegerProblem::NotWholeBytes { bits, signed } => vec![
            ("problem", identifier("not_whole_bytes")),
            ("type", identifier(&type_name(bits, signed))),
        ],
        IntegerProblem::OutsideType { bits, signed } => {
            let (low, high) = if signed {
                let half = Integer::one().shifted_left(bits.saturating_sub(1) as usize);
                (half.negated(), &half - &Integer::one())
            } else {
                (
                    Integer::zero(),
                    &Integer::one().shifted_left(bits as usize) - &Integer::one(),
                )
            };
            vec![
                ("problem", identifier("outside_type")),
                ("type", identifier(&type_name(bits, signed))),
                (
                    "low",
                    ParameterValue::Value(ResultValue::Number(Number::Integer(low))),
                ),
                (
                    "high",
                    ParameterValue::Value(ResultValue::Number(Number::Integer(high))),
                ),
            ]
        }
    }
}

fn identifier(text: &str) -> ParameterValue {
    ParameterValue::Identifier(text.to_string())
}

fn snake_case(name: &str) -> String {
    let mut text = String::new();
    for (position, character) in name.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if position > 0 {
                text.push('_');
            }
            text.push(character.to_ascii_lowercase());
        } else {
            text.push(character);
        }
    }
    text
}

fn operator_name(operator: Operator) -> String {
    calc_syntax::operator_call_name(operator)
        .map_or_else(|| snake_case(&format!("{operator:?}")), str::to_string)
}

fn operator_identifier(operator: Operator) -> ParameterValue {
    ParameterValue::Identifier(operator_name(operator))
}

fn power_operands(pool: &ExprPool, expression: ExprId) -> Option<(ExprId, ExprId)> {
    match pool.node(expression) {
        Ok(NodeView::Apply {
            head: Head::Operator(Operator::Pow),
            arguments: [base, exponent],
        }) => Some((*base, *exponent)),
        _ => None,
    }
}

fn scientific_decimal(text: &str) -> Option<String> {
    let (sign, unsigned) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text),
    };
    let fraction = unsigned.strip_prefix("0.")?;
    let digits = fraction.trim_start_matches('0');
    let leading_zeros = fraction.len() - digits.len();
    if leading_zeros < SCIENTIFIC_LEADING_ZEROS || digits.is_empty() {
        return None;
    }
    let (first, rest) = digits.split_at(1);
    let mantissa = if rest.is_empty() {
        first.to_owned()
    } else {
        format!("{first}.{rest}")
    };
    Some(format!("{sign}{mantissa}e-{}", leading_zeros + 1))
}

fn grouped_text(pool: &ExprPool, expression: ExprId) -> Option<String> {
    let Some((base, exponent)) = power_operands(pool, expression)
        .filter(|(_, exponent)| power_operands(pool, *exponent).is_some())
    else {
        let text = print_expression(pool, expression, PrintMode::Ascii).ok()?;
        return Some(match pool.node(expression) {
            Ok(NodeView::Number(_)) => scientific_decimal(&text).unwrap_or(text),
            _ => text,
        });
    };
    let base_text = print_expression(pool, base, PrintMode::Ascii).ok()?;
    let is_plain_base = matches!(
        pool.node(base),
        Ok(NodeView::Number(_) | NodeView::Symbol(_))
    ) && !base_text.starts_with('-');
    let base_text = if is_plain_base {
        base_text
    } else {
        format!("({base_text})")
    };
    Some(format!("{base_text}^({})", grouped_text(pool, exponent)?))
}

fn negative_operand_magnitude(
    pool: &mut ExprPool,
    root: ExprId,
    expression: ExprId,
    operator: Operator,
) -> Vec<(&'static str, ParameterValue)> {
    if !matches!(operator, Operator::Sqrt | Operator::Ln) {
        return Vec::new();
    }
    if carries_a_unit(pool, root) {
        return Vec::new();
    }
    let Ok(NodeView::Apply {
        arguments: [argument],
        ..
    }) = pool.node(expression)
    else {
        return Vec::new();
    };
    let argument = *argument;
    let Ok(evaluation) = evaluate_exact(pool, argument) else {
        return Vec::new();
    };
    let Some(value) = evaluation.rational_value() else {
        return Vec::new();
    };
    if !is_below_zero(value) {
        return Vec::new();
    }
    let Ok(magnitude) = value.negate_exact() else {
        return Vec::new();
    };
    let form = rational_form(pool, argument, None);
    vec![(
        MAGNITUDE_DATA,
        ParameterValue::Value(ResultValue::Expression(crate::summary::value_text_in(
            &ResultValue::Number(magnitude),
            form,
        ))),
    )]
}

fn carries_a_unit(pool: &ExprPool, root: ExprId) -> bool {
    let mut pending = vec![root];
    let mut visited = HashSet::new();
    while let Some(expression) = pending.pop() {
        if !visited.insert(expression) {
            continue;
        }
        match pool.node(expression) {
            Ok(NodeView::Quantity { .. }) => return true,
            Ok(NodeView::Apply { arguments, .. }) => pending.extend_from_slice(arguments),
            Ok(NodeView::Bind {
                arguments, body, ..
            }) => {
                pending.extend_from_slice(arguments);
                pending.push(body);
            }
            Ok(NodeView::Array { elements, .. }) => pending.extend_from_slice(elements),
            Ok(NodeView::Number(_) | NodeView::Symbol(_) | NodeView::Bound(_)) | Err(_) => {}
        }
    }
    false
}

fn is_below_zero(number: &Number) -> bool {
    match number {
        Number::Integer(integer) => integer.is_negative(),
        Number::Rational(rational) => rational.numerator().is_negative(),
        Number::F32(value) => *value < 0.0,
        Number::F64(value) => *value < 0.0,
    }
}

fn operand_readings(
    pool: &ExprPool,
    expression: ExprId,
    names: &[&'static str],
) -> Vec<(&'static str, ParameterValue)> {
    let Ok(NodeView::Apply { arguments, .. }) = pool.node(expression) else {
        return Vec::new();
    };
    names
        .iter()
        .zip(arguments)
        .filter_map(|(name, argument)| {
            grouped_text(pool, *argument)
                .map(|text| (*name, ParameterValue::Value(ResultValue::Expression(text))))
        })
        .collect()
}

fn power_argument_role(pool: &mut ExprPool, power: ExprId, argument: &Integer) -> &'static str {
    let Some((_, exponent)) = power_operands(pool, power) else {
        return ROOT_DEGREE_ROLE;
    };
    let is_exponent = evaluate_exact(pool, exponent).is_ok_and(|evaluation| {
        evaluation.rational_value() == Some(&Number::Integer(argument.clone()))
    });
    if is_exponent {
        EXPONENT_ROLE
    } else {
        ROOT_DEGREE_ROLE
    }
}

fn reading(pool: &ExprPool, expression: ExprId) -> Option<(&'static str, ParameterValue)> {
    grouped_text(pool, expression).map(|text| {
        (
            READING_DATA,
            ParameterValue::Value(ResultValue::Expression(text)),
        )
    })
}

fn count_value(count: u64) -> ParameterValue {
    ParameterValue::Value(ResultValue::Number(Number::Integer(Integer::from(count))))
}

fn relation_is_a_claim(pool: &ExprPool, expression: ExprId, free_names: &[String]) -> Diagnostic {
    let data = if free_names.is_empty() {
        vec![]
    } else {
        vec![(NAMES_DATA, identifier(&free_names.join(", ")))]
    };
    read_diagnostic(pool, RELATION_IS_A_CLAIM_CODE, expression, data)
}

fn read_diagnostic(
    pool: &ExprPool,
    code: &str,
    expression: ExprId,
    mut data: Vec<(&str, ParameterValue)>,
) -> Diagnostic {
    data.extend(reading(pool, expression));
    diagnostic(code, data)
}

fn exact_error_diagnostic(
    pool: &mut ExprPool,
    root: ExprId,
    error: ExactEvaluationError,
) -> Diagnostic {
    match error {
        ExactEvaluationError::Access(_) => diagnostic("expression_not_in_pool", vec![]),
        ExactEvaluationError::Build(_) => diagnostic("expression_pool_full", vec![]),
        ExactEvaluationError::MachineNumber(expression) => read_diagnostic(
            pool,
            "machine_number_in_exact_evaluation",
            expression,
            vec![],
        ),
        ExactEvaluationError::NotFinite(expression) => {
            read_diagnostic(pool, "not_finite", expression, vec![])
        }
        ExactEvaluationError::Quantity(error) => quantity_error_diagnostic(pool, error),
        ExactEvaluationError::UnsupportedNode(expression) => {
            read_diagnostic(pool, "unsupported_subexpression", expression, vec![])
        }
        ExactEvaluationError::CannotDecide { expression, reason } => read_diagnostic(
            pool,
            reason.code(),
            expression,
            reason
                .side_name()
                .map(|side| (SIDE_DATA, identifier(side)))
                .into_iter()
                .chain(
                    reason
                        .digits()
                        .map(|digits| (DIGITS_DATA, identifier(&digits.to_string()))),
                )
                .chain(
                    reason
                        .count()
                        .map(|count| (COUNT_DATA, identifier(&count.to_string()))),
                )
                .collect(),
        ),
        ExactEvaluationError::UnsupportedOperator {
            expression,
            operator,
        } => read_diagnostic(
            pool,
            "unsupported_operator",
            expression,
            vec![(OPERATOR_DATA, operator_identifier(operator))],
        ),
        ExactEvaluationError::UnsupportedConstant(expression) => {
            read_diagnostic(pool, "unsupported_constant", expression, vec![])
        }
        ExactEvaluationError::DivisionByZero(expression) => {
            read_diagnostic(pool, "division_by_zero", expression, vec![])
        }
        ExactEvaluationError::OutsideDomain {
            expression,
            operator,
        } => {
            let mut data = vec![(OPERATOR_DATA, operator_identifier(operator))];
            data.extend(operand_readings(pool, expression, OPERAND_NAMES));
            data.extend(negative_operand_magnitude(pool, root, expression, operator));
            read_diagnostic(pool, "outside_domain", expression, data)
        }
        ExactEvaluationError::IntegerForm {
            expression,
            problem,
        } => read_diagnostic(pool, "integer_form", expression, integer_form_data(problem)),
        ExactEvaluationError::RemainderNotComputed { expression } => {
            read_diagnostic(pool, "remainder_not_computed", expression, vec![])
        }
        ExactEvaluationError::PowerOfZeroWithoutValue { expression } => {
            read_diagnostic(pool, "power_of_zero_without_value", expression, vec![])
        }
        ExactEvaluationError::PowerOfZeroSignUndecided { expression } => {
            read_diagnostic(pool, "power_of_zero_sign_undecided", expression, vec![])
        }
        ExactEvaluationError::NotInRadicalField {
            expression,
            operator,
        } => {
            let value = match pool.node(expression) {
                Ok(NodeView::Apply { arguments, .. }) => arguments.first().copied(),
                _ => None,
            }
            .unwrap_or(expression);
            read_diagnostic(
                pool,
                "not_in_radical_field",
                value,
                vec![(OPERATOR_DATA, operator_identifier(operator))],
            )
        }
        ExactEvaluationError::NotASquareRootTerm { term, radicand } => read_diagnostic(
            pool,
            "not_a_square_root_term",
            term,
            radicand
                .map(|radicand| {
                    (
                        ARGUMENT_DATA,
                        ParameterValue::Value(ResultValue::Number(Number::Integer(radicand))),
                    )
                })
                .into_iter()
                .collect(),
        ),
        ExactEvaluationError::IntegerArgumentOutOfRange {
            expression,
            operator,
            argument,
            limit,
        } => {
            let mut data = vec![
                (OPERATOR_DATA, operator_identifier(operator)),
                (
                    ARGUMENT_DATA,
                    ParameterValue::Value(ResultValue::Number(Number::Integer(argument.clone()))),
                ),
                (LIMIT_DATA, count_value(limit)),
            ];
            data.extend(operand_readings(pool, expression, OPERAND_NAMES));
            if operator == Operator::Pow {
                data.push((
                    ROLE_DATA,
                    identifier(power_argument_role(pool, expression, &argument)),
                ));
            }
            read_diagnostic(pool, "integer_argument_out_of_range", expression, data)
        }
        ExactEvaluationError::ResultTooLarge {
            expression,
            limit_bits,
            estimated_digits,
        } => read_diagnostic(
            pool,
            if matches!(pool.node(expression), Ok(NodeView::Number(_))) {
                NUMBER_TOO_LARGE_CODE
            } else if expression == root {
                RESULT_TOO_LARGE_CODE
            } else {
                INTERMEDIATE_STEP_TOO_LARGE_CODE
            },
            expression,
            vec![
                (LIMIT_BITS_DATA, count_value(limit_bits)),
                (
                    LIMIT_DIGITS_DATA,
                    count_value(estimated_digits_of(limit_bits)),
                ),
                (ESTIMATED_DIGITS_DATA, count_value(estimated_digits)),
            ],
        ),
    }
}

fn machine_error_diagnostic(pool: &ExprPool, error: &MachineEvaluationError) -> Diagnostic {
    let code = match error {
        MachineEvaluationError::Access(_) => "expression_not_in_pool",
        MachineEvaluationError::Lower(_) => "not_lowerable",
        MachineEvaluationError::IndexRangeTooLong(expression) => {
            return read_diagnostic(pool, "index_range_too_long", *expression, vec![]);
        }
        MachineEvaluationError::Batch(_) => "batch_not_built",
        MachineEvaluationError::Select(_) => "no_backend_selected",
        MachineEvaluationError::Run(_) => "backend_run_failed",
        MachineEvaluationError::Result(_) => "result_not_representable",
        MachineEvaluationError::Quantity(error) => return quantity_error_diagnostic(pool, *error),
    };
    diagnostic(code, vec![])
}

pub(crate) fn unit_refusal(pool: &mut ExprPool, expression: ExprId) -> Option<Diagnostic> {
    if !calc_core::is_relation(pool, expression) {
        return None;
    }
    match calc_core::to_coherent_units(pool, expression) {
        Err(error @ QuantityError::DimensionMismatch { .. }) => {
            Some(quantity_error_diagnostic(pool, error))
        }
        _ => None,
    }
}

fn order_of_a_non_real(pool: &mut ExprPool, expression: ExprId) -> Option<Diagnostic> {
    let NodeView::Apply {
        head: Head::Operator(operator),
        arguments: [left, right],
    } = pool.node(expression).ok()?
    else {
        return None;
    };
    let is_order = matches!(
        operator,
        Operator::Less | Operator::LessOrEqual | Operator::Greater | Operator::GreaterOrEqual
    );
    if !is_order {
        return None;
    }
    let (left, right) = (*left, *right);
    [left, right].into_iter().find_map(|side| {
        let is_non_real = matches!(
            calc_core::exact_complex(pool, side),
            Some(Ok(calc_core::ExactComplexValue::Rational { .. }
                | calc_core::ExactComplexValue::Algebraic(_)))
        );
        let written = calc_syntax::print_expression(pool, side, calc_syntax::PrintMode::Ascii).ok();
        is_non_real.then(|| {
            diagnostic(
                ORDER_NOT_REAL_CODE,
                vec![(SIDE_DATA, identifier(&written.unwrap_or_default()))],
            )
        })
    })
}

fn quantity_error_diagnostic(pool: &ExprPool, error: QuantityError) -> Diagnostic {
    let (code, expression) = match error {
        QuantityError::Access(_) => return diagnostic("expression_not_in_pool", vec![]),
        QuantityError::Build(_) => return diagnostic("expression_pool_full", vec![]),
        QuantityError::UnitAccess(_) => return diagnostic("unit_not_in_table", vec![]),
        QuantityError::UnitProduct(_) => return diagnostic("unit_not_representable", vec![]),
        QuantityError::DimensionMismatch {
            expression,
            left,
            right,
        } => {
            let data = crate::dimension_mismatch::mismatch_data(pool, expression, &left, &right);
            return read_diagnostic(pool, "dimension_mismatch", expression, data);
        }
        QuantityError::DimensionedArgument { expression, .. } => {
            ("dimensioned_argument", expression)
        }
        QuantityError::ExponentNotConstant(expression) => {
            ("dimensioned_power_exponent_not_constant", expression)
        }
        QuantityError::FractionalDimension(expression) => ("fractional_dimension", expression),
        QuantityError::DimensionOutOfRange(expression) => ("dimension_out_of_range", expression),
        QuantityError::DifferenceAsReading {
            expression,
            difference,
        } => return difference_as_reading_diagnostic(pool, expression, difference),
        QuantityError::KindMismatch {
            expression,
            left,
            right,
        } => {
            return read_diagnostic(
                pool,
                KIND_MISMATCH_CODE,
                expression,
                vec![
                    (LEFT_KIND_DATA, identifier(left)),
                    (RIGHT_KIND_DATA, identifier(right)),
                ],
            );
        }
        QuantityError::BelowAbsoluteZero(expression) => {
            let limit = absolute_zero_text(pool, expression);
            return read_diagnostic(
                pool,
                BELOW_ABSOLUTE_ZERO_CODE,
                expression,
                vec![(LIMIT_TEXT_DATA, identifier(limit))],
            );
        }
    };
    read_diagnostic(pool, code, expression, vec![])
}

fn add_reference(
    pool: &ExprPool,
    names: &mut Vec<String>,
    symbol: SymbolId,
) -> Result<(), SessionError> {
    let name = pool.symbol_name(symbol).map_err(SessionError::Symbol)?;
    if !names.iter().any(|known| known == name) {
        names.push(name.to_string());
    }
    Ok(())
}

const ANTIDERIVATIVE_CODE: &str = "antiderivative";

const TAYLOR_POLYNOMIAL_CODE: &str = "taylor_polynomial";

pub(crate) const WORST_CASE_METHOD: &str = "worst_case";

const CHEMISTRY_COMPOSITION_METHOD: &str = "chemistry_composition";

const CHEMISTRY_BALANCE_METHOD: &str = "chemistry_balance";

const CHEMISTRY_CHECK_METHOD: &str = "chemistry_check";

const NUCLEAR_NUCLIDE_METHOD: &str = "nuclear_nuclide";

const NUCLEAR_CHECK_METHOD: &str = "nuclear_check";

fn chemistry_outside_a_property(
    pool: &ExprPool,
    expression: ExprId,
) -> Option<(ExprId, &'static str)> {
    let mut pending = vec![expression];
    while let Some(current) = pending.pop() {
        match pool.node(current) {
            Ok(NodeView::Apply {
                head: Head::Operator(Operator::MolarMass | Operator::QValue),
                ..
            }) => {}
            Ok(NodeView::Apply {
                head: Head::Operator(Operator::Substance),
                ..
            }) => return Some((current, "chemistry_substance_not_a_number")),
            Ok(NodeView::Apply {
                head: Head::Operator(Operator::Reaction),
                ..
            }) => return Some((current, "chemistry_reaction_alone")),
            Ok(NodeView::Apply {
                head: Head::Operator(Operator::Nuclide),
                ..
            }) => return Some((current, "nuclear_nuclide_not_a_number")),
            Ok(NodeView::Apply {
                head: Head::Operator(Operator::NuclearReaction),
                ..
            }) => return Some((current, "nuclear_reaction_alone")),
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
    None
}

fn contains_molar_mass(pool: &ExprPool, expression: ExprId) -> bool {
    let mut pending = vec![expression];
    while let Some(current) = pending.pop() {
        match pool.node(current) {
            Ok(NodeView::Apply {
                head: Head::Operator(Operator::MolarMass | Operator::QValue),
                ..
            }) => return true,
            Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().copied()),
            Ok(NodeView::Quantity { value, .. }) => pending.push(value),
            _ => {}
        }
    }
    false
}

fn charge_text(charge: i64) -> String {
    let sign = if charge < 0 { '-' } else { '+' };
    match charge.unsigned_abs() {
        1 => sign.to_string(),
        magnitude => format!("{magnitude}{sign}"),
    }
}

fn composition_text(elements: &[(u8, u64)]) -> String {
    elements
        .iter()
        .map(|(element, count)| {
            format!(
                "{count} {}",
                calc_syntax::element_symbol(*element).unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn reaction_text(
    texts: &[String],
    species: &[calc_core::ReactionSpecies],
    coefficients: &[Integer],
) -> String {
    let sides: Vec<bool> = species.iter().map(|one| one.is_product).collect();
    reaction_line(texts, &sides, coefficients)
}

fn reaction_line(texts: &[String], sides: &[bool], coefficients: &[Integer]) -> String {
    let side = |products: bool| {
        texts
            .iter()
            .zip(sides)
            .zip(coefficients)
            .filter(|((_, is_product), coefficient)| {
                **is_product == products && !coefficient.is_zero()
            })
            .map(|((text, _), coefficient)| {
                if coefficient == &Integer::one() {
                    text.clone()
                } else {
                    format!(
                        "{} {text}",
                        crate::decimal_text::integer_to_decimal(coefficient)
                    )
                }
            })
            .collect::<Vec<_>>()
            .join(" + ")
    };
    format!("{} -> {}", side(false), side(true))
}

fn balance_notes(totals: &[calc_core::SideTotals]) -> Vec<Diagnostic> {
    let mut notes = Vec::new();
    let elements: Vec<String> = totals
        .iter()
        .filter_map(|side| match side.quantity {
            calc_core::ConservedQuantity::Element(element) => Some(format!(
                "{} {}",
                calc_syntax::element_symbol(element).unwrap_or_default(),
                crate::decimal_text::integer_to_decimal(&side.reactants)
            )),
            _ => None,
        })
        .collect();
    if !elements.is_empty() {
        notes.push(diagnostic(
            "chemistry_each_side",
            vec![("counts", identifier(&elements.join(", ")))],
        ));
    }
    if let Some(side) = totals
        .iter()
        .find(|side| side.quantity == calc_core::ConservedQuantity::Charge)
    {
        let charge = side.reactants.to_i64().map_or_else(
            || crate::decimal_text::integer_to_decimal(&side.reactants),
            |charge| {
                if charge == 0 {
                    "0".to_owned()
                } else {
                    charge_text(charge)
                }
            },
        );
        notes.push(diagnostic(
            "chemistry_charge_each_side",
            vec![("charge", identifier(&charge))],
        ));
    }
    notes
}

fn nuclide_text(particle: calc_expr::NuclearParticle) -> String {
    match particle {
        calc_expr::NuclearParticle::Nuclide {
            mass_number,
            atomic_number,
        } => {
            let symbol = u8::try_from(atomic_number)
                .ok()
                .and_then(calc_syntax::element_symbol)
                .unwrap_or_default();
            format!("^{mass_number}{symbol}")
        }
        _ => String::new(),
    }
}

fn nuclear_side_totals(totals: &[calc_core::SideTotals]) -> Vec<Diagnostic> {
    let amount = |quantity: calc_core::ConservedQuantity| {
        totals
            .iter()
            .find(|side| side.quantity == quantity)
            .map_or_else(
                || "0".to_owned(),
                |side| crate::decimal_text::integer_to_decimal(&side.reactants),
            )
    };
    vec![diagnostic(
        "nuclear_each_side",
        vec![
            (
                "mass",
                identifier(&amount(calc_core::ConservedQuantity::NucleonNumber)),
            ),
            (
                "charge",
                identifier(&amount(calc_core::ConservedQuantity::Charge)),
            ),
            (
                "leptons",
                identifier(&amount(calc_core::ConservedQuantity::ElectronLeptonNumber)),
            ),
        ],
    )]
}

fn nuclear_side_values(side: &calc_core::SideTotals) -> Vec<(&'static str, ParameterValue)> {
    vec![
        (
            "reactants",
            identifier(&crate::decimal_text::integer_to_decimal(&side.reactants)),
        ),
        (
            "products",
            identifier(&crate::decimal_text::integer_to_decimal(&side.products)),
        ),
    ]
}

fn missing_neutrinos(side: &calc_core::SideTotals) -> String {
    let lacking = &side.reactants - &side.products;
    let (count, neutrino) = if lacking.is_negative() {
        (lacking.negated(), "\u{03BD}\u{0304}_e")
    } else {
        (lacking, "\u{03BD}_e")
    };
    if count == Integer::one() {
        neutrino.to_owned()
    } else {
        format!(
            "{} {neutrino}",
            crate::decimal_text::integer_to_decimal(&count)
        )
    }
}

fn tolerance_written_twice(pool: &ExprPool, written: ExprId) -> Option<ExprId> {
    let mut counts: HashMap<ExprId, usize> = HashMap::new();
    let mut pending = vec![written];
    while let Some(expression) = pending.pop() {
        match pool.node(expression) {
            Ok(NodeView::Apply {
                head: Head::Operator(Operator::Between | Operator::Tolerance),
                ..
            }) => {
                let count = counts.entry(expression).or_insert(0);
                *count += 1;
                if *count > 1 {
                    return Some(expression);
                }
            }
            Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().copied()),
            Ok(NodeView::Quantity { value, .. }) => pending.push(value),
            _ => {}
        }
    }
    None
}

fn is_indefinite_integral(pool: &ExprPool, expression: ExprId) -> bool {
    matches!(
        pool.node(expression),
        Ok(NodeView::Bind {
            binder: calc_expr::BinderKind::Integral,
            arguments: [],
            ..
        })
    )
}

pub const FREE_NAMES_PARAMETER: &str = "free_names";

const FREE_NAMES_CODE: &str = "free_names";

const NAMES_DATA: &str = "names";

pub fn free_names_in(computed: &ComputedResult) -> Option<Vec<String>> {
    match computed.method().parameters.get(FREE_NAMES_PARAMETER)? {
        ParameterValue::Identifier(names) => Some(names.split(", ").map(str::to_owned).collect()),
        _ => None,
    }
}

fn holds_the_free_name(pool: &ExprPool, root: ExprId, name: &str) -> bool {
    let mut visited = HashSet::new();
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if !visited.insert(expression) {
            continue;
        }
        let Ok(view) = pool.node(expression) else {
            continue;
        };
        match view {
            NodeView::Symbol(symbol) => {
                if pool.symbol_name(symbol).is_ok_and(|held| held == name) {
                    return true;
                }
            }
            NodeView::Apply { arguments, .. } => pending.extend(arguments.iter().copied()),
            NodeView::Bind {
                arguments, body, ..
            } => {
                pending.extend(arguments.iter().copied());
                pending.push(body);
            }
            NodeView::Quantity { value, .. } => pending.push(value),
            NodeView::Array { elements, .. } => pending.extend(elements.iter().copied()),
            NodeView::Number(_) | NodeView::Bound(_) => {}
        }
    }
    false
}

fn is_called(pool: &ExprPool, root: ExprId, name: &str) -> bool {
    let mut visited = HashSet::new();
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if !visited.insert(expression) {
            continue;
        }
        let Ok(view) = pool.node(expression) else {
            continue;
        };
        match view {
            NodeView::Apply { head, arguments } => {
                if let Head::Function(symbol) = head
                    && pool.symbol_name(symbol).is_ok_and(|called| called == name)
                {
                    return true;
                }
                pending.extend(arguments.iter().copied());
            }
            NodeView::Bind {
                arguments, body, ..
            } => {
                pending.extend(arguments.iter().copied());
                pending.push(body);
            }
            NodeView::Quantity { value, .. } => pending.push(value),
            NodeView::Array { elements, .. } => pending.extend(elements.iter().copied()),
            NodeView::Symbol(_) | NodeView::Number(_) | NodeView::Bound(_) => {}
        }
    }
    false
}

fn references_of(pool: &ExprPool, root: ExprId) -> Result<Vec<String>, SessionError> {
    let mut names: Vec<String> = Vec::new();
    let mut visited = HashSet::new();
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if !visited.insert(expression) {
            continue;
        }
        let Ok(view) = pool.node(expression) else {
            continue;
        };
        let children: Vec<ExprId> = match view {
            NodeView::Symbol(symbol) => {
                if pool.symbol_kind(symbol).map_err(SessionError::Symbol)? == SymbolKind::Variable {
                    add_reference(pool, &mut names, symbol)?;
                }
                Vec::new()
            }
            NodeView::Number(_) | NodeView::Bound(_) => Vec::new(),
            NodeView::Apply { head, arguments } => {
                if let Head::Function(symbol) = head {
                    add_reference(pool, &mut names, symbol)?;
                }
                arguments.to_vec()
            }
            NodeView::Bind {
                arguments, body, ..
            } => arguments.iter().copied().chain([body]).collect(),
            NodeView::Quantity { value, .. } => vec![value],
            NodeView::Array { elements, .. } => elements.to_vec(),
        };
        pending.extend(children.into_iter().rev());
    }
    Ok(names)
}

fn named_quantity(dimension: UnitDimension) -> Option<QuantityKind> {
    QuantityKind::ALL.into_iter().find(|kind| {
        kind.dimension_determines_it() && kind.dimension().exponents() == dimension.exponents()
    })
}

fn plot_variables_of(pool: &ExprPool, root: ExprId) -> Result<Vec<SymbolId>, SessionError> {
    let mut variables: Vec<SymbolId> = Vec::new();
    let mut visited = HashSet::new();
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if !visited.insert(expression) {
            continue;
        }
        let Ok(view) = pool.node(expression) else {
            continue;
        };
        let children: Vec<ExprId> = match view {
            NodeView::Symbol(symbol) => {
                let is_variable =
                    pool.symbol_kind(symbol).map_err(SessionError::Symbol)? == SymbolKind::Variable;
                if is_variable && !variables.contains(&symbol) {
                    variables.push(symbol);
                }
                Vec::new()
            }
            NodeView::Number(_) | NodeView::Bound(_) => Vec::new(),
            NodeView::Apply { arguments, .. } => arguments.to_vec(),
            NodeView::Bind {
                arguments, body, ..
            } => arguments.iter().copied().chain([body]).collect(),
            NodeView::Quantity { value, .. } => vec![value],
            NodeView::Array { elements, .. } => elements.to_vec(),
        };
        pending.extend(children.into_iter().rev());
    }
    Ok(variables)
}

fn content_display_systems() -> Vec<UnitSystem> {
    display_systems(
        concept_set()
            .map(|concepts| concepts.unit_systems.as_slice())
            .unwrap_or_default(),
    )
}

impl Session {
    pub fn new(clock: Box<dyn Clock>, backends: Vec<Box<dyn Backend>>) -> Self {
        Self {
            settings: Settings::default(),
            unit_override: None,
            applied_override: None,
            unit_systems: content_display_systems(),
            next_line_number: 1,
            rational_forms: HashMap::new(),
            view_jobs: ViewJobs::default(),
            lines: Vec::new(),
            definitions: HashMap::new(),
            referencing_lines: Vec::new(),
            solve_requests: HashMap::new(),
            escape_time_lines: HashMap::new(),
            escape_time_readings: HashMap::new(),
            pool: ExprPool::new(),
            clock: Arc::from(clock),
            replay: None,
            backends: Arc::new(backends.into_iter().map(Arc::from).collect()),
            further_backends: None,
            picture_cancellations: HashMap::new(),
            pending_pictures: HashMap::new(),
            completed_pictures: HashMap::new(),
            reading_cancellations: HashMap::new(),
            latest_picture_generations: HashMap::new(),
            next_picture_generation: 1,
            recognitions: HashMap::new(),
            pending_recognition: Vec::new(),
            pending_recognition_set: HashSet::new(),
            next_recognition_generation: 1,
        }
    }

    pub fn backend_kinds(&self) -> Vec<BackendKind> {
        self.backends.iter().map(|backend| backend.kind()).collect()
    }

    pub fn with_further_backends(
        mut self,
        further: impl FnOnce() -> Vec<Box<dyn Backend>> + Send + 'static,
    ) -> Self {
        self.further_backends = Some(Box::new(further));
        self
    }

    fn take_further_backends(&mut self) {
        let Some(further) = self.further_backends.take() else {
            return;
        };
        for backend in further() {
            self.register_backend(backend);
        }
    }

    pub fn register_backend(&mut self, backend: Box<dyn Backend>) {
        let mut backends = self.backends.as_ref().clone();
        backends.push(Arc::from(backend));
        self.backends = Arc::new(backends);
    }

    pub fn open_from_bytes(
        bytes: &[u8],
        clock: Box<dyn Clock>,
        backends: Vec<Box<dyn Backend>>,
    ) -> Result<Self, LoadError> {
        let mut pool = ExprPool::new();
        let data = session_file::decode(&mut pool, bytes)?;
        let mut session = Self {
            settings: data.settings,
            unit_override: data.unit_override,
            applied_override: None,
            unit_systems: content_display_systems(),
            next_line_number: data.next_line_number,
            rational_forms: HashMap::new(),
            view_jobs: ViewJobs::default(),
            lines: data.lines,
            definitions: HashMap::new(),
            referencing_lines: Vec::new(),
            solve_requests: HashMap::new(),
            escape_time_lines: HashMap::new(),
            escape_time_readings: HashMap::new(),
            pool,
            clock: Arc::from(clock),
            replay: None,
            backends: Arc::new(backends.into_iter().map(Arc::from).collect()),
            further_backends: None,
            picture_cancellations: HashMap::new(),
            pending_pictures: HashMap::new(),
            completed_pictures: HashMap::new(),
            reading_cancellations: HashMap::new(),
            latest_picture_generations: HashMap::new(),
            next_picture_generation: 1,
            recognitions: HashMap::new(),
            pending_recognition: Vec::new(),
            pending_recognition_set: HashSet::new(),
            next_recognition_generation: 1,
        };
        let mut refused = Vec::new();
        for position in 0..session.lines.len() {
            let input = session.lines[position].input.clone();
            if let Some(Ok(request)) = request_from_line_text(&input) {
                session
                    .solve_requests
                    .insert(session.lines[position].id, request);
                continue;
            }
            if let Some(Ok(request)) = escape_time_from_line_text(&input) {
                session
                    .escape_time_lines
                    .insert(session.lines[position].id, request);
                continue;
            }
            if let Some(Ok(request)) = escape_time_reading_from_line_text(&input) {
                session
                    .escape_time_readings
                    .insert(session.lines[position].id, request);
                continue;
            }
            let (definition, name) = match session.parse_definition(&input) {
                Ok(parsed) => parsed,
                Err(SessionError::Parse(error)) => {
                    session.lines[position].outcome =
                        Outcome::Error(input_not_parsed(&error, &input));
                    refused.push(session.lines[position].id);
                    continue;
                }
                Err(_) => continue,
            };
            if name.is_some() && name != session.lines[position].name {
                return Err(LoadError::NameDoesNotMatchInput(JsonPath(vec![
                    PathSegment::Member("lines".to_string()),
                    PathSegment::Index(position),
                    PathSegment::Member("name".to_string()),
                ])));
            }
            let id = session.lines[position].id;
            session.definitions.insert(id, definition);
            session.note_references(id);
        }
        session.fail_dependents_of(refused);
        for line in session.lines_without_a_recognition() {
            session.recognition_needed(line);
        }
        session.reapply_unit_override();
        let results: Vec<LineId> = session
            .lines
            .iter()
            .filter(|line| matches!(line.outcome, Outcome::Result(_)))
            .map(Line::id)
            .collect();
        for id in results {
            session.record_rational_form(id);
        }
        Ok(session)
    }

    pub fn save_to_bytes(&self) -> Result<Vec<u8>, SaveError> {
        session_file::encode(
            &self.pool,
            &self.settings,
            self.unit_override.as_ref(),
            self.next_line_number,
            &self.lines,
        )
    }

    #[cfg(test)]
    pub(crate) fn set_line_picture_for_tests_with_limit(
        &mut self,
        id: LineId,
        limit: Option<calc_viz::IterationLimitRule>,
    ) {
        if let Some(line) = self.lines.iter_mut().find(|line| line.id == id) {
            line.picture = Some(Box::new(Picture {
                views: Vec::new(),
                parameters: Vec::new(),
                iteration_limit: limit,
            }));
        }
    }

    #[cfg(test)]
    pub(crate) fn set_line_picture_for_tests(
        &mut self,
        id: LineId,
        views: &[crate::picture::ViewState],
        parameters: Vec<PictureParameter>,
    ) {
        let picture = Picture {
            iteration_limit: None,
            views: views
                .iter()
                .map(|view| crate::line_picture::PictureView {
                    axes: view
                        .axes
                        .iter()
                        .filter_map(|axis| {
                            axis.range
                                .as_ref()
                                .map(|range| crate::line_picture::PictureAxis {
                                    lower: range.lower.clone(),
                                    upper: range.upper.clone(),
                                    unit: axis.unit.clone(),
                                })
                        })
                        .collect(),
                    camera: view.camera.clone(),
                })
                .collect(),
            parameters,
        };
        if let Some(line) = self.lines.iter_mut().find(|line| line.id == id) {
            line.picture = Some(Box::new(picture));
        }
    }

    #[cfg(test)]
    pub(crate) fn set_unit_systems(&mut self, systems: Vec<UnitSystem>) {
        self.unit_systems = systems;
    }

    pub(crate) fn clock_resolution_nanoseconds(&self) -> u64 {
        self.clock.resolution_nanoseconds()
    }

    pub(crate) fn pool_mut(&mut self) -> &mut ExprPool {
        &mut self.pool
    }

    pub(crate) fn definition_value(&self, id: LineId) -> Option<ExprId> {
        self.definitions.get(&id).map(|definition| definition.value)
    }

    pub(crate) fn view_jobs(&self) -> &ViewJobs {
        &self.view_jobs
    }

    pub(crate) fn view_jobs_mut(&mut self) -> &mut ViewJobs {
        &mut self.view_jobs
    }

    pub(crate) fn is_expression_line(&self, id: LineId) -> bool {
        self.definitions
            .get(&id)
            .is_some_and(|definition| definition.kind != StatementKind::FunctionNaming)
            && !self.solve_requests.contains_key(&id)
    }

    pub(crate) fn view_expression(&mut self, id: LineId) -> Option<ExprId> {
        self.expanded_expression(id, UnresolvedNames::Undefined)
            .ok()
    }

    pub fn rational_form_of(&self, id: LineId) -> RationalForm {
        self.rational_forms
            .get(&id)
            .copied()
            .unwrap_or(RationalForm::Fraction)
    }

    pub(crate) fn rational_form_in(
        &mut self,
        id: LineId,
        display_unit: Option<UnitId>,
    ) -> RationalForm {
        let Ok(expression) = self.expanded_expression(id, UnresolvedNames::Undefined) else {
            return RationalForm::Fraction;
        };
        let written = match has_uncertain_inputs(&self.pool, expression) {
            Ok(false) => Some(expression),
            Ok(true) => central_expression(&mut self.pool, expression).ok(),
            Err(_) => None,
        };
        let form = match written {
            Some(written) => rational_form(&mut self.pool, written, display_unit),
            None => RationalForm::Fraction,
        };
        let asks_for_a_value = matches!(
            self.pool.node(expression),
            Ok(NodeView::Apply {
                head: Head::Operator(Operator::Wrap | Operator::InType),
                ..
            })
        );
        let has_unit = display_unit.is_some_and(|unit| {
            self.pool
                .units()
                .dimension(unit)
                .is_ok_and(|dimension| !dimension.is_dimensionless())
        });
        if has_unit || asks_for_a_value {
            return form;
        }
        match (
            form,
            self.line(id)
                .and_then(|line| calc_syntax::written_radix(&line.input)),
        ) {
            (RationalForm::Radix(_), _) | (_, None) => form,
            (_, Some(base)) => RationalForm::Radix(calc_core::RadixForm {
                base,
                bits: 0,
                signed: false,
                big_endian: None,
            }),
        }
    }

    fn record_rational_form(&mut self, id: LineId) {
        let unit = match self.line(id).map(Line::outcome) {
            Some(Outcome::Result(record)) => record.computed().unit(),
            _ => {
                self.rational_forms.remove(&id);
                return;
            }
        };
        let form = self.rational_form_in(id, unit);
        self.rational_forms.insert(id, form);
    }

    pub(crate) fn pool(&self) -> &ExprPool {
        &self.pool
    }

    pub fn line_json(&self, line: &Line) -> Result<Vec<u8>, SaveError> {
        session_file::encode_line(&self.pool, line)
    }

    pub fn line_json_compact(&self, line: &Line) -> Result<Vec<u8>, SaveError> {
        session_file::encode_line_compact(&self.pool, line)
    }

    pub fn reading_json(&self, line: LineId, record: &ResultRecord) -> Result<Vec<u8>, SaveError> {
        session_file::encode_record(&self.pool, record, line)
    }

    pub fn orbit_json(&self, orbit: Orbit) -> Vec<u8> {
        session_file::encode_orbit(orbit)
    }

    pub fn unit_override(&self) -> Option<&UnitOverride> {
        self.unit_override.as_ref()
    }

    pub fn set_unit_override(
        &mut self,
        unit_override: UnitOverride,
    ) -> Result<(), UnitOverrideError> {
        check_structure(&unit_override)?;
        self.unit_override = Some(unit_override);
        self.reapply_unit_override();
        Ok(())
    }

    pub fn clear_unit_override(&mut self) {
        self.unit_override = None;
        self.applied_override = None;
    }

    fn reapply_unit_override(&mut self) {
        let systems = concept_set()
            .map(|concepts| concepts.unit_systems.as_slice())
            .unwrap_or_default();
        self.applied_override = self
            .unit_override
            .as_ref()
            .map(|unit_override| apply_override(&mut self.pool, unit_override, systems));
    }

    pub fn applied_unit_override(&self) -> Option<&AppliedOverride> {
        self.applied_override.as_ref()
    }

    pub fn display_unit(
        &mut self,
        question: &DisplayQuestion,
        preference: Option<&UnitsChoice>,
        curriculum: Option<&CurriculumUnits>,
        coherent_only: bool,
    ) -> DisplayDecision {
        let sources = DisplaySources {
            curriculum,
            session_override: self
                .applied_override
                .as_ref()
                .map(|applied| &applied.choice),
            preference,
            systems: &self.unit_systems,
            coherent_only,
        };
        decide_display_unit(&mut self.pool, question, &sources)
    }

    pub fn settings(&self) -> Settings {
        self.settings
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub fn line(&self, id: LineId) -> Option<&Line> {
        match self.lines.binary_search_by_key(&id, |line| line.id) {
            Ok(position) => self.lines.get(position),
            Err(_) => self.lines.iter().find(|line| line.id == id),
        }
    }

    pub fn resolve(&self, name: &str) -> Option<LineId> {
        if let Some(label) = line_from_label(name) {
            return self.line(label).map(Line::id);
        }
        self.lines
            .iter()
            .find(|line| line.name.as_deref() == Some(name))
            .map(Line::id)
    }

    fn parse_definition(
        &mut self,
        text: &str,
    ) -> Result<(Definition, Option<String>), SessionError> {
        let parsed =
            parse_statement_with_unit_endings(&mut self.pool, text).map_err(SessionError::Parse)?;
        let (kind, value, name) = match parsed.statement {
            Statement::Expression(value) => (StatementKind::Expression, value, None),
            Statement::Naming { name, value } => (StatementKind::Naming, value, Some(name)),
            Statement::FunctionNaming { name, value } => {
                (StatementKind::FunctionNaming, value, Some(name))
            }
        };
        let name = name
            .map(|symbol| {
                self.pool
                    .symbol_name(symbol)
                    .map(str::to_string)
                    .map_err(SessionError::Symbol)
            })
            .transpose()?;
        let references = references_of(&self.pool, value)?;
        Ok((
            Definition {
                kind,
                value,
                references,
                unit_endings: parsed.unit_endings,
            },
            name,
        ))
    }

    pub fn identifies_number(&self, id: LineId) -> bool {
        self.definitions.get(&id).is_some_and(|definition| {
            definition.kind != StatementKind::FunctionNaming && definition.references.is_empty()
        })
    }

    fn fail_dependents_of(&mut self, mut failed: Vec<LineId>) {
        let mut pending = failed.clone();
        while let Some(failed_line) = pending.pop() {
            for position in 0..self.lines.len() {
                let id = self.lines[position].id;
                if failed.contains(&id) || !self.direct_dependencies(id).contains(&failed_line) {
                    continue;
                }
                self.lines[position].outcome = Outcome::Error(diagnostic(
                    "dependency_failed",
                    vec![(LINE_DATA, identifier(&line_label(failed_line)))],
                ));
                failed.push(id);
                pending.push(id);
            }
        }
    }

    fn direct_dependencies(&self, id: LineId) -> Vec<LineId> {
        let mut dependencies = Vec::new();
        if let Some(definition) = self.definitions.get(&id) {
            for name in &definition.references {
                if let Some(dependency) = self.resolve(name)
                    && !dependencies.contains(&dependency)
                {
                    dependencies.push(dependency);
                }
            }
        }
        dependencies
    }

    fn is_on_cycle(&self, start: LineId) -> bool {
        let mut visited = HashSet::new();
        let mut pending = self.direct_dependencies(start);
        while let Some(current) = pending.pop() {
            if current == start {
                return true;
            }
            if visited.insert(current) {
                pending.extend(self.direct_dependencies(current));
            }
        }
        false
    }

    fn dependents_of(&self, id: LineId) -> Vec<LineId> {
        self.referencing_lines
            .iter()
            .copied()
            .filter(|line| *line != id && self.references_line(*line, id))
            .collect()
    }

    fn references_line(&self, line: LineId, target: LineId) -> bool {
        self.definitions.get(&line).is_some_and(|definition| {
            definition
                .references
                .iter()
                .any(|name| self.resolve(name) == Some(target))
        })
    }

    fn note_references(&mut self, id: LineId) {
        let has_references = self
            .definitions
            .get(&id)
            .is_some_and(|definition| !definition.references.is_empty());
        let known = self.referencing_lines.binary_search(&id);
        match (has_references, known) {
            (true, Err(position)) => self.referencing_lines.insert(position, id),
            (false, Ok(position)) => {
                self.referencing_lines.remove(position);
            }
            _ => {}
        }
    }

    pub fn enter(&mut self, text: &str) -> Result<LineId, SessionError> {
        if let Some(request) = request_from_line_text(text) {
            return self.enter_solve(&request.map_err(SessionError::InvalidRequest)?);
        }
        if let Some(request) = escape_time_from_line_text(text) {
            return self.enter_escape_time(&request.map_err(SessionError::InvalidRequest)?);
        }
        if let Some(request) = escape_time_reading_from_line_text(text) {
            return self.enter_escape_time_reading(&request.map_err(SessionError::InvalidRequest)?);
        }
        let (definition, name) = self.parse_definition(text)?;
        if let Some(name) = &name
            && self.resolve(name).is_some()
        {
            return Err(SessionError::NameExists(name.clone()));
        }
        self.place(text, definition, name)
    }

    pub fn enter_question(&mut self, text: &str) -> Result<LineId, SessionError> {
        let value = parse_expression(&mut self.pool, text).map_err(SessionError::Parse)?;
        let references = references_of(&self.pool, value)?;
        let definition = Definition {
            kind: StatementKind::Expression,
            value,
            references,
            unit_endings: Vec::new(),
        };
        self.place(text, definition, None)
    }

    fn place(
        &mut self,
        text: &str,
        definition: Definition,
        name: Option<String>,
    ) -> Result<LineId, SessionError> {
        let id =
            LineId::from_number(self.next_line_number).ok_or(SessionError::LineNumbersExhausted)?;
        let next_line_number = self
            .next_line_number
            .checked_add(1)
            .ok_or(SessionError::LineNumbersExhausted)?;
        self.lines.push(Line {
            id,
            name,
            input: text.to_string(),
            outcome: Outcome::NotEvaluated,
            picture: None,
            reading: None,
        });
        self.definitions.insert(id, definition);
        self.note_references(id);
        if self.is_on_cycle(id) {
            self.lines.pop();
            self.definitions.remove(&id);
            self.note_references(id);
            return Err(SessionError::ReferenceCycle(id));
        }
        self.next_line_number = next_line_number;
        self.recompute_from(id);
        Ok(id)
    }

    pub fn enter_escape_time(&mut self, request: &EscapeTimeLine) -> Result<LineId, SessionError> {
        let id =
            LineId::from_number(self.next_line_number).ok_or(SessionError::LineNumbersExhausted)?;
        let next_line_number = self
            .next_line_number
            .checked_add(1)
            .ok_or(SessionError::LineNumbersExhausted)?;
        self.lines.push(Line {
            id,
            name: None,
            input: request.line_text(),
            outcome: Outcome::Picture,
            picture: None,
            reading: None,
        });
        self.escape_time_lines.insert(id, request.clone());
        self.next_line_number = next_line_number;
        Ok(id)
    }

    pub fn enter_escape_time_reading(
        &mut self,
        request: &EscapeTimeReadingLine,
    ) -> Result<LineId, SessionError> {
        let id =
            LineId::from_number(self.next_line_number).ok_or(SessionError::LineNumbersExhausted)?;
        let next_line_number = self
            .next_line_number
            .checked_add(1)
            .ok_or(SessionError::LineNumbersExhausted)?;
        let outcome = self.orbit_outcome(request);
        self.lines.push(Line {
            id,
            name: None,
            input: request.line_text(),
            outcome,
            picture: None,
            reading: None,
        });
        self.escape_time_readings.insert(id, request.clone());
        self.next_line_number = next_line_number;
        Ok(id)
    }

    fn orbit_outcome(&self, request: &EscapeTimeReadingLine) -> Outcome {
        let backends: Vec<&dyn Backend> = self.backends.iter().map(AsRef::as_ref).collect();
        let [real_axis, _] = calc_viz::escape_time_default_view(&request.form);
        let orbit = calc_viz::read_orbit(
            &backends,
            &OrbitRequest {
                form: request.form.clone(),
                limit_rule: IterationLimitRule::Fixed {
                    iterations: request.limit,
                },
                real_axis,
                domain: match self.settings.precision {
                    Precision::F32 => Domain::F32,
                    Precision::F64 | Precision::Exact => Domain::F64,
                },
                preference: self.settings.backend,
                real: request.at_real.clone(),
                imaginary: request.at_imaginary.clone(),
            },
        );
        match orbit {
            Ok(orbit) => Outcome::EscapeTimeReading(orbit),
            Err(_) => Outcome::Error(diagnostic(ORBIT_NOT_READ_CODE, Vec::new())),
        }
    }

    pub fn escape_time_reading_of(&self, id: LineId) -> Option<&EscapeTimeReadingLine> {
        self.escape_time_readings.get(&id)
    }

    pub fn escape_time_line_of(&self, id: LineId) -> Option<&EscapeTimeLine> {
        self.escape_time_lines.get(&id)
    }

    pub fn edit(&mut self, id: LineId, text: &str) -> Result<Vec<LineId>, SessionError> {
        let position = self
            .lines
            .iter()
            .position(|line| line.id == id)
            .ok_or(SessionError::UnknownLine(id))?;
        if let Some(request) = escape_time_from_line_text(text) {
            let request = request.map_err(SessionError::InvalidRequest)?;
            let dependents = self.dependents_of(id);
            if self.lines[position].name.is_some() && !dependents.is_empty() {
                return Err(SessionError::NameInUse {
                    line: id,
                    dependents,
                });
            }
            self.lines[position].input = request.line_text();
            self.lines[position].name = None;
            self.lines[position].outcome = Outcome::Picture;
            self.lines[position].picture = None;
            self.definitions.remove(&id);
            self.note_references(id);
            self.escape_time_lines.insert(id, request);
            return Ok(self.recompute_from(id));
        }
        self.escape_time_lines.remove(&id);
        let (definition, stated_name) = self.parse_definition(text)?;
        let old_name = self.lines[position].name.clone();
        let new_name = stated_name.or_else(|| old_name.clone());
        if new_name != old_name {
            if let Some(name) = &new_name
                && self.resolve(name).is_some_and(|holder| holder != id)
            {
                return Err(SessionError::NameExists(name.clone()));
            }
            let dependents = self.dependents_of(id);
            if old_name.is_some() && !dependents.is_empty() {
                return Err(SessionError::NameInUse {
                    line: id,
                    dependents,
                });
            }
        }
        let old_input = std::mem::replace(&mut self.lines[position].input, text.to_string());
        let old_line_name = std::mem::replace(&mut self.lines[position].name, new_name);
        let old_definition = self.definitions.insert(id, definition);
        self.note_references(id);
        if self.is_on_cycle(id) {
            self.lines[position].input = old_input;
            self.lines[position].name = old_line_name;
            match old_definition {
                Some(definition) => self.definitions.insert(id, definition),
                None => self.definitions.remove(&id),
            };
            self.note_references(id);
            return Err(SessionError::ReferenceCycle(id));
        }
        Ok(self.recompute_from(id))
    }

    pub fn set_precision(&mut self, precision: Precision) -> Vec<LineId> {
        self.settings.precision = precision;
        let every_line: Vec<LineId> = self.lines.iter().map(Line::id).collect();
        self.recompute(every_line)
    }

    pub fn is_definition(&self, id: LineId) -> bool {
        self.definitions
            .get(&id)
            .is_some_and(|definition| definition.kind != StatementKind::Expression)
    }

    pub fn check_line(&mut self, id: LineId) -> ClaimOutcome {
        if self.is_definition(id) {
            return ClaimOutcome::Definition;
        }
        let expression = match self.expanded_expression(id, UnresolvedNames::Undefined) {
            Ok(expression) => expression,
            Err(diagnostic) => return self.unreachable_claim(id, diagnostic),
        };
        let expression = self.inline_function_calls(expression).unwrap_or(expression);
        if holds_a_conversion(&self.pool, expression) {
            self.take_further_backends();
        }
        let mut uses = Vec::new();
        let expression = evaluated_conversions(
            &mut self.pool,
            &self.backends,
            self.settings,
            expression,
            &mut uses,
        );
        if let Some(diagnostic) = unit_refusal(&mut self.pool, expression) {
            return ClaimOutcome::Refused(diagnostic);
        }
        if let Some(diagnostic) = order_of_a_non_real(&mut self.pool, expression) {
            return ClaimOutcome::Refused(diagnostic);
        }
        match calc_core::check_relation(&mut self.pool, expression) {
            Some(calc_core::Verdict::Undecided) => {
                match calc_core::check_relation_over_ranges(&mut self.pool, expression) {
                    Some(verdict) => ClaimOutcome::OverRanges(verdict),
                    None => ClaimOutcome::Checked(calc_core::Verdict::Undecided),
                }
            }
            Some(verdict) => ClaimOutcome::Checked(verdict),
            None => ClaimOutcome::NotARelation,
        }
    }

    fn unreachable_claim(&self, id: LineId, diagnostic: Diagnostic) -> ClaimOutcome {
        let written = self.definitions.get(&id).map(|definition| definition.value);
        match written {
            Some(value)
                if diagnostic.code == UNDEFINED_NAME_CODE
                    && calc_core::is_relation(&self.pool, value) =>
            {
                ClaimOutcome::UnknownName(diagnostic)
            }
            _ => ClaimOutcome::NotARelation,
        }
    }

    pub(crate) fn substitution_in(&mut self, id: LineId) -> Vec<(String, String)> {
        let Some(definition) = self.definitions.get(&id).cloned() else {
            return Vec::new();
        };
        let mut substituted = Vec::new();
        for name in &definition.references {
            let Some(line) = self.resolve(name) else {
                continue;
            };
            if self
                .definitions
                .get(&line)
                .is_some_and(|other| other.kind == StatementKind::FunctionNaming)
            {
                continue;
            }
            let Ok(expression) = self.expanded_expression(line, UnresolvedNames::Free) else {
                continue;
            };
            let value = match self.line(line).map(Line::outcome) {
                Some(Outcome::Result(record)) => {
                    Some(crate::summary::value_text(record.computed().value()))
                }
                _ => None,
            };
            let written = match value {
                Some(value) => value,
                None => match print_expression(&self.pool, expression, PrintMode::Ascii) {
                    Ok(written) => written,
                    Err(_) => continue,
                },
            };
            substituted.push((name.clone(), written));
        }
        substituted
    }

    pub(crate) fn naming_as_equation(&mut self, id: LineId) -> Option<(&mut ExprPool, ExprId)> {
        let definition = self.definitions.get(&id)?;
        if definition.kind != StatementKind::Naming {
            return None;
        }
        let value = definition.value;
        let name = self.line(id)?.name()?.to_owned();
        let symbol = self.pool.intern_symbol(&name, SymbolKind::Variable).ok()?;
        let named = self.pool.symbol(symbol).ok()?;
        let equation = self
            .pool
            .apply(Head::Operator(Operator::Equal), &[named, value])
            .ok()?;
        Some((&mut self.pool, equation))
    }

    pub(crate) fn asked(&mut self, id: LineId) -> Option<(&mut ExprPool, ExprId)> {
        let expression = self.expanded_expression(id, UnresolvedNames::Free).ok()?;
        let expression = self.inline_function_calls(expression).unwrap_or(expression);
        Some((&mut self.pool, expression))
    }

    pub fn replay(&mut self) -> &ReplayReport {
        let stored: Vec<(LineId, Outcome)> = self
            .lines
            .iter()
            .map(|line| (line.id(), line.outcome().clone()))
            .collect();
        let every_line: Vec<LineId> = stored.iter().map(|(id, _)| *id).collect();
        let recognition_was_run = stored.iter().any(|(_, outcome)| {
            matches!(
                outcome,
                Outcome::Result(record) if record.recognized().is_some()
            )
        });
        self.recompute(every_line);
        if recognition_was_run {
            self.run_pending_recognition();
        }
        let lines = stored
            .into_iter()
            .map(|(id, outcome)| {
                let replayed = self
                    .line(id)
                    .map(Line::outcome)
                    .cloned()
                    .unwrap_or(Outcome::NotEvaluated);
                compare_outcomes(&self.pool, id, &outcome, &replayed)
            })
            .collect();
        let run = ReplayRun {
            at: self.clock.now_utc(),
            report: ReplayReport { lines },
        };
        &self.replay.insert(run).report
    }

    pub fn replay_report(&self) -> Option<&ReplayReport> {
        self.replay.as_ref().map(|run| &run.report)
    }

    pub(crate) fn replay_state(&self) -> ReplayState {
        let Some(run) = &self.replay else {
            return ReplayState::NotRun;
        };
        let today = same_day(
            run.at.milliseconds_since_unix_epoch(),
            self.clock.now_utc().milliseconds_since_unix_epoch(),
        );
        if run.report.differing() == 0 {
            ReplayState::Verified { at: run.at, today }
        } else {
            ReplayState::Differs { at: run.at, today }
        }
    }

    pub fn enter_solve(&mut self, request: &SolveRequest) -> Result<LineId, SessionError> {
        let id =
            LineId::from_number(self.next_line_number).ok_or(SessionError::LineNumbersExhausted)?;
        let next_line_number = self
            .next_line_number
            .checked_add(1)
            .ok_or(SessionError::LineNumbersExhausted)?;
        let outcome = self.solve_outcome(request)?;
        self.lines.push(Line {
            id,
            name: None,
            input: request.line_text(),
            outcome,
            picture: None,
            reading: None,
        });
        self.solve_requests.insert(id, request.clone());
        self.next_line_number = next_line_number;
        self.recognition_needed(id);
        Ok(id)
    }

    pub fn mark(
        &mut self,
        id: LineId,
        role: &str,
        mark: RoleMark,
    ) -> Result<Vec<LineId>, SessionError> {
        let mut request = self.solve_request(id)?;
        if request.phase == Phase::Reachable && mark == RoleMark::NotObtainable {
            return Err(unexpected_member("not_obtainable"));
        }
        request.obtainable.retain(|name| name != role);
        request.not_obtainable.retain(|name| name != role);
        match mark {
            RoleMark::Obtainable => request.obtainable.push(role.to_owned()),
            RoleMark::NotObtainable => request.not_obtainable.push(role.to_owned()),
            RoleMark::Unmarked => {}
        }
        if let Some(path) = request.has_mark_conflict() {
            return Err(SessionError::InvalidRequest(RequestError::new(
                RequestErrorCode::ContradictoryMarks,
                path,
            )));
        }
        self.replace_request(id, request)
    }

    pub fn set_given(
        &mut self,
        id: LineId,
        role: &str,
        given: Option<GivenInput>,
    ) -> Result<Vec<LineId>, SessionError> {
        let mut request = self.solve_request(id)?;
        let position = request.given.iter().position(|stated| stated.name == role);
        match (given, position) {
            (Some(value), Some(position)) => request.given[position] = value,
            (Some(value), None) => {
                request.obtainable.retain(|name| name != role);
                request.given.push(value);
            }
            (None, Some(position)) => {
                request.given.remove(position);
            }
            (None, None) => {}
        }
        if request.phase != Phase::Reachable {
            request.phase = Phase::Evaluate;
        }
        self.replace_request(id, request)
    }

    pub fn set_criterion(
        &mut self,
        id: LineId,
        criterion: SolveCriterion,
    ) -> Result<Vec<LineId>, SessionError> {
        let mut request = self.solve_request(id)?;
        if request.phase == Phase::Reachable {
            return Err(unexpected_member("criterion"));
        }
        if criterion.by.is_empty() {
            return Err(SessionError::InvalidRequest(RequestError::new(
                RequestErrorCode::UnknownCriterion,
                JsonPath::default().member("criterion").member("by"),
            )));
        }
        request.criterion = criterion;
        self.replace_request(id, request)
    }

    pub fn set_cap(&mut self, id: LineId, cap: u32) -> Result<Vec<LineId>, SessionError> {
        let mut request = self.solve_request(id)?;
        if !(1..=LARGEST_CAP).contains(&cap) {
            return Err(SessionError::InvalidRequest(RequestError::new(
                RequestErrorCode::InvalidCap,
                JsonPath::default().member("cap"),
            )));
        }
        if request.phase != Phase::Reachable {
            let grown = u128::from(request.work_budget) * u128::from(cap) / u128::from(request.cap);
            request.work_budget = u64::try_from(grown)
                .unwrap_or(LARGEST_WORK_BUDGET)
                .clamp(1, LARGEST_WORK_BUDGET);
        }
        request.cap = cap;
        self.replace_request(id, request)
    }

    pub fn find_ways(&mut self, id: LineId) -> Result<Vec<LineId>, SessionError> {
        let mut request = self.solve_request(id)?;
        if request.phase == Phase::Reachable {
            return Err(unexpected_member("wanted"));
        }
        request.phase = Phase::Ways;
        self.replace_request(id, request)
    }

    pub fn choose_wanted(
        &mut self,
        id: LineId,
        role: &str,
        criterion: SolveCriterion,
        cap: u32,
    ) -> Result<Vec<LineId>, SessionError> {
        let request = self.solve_request(id)?;
        if request.phase != Phase::Reachable {
            return Err(unexpected_member("wanted"));
        }
        if !(1..=LARGEST_CAP).contains(&cap) {
            return Err(SessionError::InvalidRequest(RequestError::new(
                RequestErrorCode::InvalidCap,
                JsonPath::default().member("cap"),
            )));
        }
        let object = match (self.line(id).map(Line::outcome), &request.object) {
            (Some(Outcome::Reachable(answer)), _) => Some(answer.object.clone()),
            (_, Some(object)) => Some(object.clone()),
            (_, None) => match answer_reachable(&mut self.pool, &request) {
                Ok(answer) => Some(answer.object),
                Err(SolveFailure::InvalidRequest(error)) => {
                    return Err(SessionError::InvalidRequest(error));
                }
                Err(SolveFailure::Internal(_)) => None,
            },
        };
        let chosen = SolveRequest {
            phase: Phase::Ways,
            object,
            wanted: Some(role.to_owned()),
            criterion,
            cap,
            ..request
        };
        self.replace_request(id, chosen)
    }

    pub fn solve_request_of(&self, id: LineId) -> Option<&SolveRequest> {
        self.solve_requests.get(&id)
    }

    fn solve_request(&self, id: LineId) -> Result<SolveRequest, SessionError> {
        if self.line(id).is_none() {
            return Err(SessionError::UnknownLine(id));
        }
        self.solve_requests
            .get(&id)
            .cloned()
            .ok_or(SessionError::NotASolveLine(id))
    }

    pub(crate) fn solve_working(&mut self, id: LineId) -> Option<crate::solve::SolveWorking> {
        let request = self.solve_requests.get(&id)?.clone();
        let backends: Vec<&dyn Backend> = self.backends.iter().map(AsRef::as_ref).collect();
        let evaluation = EvaluationContext {
            backends: &backends,
            clock: self.clock.as_ref(),
        };
        crate::solve::solve_working(&mut self.pool, &request, &evaluation)
    }

    fn solve_outcome(&mut self, request: &SolveRequest) -> Result<Outcome, SessionError> {
        let backends: Vec<&dyn Backend> = self.backends.iter().map(AsRef::as_ref).collect();
        let evaluation = EvaluationContext {
            backends: &backends,
            clock: self.clock.as_ref(),
        };
        if request.phase == Phase::Reachable {
            return match answer_reachable(&mut self.pool, request) {
                Ok(answer) => Ok(Outcome::Reachable(Box::new(answer))),
                Err(SolveFailure::InvalidRequest(error)) => {
                    Err(SessionError::InvalidRequest(error))
                }
                Err(SolveFailure::Internal(_)) => {
                    Ok(Outcome::Error(diagnostic(SOLVE_FAILED_CODE, vec![])))
                }
            };
        }
        match answer_request(&mut self.pool, request, &evaluation) {
            Ok(answer) => Ok(Outcome::Answer(Box::new(answer))),
            Err(SolveFailure::InvalidRequest(error)) => Err(SessionError::InvalidRequest(error)),
            Err(SolveFailure::Internal(_)) => {
                Ok(Outcome::Error(diagnostic(SOLVE_FAILED_CODE, vec![])))
            }
        }
    }

    fn replace_request(
        &mut self,
        id: LineId,
        request: SolveRequest,
    ) -> Result<Vec<LineId>, SessionError> {
        let outcome = self.solve_outcome(&request)?;
        if let Some(line) = self.lines.iter_mut().find(|line| line.id == id) {
            line.input = request.line_text();
            line.outcome = outcome;
        }
        self.solve_requests.insert(id, request);
        let mut affected = vec![id];
        affected.extend(
            self.recompute_from(id)
                .into_iter()
                .filter(|line| *line != id),
        );
        Ok(affected)
    }

    fn recompute_from(&mut self, changed: LineId) -> Vec<LineId> {
        self.recompute(vec![changed])
    }

    fn lines_without_a_recognition(&self) -> Vec<LineId> {
        self.lines
            .iter()
            .filter(|line| match line.outcome() {
                Outcome::Result(record) => {
                    !matches!(record.recognition(), RecordRecognition::Ran(_))
                }
                _ => true,
            })
            .map(Line::id)
            .collect()
    }

    fn recognition_needed(&mut self, id: LineId) {
        self.cancel_recognition(id);
        if self.pending_recognition_set.insert(id) {
            self.pending_recognition.push(id);
        }
    }

    pub fn cancel_recognition(&mut self, id: LineId) {
        if let Some(entry) = self.recognitions.remove(&id) {
            entry.cancellation.store(true, AtomicOrdering::SeqCst);
        }
    }

    pub fn pending_recognition(&self) -> Vec<LineId> {
        self.pending_recognition.clone()
    }

    pub fn is_recognition_pending(&self, id: LineId) -> bool {
        self.pending_recognition.contains(&id)
    }

    pub fn run_pending_recognition(&mut self) {
        let (sender, receiver) = std::sync::mpsc::channel();
        for id in self.pending_recognition() {
            let Some(mut job) = self.recognition_job(id, sender.clone()) else {
                continue;
            };
            while job.step() == JobState::Pending {}
            while let Ok(event) = receiver.try_recv() {
                self.apply_recognition(&event);
            }
        }
    }

    pub fn start_pending_recognition(
        &mut self,
        events: &Sender<RecognitionEvent>,
    ) -> Vec<RecognitionJob> {
        self.pending_recognition()
            .into_iter()
            .filter_map(|id| self.recognition_job(id, events.clone()))
            .collect()
    }

    pub fn recognition_state(&self, id: LineId) -> Option<RecognitionState> {
        self.recognitions.get(&id).map(|entry| entry.state)
    }

    pub fn recognition_job(
        &mut self,
        id: LineId,
        events: Sender<RecognitionEvent>,
    ) -> Option<RecognitionJob> {
        self.pending_recognition.retain(|pending| *pending != id);
        self.pending_recognition_set.remove(&id);
        let work = self.recognition_work(id)?;
        let Ok(concepts) = concept_set() else {
            self.set_recognition_state(
                id,
                RecognitionState::Unavailable(RecognitionUnavailable::ConceptSetNotLoaded),
            );
            return None;
        };
        self.cancel_recognition(id);
        let cancellation = Arc::new(AtomicBool::new(false));
        let generation = self.next_recognition_generation;
        self.next_recognition_generation = self.next_recognition_generation.saturating_add(1);
        self.recognitions.insert(
            id,
            RecognitionEntry {
                generation,
                state: RecognitionState::Running,
                cancellation: Arc::clone(&cancellation),
            },
        );
        Some(RecognitionJob {
            work,
            concept_set_version: concept_set_version_text(concepts.version),
            line: id,
            generation,
            cancellation,
            events,
        })
    }

    fn recognition_work(&mut self, id: LineId) -> Option<RecognitionWork> {
        match self.line(id)?.outcome() {
            Outcome::Answer(answer) => {
                let rules = crate::recognition::derivation_of(answer);
                (!rules.is_empty()).then_some(RecognitionWork::Rules { rules })
            }
            Outcome::Result(_) => {
                let written = self.definition_value(id)?;
                let text = print_expression(self.pool(), written, PrintMode::Ascii).ok()?;
                let concepts = concept_set().ok()?;
                let mut pool = ExprPool::new();
                let content = concepts.patterns(&mut pool).ok()?;
                let (patterns, variables) = matcher_patterns(&content);
                let root = parse_expression(&mut pool, &text).ok()?;
                Some(RecognitionWork::Match {
                    pool: Box::new(pool),
                    root,
                    patterns,
                    variables,
                    limits: RecognitionLimits::default(),
                })
            }
            _ => None,
        }
    }

    fn set_recognition_state(&mut self, id: LineId, state: RecognitionState) {
        if let Some(entry) = self.recognitions.get_mut(&id) {
            entry.state = state;
            return;
        }
        let generation = self.next_recognition_generation;
        self.next_recognition_generation = self.next_recognition_generation.saturating_add(1);
        self.recognitions.insert(
            id,
            RecognitionEntry {
                generation,
                state,
                cancellation: Arc::new(AtomicBool::new(false)),
            },
        );
    }

    pub fn apply_recognition(&mut self, event: &RecognitionEvent) -> bool {
        let id = event.line();
        let Some(entry) = self.recognitions.get_mut(&id) else {
            return false;
        };
        if entry.generation != event.generation() {
            return false;
        }
        match event {
            RecognitionEvent::Unavailable { reason, .. } => {
                entry.state = RecognitionState::Unavailable(*reason);
                if let Some(line) = self.lines.iter_mut().find(|line| line.id == id)
                    && let Outcome::Result(record) = &line.outcome
                {
                    line.outcome = Outcome::Result(Box::new(
                        record
                            .as_ref()
                            .clone()
                            .with_recognition(RecordRecognition::Unavailable(*reason)),
                    ));
                }
                true
            }
            RecognitionEvent::Ready { recognized, .. } => {
                entry.state = RecognitionState::Ready;
                if let Some(line) = self.lines.iter_mut().find(|line| line.id == id)
                    && let Outcome::Result(record) = &line.outcome
                {
                    line.outcome = Outcome::Result(Box::new(
                        record.as_ref().clone().with_recognized(recognized.clone()),
                    ));
                }
                true
            }
        }
    }

    fn recompute(&mut self, changed: Vec<LineId>) -> Vec<LineId> {
        let mut affected: HashSet<LineId> = changed.iter().copied().collect();
        let mut pending = changed;
        while let Some(current) = pending.pop() {
            for dependent in self.dependents_of(current) {
                if affected.insert(dependent) {
                    pending.push(dependent);
                }
            }
        }
        self.recompute_affected(affected)
    }

    fn recompute_affected(&mut self, affected: HashSet<LineId>) -> Vec<LineId> {
        let mut stack_order: Vec<LineId> = affected.iter().copied().collect();
        stack_order.sort_unstable();
        let mut order: Vec<LineId> = Vec::new();
        while order.len() < affected.len() {
            let Some(next) = stack_order.iter().copied().find(|line| {
                !order.contains(line)
                    && self.direct_dependencies(*line).iter().all(|dependency| {
                        !affected.contains(dependency) || order.contains(dependency)
                    })
            }) else {
                break;
            };
            order.push(next);
        }
        for line in &order {
            let outcome = self.evaluate_line(*line);
            match self.lines.binary_search_by_key(line, |stored| stored.id) {
                Ok(index) => self.lines[index].outcome = outcome,
                Err(_) => {
                    if let Some(stored) = self.lines.iter_mut().find(|stored| stored.id == *line) {
                        stored.outcome = outcome;
                    }
                }
            }
            self.record_rational_form(*line);
            self.recognition_needed(*line);
        }
        order
    }

    fn ancestors_in_dependency_order(&self, id: LineId) -> Vec<LineId> {
        let mut order = Vec::new();
        let mut visited = HashSet::new();
        let mut pending = vec![(id, false)];
        while let Some((current, children_are_done)) = pending.pop() {
            if children_are_done {
                order.push(current);
                continue;
            }
            if !visited.insert(current) {
                continue;
            }
            pending.push((current, true));
            for dependency in self.direct_dependencies(current).into_iter().rev() {
                if !visited.contains(&dependency) {
                    pending.push((dependency, false));
                }
            }
        }
        order
    }

    pub(crate) fn reference_cycle(line: LineId) -> Diagnostic {
        diagnostic(
            REFERENCE_CYCLE_CODE,
            vec![(LINE_DATA, identifier(&line_label(line)))],
        )
    }

    pub(crate) fn undefined_name(name: &str) -> Diagnostic {
        let mut data = vec![(NAME_DATA, identifier(name))];
        if let Some(constant) = constant_of_bare_symbol(name) {
            data.push((CONSTANT_DATA, identifier(constant.symbol)));
        }
        diagnostic(UNDEFINED_NAME_CODE, data)
    }

    fn undefined_name_in_line(
        &mut self,
        id: LineId,
        definition: &Definition,
        name: &str,
    ) -> Diagnostic {
        let input = self
            .line(id)
            .map(|line| line.input.clone())
            .unwrap_or_default();
        let is_unit_spelling = self.pool.units_mut().lookup(name).is_ok();
        let attempt = definition.unit_endings.iter().find_map(|ending| {
            let right_unit = ending.right_unit.as_ref()?;
            let applies = is_unit_spelling
                && ending.operator == Operator::Div
                && ending.quantity == ending.left_operand
                && input.get(right_unit.name.clone()) == Some(name);
            applies.then(|| unit_ended_at_space(&input, ending, right_unit))
        });
        attempt
            .flatten()
            .unwrap_or_else(|| Self::undefined_name(name))
    }

    pub(crate) fn expanded_expression(
        &mut self,
        id: LineId,
        unresolved_names: UnresolvedNames,
    ) -> Result<ExprId, Diagnostic> {
        self.expansion_of(id, unresolved_names, None)
    }

    fn expanded_with_named_ranges(
        &mut self,
        id: LineId,
        unresolved_names: UnresolvedNames,
    ) -> Result<(ExprId, Vec<calc_core::NamedRange>), Diagnostic> {
        let mut named = Vec::new();
        let expression = self.expansion_of(id, unresolved_names, Some(&mut named))?;
        Ok((expression, named))
    }

    fn expansion_of(
        &mut self,
        id: LineId,
        unresolved_names: UnresolvedNames,
        mut named_ranges: Option<&mut Vec<calc_core::NamedRange>>,
    ) -> Result<ExprId, Diagnostic> {
        let mut expansions: HashMap<LineId, ExprId> = HashMap::new();
        for line in self.ancestors_in_dependency_order(id) {
            let Some(definition) = self.definitions.get(&line).cloned() else {
                return Err(diagnostic(
                    "dependency_failed",
                    vec![(LINE_DATA, identifier(&line_label(line)))],
                ));
            };
            let mut replacements: HashMap<SymbolId, ExprId> = HashMap::new();
            for name in &definition.references {
                let resolved = self.resolve(name);
                if resolved.is_none()
                    && let Some(constant) = physical_constant(name)
                {
                    let expression = constant_expression(&mut self.pool, constant)
                        .map_err(|_| Self::undefined_name(name))?;
                    let symbol = self
                        .pool
                        .lookup_symbol(name)
                        .ok_or_else(|| Self::undefined_name(name))?;
                    replacements.insert(symbol, expression);
                    continue;
                }
                if resolved.is_none() && unresolved_names == UnresolvedNames::Free {
                    continue;
                }
                let dependency = resolved.ok_or_else(|| Self::undefined_name(name))?;
                let expansion = expansions
                    .get(&dependency)
                    .copied()
                    .ok_or_else(|| Self::undefined_name(name))?;
                let symbol = self
                    .pool
                    .lookup_symbol(name)
                    .ok_or_else(|| Self::undefined_name(name))?;
                replacements.insert(symbol, expansion);
            }
            let mut expansion = substitute_symbols(&mut self.pool, definition.value, &replacements)
                .and_then(|expansion| bound_by_name(&mut self.pool, expansion))
                .map_err(|_| diagnostic("substitution_failed", vec![]))?;
            if line != id
                && let Some(collected) = named_ranges.as_deref_mut()
                && let Some(name) = self.line(line).and_then(|found| found.name.clone())
            {
                let (tagged, found) = calc_core::named_ranges(&mut self.pool, expansion, &name)
                    .ok_or_else(|| diagnostic("substitution_failed", vec![]))?;
                expansion = tagged;
                collected.extend(found);
            }
            expansions.insert(line, expansion);
        }
        expansions
            .get(&id)
            .copied()
            .ok_or_else(|| diagnostic("substitution_failed", vec![]))
    }

    fn evaluate_line(&mut self, id: LineId) -> Outcome {
        if let Some(request) = self.solve_requests.get(&id).cloned() {
            return match self.solve_outcome(&request) {
                Ok(outcome) => outcome,
                Err(SessionError::InvalidRequest(error)) => Outcome::Error(error.diagnostic()),
                Err(_) => Outcome::Error(diagnostic(SOLVE_FAILED_CODE, vec![])),
            };
        }
        let Some(definition) = self.definitions.get(&id).cloned() else {
            return Outcome::NotEvaluated;
        };
        if definition.kind == StatementKind::FunctionNaming {
            return Outcome::Defined;
        }
        let started = self.clock.monotonic_nanoseconds();
        let mut dependencies = Vec::new();
        let mut free_names: Vec<String> = Vec::new();
        for name in &definition.references {
            let Some(dependency) = self.resolve(name) else {
                if physical_constant(name).is_some() {
                    continue;
                }
                if self.may_stay_free(&definition, name) {
                    if !free_names.contains(name) {
                        free_names.push(name.clone());
                    }
                    continue;
                }
                return Outcome::Error(self.undefined_name_in_line(id, &definition, name));
            };
            let is_function = self
                .definitions
                .get(&dependency)
                .is_some_and(|other| other.kind == StatementKind::FunctionNaming);
            let has_result = self
                .line(dependency)
                .is_some_and(|line| matches!(line.outcome, Outcome::Result(_)));
            if !is_function && !has_result {
                return Outcome::Error(diagnostic(
                    "dependency_failed",
                    vec![(LINE_DATA, identifier(&line_label(dependency)))],
                ));
            }
            if !dependencies.contains(&dependency) {
                dependencies.push(dependency);
            }
            if let Some(names) = self.free_names_of(dependency) {
                for name in names {
                    if !free_names.contains(&name) {
                        free_names.push(name);
                    }
                }
            }
        }
        let unresolved = if free_names.is_empty() {
            UnresolvedNames::Undefined
        } else {
            UnresolvedNames::Free
        };
        let expression = match self.expanded_expression(id, unresolved) {
            Ok(expression) => expression,
            Err(error) => return Outcome::Error(error),
        };
        let expression = self.inline_function_calls(expression).unwrap_or(expression);
        if let Some(outcome) = self.chemistry_answer(expression, &dependencies, started) {
            return outcome;
        }
        if is_indefinite_integral(&self.pool, expression) {
            return self.antiderivative_answer(expression, dependencies, started);
        }
        if let Some(found) = calc_core::taylor_polynomial(&mut self.pool, expression) {
            return self.taylor_answer(found, dependencies, started);
        }
        if calc_core::has_tolerance(&self.pool, expression) {
            return self.worst_case_answer(id, definition.value, unresolved, dependencies, started);
        }
        let free_names: Vec<String> = free_names
            .into_iter()
            .filter(|name| holds_the_free_name(&self.pool, expression, name))
            .collect();
        if let Some(error) = calc_core::taken_apart_outside_the_field(&self.pool, expression) {
            return Outcome::Error(exact_error_diagnostic(&mut self.pool, expression, error));
        }
        if calc_core::is_relation(&self.pool, expression) {
            return Outcome::Error(relation_is_a_claim(&self.pool, expression, &free_names));
        }
        if let Some(name) = free_names.first() {
            let name = name.clone();
            return match self.free_name_answer(expression, &free_names, dependencies, started) {
                Some(outcome) => outcome,
                None => Outcome::Error(self.undefined_name_in_line(id, &definition, &name)),
            };
        }
        let expression = match substituted_enclosures(&mut self.pool, expression) {
            Ok(expression) => expression,
            Err(error) => return Outcome::Error(error),
        };
        let is_measured = match has_uncertain_inputs(&self.pool, expression) {
            Ok(is_measured) => is_measured,
            Err(_) => return Outcome::Error(diagnostic(UNCERTAINTY_NOT_PROPAGATED_CODE, vec![])),
        };
        let central = if is_measured {
            match central_expression(&mut self.pool, expression) {
                Ok(central) => central,
                Err(error) => return Outcome::Error(uncertainty_diagnostic(&self.pool, &error)),
            }
        } else {
            expression
        };
        let evaluated = match (self.exactly_evaluated(central), self.settings.precision) {
            (ExactOutcome::Rational(evaluated), _) => Ok(*evaluated),
            (ExactOutcome::Failed(error), _) => Err(error),
            (ExactOutcome::NeedsMachine(error), Precision::Exact) => Err(error),
            (ExactOutcome::NeedsMachine(_), Precision::F64) => {
                self.machine_evaluation(central, evaluate_f64, Domain::F64)
            }
            (ExactOutcome::NeedsMachine(_), Precision::F32) => {
                self.machine_evaluation(central, evaluate_f32, Domain::F32)
            }
        };
        let (computed, backend) = match evaluated {
            Ok(evaluated) => evaluated,
            Err(error) => return Outcome::Error(error),
        };
        let computed = self.with_conversions_named(central, computed);
        let computed = if is_measured {
            match with_propagated_uncertainty(&mut self.pool, expression, computed) {
                Ok(propagated) => propagated,
                Err(error) => return Outcome::Error(uncertainty_diagnostic(&self.pool, &error)),
            }
        } else {
            computed
        };
        let computed = match temperature_difference_note(&self.pool, expression)
            .or_else(|| temperature_reading_note(&self.pool, expression))
        {
            Some(note) => {
                let mut method = computed.method().clone();
                method.notes.push(note);
                computed.with_method(method)
            }
            None => {
                let notes = temperature_difference_entry_notes(&self.pool, expression);
                if notes.is_empty() {
                    computed
                } else {
                    let mut method = computed.method().clone();
                    method.notes.extend(notes);
                    computed.with_method(method)
                }
            }
        };
        let computed = if holds_zero_to_the_zero(&mut self.pool, expression, false) {
            let mut method = computed.method().clone();
            method.notes.push(diagnostic(ZERO_TO_THE_ZERO_NOTE, vec![]));
            computed.with_method(method)
        } else {
            computed
        };
        let computed = match self.exact_route_note(&computed, expression) {
            Some(note) => {
                let mut method = computed.method().clone();
                method.notes.push(note);
                computed.with_method(method)
            }
            None => computed,
        };
        let computed = self.without_a_handed_in_infinity(computed, &dependencies);
        let finished = self.clock.monotonic_nanoseconds();
        Outcome::Result(Box::new(ResultRecord::new(
            computed,
            backend,
            dependencies,
            self.clock.now_utc(),
            Duration::from_nanos(finished.saturating_sub(started)),
            CalculatorVersion::current(),
        )))
    }

    fn may_stay_free(&mut self, definition: &Definition, name: &str) -> bool {
        let is_unit_spelling = self.pool.units_mut().lookup(name).is_ok();
        !is_unit_spelling
            && constant_of_bare_symbol(name).is_none()
            && crate::session_file::line_from_label(name).is_none()
            && !is_called(&self.pool, definition.value, name)
    }

    fn free_names_of(&self, line: LineId) -> Option<Vec<String>> {
        let Some(Outcome::Result(record)) = self.line(line).map(Line::outcome) else {
            return None;
        };
        free_names_in(record.computed())
    }

    fn free_name_answer(
        &mut self,
        expression: ExprId,
        free_names: &[String],
        dependencies: Vec<LineId>,
        started: u64,
    ) -> Option<Outcome> {
        if calc_core::is_relation(&self.pool, expression) {
            return None;
        }
        if let Some(division) = division_by_identical_zero(&mut self.pool, expression) {
            return Some(Outcome::Error(read_diagnostic(
                &self.pool,
                DIVISION_BY_ZERO_CODE,
                division,
                vec![],
            )));
        }
        let (normal_form, excluding) = normal_form_with_condition(&mut self.pool, expression)?;
        let mut method = Method::named(EXACT_EVALUATION_METHOD);
        if !excluding.is_empty() {
            method.notes.push(valid_where_note(&excluding));
        }
        if holds_zero_to_the_zero(&mut self.pool, expression, true) {
            method.notes.push(diagnostic(ZERO_TO_THE_ZERO_NOTE, vec![]));
        }
        let number = match self.pool.node(normal_form) {
            Ok(NodeView::Number(number)) => self.pool.number_value(number).ok().cloned(),
            _ => None,
        };
        let (kind, value) = match number {
            Some(number) => (ResultKind::ExactRational, ResultValue::Number(number)),
            None => {
                method.parameters.insert(
                    FREE_NAMES_PARAMETER.to_string(),
                    identifier(&free_names.join(", ")),
                );
                method.notes.push(diagnostic(
                    FREE_NAMES_CODE,
                    vec![(NAMES_DATA, identifier(&free_names.join(", ")))],
                ));
                let text = calc_syntax::print_expression(
                    &self.pool,
                    normal_form,
                    calc_syntax::PrintMode::Ascii,
                )
                .ok()?;
                (ResultKind::Symbolic, ResultValue::Expression(text))
            }
        };
        let computed = ComputedResult::new(kind, value, None, method).ok()?;
        let finished = self.clock.monotonic_nanoseconds();
        Some(Outcome::Result(Box::new(ResultRecord::new(
            computed,
            Vec::new(),
            dependencies,
            self.clock.now_utc(),
            Duration::from_nanos(finished.saturating_sub(started)),
            CalculatorVersion::current(),
        ))))
    }

    fn chemistry_result(
        &mut self,
        text: String,
        method: Method,
        dependencies: Vec<LineId>,
        started: u64,
    ) -> Outcome {
        let Ok(computed) = ComputedResult::new(
            ResultKind::Symbolic,
            ResultValue::Expression(text),
            None,
            method,
        ) else {
            return Outcome::Error(diagnostic("result_not_representable", vec![]));
        };
        let finished = self.clock.monotonic_nanoseconds();
        Outcome::Result(Box::new(ResultRecord::new(
            computed,
            Vec::new(),
            dependencies,
            self.clock.now_utc(),
            Duration::from_nanos(finished.saturating_sub(started)),
            CalculatorVersion::current(),
        )))
    }

    fn chemistry_answer(
        &mut self,
        expression: ExprId,
        dependencies: &[LineId],
        started: u64,
    ) -> Option<Outcome> {
        let head = match self.pool.node(expression) {
            Ok(NodeView::Apply {
                head: Head::Operator(operator),
                ..
            }) => Some(operator),
            _ => None,
        };
        match head {
            Some(Operator::Substance) => {
                return Some(self.substance_answer(expression, dependencies.to_vec(), started));
            }
            Some(Operator::Reaction) => {
                return Some(self.reaction_answer(expression, dependencies.to_vec(), started));
            }
            Some(Operator::Nuclide) => {
                return Some(self.nuclide_answer(expression, dependencies.to_vec(), started));
            }
            Some(Operator::NuclearReaction) => {
                return Some(self.nuclear_reaction_answer(
                    expression,
                    dependencies.to_vec(),
                    started,
                ));
            }
            _ => {}
        }
        let (stray, code) = chemistry_outside_a_property(&self.pool, expression)?;
        Some(Outcome::Error(read_diagnostic(
            &self.pool,
            code,
            stray,
            vec![],
        )))
    }

    fn substance_answer(
        &mut self,
        expression: ExprId,
        dependencies: Vec<LineId>,
        started: u64,
    ) -> Outcome {
        let Some(composition) = calc_core::composition_of(&self.pool, expression) else {
            return Outcome::Error(diagnostic("result_not_representable", vec![]));
        };
        let mut method = Method::named(CHEMISTRY_COMPOSITION_METHOD);
        if composition.charge != 0 {
            method.notes.push(diagnostic(
                "chemistry_charge",
                vec![("charge", identifier(&charge_text(composition.charge)))],
            ));
        }
        let text = composition_text(&composition.elements);
        self.chemistry_result(text, method, dependencies, started)
    }

    fn reaction_answer(
        &mut self,
        expression: ExprId,
        dependencies: Vec<LineId>,
        started: u64,
    ) -> Outcome {
        let not_representable = || Outcome::Error(diagnostic("result_not_representable", vec![]));
        let (Some(species), Some(nodes)) = (
            calc_core::reaction_of(&self.pool, expression),
            calc_core::species_nodes(&self.pool, expression),
        ) else {
            return not_representable();
        };
        let Some(texts) = nodes
            .iter()
            .map(|node| calc_core::written_text(&self.pool, *node))
            .collect::<Option<Vec<String>>>()
        else {
            return not_representable();
        };
        let quantities = calc_core::chemical_quantities(&species);
        let balancing = match calc_core::balance(&species, &quantities) {
            Ok(balancing) => balancing,
            Err(refusal) => {
                let (code, data) = match refusal {
                    calc_core::BalanceRefusal::TooManySpeciesForRays { limit, independent } => (
                        "chemistry_too_many_species",
                        vec![
                            ("limit", identifier(&limit.to_string())),
                            ("count", identifier(&independent.to_string())),
                        ],
                    ),
                    calc_core::BalanceRefusal::NoSpecies => ("chemistry_empty", vec![]),
                };
                return Outcome::Error(read_diagnostic(&self.pool, code, expression, data));
            }
        };
        let with_reading = |code: &str, data: Vec<(&str, ParameterValue)>, pool: &ExprPool| {
            Outcome::Error(read_diagnostic(pool, code, expression, data))
        };
        match balancing {
            calc_core::Balancing::Balanced {
                coefficients,
                totals,
            } => {
                let mut method = Method::named(CHEMISTRY_BALANCE_METHOD);
                method.notes.extend(balance_notes(&totals));
                let text = reaction_text(&texts, &species, &coefficients);
                self.chemistry_result(text, method, dependencies, started)
            }
            calc_core::Balancing::WrittenBalances { totals } => {
                let mut method = Method::named(CHEMISTRY_CHECK_METHOD);
                method.notes.extend(balance_notes(&totals));
                let written: Vec<Integer> = species
                    .iter()
                    .map(|one| Integer::from(one.written.unwrap_or(1)))
                    .collect();
                let text = reaction_text(&texts, &species, &written);
                self.chemistry_result(text, method, dependencies, started)
            }
            calc_core::Balancing::WrittenFails { first } => {
                let totals = vec![
                    (
                        "reactants",
                        identifier(&crate::decimal_text::integer_to_decimal(&first.reactants)),
                    ),
                    (
                        "products",
                        identifier(&crate::decimal_text::integer_to_decimal(&first.products)),
                    ),
                ];
                match first.quantity {
                    calc_core::ConservedQuantity::Element(element) => {
                        let mut data = totals;
                        data.push((
                            "element",
                            identifier(calc_syntax::element_symbol(element).unwrap_or_default()),
                        ));
                        with_reading("chemistry_element_not_conserved", data, &self.pool)
                    }
                    calc_core::ConservedQuantity::Charge => {
                        with_reading("chemistry_charge_not_conserved", totals, &self.pool)
                    }
                    calc_core::ConservedQuantity::NucleonNumber
                    | calc_core::ConservedQuantity::ElectronLeptonNumber => not_representable(),
                }
            }
            calc_core::Balancing::NotUnique {
                independent,
                reactions,
            } if reactions.is_empty() => with_reading(
                "chemistry_no_reaction_with_these_sides",
                vec![("count", identifier(&independent.to_string()))],
                &self.pool,
            ),
            calc_core::Balancing::NotUnique {
                independent,
                reactions,
            } => {
                let listed: Vec<String> = reactions
                    .iter()
                    .map(|coefficients| reaction_text(&texts, &species, coefficients))
                    .collect();
                with_reading(
                    "chemistry_not_unique",
                    vec![
                        ("count", identifier(&independent.to_string())),
                        ("reactions", identifier(&listed.join("; "))),
                    ],
                    &self.pool,
                )
            }
            calc_core::Balancing::Impossible => {
                with_reading("chemistry_impossible", vec![], &self.pool)
            }
            calc_core::Balancing::TakesNoPart(index) => with_reading(
                "chemistry_takes_no_part",
                vec![("species", identifier(&texts[index]))],
                &self.pool,
            ),
            calc_core::Balancing::OtherSide(index) => with_reading(
                "chemistry_other_side",
                vec![("species", identifier(&texts[index]))],
                &self.pool,
            ),
        }
    }

    fn nuclide_answer(
        &mut self,
        expression: ExprId,
        dependencies: Vec<LineId>,
        started: u64,
    ) -> Outcome {
        let Some(particle) = calc_core::nuclide_of(&self.pool, expression) else {
            return Outcome::Error(diagnostic("result_not_representable", vec![]));
        };
        let written = calc_core::written_text(&self.pool, expression).unwrap_or_default();
        let mut method = Method::named(NUCLEAR_NUCLIDE_METHOD);
        let text = match particle {
            calc_expr::NuclearParticle::Nuclide {
                mass_number,
                atomic_number,
            } => {
                let note = match calc_core::mass_status(particle) {
                    calc_core::MassStatus::Measured => "nuclear_measured_mass",
                    calc_core::MassStatus::Estimated => "nuclear_estimated_mass_note",
                    calc_core::MassStatus::NotInTable => "nuclear_no_mass_note",
                };
                method.notes.push(diagnostic(note, vec![]));
                format!(
                    "{written}: A {mass_number}, Z {atomic_number}, N {}",
                    mass_number - atomic_number
                )
            }
            other => format!(
                "{written}: A 0, charge {}, L_e {}",
                other.charge(),
                other.electron_leptons()
            ),
        };
        self.chemistry_result(text, method, dependencies, started)
    }

    fn nuclear_reaction_answer(
        &mut self,
        expression: ExprId,
        dependencies: Vec<LineId>,
        started: u64,
    ) -> Outcome {
        let not_representable = || Outcome::Error(diagnostic("result_not_representable", vec![]));
        let (Some(species), Some(nodes)) = (
            calc_core::nuclear_reaction_of(&self.pool, expression),
            calc_core::species_nodes(&self.pool, expression),
        ) else {
            return not_representable();
        };
        let Some(texts) = nodes
            .iter()
            .map(|node| calc_core::written_text(&self.pool, *node))
            .collect::<Option<Vec<String>>>()
        else {
            return not_representable();
        };
        let sides: Vec<bool> = species.iter().map(|one| one.is_product).collect();
        let has_photons = species
            .iter()
            .any(|one| one.particle == calc_expr::NuclearParticle::Photon);
        let with_reading = |code: &str, data: Vec<(&str, ParameterValue)>, pool: &ExprPool| {
            Outcome::Error(read_diagnostic(pool, code, expression, data))
        };
        let (coefficients, totals) = match calc_core::balance_nuclear(&species) {
            Ok(calc_core::NuclearBalancing::Checked {
                coefficients,
                totals,
            }) => (coefficients, totals),
            Ok(calc_core::NuclearBalancing::Fails {
                first,
                only_balance,
            }) => {
                let mut data = nuclear_side_values(&first);
                let code = match first.quantity {
                    calc_core::ConservedQuantity::NucleonNumber => {
                        "nuclear_mass_number_not_conserved"
                    }
                    calc_core::ConservedQuantity::ElectronLeptonNumber => {
                        data.push(("neutrinos", identifier(&missing_neutrinos(&first))));
                        "nuclear_lepton_number_not_conserved"
                    }
                    _ => "nuclear_charge_not_conserved",
                };
                let code = match only_balance {
                    Some(coefficients) => {
                        data.push((
                            "balance",
                            identifier(&reaction_line(&texts, &sides, &coefficients)),
                        ));
                        match code {
                            "nuclear_mass_number_not_conserved" => {
                                "nuclear_mass_number_not_conserved_balance"
                            }
                            "nuclear_lepton_number_not_conserved" => {
                                "nuclear_lepton_number_not_conserved_balance"
                            }
                            _ => "nuclear_charge_not_conserved_balance",
                        }
                    }
                    None => code,
                };
                return with_reading(code, data, &self.pool);
            }
            Err(_) => return with_reading("nuclear_empty", vec![], &self.pool),
        };
        let mut method = Method::named(NUCLEAR_CHECK_METHOD);
        method.notes.extend(nuclear_side_totals(&totals));
        if has_photons {
            method.notes.push(diagnostic("nuclear_photons", vec![]));
        }
        let text = reaction_line(&texts, &sides, &coefficients);
        self.chemistry_result(text, method, dependencies, started)
    }

    fn taylor_answer(
        &mut self,
        found: Result<(ExprId, bool), calc_core::TaylorRefusal>,
        dependencies: Vec<LineId>,
        started: u64,
    ) -> Outcome {
        let (polynomial, about_zero) = match found {
            Ok(found) => found,
            Err(refusal) => {
                let code = match refusal {
                    calc_core::TaylorRefusal::NotAnalytic => "taylor_not_analytic",
                    calc_core::TaylorRefusal::OrderNotWhole => "taylor_order_not_whole",
                    calc_core::TaylorRefusal::OrderTooHigh => "taylor_order_too_high",
                    calc_core::TaylorRefusal::NotDefinedAtPoint => "taylor_not_defined_at_point",
                };
                return Outcome::Error(diagnostic(code, vec![]));
            }
        };
        let written = if about_zero {
            calc_core::expanded(&mut self.pool, polynomial).unwrap_or(polynomial)
        } else {
            polynomial
        };
        let Some(text) = crate::displayed_value::exact_expression_text(&mut self.pool, written)
        else {
            return Outcome::Error(diagnostic("result_not_representable", vec![]));
        };
        let mut method = Method::named(EXACT_EVALUATION_METHOD);
        method
            .notes
            .push(diagnostic(TAYLOR_POLYNOMIAL_CODE, vec![]));
        let Ok(computed) = ComputedResult::new(
            ResultKind::Symbolic,
            ResultValue::Expression(text),
            None,
            method,
        ) else {
            return Outcome::Error(diagnostic("result_not_representable", vec![]));
        };
        let finished = self.clock.monotonic_nanoseconds();
        Outcome::Result(Box::new(ResultRecord::new(
            computed,
            Vec::new(),
            dependencies,
            self.clock.now_utc(),
            Duration::from_nanos(finished.saturating_sub(started)),
            CalculatorVersion::current(),
        )))
    }

    fn worst_case_answer(
        &mut self,
        id: LineId,
        written: ExprId,
        unresolved: UnresolvedNames,
        dependencies: Vec<LineId>,
        started: u64,
    ) -> Outcome {
        if let Some(twice) = tolerance_written_twice(&self.pool, written) {
            return Outcome::Error(read_diagnostic(
                &self.pool,
                "tolerance_written_twice",
                twice,
                vec![],
            ));
        }
        let (expression, named) = match self.expanded_with_named_ranges(id, unresolved) {
            Ok(found) => found,
            Err(error) => return Outcome::Error(error),
        };
        if let Some(function) = self.function_with_a_range(expression) {
            return Outcome::Error(diagnostic(
                "worst_case_range_in_function_body",
                vec![("name", identifier(&function))],
            ));
        }
        let expression = self.inline_function_calls(expression).unwrap_or(expression);
        let found = match calc_core::worst_case_with(&mut self.pool, expression, &named) {
            Some(Ok(found)) => found,
            Some(Err(refusal)) => {
                return Outcome::Error(self.worst_case_refusal(&refusal, written));
            }
            None => return Outcome::Error(diagnostic("result_not_representable", vec![])),
        };
        let decimal = self.line(id).is_some_and(|line| line.input.contains('.'))
            || contains_molar_mass(&self.pool, expression);
        let (Some((low, unit)), Some((high, _))) = (
            self.end_text(found.low, decimal),
            self.end_text(found.high, decimal),
        ) else {
            return Outcome::Error(diagnostic("result_not_representable", vec![]));
        };
        let unit_text = unit
            .and_then(|unit| crate::unit_display::display_text(&self.pool, unit))
            .map(|text| format!(" {text}"))
            .unwrap_or_default();
        let enclosed = |value: String| {
            if !unit_text.is_empty() && value.contains(['/', '*', ' ']) {
                format!("({value})")
            } else {
                value
            }
        };
        let text = format!(
            "between({}{unit_text}, {}{unit_text})",
            enclosed(low),
            enclosed(high)
        );
        let mut method = Method::named(WORST_CASE_METHOD);
        self.add_molar_mass_notes(expression, &mut method);
        self.add_q_value_notes(expression, &mut method);
        for (code, corner) in [
            ("worst_case_low", &found.low_corner),
            ("worst_case_high", &found.high_corner),
        ] {
            let parts: Vec<String> = corner
                .iter()
                .filter_map(|corner| {
                    let written = match &corner.name {
                        Some(name) => name.clone(),
                        None => {
                            print_expression(&self.pool, corner.range, PrintMode::Ascii).ok()?
                        }
                    };
                    let (value, unit) = self.end_text(corner.end, decimal)?;
                    let unit = unit
                        .and_then(|unit| crate::unit_display::display_text(&self.pool, unit))
                        .map(|text| format!(" {text}"))
                        .unwrap_or_default();
                    Some(format!("{written} = {value}{unit}"))
                })
                .collect();
            method.notes.push(diagnostic(
                code,
                vec![("corner", identifier(&parts.join("; ")))],
            ));
        }
        let Ok(computed) = ComputedResult::new(
            ResultKind::Symbolic,
            ResultValue::Expression(text),
            None,
            method,
        ) else {
            return Outcome::Error(diagnostic("result_not_representable", vec![]));
        };
        let finished = self.clock.monotonic_nanoseconds();
        Outcome::Result(Box::new(ResultRecord::new(
            computed,
            Vec::new(),
            dependencies,
            self.clock.now_utc(),
            Duration::from_nanos(finished.saturating_sub(started)),
            CalculatorVersion::current(),
        )))
    }

    fn end_text(&mut self, end: ExprId, decimal: bool) -> Option<(String, Option<UnitId>)> {
        let mut evaluation = evaluate_exact(&mut self.pool, end).ok()?;
        if evaluation.rational_value().is_none()
            && let Some(collected) = calc_core::expanded(&mut self.pool, evaluation.expression())
            && let Ok(reduced) = evaluate_exact(&mut self.pool, collected)
            && reduced.rational_value().is_some()
        {
            evaluation = reduced;
        }
        let Some(number) = evaluation.rational_value().cloned() else {
            let text = crate::displayed_value::exact_expression_text(
                &mut self.pool,
                evaluation.expression(),
            )?;
            return Some((text, evaluation.unit()));
        };
        let text = if decimal && crate::readable::terminates(&number) {
            crate::summary::number_text(&number)
        } else {
            crate::summary::value_text_in(&ResultValue::Number(number), RationalForm::Fraction)
        };
        Some((text, evaluation.unit()))
    }

    fn worst_case_refusal(
        &self,
        refusal: &calc_core::WorstCaseRefusal,
        written: ExprId,
    ) -> Diagnostic {
        use calc_core::WorstCaseRefusal;
        match refusal {
            WorstCaseRefusal::TooMany => diagnostic(
                "worst_case_too_many",
                vec![("limit", identifier(&calc_core::TOLERANCE_LIMIT.to_string()))],
            ),
            WorstCaseRefusal::EndpointNotExact(node) => {
                read_diagnostic(&self.pool, "worst_case_endpoint_not_exact", *node, vec![])
            }
            WorstCaseRefusal::EndpointNotEnclosed(node) => read_diagnostic(
                &self.pool,
                "worst_case_endpoint_not_enclosed",
                *node,
                vec![],
            ),
            WorstCaseRefusal::EndsOfTwoDimensions(node) => read_diagnostic(
                &self.pool,
                "worst_case_ends_of_two_dimensions",
                *node,
                vec![],
            ),
            WorstCaseRefusal::DivisorReachesZero(_) => read_diagnostic(
                &self.pool,
                "worst_case_divisor_reaches_zero",
                written,
                vec![],
            ),
            WorstCaseRefusal::EndpointsReversed(node) => {
                read_diagnostic(&self.pool, "worst_case_endpoints_reversed", *node, vec![])
            }
            WorstCaseRefusal::NegativeTolerance(node) => {
                read_diagnostic(&self.pool, "worst_case_negative_tolerance", *node, vec![])
            }
            WorstCaseRefusal::NotMonotone(node) => {
                read_diagnostic(&self.pool, "worst_case_not_monotone", *node, vec![])
            }
            WorstCaseRefusal::NotASubstance(node) => read_diagnostic(
                &self.pool,
                "chemistry_molar_mass_needs_a_substance",
                *node,
                vec![],
            ),
            WorstCaseRefusal::MolarMass(node, refusal) => match refusal {
                calc_core::MolarMassRefusal::NoStandardAtomicWeight(element) => read_diagnostic(
                    &self.pool,
                    "chemistry_no_standard_atomic_weight",
                    *node,
                    vec![(
                        "element",
                        identifier(calc_syntax::element_symbol(*element).unwrap_or_default()),
                    )],
                ),
                calc_core::MolarMassRefusal::Charged => {
                    read_diagnostic(&self.pool, "chemistry_molar_mass_of_an_ion", *node, vec![])
                }
                calc_core::MolarMassRefusal::NoElements => read_diagnostic(
                    &self.pool,
                    "chemistry_molar_mass_of_no_element",
                    *node,
                    vec![],
                ),
            },
            WorstCaseRefusal::QValue(node, refusal) => match refusal {
                calc_core::QValueRefusal::NotANuclearReaction => read_diagnostic(
                    &self.pool,
                    "nuclear_q_value_needs_a_reaction",
                    *node,
                    vec![],
                ),
                calc_core::QValueRefusal::DoesNotBalance => read_diagnostic(
                    &self.pool,
                    "nuclear_q_value_does_not_balance",
                    *node,
                    vec![],
                ),
                calc_core::QValueRefusal::EstimatedMass(particle) => read_diagnostic(
                    &self.pool,
                    "nuclear_estimated_mass",
                    *node,
                    vec![("nuclide", identifier(&nuclide_text(*particle)))],
                ),
                calc_core::QValueRefusal::NoMass(particle) => read_diagnostic(
                    &self.pool,
                    "nuclear_no_mass",
                    *node,
                    vec![("nuclide", identifier(&nuclide_text(*particle)))],
                ),
            },
            WorstCaseRefusal::Unsupported => diagnostic("worst_case_unsupported", vec![]),
        }
    }

    fn add_molar_mass_notes(&self, expression: ExprId, method: &mut Method) {
        let mut natural: Vec<u8> = Vec::new();
        let mut expanded: Vec<u8> = Vec::new();
        let mut pending = vec![expression];
        let mut any = false;
        while let Some(current) = pending.pop() {
            match self.pool.node(current) {
                Ok(NodeView::Apply {
                    head: Head::Operator(Operator::MolarMass),
                    arguments: [substance],
                }) => {
                    let range = calc_core::composition_of(&self.pool, *substance)
                        .and_then(|composition| calc_core::molar_mass(&composition).ok());
                    if let Some(range) = range {
                        any = true;
                        for (from, into) in [
                            (range.natural_interval, &mut natural),
                            (range.expanded_uncertainty, &mut expanded),
                        ] {
                            for element in from {
                                if !into.contains(&element) {
                                    into.push(element);
                                }
                            }
                        }
                    }
                }
                Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().copied()),
                Ok(NodeView::Quantity { value, .. }) => pending.push(value),
                _ => {}
            }
        }
        if !any {
            return;
        }
        method.parameters.insert(
            "atomic_weights".to_owned(),
            ParameterValue::Identifier(calc_core::ATOMIC_WEIGHT_TABLE.to_owned()),
        );
        method.parameters.insert(
            "molar_mass_constant".to_owned(),
            ParameterValue::Identifier(calc_core::MOLAR_MASS_CONSTANT_TABLE.to_owned()),
        );
        method.notes.push(diagnostic("chemistry_tables", vec![]));
        let symbols = |elements: &[u8]| {
            elements
                .iter()
                .filter_map(|element| calc_syntax::element_symbol(*element))
                .collect::<Vec<_>>()
                .join(", ")
        };
        if !natural.is_empty() {
            method.notes.push(diagnostic(
                "chemistry_natural_interval",
                vec![("elements", identifier(&symbols(&natural)))],
            ));
        }
        if !expanded.is_empty() {
            method.notes.push(diagnostic(
                "chemistry_expanded_uncertainty",
                vec![("elements", identifier(&symbols(&expanded)))],
            ));
        }
    }

    fn add_q_value_notes(&self, expression: ExprId, method: &mut Method) {
        let mut pending = vec![expression];
        let mut any = false;
        let mut notes: Vec<Diagnostic> = Vec::new();
        while let Some(current) = pending.pop() {
            match self.pool.node(current) {
                Ok(NodeView::Apply {
                    head: Head::Operator(Operator::QValue),
                    arguments: [reaction],
                }) => {
                    let Some(species) = calc_core::nuclear_reaction_of(&self.pool, *reaction)
                    else {
                        continue;
                    };
                    let Ok(calc_core::NuclearBalancing::Checked { coefficients, .. }) =
                        calc_core::balance_nuclear(&species)
                    else {
                        continue;
                    };
                    let Ok(range) = calc_core::q_value(&species, &coefficients) else {
                        continue;
                    };
                    any = true;
                    let present = |particle: calc_expr::NuclearParticle, is_product: bool| {
                        species.iter().zip(&coefficients).any(|(one, coefficient)| {
                            one.particle == particle
                                && one.is_product == is_product
                                && !coefficient.is_zero()
                        })
                    };
                    if range.is_decay && present(calc_expr::NuclearParticle::Electron, false) {
                        notes.push(diagnostic("nuclear_q_capture", vec![]));
                    }
                    if present(calc_expr::NuclearParticle::Positron, true) {
                        notes.push(diagnostic("nuclear_q_positron", vec![]));
                    }
                    if range.is_decay {
                        match range.sign {
                            Some(std::cmp::Ordering::Less) => {
                                notes.push(diagnostic("nuclear_q_decay_impossible", vec![]));
                            }
                            None => notes.push(diagnostic("nuclear_q_sign_undecided", vec![])),
                            Some(_) => {}
                        }
                    }
                }
                Ok(NodeView::Apply { arguments, .. }) => pending.extend(arguments.iter().copied()),
                Ok(NodeView::Quantity { value, .. }) => pending.push(value),
                _ => {}
            }
        }
        if !any {
            return;
        }
        method.parameters.insert(
            "atomic_masses".to_owned(),
            ParameterValue::Identifier(calc_core::ATOMIC_MASS_TABLE.to_owned()),
        );
        method.parameters.insert(
            "nuclear_constants".to_owned(),
            ParameterValue::Identifier(calc_core::NUCLEAR_CONSTANT_TABLE.to_owned()),
        );
        method.parameters.insert(
            "coverage_factor".to_owned(),
            ParameterValue::Identifier(calc_core::COVERAGE_FACTOR.to_string()),
        );
        method.notes.push(diagnostic("nuclear_q_tables", vec![]));
        method.notes.push(diagnostic("nuclear_q_atomic", vec![]));
        method
            .notes
            .push(diagnostic("nuclear_q_ground_state", vec![]));
        method.notes.extend(notes);
    }

    fn antiderivative_answer(
        &mut self,
        expression: ExprId,
        dependencies: Vec<LineId>,
        started: u64,
    ) -> Outcome {
        let evaluation = match evaluate_exact(&mut self.pool, expression) {
            Ok(evaluation) => evaluation,
            Err(error) => {
                return Outcome::Error(exact_error_diagnostic(&mut self.pool, expression, error));
            }
        };
        let found = evaluation.expression();
        let written = calc_core::expanded(&mut self.pool, found).unwrap_or(found);
        let Some(text) = crate::displayed_value::exact_expression_text(&mut self.pool, written)
        else {
            return Outcome::Error(diagnostic("result_not_representable", vec![]));
        };
        let mut method = Method::named(EXACT_EVALUATION_METHOD);
        method.notes.push(diagnostic(ANTIDERIVATIVE_CODE, vec![]));
        let Ok(computed) = ComputedResult::new(
            ResultKind::Symbolic,
            ResultValue::Expression(text),
            None,
            method,
        ) else {
            return Outcome::Error(diagnostic("result_not_representable", vec![]));
        };
        let finished = self.clock.monotonic_nanoseconds();
        Outcome::Result(Box::new(ResultRecord::new(
            computed,
            Vec::new(),
            dependencies,
            self.clock.now_utc(),
            Duration::from_nanos(finished.saturating_sub(started)),
            CalculatorVersion::current(),
        )))
    }

    fn exactly_evaluated(&mut self, expression: ExprId) -> ExactOutcome {
        if holds_a_conversion(&self.pool, expression) {
            self.take_further_backends();
        }
        evaluate_exactly(&mut self.pool, &self.backends, self.settings, expression)
    }

    fn without_a_handed_in_infinity(
        &self,
        computed: ComputedResult,
        dependencies: &[LineId],
    ) -> ComputedResult {
        let made_here = !dependencies.iter().any(|dependency| {
            matches!(
                self.line(*dependency).map(Line::outcome),
                Some(Outcome::Result(record)) if !holds_only_finite(record.computed().value())
            )
        });
        if made_here || !computed.method().notes.iter().any(is_a_finite_operand_note) {
            return computed;
        }
        let mut method = computed.method().clone();
        method.notes.retain(|note| !is_a_finite_operand_note(note));
        computed.with_method(method)
    }

    fn with_conversions_named(
        &mut self,
        expression: ExprId,
        computed: ComputedResult,
    ) -> ComputedResult {
        if computed.method().name != EXACT_EVALUATION_METHOD
            || computed.kind() == ResultKind::MachineFloat
        {
            return computed;
        }
        let conversions = conversions_below_root(&self.pool, expression);
        if conversions.is_empty() {
            return computed;
        }
        let mut method = computed.method().clone();
        EXACT_AFTER_CONVERSION_METHOD.clone_into(&mut method.name);
        for conversion in conversions {
            let mut uses = Vec::new();
            let machine = evaluated_conversions(
                &mut self.pool,
                &self.backends,
                self.settings,
                conversion,
                &mut uses,
            );
            if let Some(note) = machine_conversion_note(&mut self.pool, conversion, machine) {
                method.notes.push(note);
            }
        }
        computed.with_method(method)
    }

    fn exact_route_note(
        &mut self,
        computed: &ComputedResult,
        expression: ExprId,
    ) -> Option<Diagnostic> {
        if computed.kind() != ResultKind::MachineFloat {
            return None;
        }
        let value = scalar_number(computed.value())?;
        let RoundingError::Bound(bound) = computed.rounding_error() else {
            return None;
        };
        let bound = scalar_number(bound)?;
        if !bound_is_not_smaller(value, bound) {
            return None;
        }
        let (argument, operator) = written_conversion(&self.pool, expression)?;
        let exactly = evaluate_exact(&mut self.pool, argument).ok()?;
        if exactly.rational_value().is_some_and(is_zero) {
            return None;
        }
        Some(diagnostic(
            "bound_not_smaller_than_value",
            vec![(CONVERSION_DATA, identifier(operator))],
        ))
    }

    fn machine_evaluation(
        &mut self,
        expression: ExprId,
        evaluate: MachineEvaluator,
        width: Domain,
    ) -> Result<(ComputedResult, Vec<BackendUse>), Diagnostic> {
        self.take_further_backends();
        evaluate_in_machine(
            &mut self.pool,
            &self.backends,
            self.settings,
            expression,
            evaluate,
            width,
        )
    }

    pub fn latest_picture_generation(&self, line: LineId, slot: PictureSlot) -> Option<u64> {
        self.latest_picture_generations.get(&(line, slot)).copied()
    }

    pub fn cancel_plot(&mut self, line: LineId, slot: PictureSlot) {
        if let Some(cancellation) = self.picture_cancellations.remove(&(line, slot)) {
            cancellation.store(true, AtomicOrdering::SeqCst);
        }
        self.latest_picture_generations.remove(&(line, slot));
        self.pending_pictures.remove(&(line, slot));
        self.completed_pictures.remove(&(line, slot));
        self.cancel_read(line, slot);
    }

    pub fn cancel_read(&mut self, line: LineId, slot: PictureSlot) {
        if let Some(cancellation) = self.reading_cancellations.remove(&(line, slot)) {
            cancellation.store(true, AtomicOrdering::SeqCst);
        }
    }

    fn take_completed_picture(&mut self, place: (LineId, PictureSlot)) {
        let Some((plan, completion)) = self.pending_pictures.get(&place) else {
            return;
        };
        let Some(scene) = completion
            .lock()
            .ok()
            .and_then(|completed| completed.clone())
            .filter(|scene| scene.generation == plan.generation)
        else {
            return;
        };
        let plan = plan.clone();
        self.pending_pictures.remove(&place);
        self.completed_pictures.insert(place, (plan, scene));
    }

    fn plotted_expression(
        &mut self,
        line: LineId,
    ) -> Result<(ExprId, PlotVariables, String), PlotError> {
        let input = self
            .line(line)
            .ok_or(PlotError::UnknownLine(line))?
            .input()
            .to_string();
        let kind = match self.definitions.get(&line) {
            Some(definition) if !self.solve_requests.contains_key(&line) => definition.kind,
            _ => return Err(PlotError::NotPlottable(line)),
        };
        let expanded = self
            .expanded_expression(line, UnresolvedNames::Free)
            .map_err(PlotError::Expansion)?;
        let (expression, symbols) = if kind == StatementKind::FunctionNaming {
            let opened = open_lambda_chain(&mut self.pool, expanded)
                .map_err(|_| PlotError::NotPlottable(line))?;
            let body = self
                .inline_function_calls(opened.body)
                .ok_or(PlotError::NotPlottable(line))?;
            (body, opened.parameters)
        } else {
            let inlined = self
                .inline_function_calls(expanded)
                .ok_or(PlotError::NotPlottable(line))?;
            let symbols = plot_variables_of(&self.pool, inlined)
                .map_err(|_| PlotError::NotPlottable(line))?;
            (inlined, symbols)
        };
        let mut variables = Vec::with_capacity(symbols.len());
        for symbol in symbols {
            let name = self
                .pool
                .symbol_name(symbol)
                .map_err(|_| PlotError::NotPlottable(line))?
                .to_string();
            variables.push((symbol, name));
        }
        Ok((expression, variables, input))
    }

    fn free_variables(
        &mut self,
        line: LineId,
        expression: ExprId,
    ) -> Result<Vec<(SymbolId, String)>, PlotError> {
        let symbols =
            plot_variables_of(&self.pool, expression).map_err(|_| PlotError::NotPlottable(line))?;
        symbols
            .into_iter()
            .map(|symbol| {
                let name = self
                    .pool
                    .symbol_name(symbol)
                    .map_err(|_| PlotError::NotPlottable(line))?
                    .to_string();
                Ok((symbol, name))
            })
            .collect()
    }

    fn with_parameters(
        &mut self,
        line: LineId,
        expression: ExprId,
        variables: Vec<(SymbolId, String)>,
        parameters: &[PictureParameter],
    ) -> Result<(ExprId, Vec<(SymbolId, String)>), PlotError> {
        if parameters.is_empty() {
            return Ok((expression, variables));
        }
        let free = self.free_variables(line, expression)?;
        let mut replacements = HashMap::new();
        for parameter in parameters {
            let (symbol, _) = variables
                .iter()
                .chain(free.iter())
                .find(|(_, name)| *name == parameter.name)
                .ok_or_else(|| PlotError::UnknownParameter(parameter.name.clone()))?;
            let value = self
                .pool
                .number(parameter.value.clone())
                .map_err(|_| PlotError::NotPlottable(line))?;
            replacements.insert(*symbol, value);
        }
        let substituted = substitute_symbols(&mut self.pool, expression, &replacements)
            .map_err(|_| PlotError::NotPlottable(line))?;
        let had_axes = !variables.is_empty();
        let remaining: Vec<(SymbolId, String)> = variables
            .into_iter()
            .filter(|(symbol, _)| !replacements.contains_key(symbol))
            .collect();
        if had_axes && remaining.is_empty() {
            return Err(PlotError::NoAxisLeft(line));
        }
        Ok((substituted, remaining))
    }

    pub fn is_plottable(&mut self, line: LineId) -> bool {
        self.picture_defaults(line, None).is_ok()
    }

    pub fn picture_defaults(
        &mut self,
        line: LineId,
        parameters: Option<&[PictureParameter]>,
    ) -> Result<PictureDefaults, PlotError> {
        self.defaults_of(line, parameters, true)
    }

    pub fn default_picture(
        &mut self,
        line: LineId,
        parameters: Option<&[PictureParameter]>,
    ) -> Result<PictureDefaults, PlotError> {
        self.defaults_of(line, parameters, false)
    }

    fn defaults_of(
        &mut self,
        line: LineId,
        parameters: Option<&[PictureParameter]>,
        with_stored: bool,
    ) -> Result<PictureDefaults, PlotError> {
        let stored = self
            .line(line)
            .and_then(Line::picture)
            .filter(|_| with_stored)
            .cloned();
        if let Some(escape) = self.escape_time_lines.get(&line) {
            let name = escape.axis_name().to_owned();
            let views = match &stored {
                Some(picture) => stored_views(picture),
                None => Self::escape_time_views(&escape.form),
            };
            return Ok(PictureDefaults {
                axis_titles: views
                    .iter()
                    .map(|_| {
                        vec![
                            AxisTitle::RealPart(name.clone()),
                            AxisTitle::ImaginaryPart(name.clone()),
                        ]
                    })
                    .collect(),
                views,
                value_name: name,
                shape: PictureShape::EscapeTime,
                legend: Some(ColourLegend::EscapeTime),
            });
        }
        let parameters = match parameters {
            Some(parameters) => parameters.to_vec(),
            None => stored
                .as_ref()
                .map(|picture| picture.parameters.clone())
                .unwrap_or_default(),
        };
        let label = self
            .line(line)
            .and_then(Line::name)
            .map_or_else(|| line_label(line), str::to_owned);
        let (expression, variables, _) = self.plotted_expression(line)?;
        let (expression, variables) =
            self.with_parameters(line, expression, variables, &parameters)?;
        let is_space = stored
            .as_ref()
            .is_some_and(|picture| picture.views.iter().any(|view| view.camera.is_some()));
        let outline = outline_for(
            line,
            variables.len(),
            self.is_complex_expression(expression),
            is_space,
        )?;
        let views = match stored {
            Some(picture) => stored_views(&picture),
            None => default_views(outline),
        };
        let axis_titles = self.axis_titles_of(line, (expression, &variables), outline, &views)?;
        Ok(PictureDefaults {
            views,
            axis_titles,
            value_name: label,
            shape: outline.shape,
            legend: outline_legend(outline),
        })
    }

    pub fn axis_titles(
        &mut self,
        line: LineId,
        parameters: Option<&[PictureParameter]>,
        views: &[ViewState],
    ) -> Result<Vec<Vec<AxisTitle>>, PlotError> {
        if let Some(escape) = self.escape_time_lines.get(&line) {
            let name = escape.axis_name().to_owned();
            return Ok(views
                .iter()
                .map(|_| {
                    vec![
                        AxisTitle::RealPart(name.clone()),
                        AxisTitle::ImaginaryPart(name.clone()),
                    ]
                })
                .collect());
        }
        let parameters = parameters.unwrap_or_default().to_vec();
        let (expression, variables, _) = self.plotted_expression(line)?;
        let (expression, variables) =
            self.with_parameters(line, expression, variables, &parameters)?;
        let is_space = views.iter().any(|view| view.camera.is_some());
        let outline = outline_for(
            line,
            variables.len(),
            self.is_complex_expression(expression),
            is_space,
        )?;
        self.axis_titles_of(line, (expression, &variables), outline, views)
    }

    fn axis_titles_of(
        &mut self,
        line: LineId,
        (expression, variables): (ExprId, &[(SymbolId, String)]),
        outline: PictureOutline,
        views: &[ViewState],
    ) -> Result<Vec<Vec<AxisTitle>>, PlotError> {
        let label = self
            .line(line)
            .and_then(Line::name)
            .map_or_else(|| line_label(line), str::to_owned);
        let is_named = self.line(line).and_then(Line::name).is_some();
        let mut kinds: Vec<Vec<Option<QuantityKind>>> = Vec::with_capacity(views.len());
        for (index, view) in views.iter().enumerate() {
            let sampled = self.sampled_dimensions(view, outline.sampled_axes, index)?;
            let value = self.sampled_value_dimension(
                expression,
                variables,
                (view, index),
                outline.sampled_axes,
            )?;
            kinds.push(
                (0..view.axes.len())
                    .map(|axis| named_quantity(sampled.get(axis).copied().unwrap_or(value)))
                    .collect(),
            );
        }
        let titles = views
            .iter()
            .enumerate()
            .map(|(index, view)| {
                (0..view.axes.len())
                    .map(|axis| {
                        let variable = variables
                            .get(axis.min(variables.len().saturating_sub(1)))
                            .filter(|_| axis < outline.sampled_axes)
                            .map(|(_, name)| name.clone());
                        let kind = kinds
                            .get(index)
                            .and_then(|view| view.get(axis).copied())
                            .flatten()
                            .filter(|_| variable.is_some() || !is_named);
                        match (outline.shape, axis, variable) {
                            (PictureShape::ComplexGrid, 0, Some(name)) => AxisTitle::RealPart(name),
                            (PictureShape::ComplexGrid, _, Some(name)) => {
                                AxisTitle::ImaginaryPart(name)
                            }
                            (_, _, variable) => match kind {
                                Some(kind) => AxisTitle::Quantity(kind),
                                None => AxisTitle::Name(variable.unwrap_or_else(|| label.clone())),
                            },
                        }
                    })
                    .collect()
            })
            .collect();
        Ok(titles)
    }

    fn is_complex_expression(&self, root: ExprId) -> bool {
        let imaginary = BuiltinConstant::ImaginaryUnit.symbol();
        let mut pending = vec![root];
        let mut visited = HashSet::new();
        while let Some(expression) = pending.pop() {
            if !visited.insert(expression) {
                continue;
            }
            match self.pool.node(expression) {
                Ok(NodeView::Symbol(symbol)) if symbol == imaginary => return true,
                Ok(NodeView::Apply { arguments, .. }) => pending.extend_from_slice(arguments),
                Ok(NodeView::Bind {
                    arguments, body, ..
                }) => {
                    pending.extend_from_slice(arguments);
                    pending.push(body);
                }
                Ok(NodeView::Quantity { value, .. }) => pending.push(value),
                Ok(NodeView::Array { elements, .. }) => pending.extend_from_slice(elements),
                Ok(NodeView::Number(_) | NodeView::Symbol(_) | NodeView::Bound(_)) | Err(_) => {}
            }
        }
        false
    }

    fn unit_quantity(
        &mut self,
        text: &str,
        (view, axis): (usize, usize),
    ) -> Result<ExprId, PlotError> {
        let unknown = PlotError::UnknownUnit { view, axis };
        parse_expression(&mut self.pool, &format!("{UNIT_VALUE_PREFIX}{text}")).map_err(|_| unknown)
    }

    fn sampled_dimension(
        &mut self,
        unit: &AxisDisplayUnit,
        (view, axis): (usize, usize),
    ) -> Result<UnitDimension, PlotError> {
        match unit {
            AxisDisplayUnit::Coherent => Ok(UnitDimension::DIMENSIONLESS),
            AxisDisplayUnit::Unit(text) => {
                let unknown = PlotError::UnknownUnit { view, axis };
                let quantity = self.unit_quantity(text, (view, axis))?;
                let NodeView::Quantity { unit: parsed, .. } =
                    self.pool.node(quantity).map_err(|_| unknown.clone())?
                else {
                    return Err(unknown);
                };
                self.pool.units().dimension(parsed).map_err(|_| unknown)
            }
            AxisDisplayUnit::TemperatureScale(_) => Ok(UnitDimension::of_base(
                BaseDimension::ThermodynamicTemperature,
            )),
        }
    }

    fn sampled_dimensions(
        &mut self,
        view: &ViewState,
        sampled_axes: usize,
        view_index: usize,
    ) -> Result<Vec<UnitDimension>, PlotError> {
        let units: Vec<AxisDisplayUnit> = view
            .axes
            .iter()
            .take(sampled_axes)
            .map(|axis| axis.unit.clone())
            .collect();
        units
            .into_iter()
            .enumerate()
            .map(|(axis, unit)| self.sampled_dimension(&unit, (view_index, axis)))
            .collect()
    }

    fn sampled_value_dimension(
        &mut self,
        expression: ExprId,
        variables: &[(SymbolId, String)],
        (view, view_index): (&ViewState, usize),
        sampled_axes: usize,
    ) -> Result<UnitDimension, PlotError> {
        let units: Vec<(SymbolId, AxisDisplayUnit)> = variables
            .iter()
            .map(|(symbol, _)| *symbol)
            .zip(
                view.axes
                    .iter()
                    .take(sampled_axes)
                    .map(|axis| axis.unit.clone()),
            )
            .collect();
        let mut replacements = HashMap::new();
        for (axis, (symbol, unit)) in units.into_iter().enumerate() {
            let text = match unit {
                AxisDisplayUnit::Coherent => continue,
                AxisDisplayUnit::Unit(text) => text,
                AxisDisplayUnit::TemperatureScale(_) => COHERENT_TEMPERATURE.to_owned(),
            };
            let quantity = self.unit_quantity(&text, (view_index, axis))?;
            replacements.insert(symbol, quantity);
        }
        if replacements.is_empty() {
            return Ok(self.value_dimension(expression));
        }
        let substituted = substitute_symbols(&mut self.pool, expression, &replacements)
            .map_err(|_| PlotError::Sample(SampleError::ConversionNotBuilt(expression)))?;
        self.checked_value_dimension(substituted)
    }

    fn axis_unit(
        &mut self,
        unit: &AxisDisplayUnit,
        dimension: UnitDimension,
        view: usize,
        axis: usize,
    ) -> Result<AxisUnit, PlotError> {
        let other_dimension = PlotError::UnitOfOtherDimension { view, axis };
        match unit {
            AxisDisplayUnit::Coherent => {
                if dimension.is_dimensionless() {
                    return Ok(AxisUnit::dimensionless());
                }
                let coherent = self
                    .pool
                    .units_mut()
                    .named_coherent_unit(&dimension)
                    .map_err(|_| other_dimension.clone())?;
                Ok(AxisUnit::Coherent {
                    symbol: display_text(&self.pool, coherent).unwrap_or_default(),
                })
            }
            AxisDisplayUnit::Unit(text) => {
                let unknown = PlotError::UnknownUnit { view, axis };
                let quantity =
                    parse_expression(&mut self.pool, &format!("{UNIT_VALUE_PREFIX}{text}"))
                        .map_err(|_| unknown.clone())?;
                let NodeView::Quantity { unit: parsed, .. } =
                    self.pool.node(quantity).map_err(|_| unknown.clone())?
                else {
                    return Err(unknown);
                };
                let units = self.pool.units();
                if units.dimension(parsed).map_err(|_| unknown.clone())? != dimension {
                    return Err(other_dimension);
                }
                let scale = units.scale_factor(parsed).map_err(|_| unknown.clone())?;
                let factor = Number::fraction(scale.numerator(), scale.denominator())
                    .map_err(|_| unknown.clone())?;
                let pi_exponent = i8::try_from(scale.pi_exponent()).map_err(|_| unknown)?;
                Ok(AxisUnit::Unit {
                    factor,
                    pi_exponent,
                    symbol: display_text(&self.pool, parsed).unwrap_or_default(),
                })
            }
            AxisDisplayUnit::TemperatureScale(scale) => {
                if dimension != UnitDimension::of_base(BaseDimension::ThermodynamicTemperature) {
                    return Err(other_dimension);
                }
                Ok(AxisUnit::TemperatureScale {
                    factor: scale.factor(),
                    offset: scale.offset(),
                    symbol: scale.display_symbol().to_string(),
                })
            }
        }
    }

    fn escape_time_views(form: &calc_viz::EscapeTimeForm) -> Vec<ViewState> {
        let [real, imaginary] = calc_viz::escape_time_default_view(form);
        vec![ViewState {
            axes: [real, imaginary]
                .into_iter()
                .map(|range| AxisState {
                    range: Some(range),
                    unit: AxisDisplayUnit::Coherent,
                })
                .collect(),
            camera: None,
        }]
    }

    fn plot_escape_time(
        &mut self,
        escape: &EscapeTimeLine,
        request: &PlotRequest,
        events: Sender<PictureEvent>,
    ) -> Result<PlotJob, PlotError> {
        let line = request.line;
        if request
            .parameters
            .as_ref()
            .is_some_and(|parameters| !parameters.is_empty())
        {
            return Err(PlotError::ParametersNotApplicable(line));
        }
        let stored = self.line(line).and_then(Line::picture).cloned();
        let views = match (&request.views, &stored) {
            (Some(views), _) => views.clone(),
            (None, Some(picture)) => stored_views(picture),
            (None, None) => Self::escape_time_views(&escape.form),
        };
        if views.len() != 1 {
            return Err(PlotError::ViewCount {
                expected: 1,
                found: views.len(),
            });
        }
        let limit_rule = request
            .iteration_limit
            .or_else(|| stored.as_ref().and_then(|picture| picture.iteration_limit))
            .unwrap_or(IterationLimitRule::DEFAULT);
        let symbol = self
            .pool
            .intern_symbol(escape.axis_name(), SymbolKind::Variable)
            .map_err(|_| PlotError::NotPlottable(line))?;
        let mut axes = Vec::with_capacity(ESCAPE_TIME_AXES);
        for (axis_index, axis) in views[0].axes.iter().enumerate() {
            if views[0].axes.len() != ESCAPE_TIME_AXES {
                break;
            }
            let divisions = request
                .divisions
                .first()
                .and_then(|view| view.get(axis_index))
                .copied()
                .filter(|divisions| *divisions > 0)
                .ok_or(PlotError::DivisionsMissing {
                    view: 0,
                    axis: axis_index,
                })?;
            let bounds = match &axis.range {
                Some(range) => {
                    if range
                        .upper
                        .sub_exact(&range.lower)
                        .ok()
                        .is_none_or(|width| !is_positive(&width))
                    {
                        return Err(PlotError::EmptyInterval {
                            view: 0,
                            axis: axis_index,
                        });
                    }
                    AxisBounds::Exact(range.clone())
                }
                None => AxisBounds::Default,
            };
            axes.push(SampleAxis {
                symbol,
                name: escape.axis_name().to_owned(),
                bounds,
                divisions,
                dimension: Dimension::DIMENSIONLESS,
                unit: AxisUnit::dimensionless(),
            });
        }
        if axes.len() != ESCAPE_TIME_AXES {
            return Err(PlotError::ViewAxisCount {
                view: 0,
                expected: ESCAPE_TIME_AXES,
                found: views[0].axes.len(),
            });
        }
        let sampling = SampleRequest {
            result: u32::try_from(line.number())
                .map(ResultId)
                .map_err(|_| PlotError::LineNumberTooLarge(line))?,
            expression_text: self
                .line(line)
                .map(|line| line.input().to_owned())
                .unwrap_or_default(),
            shape: SampledShape::EscapeTime,
            axes,
            value_range: None,
            value_dimension: Dimension::DIMENSIONLESS,
            value_unit: AxisUnit::dimensionless(),
            value_divisions: ESCAPE_TIME_VALUE_DIVISIONS,
            domain: match self.settings.precision {
                Precision::F32 => Domain::F32,
                Precision::F64 | Precision::Exact => Domain::F64,
            },
            preference: self.settings.backend,
            sample_limit: ESCAPE_TIME_SAMPLE_LIMIT,
            kind: KindColour::Sampled,
            references: Vec::new(),
            escape_time: Some(EscapeTimeRequest {
                form: escape.form.clone(),
                limit_rule,
            }),
        };
        let plan =
            calc_viz::plan_refinement(&mut self.pool, &sampling).map_err(PlotError::Sample)?;
        let (address, cancellation, completion) =
            self.recorded_picture(line, request.slot, &views, (&[], Some(limit_rule)));
        Ok(PlotJob::new(
            PlotWork::Refinement(Box::new(plan)),
            vec![None],
            Arc::clone(&self.backends),
            cancellation,
            events,
            address,
            completion,
        ))
    }

    fn recorded_picture(
        &mut self,
        line: LineId,
        slot: PictureSlot,
        views: &[ViewState],
        (parameters, iteration_limit): (&[PictureParameter], Option<IterationLimitRule>),
    ) -> (PictureAddress, Arc<AtomicBool>, SceneCompletion) {
        let (address, cancellation) = self.next_picture_address(line, slot);
        let place = (line, slot);
        self.take_completed_picture(place);
        let completion: SceneCompletion = Arc::new(Mutex::new(None));
        self.pending_pictures.insert(
            place,
            (
                PicturePlanRecord {
                    generation: address.generation,
                    units: views
                        .iter()
                        .map(|view| view.axes.iter().map(|axis| axis.unit.clone()).collect())
                        .collect(),
                    cameras: views.iter().map(|view| view.camera.clone()).collect(),
                    parameters: parameters.to_vec(),
                    iteration_limit,
                },
                Arc::clone(&completion),
            ),
        );
        (address, cancellation, completion)
    }

    fn next_picture_address(
        &mut self,
        line: LineId,
        slot: PictureSlot,
    ) -> (PictureAddress, Arc<AtomicBool>) {
        let cancellation = Arc::new(AtomicBool::new(false));
        let place = (line, slot);
        if let Some(previous) = self
            .picture_cancellations
            .insert(place, Arc::clone(&cancellation))
        {
            previous.store(true, AtomicOrdering::SeqCst);
        }
        let generation = self.next_picture_generation;
        self.next_picture_generation = self.next_picture_generation.saturating_add(1);
        self.latest_picture_generations.insert(place, generation);
        (
            PictureAddress {
                line,
                slot,
                generation,
            },
            cancellation,
        )
    }

    pub fn plot(
        &mut self,
        request: &PlotRequest,
        events: Sender<PictureEvent>,
    ) -> Result<PlotJob, PlotError> {
        self.take_further_backends();
        let line = request.line;
        if let Some(escape) = self.escape_time_lines.get(&line).cloned() {
            return self.plot_escape_time(&escape, request, events);
        }
        if request.iteration_limit.is_some() {
            return Err(PlotError::IterationLimitNotApplicable(line));
        }
        let stored = self.line(line).and_then(Line::picture).cloned();
        let parameters = match &request.parameters {
            Some(parameters) => parameters.clone(),
            None => stored
                .as_ref()
                .map(|picture| picture.parameters.clone())
                .unwrap_or_default(),
        };
        let (expression, variables, input) = self.plotted_expression(line)?;
        let (expression, variables) =
            self.with_parameters(line, expression, variables, &parameters)?;
        let chosen = match (&request.views, &stored) {
            (Some(views), _) => Some(views.clone()),
            (None, Some(picture)) => Some(stored_views(picture)),
            (None, None) => None,
        };
        let is_space = chosen
            .as_ref()
            .is_some_and(|views| views.iter().any(|view| view.camera.is_some()));
        let outline = outline_for(
            line,
            variables.len(),
            self.is_complex_expression(expression),
            is_space,
        )?;
        let views = chosen.unwrap_or_else(|| default_views(outline));
        if views.len() != 1 {
            return Err(PlotError::ViewCount {
                expected: 1,
                found: views.len(),
            });
        }
        let mut axes = Vec::new();
        let mut value_dimension = self.value_dimension(expression);
        for (view_index, view) in views.iter().enumerate() {
            let sampled = self.sampled_dimensions(view, outline.sampled_axes, view_index)?;
            value_dimension = self.sampled_value_dimension(
                expression,
                &variables,
                (view, view_index),
                outline.sampled_axes,
            )?;
            if view.axes.len() != outline.view_axes {
                return Err(PlotError::ViewAxisCount {
                    view: view_index,
                    expected: outline.view_axes,
                    found: view.axes.len(),
                });
            }
            for (axis_index, axis) in view.axes.iter().enumerate() {
                let divisions = request
                    .divisions
                    .get(view_index)
                    .and_then(|view| view.get(axis_index))
                    .copied()
                    .filter(|divisions| *divisions > 0)
                    .ok_or(PlotError::DivisionsMissing {
                        view: view_index,
                        axis: axis_index,
                    })?;
                if let Some(range) = &axis.range
                    && range
                        .upper
                        .sub_exact(&range.lower)
                        .ok()
                        .is_none_or(|width| !is_positive(&width))
                {
                    return Err(PlotError::EmptyInterval {
                        view: view_index,
                        axis: axis_index,
                    });
                }
                let dimension = sampled.get(axis_index).copied().unwrap_or(value_dimension);
                let unit = self.axis_unit(&axis.unit, dimension, view_index, axis_index)?;
                axes.push(PlotAxis {
                    range: axis.range.clone(),
                    divisions,
                    dimension: Dimension {
                        exponents: dimension.exponents(),
                    },
                    unit,
                });
            }
        }
        let sampling = sample_request(&PlotPlan {
            line,
            outline,
            expression,
            variables: &variables,
            input: &input,
            axes,
            value_dimension: Dimension {
                exponents: value_dimension.exponents(),
            },
            settings: self.settings,
        })?;
        let run =
            calc_viz::prepare_sampling(&mut self.pool, &sampling).map_err(PlotError::Sample)?;
        let (address, cancellation, completion) =
            self.recorded_picture(line, request.slot, &views, (&parameters, None));
        Ok(PlotJob::new(
            PlotWork::Expression(Box::new(run)),
            views.iter().map(|view| view.camera.clone()).collect(),
            Arc::clone(&self.backends),
            cancellation,
            events,
            address,
            completion,
        ))
    }

    fn settle_escape_time_picture(&mut self, request: &SettleRequest) -> Result<LineId, PlotError> {
        let line = request.line;
        if !request.parameters.is_empty() {
            return Err(PlotError::ParametersNotApplicable(line));
        }
        if request.views.len() != 1 {
            return Err(PlotError::ViewCount {
                expected: 1,
                found: request.views.len(),
            });
        }
        let view = &request.views[0];
        if view.axes.len() != ESCAPE_TIME_AXES {
            return Err(PlotError::ViewAxisCount {
                view: 0,
                expected: ESCAPE_TIME_AXES,
                found: view.axes.len(),
            });
        }
        if view.camera.is_some() {
            return Err(PlotError::ViewKindMismatch(line));
        }
        let mut axes = Vec::with_capacity(ESCAPE_TIME_AXES);
        for (axis_index, axis) in view.axes.iter().enumerate() {
            let range = axis.range.as_ref().ok_or(PlotError::RangeMissing {
                view: 0,
                axis: axis_index,
            })?;
            if range
                .upper
                .sub_exact(&range.lower)
                .ok()
                .is_none_or(|width| !is_positive(&width))
            {
                return Err(PlotError::EmptyInterval {
                    view: 0,
                    axis: axis_index,
                });
            }
            self.axis_unit(&axis.unit, UnitDimension::DIMENSIONLESS, 0, axis_index)?;
            axes.push(crate::line_picture::PictureAxis {
                lower: range.lower.clone(),
                upper: range.upper.clone(),
                unit: axis.unit.clone(),
            });
        }
        let picture = Picture {
            views: vec![crate::line_picture::PictureView { axes, camera: None }],
            parameters: Vec::new(),
            iteration_limit: request.iteration_limit,
        };
        let stored = self
            .lines
            .iter_mut()
            .find(|stored| stored.id == line)
            .ok_or(PlotError::UnknownLine(line))?;
        stored.picture = Some(Box::new(picture));
        Ok(line)
    }

    pub fn settle_picture(&mut self, request: &SettleRequest) -> Result<LineId, PlotError> {
        let line = request.line;
        if self.escape_time_lines.contains_key(&line) {
            return self.settle_escape_time_picture(request);
        }
        if request.iteration_limit.is_some() {
            return Err(PlotError::IterationLimitNotApplicable(line));
        }
        let (expression, variables, _) = self.plotted_expression(line)?;
        let (expression, variables) =
            self.with_parameters(line, expression, variables, &request.parameters)?;
        let is_space = request.views.iter().any(|view| view.camera.is_some());
        let outline = outline_for(
            line,
            variables.len(),
            self.is_complex_expression(expression),
            is_space,
        )?;
        if request.views.len() != 1 {
            return Err(PlotError::ViewCount {
                expected: 1,
                found: request.views.len(),
            });
        }
        let mut settled = Vec::new();
        for (view_index, view) in request.views.iter().enumerate() {
            if view.axes.len() != outline.view_axes {
                return Err(PlotError::ViewAxisCount {
                    view: view_index,
                    expected: outline.view_axes,
                    found: view.axes.len(),
                });
            }
            if (outline.view_axes == SPACE_VIEW_AXES) != view.camera.is_some() {
                return Err(PlotError::ViewKindMismatch(line));
            }
            let sampled = self.sampled_dimensions(view, outline.sampled_axes, view_index)?;
            let value_dimension = self.sampled_value_dimension(
                expression,
                &variables,
                (view, view_index),
                outline.sampled_axes,
            )?;
            let mut axes = Vec::new();
            for (axis_index, axis) in view.axes.iter().enumerate() {
                let range = axis.range.as_ref().ok_or(PlotError::RangeMissing {
                    view: view_index,
                    axis: axis_index,
                })?;
                if range
                    .upper
                    .sub_exact(&range.lower)
                    .ok()
                    .is_none_or(|width| !is_positive(&width))
                {
                    return Err(PlotError::EmptyInterval {
                        view: view_index,
                        axis: axis_index,
                    });
                }
                let dimension = sampled.get(axis_index).copied().unwrap_or(value_dimension);
                self.axis_unit(&axis.unit, dimension, view_index, axis_index)?;
                axes.push(crate::line_picture::PictureAxis {
                    lower: range.lower.clone(),
                    upper: range.upper.clone(),
                    unit: axis.unit.clone(),
                });
            }
            settled.push(crate::line_picture::PictureView {
                axes,
                camera: view.camera.clone(),
            });
        }
        let picture = Picture {
            views: settled,
            parameters: request.parameters.clone(),
            iteration_limit: None,
        };
        let stored = self
            .lines
            .iter_mut()
            .find(|stored| stored.id == line)
            .ok_or(PlotError::UnknownLine(line))?;
        stored.picture = Some(Box::new(picture));
        Ok(line)
    }

    fn prepared_orbit(&mut self, request: &ReadRequest) -> Result<OrbitRequest, ReadError> {
        let escape = self
            .escape_time_lines
            .get(&request.line)
            .cloned()
            .ok_or(ReadError::ReadingNotBuilt)?;
        let (plan, completed, coordinates, _) = self.reading_coordinates(request)?;
        let axes = completed.views.first().ok_or(ReadError::SceneNotComplete)?;
        let real_axis = axes
            .first()
            .map(|axis| axis.range.clone())
            .ok_or(ReadError::SceneNotComplete)?;
        let (Some(real), Some(imaginary)) = (coordinates.first(), coordinates.get(1)) else {
            return Err(ReadError::CoordinateCount {
                expected: ESCAPE_TIME_AXES,
                found: coordinates.len(),
            });
        };
        Ok(OrbitRequest {
            form: escape.form,
            limit_rule: plan.iteration_limit.unwrap_or(IterationLimitRule::DEFAULT),
            real_axis,
            domain: match self.settings.precision {
                Precision::F32 => Domain::F32,
                Precision::F64 | Precision::Exact => Domain::F64,
            },
            preference: self.settings.backend,
            real: real.clone(),
            imaginary: imaginary.clone(),
        })
    }

    pub fn read(
        &mut self,
        request: &ReadRequest,
        events: Sender<ReadEvent>,
    ) -> Result<ReadJob, ReadError> {
        self.take_further_backends();
        if self.escape_time_lines.contains_key(&request.line) {
            let orbit = self.prepared_orbit(request)?;
            let place = (request.line, request.slot);
            let cancellation = Arc::new(AtomicBool::new(false));
            if let Some(previous) = self
                .reading_cancellations
                .insert(place, Arc::clone(&cancellation))
            {
                previous.store(true, AtomicOrdering::SeqCst);
            }
            return Ok(ReadJob::orbit(
                Box::new(orbit),
                Arc::clone(&self.backends),
                PictureAddress {
                    line: request.line,
                    slot: request.slot,
                    generation: request.generation,
                },
                events,
                cancellation,
            ));
        }
        let prepared = self.prepare_reading(request)?;
        let place = (request.line, request.slot);
        let cancellation = Arc::new(AtomicBool::new(false));
        if let Some(previous) = self
            .reading_cancellations
            .insert(place, Arc::clone(&cancellation))
        {
            previous.store(true, AtomicOrdering::SeqCst);
        }
        Ok(ReadJob::new(
            prepared.text,
            Arc::clone(&self.backends),
            self.settings,
            Arc::clone(&self.clock),
            PictureAddress {
                line: request.line,
                slot: request.slot,
                generation: request.generation,
            },
            events,
            cancellation,
        ))
    }

    fn commit_orbit_reading(&mut self, request: &ReadRequest) -> Result<LineId, ReadError> {
        let escape = self
            .escape_time_lines
            .get(&request.line)
            .cloned()
            .ok_or(ReadError::ReadingNotBuilt)?;
        let (plan, completed, coordinates, is_snapped) = self.reading_coordinates(request)?;
        let axes = completed.views.first().ok_or(ReadError::SceneNotComplete)?;
        let real_axis = axes
            .first()
            .map(|axis| axis.range.clone())
            .ok_or(ReadError::SceneNotComplete)?;
        let (Some(at_real), Some(at_imaginary)) = (coordinates.first(), coordinates.get(1)) else {
            return Err(ReadError::CoordinateCount {
                expected: ESCAPE_TIME_AXES,
                found: coordinates.len(),
            });
        };
        let limit = calc_viz::escape_time_limit(
            plan.iteration_limit.unwrap_or(IterationLimitRule::DEFAULT),
            &escape.form,
            &real_axis,
        )
        .ok_or(ReadError::ReadingNotBuilt)?;
        let reading_line = EscapeTimeReadingLine {
            form: escape.form,
            at_real: at_real.clone(),
            at_imaginary: at_imaginary.clone(),
            limit,
        };
        let id = self
            .enter(&reading_line.line_text())
            .map_err(|error| ReadError::NotEntered(Box::new(error)))?;
        let view = crate::line_picture::PictureView {
            axes: axes
                .iter()
                .enumerate()
                .map(|(index, axis)| crate::line_picture::PictureAxis {
                    lower: axis.range.lower.clone(),
                    upper: axis.range.upper.clone(),
                    unit: plan
                        .units
                        .first()
                        .and_then(|units| units.get(index))
                        .cloned()
                        .unwrap_or(AxisDisplayUnit::Coherent),
                })
                .collect(),
            camera: None,
        };
        if let Some(line) = self.lines.iter_mut().find(|line| line.id == id) {
            line.reading = Some(Box::new(LineReading {
                source: request.line,
                view,
                divisions: axes.iter().map(|axis| axis.divisions).collect(),
                at: coordinates,
                snapped: is_snapped,
            }));
        }
        Ok(id)
    }

    pub fn commit_reading(&mut self, request: &ReadRequest) -> Result<LineId, ReadError> {
        if self.escape_time_lines.contains_key(&request.line) {
            return self.commit_orbit_reading(request);
        }
        let prepared = self.prepare_reading(request)?;
        let call = prepared
            .call
            .ok_or(ReadError::ReadingNeedsFunctionForm(request.line))?;
        let id = self
            .enter(&call)
            .map_err(|error| ReadError::NotEntered(Box::new(error)))?;
        if let Some(line) = self.lines.iter_mut().find(|line| line.id == id) {
            line.reading = Some(Box::new(prepared.reading));
        }
        Ok(id)
    }

    fn reading_coordinates(
        &mut self,
        request: &ReadRequest,
    ) -> Result<(PicturePlanRecord, CompletedScene, Vec<Number>, bool), ReadError> {
        let place = (request.line, request.slot);
        self.take_completed_picture(place);
        let (plan, completed) = self
            .completed_pictures
            .get(&place)
            .filter(|(plan, _)| plan.generation == request.generation)
            .cloned()
            .ok_or(ReadError::SceneNotComplete)?;
        let names = match completed
            .layers
            .get(request.layer)
            .and_then(|readings| readings.first())
        {
            Some(Reading::ValueAt { variables }) => variables.clone(),
            None => {
                return Err(ReadError::LayerHasNoReadings {
                    layer: request.layer,
                });
            }
        };
        if request.at.len() != names.len() {
            return Err(ReadError::CoordinateCount {
                expected: names.len(),
                found: request.at.len(),
            });
        }
        let axes = completed.views.first().ok_or(ReadError::SceneNotComplete)?;
        let mut coordinates = Vec::new();
        let mut is_snapped = false;
        for (index, coordinate) in request.at.iter().enumerate() {
            let value = match coordinate {
                ReadCoordinate::Exact(value) => value.clone(),
                ReadCoordinate::Pointer(pointer) => {
                    let axis = axes
                        .get(index)
                        .ok_or(ReadError::CoordinateNotExact { axis: index })?;
                    let exact = Number::F64(*pointer)
                        .to_exact()
                        .map_err(|_| ReadError::CoordinateNotExact { axis: index })?;
                    let step = axis
                        .range
                        .upper
                        .sub_exact(&axis.range.lower)
                        .ok()
                        .and_then(|width| {
                            width
                                .div_exact(&Number::from(i64::from(axis.divisions)))
                                .ok()
                        })
                        .ok_or(ReadError::CoordinateNotExact { axis: index })?;
                    is_snapped = true;
                    snapped_coordinate(&exact, &step)
                        .ok_or(ReadError::CoordinateNotExact { axis: index })?
                }
            };
            coordinates.push(value);
        }
        Ok((plan, completed, coordinates, is_snapped))
    }

    fn prepare_reading(&mut self, request: &ReadRequest) -> Result<PreparedReading, ReadError> {
        let (plan, completed, coordinates, is_snapped) = self.reading_coordinates(request)?;
        let axes = completed.views.first().ok_or(ReadError::SceneNotComplete)?;
        let names = match completed
            .layers
            .get(request.layer)
            .and_then(|readings| readings.first())
        {
            Some(Reading::ValueAt { variables }) => variables.clone(),
            None => {
                return Err(ReadError::LayerHasNoReadings {
                    layer: request.layer,
                });
            }
        };
        let (expression, variables, _) = self
            .plotted_expression(request.line)
            .map_err(ReadError::Plot)?;
        let (expression, variables) = self
            .with_parameters(request.line, expression, variables, &plan.parameters)
            .map_err(ReadError::Plot)?;
        let mut replacements = HashMap::new();
        for (name, value) in names.iter().zip(&coordinates) {
            let Some((symbol, _)) = variables.iter().find(|(_, variable)| variable == name) else {
                continue;
            };
            let node = self
                .pool
                .number(value.clone())
                .map_err(|_| ReadError::ReadingNotBuilt)?;
            replacements.insert(*symbol, node);
        }
        let read_at = substitute_symbols(&mut self.pool, expression, &replacements)
            .map_err(|_| ReadError::ReadingNotBuilt)?;
        let text = print_expression(&self.pool, read_at, PrintMode::Ascii)
            .map_err(|_| ReadError::ReadingNotBuilt)?;
        let call = self.reading_call(request.line, &coordinates)?;
        let view = crate::line_picture::PictureView {
            axes: axes
                .iter()
                .enumerate()
                .map(|(index, axis)| crate::line_picture::PictureAxis {
                    lower: axis.range.lower.clone(),
                    upper: axis.range.upper.clone(),
                    unit: plan
                        .units
                        .first()
                        .and_then(|units| units.get(index))
                        .cloned()
                        .unwrap_or(AxisDisplayUnit::Coherent),
                })
                .collect(),
            camera: plan.cameras.first().cloned().flatten(),
        };
        Ok(PreparedReading {
            text,
            call,
            reading: LineReading {
                source: request.line,
                view,
                divisions: axes.iter().map(|axis| axis.divisions).collect(),
                at: coordinates,
                snapped: is_snapped,
            },
        })
    }

    fn reading_call(
        &mut self,
        line: LineId,
        coordinates: &[Number],
    ) -> Result<Option<String>, ReadError> {
        let is_function = self
            .definitions
            .get(&line)
            .is_some_and(|definition| definition.kind == StatementKind::FunctionNaming);
        let Some(name) = self.line(line).and_then(Line::name).map(str::to_string) else {
            return Ok(None);
        };
        if !is_function {
            return Ok(None);
        }
        let mut arguments = Vec::new();
        for value in coordinates {
            let node = self
                .pool
                .number(value.clone())
                .map_err(|_| ReadError::ReadingNotBuilt)?;
            arguments.push(
                print_expression(&self.pool, node, PrintMode::Ascii)
                    .map_err(|_| ReadError::ReadingNotBuilt)?,
            );
        }
        Ok(Some(format!("{name}({})", arguments.join(", "))))
    }

    fn checked_value_dimension(&mut self, expression: ExprId) -> Result<UnitDimension, PlotError> {
        let coherent = match to_coherent_units(&mut self.pool, expression) {
            Ok(coherent) => coherent,
            Err(error) => {
                return Err(PlotError::Expansion(quantity_error_diagnostic(
                    &self.pool, error,
                )));
            }
        };
        Ok(coherent
            .unit
            .and_then(|unit| self.pool.units().dimension(unit).ok())
            .unwrap_or(UnitDimension::DIMENSIONLESS))
    }

    fn value_dimension(&mut self, expression: ExprId) -> UnitDimension {
        to_coherent_units(&mut self.pool, expression)
            .ok()
            .and_then(|coherent| coherent.unit)
            .and_then(|unit| self.pool.units().dimension(unit).ok())
            .unwrap_or(UnitDimension::DIMENSIONLESS)
    }

    fn function_with_a_range(&self, expression: ExprId) -> Option<String> {
        let mut pending = vec![expression];
        while let Some(node) = pending.pop() {
            match self.pool.node(node) {
                Ok(NodeView::Apply { head, arguments }) => {
                    if let Head::Function(symbol) = head
                        && let Ok(name) = self.pool.symbol_name(symbol)
                        && let Some(line) = self.resolve(name)
                        && let Some(definition) = self.definitions.get(&line)
                        && definition.kind == StatementKind::FunctionNaming
                        && calc_core::has_tolerance(&self.pool, definition.value)
                    {
                        return Some(name.to_string());
                    }
                    pending.extend(arguments.iter().copied());
                }
                Ok(NodeView::Quantity { value, .. }) => pending.push(value),
                _ => {}
            }
        }
        None
    }

    fn inline_function_calls(&mut self, expression: ExprId) -> Option<ExprId> {
        let view = self.pool.node(expression).ok()?;
        let rebuilt = match view {
            NodeView::Number(_) | NodeView::Symbol(_) | NodeView::Bound(_) => expression,
            NodeView::Apply { head, arguments } => {
                let arguments = arguments.to_vec();
                let inlined = arguments
                    .into_iter()
                    .map(|argument| self.inline_function_calls(argument))
                    .collect::<Option<Vec<_>>>()?;
                let called = match head {
                    Head::Function(symbol) => {
                        let name = self.pool.symbol_name(symbol).ok()?.to_string();
                        self.resolve(&name).filter(|line| {
                            self.definitions.get(line).is_some_and(|definition| {
                                definition.kind == StatementKind::FunctionNaming
                            })
                        })
                    }
                    Head::Operator(_) => None,
                };
                match called {
                    Some(line) => {
                        let function =
                            self.expanded_expression(line, UnresolvedNames::Free).ok()?;
                        let applied = apply_lambda(&mut self.pool, function, &inlined).ok()?;
                        self.inline_function_calls(applied)?
                    }
                    None => self.pool.apply(head, &inlined).ok()?,
                }
            }
            NodeView::Bind {
                binder,
                arguments,
                body,
            } => {
                let arguments = arguments.to_vec();
                let inlined = arguments
                    .into_iter()
                    .map(|argument| self.inline_function_calls(argument))
                    .collect::<Option<Vec<_>>>()?;
                let body = self.inline_function_calls(body)?;
                let name = self.pool.bound_name(expression).map(str::to_string);
                let rebuilt = self.pool.bind(binder, &inlined, body).ok()?;
                if let Some(name) = name {
                    self.pool.record_bound_name(rebuilt, &name).ok()?;
                }
                rebuilt
            }
            NodeView::Quantity { value, unit } => {
                let value = self.inline_function_calls(value)?;
                self.pool.quantity(value, unit).ok()?
            }
            NodeView::Array { shape, elements } => {
                let shape = shape.to_vec();
                let elements = elements.to_vec();
                let inlined = elements
                    .into_iter()
                    .map(|element| self.inline_function_calls(element))
                    .collect::<Option<Vec<_>>>()?;
                self.pool.array(&shape, &inlined).ok()?
            }
        };
        Some(rebuilt)
    }
}

fn machine_conversion_in(pool: &ExprPool, expression: ExprId) -> Option<ExprId> {
    match pool.node(expression).ok()? {
        NodeView::Apply {
            head: Head::Operator(Operator::ToF64 | Operator::ToF32),
            ..
        } => Some(expression),
        NodeView::Apply { arguments, .. } => arguments
            .iter()
            .find_map(|argument| machine_conversion_in(pool, *argument)),
        _ => None,
    }
}

fn evaluate_array(
    pool: &mut ExprPool,
    array: Result<calc_core::ArrayExpression, calc_core::ArrayError>,
) -> ExactOutcome {
    let array = match array {
        Ok(array) => array,
        Err(error) => return ExactOutcome::Failed(ordering_diagnostic(pool, error)),
    };
    let mut values = Vec::with_capacity(array.elements.len());
    let mut unit = None;
    let mut first_entry: Option<ExprId> = None;
    let mut written = Vec::with_capacity(array.elements.len());
    let mut widest = ResultKind::ExactRational;
    for element in &array.elements {
        let evaluation = match evaluate_exact(pool, *element) {
            Ok(evaluation) => evaluation,
            Err(error) => {
                let diagnostic = match machine_conversion_in(pool, *element) {
                    Some(conversion) => {
                        read_diagnostic(pool, "array_machine_entry", conversion, vec![])
                    }
                    None => exact_error_diagnostic(pool, *element, error),
                };
                return ExactOutcome::Failed(diagnostic);
            }
        };
        written.push(evaluation.expression());
        let Some(number) = evaluation.rational_value() else {
            widest = match (widest, evaluation.kind()) {
                (ResultKind::Symbolic, _) | (_, ResultKind::Symbolic) => ResultKind::Symbolic,
                (_, kind @ ResultKind::Algebraic) => kind,
                (kind, _) => kind,
            };
            if evaluation.unit().is_some()
                || !matches!(widest, ResultKind::Algebraic | ResultKind::Symbolic)
            {
                return ExactOutcome::Failed(diagnostic(
                    "result_kind_not_representable",
                    vec![(KIND_DATA, identifier(kind_name(evaluation.kind())))],
                ));
            }
            continue;
        };
        if values.is_empty() {
            unit = evaluation.unit();
            first_entry = Some(*element);
        } else if unit != evaluation.unit() {
            if let Some(mismatch) =
                entry_kinds_differ(pool, first_entry, *element, unit, evaluation.unit())
            {
                return ExactOutcome::Failed(mismatch);
            }
            return ExactOutcome::Failed(array_diagnostic(
                pool,
                calc_core::ArrayError::UnitsDiffer,
            ));
        }
        values.push(ResultValue::Number(number.clone()));
    }
    if widest != ResultKind::ExactRational {
        return array_of_exact_expressions(pool, &array.shape, &written, widest);
    }
    match ComputedResult::new(
        ResultKind::ExactRational,
        ResultValue::Array {
            shape: array.shape.iter().map(|length| *length as usize).collect(),
            elements: values,
        },
        unit,
        Method::named(EXACT_EVALUATION_METHOD),
    ) {
        Ok(computed) => ExactOutcome::Rational(Box::new((computed, Vec::new()))),
        Err(_) => ExactOutcome::Failed(diagnostic("result_not_representable", vec![])),
    }
}

fn entry_text(pool: &mut ExprPool, element: ExprId) -> Option<String> {
    let text = crate::displayed_value::entry_expression_text(pool, element)?;
    let is_rational = matches!(pool.node(element), Ok(NodeView::Number(_)));
    Some(if is_rational {
        text.replace(SPACED_DIVISION, WRITTEN_DIVISION)
    } else {
        text
    })
}

fn array_text(pool: &mut ExprPool, shape: &[u32], elements: &[ExprId]) -> Option<String> {
    let texts = elements
        .iter()
        .map(|element| entry_text(pool, *element))
        .collect::<Option<Vec<_>>>()?;
    let row_length = match shape {
        [_] => texts.len(),
        [_, columns] => usize::try_from(*columns).ok()?,
        _ => return None,
    };
    let rows: Vec<String> = texts
        .chunks(row_length.max(1))
        .map(|row| row.join(ENTRY_SEPARATOR))
        .collect();
    Some(format!("[{}]", rows.join(ROW_SEPARATOR)))
}

fn array_of_exact_expressions(
    pool: &mut ExprPool,
    shape: &[u32],
    elements: &[ExprId],
    kind: ResultKind,
) -> ExactOutcome {
    let Some(text) = array_text(pool, shape, elements) else {
        return ExactOutcome::Failed(diagnostic("result_not_representable", vec![]));
    };
    match ComputedResult::new(
        kind,
        ResultValue::Expression(text),
        None,
        Method::named(EXACT_EVALUATION_METHOD),
    ) {
        Ok(computed) => ExactOutcome::Rational(Box::new((computed, Vec::new()))),
        Err(_) => ExactOutcome::Failed(diagnostic("result_not_representable", vec![])),
    }
}

impl Session {
    pub fn sort_trace(&mut self, id: LineId) -> Option<Vec<crate::sort_summary::TraceStep>> {
        let expression = self
            .expanded_expression(id, UnresolvedNames::Undefined)
            .ok()?;
        let expression = self.inline_function_calls(expression).unwrap_or(expression);
        let expression = calc_core::reduce_arrays(&mut self.pool, expression).ok()?;
        let (_, report) =
            calc_core::named_sort(&mut self.pool, expression, calc_sort::StepRecording::Record)?
                .ok()?;
        let values = report
            .records
            .iter()
            .map(|record| traced_text(&mut self.pool, *record))
            .collect::<Option<Vec<_>>>()?;
        let keys = match &report.keys {
            None => None,
            Some(keys) => Some(
                keys.iter()
                    .map(|key| {
                        let value = evaluate_exact(&mut self.pool, *key)
                            .ok()
                            .map_or(*key, |evaluation| evaluation.expression());
                        traced_text(&mut self.pool, value)
                    })
                    .collect::<Option<Vec<_>>>()?,
            ),
        };
        Some(crate::sort_summary::trace_steps(
            &report,
            &values,
            keys.as_deref(),
        ))
    }
}

fn traced_text(pool: &mut ExprPool, expression: ExprId) -> Option<String> {
    match pool.node(expression).ok()? {
        NodeView::Array { shape, elements } => {
            let (shape, elements) = (shape.to_vec(), elements.to_vec());
            array_text(pool, &shape, &elements)
        }
        _ => entry_text(pool, expression),
    }
}

fn keys_differ_in_dimension(
    pool: &mut ExprPool,
    (first, first_key): (usize, ExprId),
    (second, second_key): (usize, ExprId),
    by_key: bool,
) -> Option<Diagnostic> {
    let dimension = |pool: &mut ExprPool, key: ExprId| {
        let unit = evaluate_exact(pool, key).ok()?.unit();
        match unit {
            Some(unit) => pool.units().dimension(unit).ok(),
            None => Some(UnitDimension::DIMENSIONLESS),
        }
    };
    let left = dimension(pool, first_key)?;
    let right = dimension(pool, second_key)?;
    let pair = pool.array(&[2], &[first_key, second_key]).ok()?;
    let position =
        |index: usize| count_value(u64::try_from(index).map_or(u64::MAX, |index| index + 1));
    let text = |pool: &mut ExprPool, key: ExprId| {
        let shown = match evaluate_exact(pool, key) {
            Ok(evaluation) if by_key => match evaluation.unit() {
                Some(unit) => pool.quantity(evaluation.expression(), unit).unwrap_or(key),
                None => evaluation.expression(),
            },
            _ => key,
        };
        grouped_text(pool, shown).map(|text| ParameterValue::Value(ResultValue::Expression(text)))
    };
    let mut data = crate::dimension_mismatch::mismatch_data(pool, pair, &left, &right);
    data.extend([
        (FIRST_DATA, text(pool, first_key)?),
        (SECOND_DATA, text(pool, second_key)?),
        (FIRST_POSITION_DATA, position(first)),
        (SECOND_POSITION_DATA, position(second)),
    ]);
    let code = if by_key {
        "sort_keyed_keys_differ_in_dimension"
    } else {
        "sort_keys_differ_in_dimension"
    };
    Some(diagnostic(code, data))
}

fn entry_kinds_differ(
    pool: &mut ExprPool,
    first: Option<ExprId>,
    other: ExprId,
    first_unit: Option<UnitId>,
    other_unit: Option<UnitId>,
) -> Option<Diagnostic> {
    let dimension_of = |pool: &ExprPool, unit: Option<UnitId>| match unit {
        Some(unit) => pool.units().dimension(unit).ok(),
        None => Some(UnitDimension::DIMENSIONLESS),
    };
    let left = dimension_of(pool, first_unit)?;
    let right = dimension_of(pool, other_unit)?;
    if left == right {
        return None;
    }
    let pair = pool.array(&[2], &[first?, other]).ok()?;
    let data = crate::dimension_mismatch::mismatch_data(pool, pair, &left, &right);
    Some(read_diagnostic(pool, "dimension_mismatch", pair, data))
}

fn sort_method(report: &calc_core::SortReport) -> Method {
    let mut method = Method::named(sort_method_name(report.spec.method));
    if let calc_expr::SortMethod::Quick(partition) = report.spec.method {
        method.parameters.insert(
            crate::sort_summary::PARTITION_PARAMETER.to_string(),
            identifier(crate::sort_summary::partition_name(partition)),
        );
    }
    if let Some(form) = crate::sort_summary::form_name(report.spec.method) {
        let parameter = if matches!(report.spec.method, calc_expr::SortMethod::Shell(_)) {
            crate::sort_summary::GAPS_PARAMETER
        } else {
            crate::sort_summary::FORM_PARAMETER
        };
        method
            .parameters
            .insert(parameter.to_string(), identifier(form));
    }
    if let Some(limit) = report.limit {
        method.parameters.insert(
            crate::sort_summary::LIMIT_PARAMETER.to_string(),
            count_value(limit),
        );
    }
    if let Some(base) = report.base {
        method.parameters.insert(
            crate::sort_summary::BASE_PARAMETER.to_string(),
            count_value(base),
        );
    }
    let order = match report.spec.order {
        calc_expr::SortOrder::Increasing => crate::sort_summary::INCREASING_ORDER,
        calc_expr::SortOrder::Decreasing => crate::sort_summary::DECREASING_ORDER,
    };
    method.parameters.insert(
        crate::sort_summary::ORDER_PARAMETER.to_string(),
        identifier(order),
    );
    method
}

fn sort_method_name(method: calc_expr::SortMethod) -> &'static str {
    match method {
        calc_expr::SortMethod::Insertion => calc_core::INSERTION_SORT_METHOD,
        calc_expr::SortMethod::BinaryInsertion => calc_core::BINARY_INSERTION_SORT_METHOD,
        calc_expr::SortMethod::Selection => calc_core::SELECTION_SORT_METHOD,
        calc_expr::SortMethod::Bubble(_) => calc_core::BUBBLE_SORT_METHOD,
        calc_expr::SortMethod::Merge => calc_core::MERGE_SORT_METHOD,
        calc_expr::SortMethod::Heap => calc_core::HEAP_SORT_METHOD,
        calc_expr::SortMethod::Quick(_) => calc_core::QUICK_SORT_METHOD,
        calc_expr::SortMethod::Counting => calc_core::COUNTING_SORT_METHOD,
        calc_expr::SortMethod::DoubleSelection => calc_core::DOUBLE_SELECTION_SORT_METHOD,
        calc_expr::SortMethod::CocktailShaker(_) => calc_core::COCKTAIL_SHAKER_SORT_METHOD,
        calc_expr::SortMethod::Gnome => calc_core::GNOME_SORT_METHOD,
        calc_expr::SortMethod::OddEven(_) => calc_core::ODD_EVEN_SORT_METHOD,
        calc_expr::SortMethod::Comb(_) => calc_core::COMB_SORT_METHOD,
        calc_expr::SortMethod::Cycle => calc_core::CYCLE_SORT_METHOD,
        calc_expr::SortMethod::Pancake => calc_core::PANCAKE_SORT_METHOD,
        calc_expr::SortMethod::Shell(_) => calc_core::SHELL_SORT_METHOD,
        calc_expr::SortMethod::BottomUpMerge => calc_core::BOTTOM_UP_MERGE_SORT_METHOD,
        calc_expr::SortMethod::NaturalMerge => calc_core::NATURAL_MERGE_SORT_METHOD,
        calc_expr::SortMethod::Radix => calc_core::RADIX_SORT_METHOD,
        calc_expr::SortMethod::Bead => calc_core::BEAD_SORT_METHOD,
        calc_expr::SortMethod::Bitonic => calc_core::BITONIC_SORT_METHOD,
        calc_expr::SortMethod::Bogo => calc_core::BOGO_SORT_METHOD,
    }
}

fn sorted_outcome(
    pool: &mut ExprPool,
    outcome: Result<(calc_core::ArrayExpression, calc_core::SortReport), calc_core::SortFailure>,
) -> ExactOutcome {
    let (array, report) = match outcome {
        Ok(sorted) => sorted,
        Err(calc_core::SortFailure::Undecided {
            left,
            right,
            counts,
            ..
        }) => {
            let mut failure =
                array_diagnostic(pool, calc_core::ArrayError::NotComparable(left, right));
            failure.code = SORT_NOT_COMPARABLE.to_string();
            failure.data.insert(
                COMPARISONS_DATA.to_string(),
                count_value(counts.comparisons),
            );
            failure
                .data
                .insert(WRITES_DATA.to_string(), count_value(counts.writes));
            return ExactOutcome::Failed(failure);
        }
        Err(calc_core::SortFailure::Array(error)) => {
            return ExactOutcome::Failed(ordering_diagnostic(pool, error));
        }
        Err(
            failure @ (calc_core::SortFailure::KeysDifferInDimension { .. }
            | calc_core::SortFailure::LimitReached { .. }
            | calc_core::SortFailure::LengthNotAPowerOfTwo { .. }
            | calc_core::SortFailure::BeadTakesNoKey
            | calc_core::SortFailure::BeadBelowZero(_)),
        ) => {
            return ExactOutcome::Failed(ordering_diagnostic(pool, failure.to_array_error()));
        }
        Err(calc_core::SortFailure::TooManyBeads { beads, limit }) => {
            return ExactOutcome::Failed(diagnostic(
                "sort_too_many_beads",
                vec![
                    (
                        BEADS_DATA,
                        ParameterValue::Value(ResultValue::Number(Number::Integer(beads))),
                    ),
                    (LIMIT_DATA, count_value(limit)),
                ],
            ));
        }
        Err(failure @ calc_core::SortFailure::BaseRefused(_)) => {
            return ExactOutcome::Failed(ordering_diagnostic(pool, failure.to_array_error()));
        }
        Err(calc_core::SortFailure::RangeTooWide { range, limit }) => {
            return ExactOutcome::Failed(diagnostic(
                SORT_RANGE_TOO_WIDE,
                vec![
                    (
                        RANGE_DATA,
                        ParameterValue::Value(ResultValue::Number(Number::Integer(range))),
                    ),
                    (LIMIT_DATA, count_value(limit)),
                ],
            ));
        }
    };
    let length = u64::try_from(report.records.len()).unwrap_or(u64::MAX);
    let record = calc_core::SortRecord {
        from_positions: report.arrangement.as_ref().map(|arrangement| {
            arrangement
                .iter()
                .map(|&index| u64::try_from(index).map_or(u64::MAX, |index| index + 1))
                .collect()
        }),
        comparisons: report.counts.comparisons,
        writes: report.counts.writes,
        key_evaluations: report.keys.as_ref().map(|_| length),
        tallies: (report.key_range.is_some()
            || report.base.is_some()
            || report.spec.method == calc_expr::SortMethod::Bead)
            .then_some(report.counts.tallies),
        key_range: report.key_range,
        draws: random_pivot_seed(&report).map(|_| report.counts.draws),
        flips: (report.spec.method == calc_expr::SortMethod::Pancake)
            .then_some(report.counts.flips),
    };
    match evaluate_array(pool, Ok(array)) {
        ExactOutcome::Rational(sorted) => {
            let (computed, uses) = *sorted;
            let mut computed = computed.with_method(sort_method(&report)).with_sort(record);
            if let Some(seed) = random_pivot_seed(&report) {
                computed = computed.with_seed(calc_core::Seed {
                    value: seed,
                    generator: calc_core::PHILOX_GENERATOR.to_string(),
                });
            }
            ExactOutcome::Rational(Box::new((computed, uses)))
        }
        other => other,
    }
}

fn generated_outcome(
    pool: &mut ExprPool,
    generated: Result<calc_core::GeneratedBlock, calc_core::GeneratorRefusal>,
) -> ExactOutcome {
    let block = match generated {
        Ok(block) => block,
        Err(refusal) => return ExactOutcome::Failed(generator_refusal_diagnostic(pool, refusal)),
    };
    let mut elements = Vec::with_capacity(block.words.len());
    for word in block.words {
        match pool.number(Number::Integer(Integer::from(u64::from(word)))) {
            Ok(element) => elements.push(element),
            Err(_) => return ExactOutcome::Failed(diagnostic("result_not_representable", vec![])),
        }
    }
    let array = calc_core::ArrayExpression {
        shape: vec![4],
        elements,
    };
    let mut method = Method::named(calc_core::PHILOX_METHOD);
    method
        .parameters
        .insert(STREAM_PARAMETER.to_string(), count_value(block.stream));
    method
        .parameters
        .insert(INDEX_PARAMETER.to_string(), count_value(block.index));
    match evaluate_array(pool, Ok(array)) {
        ExactOutcome::Rational(generated) => {
            let (computed, uses) = *generated;
            let computed = computed.with_method(method);
            ExactOutcome::Rational(Box::new((computed, uses)))
        }
        other => other,
    }
}

fn generator_refusal_diagnostic(
    pool: &ExprPool,
    refusal: calc_core::GeneratorRefusal,
) -> Diagnostic {
    let is_machine = machine_conversion_in(pool, refusal.expression).is_some();
    let code = match (refusal.argument, is_machine) {
        (calc_core::GeneratorArgument::Seed, false) => "generator_seed_refused",
        (calc_core::GeneratorArgument::Stream, false) => "generator_stream_refused",
        (calc_core::GeneratorArgument::Index, false) => "generator_index_refused",
        (calc_core::GeneratorArgument::Seed, true) => "generator_seed_machine",
        (calc_core::GeneratorArgument::Stream, true) => "generator_stream_machine",
        (calc_core::GeneratorArgument::Index, true) => "generator_index_machine",
    };
    let data = grouped_text(pool, refusal.expression)
        .map(|text| {
            (
                FIRST_DATA,
                ParameterValue::Value(ResultValue::Expression(text)),
            )
        })
        .into_iter()
        .collect();
    diagnostic(code, data)
}

fn generator_calls(pool: &mut ExprPool, expression: ExprId) -> Vec<calc_core::GeneratedBlock> {
    let mut calls = Vec::new();
    let mut found: Vec<calc_core::GeneratedBlock> = Vec::new();
    let mut pending = vec![expression];
    while let Some(current) = pending.pop() {
        match pool.node(current) {
            Ok(NodeView::Apply {
                head: Head::Operator(Operator::Philox4x32_10),
                ..
            }) => calls.push(current),
            Ok(NodeView::Apply { arguments, .. }) => {
                pending.extend(arguments.iter().rev().copied())
            }
            Ok(NodeView::Quantity { value, .. }) => pending.push(value),
            Ok(NodeView::Array { elements, .. }) => pending.extend(elements.iter().rev().copied()),
            Ok(NodeView::Bind {
                arguments, body, ..
            }) => {
                pending.push(body);
                pending.extend(arguments.iter().rev().copied());
            }
            _ => {}
        }
    }
    for call in calls {
        if let Some(Ok(block)) = calc_core::philox_block(pool, call)
            && !found.contains(&block)
        {
            found.push(block);
        }
    }
    found
}

fn with_generator_notes(
    pool: &mut ExprPool,
    expression: ExprId,
    outcome: ExactOutcome,
) -> ExactOutcome {
    let ExactOutcome::Rational(result) = outcome else {
        return outcome;
    };
    let blocks = generator_calls(pool, expression);
    let Some(first) = blocks.first() else {
        return ExactOutcome::Rational(result);
    };
    let (computed, uses) = *result;
    let mut method = computed.method().clone();
    for block in &blocks {
        method.notes.push(diagnostic(
            "generator_layout",
            vec![
                (SEED_DATA, count_value(block.seed)),
                (STREAM_PARAMETER, count_value(block.stream)),
                (INDEX_PARAMETER, count_value(block.index)),
            ],
        ));
    }
    method
        .notes
        .push(diagnostic("generator_statistical", vec![]));
    let mut computed = computed.with_method(method);
    if blocks.iter().all(|block| block.seed == first.seed) {
        computed = computed.with_seed(calc_core::Seed {
            value: first.seed,
            generator: calc_core::PHILOX_GENERATOR.to_string(),
        });
    }
    ExactOutcome::Rational(Box::new((computed, uses)))
}

fn random_pivot_seed(report: &calc_core::SortReport) -> Option<u64> {
    match report.spec.method {
        calc_expr::SortMethod::Quick(calc_expr::SortPartition::LomutoRandom)
        | calc_expr::SortMethod::Bogo => report.seed,
        _ => None,
    }
}

fn sort_refusal_diagnostic(pool: &mut ExprPool, refusal: calc_core::SortRefusal) -> Diagnostic {
    match refusal {
        calc_core::SortRefusal::KeysDifferInDimension {
            first,
            second,
            first_key,
            second_key,
            by_key,
        } => keys_differ_in_dimension(pool, (first, first_key), (second, second_key), by_key)
            .unwrap_or_else(|| array_diagnostic(pool, calc_core::ArrayError::UnitsDiffer)),
        calc_core::SortRefusal::LimitReached { limit, counts } => diagnostic(
            "sort_limit_reached",
            vec![
                (LIMIT_DATA, count_value(limit)),
                (COMPARISONS_DATA, count_value(counts.comparisons)),
                (WRITES_DATA, count_value(counts.writes)),
                (DRAWS_DATA, count_value(counts.draws)),
            ],
        ),
        calc_core::SortRefusal::LengthNotAPowerOfTwo { length } => {
            let below = 1_usize << length.ilog2();
            let whole = |value: usize| count_value(u64::try_from(value).unwrap_or(u64::MAX));
            diagnostic(
                "sort_length_not_power_of_two",
                vec![
                    (LENGTH_DATA, whole(length)),
                    (BELOW_DATA, whole(below)),
                    (ABOVE_DATA, whole(below.saturating_mul(2))),
                ],
            )
        }
        calc_core::SortRefusal::BeadTakesNoKey => diagnostic("sort_bead_no_key", vec![]),
        calc_core::SortRefusal::BeadBelowZero(entry) => {
            let data = grouped_text(pool, entry)
                .map(|text| {
                    (
                        FIRST_DATA,
                        ParameterValue::Value(ResultValue::Expression(text)),
                    )
                })
                .into_iter()
                .collect();
            diagnostic("sort_bead_below_zero", data)
        }
        calc_core::SortRefusal::BaseRefused(refused) => match refused {
            calc_sort::BaseRefused::TooSmall => diagnostic("sort_base_too_small", vec![]),
            calc_sort::BaseRefused::TooLarge { limit } => diagnostic(
                "sort_base_too_large",
                vec![(LIMIT_DATA, count_value(limit))],
            ),
        },
    }
}

fn ordering_diagnostic(pool: &mut ExprPool, error: calc_core::ArrayError) -> Diagnostic {
    if let calc_core::ArrayError::SortRefused(refusal) = error {
        return sort_refusal_diagnostic(pool, refusal);
    }
    if let calc_core::ArrayError::EntryWithoutValue(entry) = error
        && let Err(failure) = evaluate_exact(pool, entry)
    {
        return exact_error_diagnostic(pool, entry, failure);
    }
    array_diagnostic(pool, error)
}

fn array_diagnostic(pool: &ExprPool, error: calc_core::ArrayError) -> Diagnostic {
    if let calc_core::ArrayError::NotComparable(first, second) = error {
        let data = [(FIRST_DATA, first), (SECOND_DATA, second)]
            .into_iter()
            .filter_map(|(name, expression)| {
                grouped_text(pool, expression)
                    .map(|text| (name, ParameterValue::Value(ResultValue::Expression(text))))
            })
            .collect();
        return diagnostic("array_not_comparable", data);
    }
    if let calc_core::ArrayError::KeyWithUnit(entry, method) = error {
        let data = grouped_text(pool, entry)
            .map(|text| {
                (
                    FIRST_DATA,
                    ParameterValue::Value(ResultValue::Expression(text)),
                )
            })
            .into_iter()
            .chain([(METHOD_DATA, identifier(sort_method_name(method)))])
            .collect();
        return diagnostic("array_key_with_unit", data);
    }
    if let calc_core::ArrayError::GeneratorRefused(refusal) = error {
        return generator_refusal_diagnostic(pool, refusal);
    }
    if let calc_core::ArrayError::FunctionKeyNotWhole(key, method) = error {
        let code = match machine_conversion_in(pool, key) {
            Some(_) => "array_function_key_machine",
            None => "array_key_not_whole",
        };
        let data = grouped_text(pool, key)
            .map(|text| {
                (
                    FIRST_DATA,
                    ParameterValue::Value(ResultValue::Expression(text)),
                )
            })
            .into_iter()
            .chain([(METHOD_DATA, identifier(sort_method_name(method)))])
            .collect();
        return diagnostic(code, data);
    }
    if let calc_core::ArrayError::KeyNotWhole(entry, method) = error {
        if let Some(conversion) = machine_conversion_in(pool, entry) {
            return read_diagnostic(pool, "array_machine_entry", conversion, vec![]);
        }
        let data = grouped_text(pool, entry)
            .map(|text| {
                (
                    FIRST_DATA,
                    ParameterValue::Value(ResultValue::Expression(text)),
                )
            })
            .into_iter()
            .chain([(METHOD_DATA, identifier(sort_method_name(method)))])
            .collect();
        return diagnostic("array_key_not_whole", data);
    }
    if let calc_core::ArrayError::NotReal(entry) = error {
        let data = grouped_text(pool, entry)
            .map(|text| {
                (
                    FIRST_DATA,
                    ParameterValue::Value(ResultValue::Expression(text)),
                )
            })
            .into_iter()
            .collect();
        return diagnostic("array_not_real", data);
    }
    if let calc_core::ArrayError::Empty(operation) | calc_core::ArrayError::NotAMatrix(operation) =
        error
    {
        let code = match error {
            calc_core::ArrayError::Empty(_) => "array_empty",
            _ => "array_not_a_matrix",
        };
        return diagnostic(code, vec![(OPERATION_DATA, identifier(operation.name()))]);
    }
    let code = match error {
        calc_core::ArrayError::ShapesDiffer => "array_shapes_differ",
        calc_core::ArrayError::NotSquare => "array_not_square",
        calc_core::ArrayError::EmptyList => "array_empty",
        calc_core::ArrayError::PositionNotWhole => "array_position_not_whole",
        calc_core::ArrayError::PositionOutsideList => "array_position_outside",
        calc_core::ArrayError::NotComparable(..) => "array_not_comparable",
        calc_core::ArrayError::RecordsNeedKey => "array_records_need_key",
        calc_core::ArrayError::NotReal(_) => "array_not_real",
        calc_core::ArrayError::EntryWithoutValue(_) => "array_entry_without_value",
        calc_core::ArrayError::KeyNotWhole(..) | calc_core::ArrayError::FunctionKeyNotWhole(..) => {
            "array_key_not_whole"
        }
        calc_core::ArrayError::KeyWithUnit(..) => "array_key_with_unit",
        calc_core::ArrayError::RangeTooWide => "array_range_too_wide",
        calc_core::ArrayError::GeneratorRefused(_) => "generator_refused",
        calc_core::ArrayError::NotAList => "array_not_a_list",
        calc_core::ArrayError::UnitsDiffer => "array_units_differ",
        calc_core::ArrayError::RanksDiffer => "array_ranks_differ",
        calc_core::ArrayError::Nested => "array_nested",
        calc_core::ArrayError::BodyAndPointAreArrays => "array_body_and_point",
        calc_core::ArrayError::PowerOfArray => "array_power",
        calc_core::ArrayError::ProductOfLists => "array_product_of_lists",
        calc_core::ArrayError::NotRectangular => "array_not_rectangular",
        calc_core::ArrayError::Unsupported | calc_core::ArrayError::SortRefused(_) => {
            "array_unsupported"
        }
        calc_core::ArrayError::NotInvertible => "array_not_invertible",
        calc_core::ArrayError::EntriesNotExact => "array_entries_not_exact",
        calc_core::ArrayError::EntriesNotRational => "array_entries_not_rational",
        calc_core::ArrayError::EigenvaluesNotReal => "array_eigenvalues_not_real",
        calc_core::ArrayError::EigenvaluesNotRadical => "array_eigenvalues_not_radical",
        calc_core::ArrayError::Empty(_) | calc_core::ArrayError::NotAMatrix(_) => "array_empty",
    };
    diagnostic(code, vec![])
}

fn temperature_difference_symbol(pool: &ExprPool, unit: UnitId) -> Option<&'static str> {
    let units = pool.units();
    let [factor] = units.factors(unit).ok()? else {
        return None;
    };
    if factor.exponent() != 1 {
        return None;
    }
    match units.symbol(factor.named_unit()).ok()? {
        CELSIUS_DIFFERENCE_SYMBOL => Some(CELSIUS_DIFFERENCE_SYMBOL),
        FAHRENHEIT_DIFFERENCE_SYMBOL => Some(FAHRENHEIT_DIFFERENCE_SYMBOL),
        _ => None,
    }
}

fn reading_note(
    pool: &ExprPool,
    symbol: &str,
    argument: ExprId,
    converts_into_the_scale: bool,
    stands_alone: bool,
) -> Option<Diagnostic> {
    let scale = match symbol {
        CELSIUS_DIFFERENCE_SYMBOL => "celsius",
        _ => "fahrenheit",
    };
    let direction = if converts_into_the_scale {
        "to"
    } else {
        "from"
    };
    let operator = format!("{direction}_{scale}");
    let display = match symbol {
        CELSIUS_DIFFERENCE_SYMBOL => "°C",
        _ => "°F",
    };
    let mut data = vec![
        (UNIT_SYMBOL_DATA, identifier(display)),
        (OPERATOR_DATA_NAME, identifier(&operator)),
    ];
    if stands_alone {
        let written = match written_difference(pool, argument) {
            Some((source, number)) if converts_into_the_scale => {
                format!("{operator}(from_{source}({number}))")
            }
            _ => format!(
                "{operator}({})",
                print_expression(pool, argument, PrintMode::Ascii).ok()?
            ),
        };
        data.push((READING_TEXT_DATA, identifier(&written)));
    }
    Some(diagnostic(TEMPERATURE_DIFFERENCE_NOTE, data))
}

fn written_difference(pool: &ExprPool, expression: ExprId) -> Option<(&'static str, String)> {
    let Ok(NodeView::Quantity { value, unit }) = pool.node(expression) else {
        return None;
    };
    let scale = match temperature_difference_symbol(pool, unit)? {
        CELSIUS_DIFFERENCE_SYMBOL => "celsius",
        _ => "fahrenheit",
    };
    let number = print_expression(pool, value, PrintMode::Ascii).ok()?;
    Some((scale, number))
}

fn difference_as_reading_diagnostic(
    pool: &ExprPool,
    expression: ExprId,
    difference: ExprId,
) -> Diagnostic {
    let target = match pool.node(expression) {
        Ok(NodeView::Apply {
            head: Head::Operator(Operator::ToFahrenheit),
            ..
        }) => "to_fahrenheit",
        _ => "to_celsius",
    };
    let written = print_expression(pool, difference, PrintMode::Ascii).unwrap_or_default();
    let (source, number) =
        written_difference(pool, difference).unwrap_or(("celsius", String::new()));
    let display = match source {
        "celsius" => "°C",
        _ => "°F",
    };
    diagnostic(
        DIFFERENCE_AS_READING_CODE,
        vec![
            (WRITTEN_DATA, identifier(&written)),
            (NUMBER_DATA, identifier(&number)),
            (UNIT_SYMBOL_DATA, identifier(display)),
            (
                READING_TEXT_DATA,
                identifier(&format!("{target}(from_{source}({number}))")),
            ),
        ],
    )
}

fn absolute_zero_text(pool: &ExprPool, expression: ExprId) -> &'static str {
    match pool.node(expression) {
        Ok(NodeView::Apply {
            head: Head::Operator(Operator::FromCelsius),
            ..
        }) => "-273.15 °C",
        Ok(NodeView::Apply {
            head: Head::Operator(Operator::FromFahrenheit),
            ..
        }) => "-459.67 °F",
        _ => "0 K",
    }
}

fn holds_a_free_name(pool: &ExprPool, expression: ExprId) -> bool {
    match pool.node(expression) {
        Ok(NodeView::Symbol(symbol)) => !calc_core::is_a_constant(pool, symbol),
        Ok(NodeView::Apply { arguments, .. }) => arguments
            .iter()
            .any(|argument| holds_a_free_name(pool, *argument)),
        _ => false,
    }
}

pub(crate) fn holds_zero_to_the_zero(pool: &mut ExprPool, expression: ExprId, free: bool) -> bool {
    let is_zero = |pool: &mut ExprPool, part: ExprId| {
        calc_core::evaluate_exact(pool, part)
            .ok()
            .and_then(|evaluation| evaluation.rational_value().cloned())
            .is_some_and(|value| matches!(value, Number::Integer(ref integer) if integer.is_zero()))
    };
    let Ok(view) = pool.node(expression) else {
        return false;
    };
    match view {
        NodeView::Apply {
            head: Head::Operator(Operator::Pow),
            arguments: [base, exponent],
        } => {
            let (base, exponent) = (*base, *exponent);
            let base_may_be_zero = is_zero(pool, base) || (free && holds_a_free_name(pool, base));
            (base_may_be_zero && is_zero(pool, exponent))
                || holds_zero_to_the_zero(pool, base, free)
                || holds_zero_to_the_zero(pool, exponent, free)
        }
        NodeView::Apply { arguments, .. } => {
            let arguments = arguments.to_vec();
            arguments
                .into_iter()
                .any(|argument| holds_zero_to_the_zero(pool, argument, free))
        }
        _ => false,
    }
}

pub(crate) fn normal_form_with_condition(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Option<(ExprId, Vec<String>)> {
    let holds_a_quotient =
        !calc_core::is_relation(pool, expression) && holds_a_division(pool, expression);
    if let Some(reduced) = holds_a_quotient
        .then(|| calc_core::reduced_quotient(pool, expression))
        .flatten()
    {
        let excluding = reduced
            .excluding
            .iter()
            .map(|denominator| {
                calc_syntax::print_expression(pool, *denominator, calc_syntax::PrintMode::Ascii)
                    .ok()
            })
            .collect::<Option<Vec<String>>>()?;
        return Some((reduced.expression, excluding));
    }
    Some((calc_core::expanded(pool, expression)?, Vec::new()))
}

fn holds_a_division(pool: &ExprPool, expression: ExprId) -> bool {
    match pool.node(expression) {
        Ok(NodeView::Apply {
            head: Head::Operator(Operator::Div),
            ..
        }) => true,
        Ok(NodeView::Apply { arguments, .. }) => arguments
            .iter()
            .any(|argument| holds_a_division(pool, *argument)),
        _ => false,
    }
}

pub(crate) fn division_by_identical_zero(
    pool: &mut ExprPool,
    expression: ExprId,
) -> Option<ExprId> {
    let NodeView::Apply { head, arguments } = pool.node(expression).ok()? else {
        return None;
    };
    let arguments = arguments.to_vec();
    if let (Head::Operator(Operator::Div), [_, divisor]) = (head, arguments.as_slice()) {
        let is_zero = calc_core::expanded(pool, *divisor).is_some_and(|normal| {
            matches!(
                pool.node(normal),
                Ok(NodeView::Number(number))
                    if pool.number_value(number).is_ok_and(|value| value == &Number::from(0_i64))
            )
        });
        if is_zero {
            return Some(expression);
        }
    }
    arguments
        .into_iter()
        .find_map(|argument| division_by_identical_zero(pool, argument))
}

fn valid_where_note(excluding: &[String]) -> Diagnostic {
    let elements = excluding
        .iter()
        .map(|text| ResultValue::Expression(text.clone()))
        .collect::<Vec<_>>();
    diagnostic(
        VALID_WHERE_CODE,
        vec![(
            EXCLUDING_DATA,
            ParameterValue::Value(ResultValue::Array {
                shape: vec![elements.len()],
                elements,
            }),
        )],
    )
}

pub fn valid_where(note: &Diagnostic) -> Option<Vec<String>> {
    if note.code != VALID_WHERE_CODE {
        return None;
    }
    let Some(ParameterValue::Value(ResultValue::Array { elements, .. })) =
        note.data.get(EXCLUDING_DATA)
    else {
        return None;
    };
    elements
        .iter()
        .map(|element| match element {
            ResultValue::Expression(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn temperature_reading_note(pool: &ExprPool, expression: ExprId) -> Option<Diagnostic> {
    let scale = match pool.node(expression).ok()? {
        NodeView::Apply {
            head: Head::Operator(Operator::ToCelsius),
            ..
        } => "Celsius",
        NodeView::Apply {
            head: Head::Operator(Operator::ToFahrenheit),
            ..
        } => "Fahrenheit",
        _ => return None,
    };
    Some(diagnostic(
        TEMPERATURE_READING_NOTE,
        vec![(SCALE_DATA, identifier(scale))],
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TemperatureRole {
    Reading,
    Difference,
}

pub(crate) fn temperature_role(pool: &ExprPool, expression: ExprId) -> Option<TemperatureRole> {
    use TemperatureRole::{Difference, Reading};
    match pool.node(expression).ok()? {
        NodeView::Apply {
            head: Head::Operator(Operator::FromCelsius | Operator::FromFahrenheit),
            ..
        } => Some(Reading),
        NodeView::Quantity { unit, .. } => {
            temperature_difference_symbol(pool, unit).map(|_| Difference)
        }
        NodeView::Apply {
            head: Head::Operator(Operator::Add),
            arguments: [left, right],
        } => match (
            temperature_role(pool, *left),
            temperature_role(pool, *right),
        ) {
            (Some(Reading), Some(Reading)) => None,
            (Some(Reading), _) | (_, Some(Reading)) => Some(Reading),
            (Some(Difference), _) | (_, Some(Difference)) => Some(Difference),
            (None, None) => None,
        },
        NodeView::Apply {
            head: Head::Operator(Operator::Sub),
            arguments: [left, right],
        } => match (
            temperature_role(pool, *left),
            temperature_role(pool, *right),
        ) {
            (Some(Reading), Some(Reading)) => Some(Difference),
            (Some(Reading), _) => Some(Reading),
            (_, Some(Reading)) => None,
            (Some(Difference), _) | (_, Some(Difference)) => Some(Difference),
            (None, None) => None,
        },
        NodeView::Apply {
            head: Head::Operator(Operator::Neg),
            arguments: [inner],
        } => temperature_role(pool, *inner).filter(|role| *role == Difference),
        NodeView::Apply {
            head: Head::Operator(Operator::Mul | Operator::Div),
            arguments: [left, right],
        } => match (
            temperature_role(pool, *left),
            temperature_role(pool, *right),
        ) {
            (Some(Difference), None) | (None, Some(Difference)) => Some(Difference),
            _ => None,
        },
        NodeView::Apply {
            head: Head::Operator(Operator::ConvertUnit),
            arguments: [value, target],
        } => match pool.node(*target).ok()? {
            NodeView::Quantity { unit, .. }
                if temperature_difference_symbol(pool, unit).is_some() =>
            {
                Some(Difference)
            }
            _ => temperature_role(pool, *value),
        },
        _ => None,
    }
}

fn difference_symbols_written(pool: &ExprPool, expression: ExprId) -> HashSet<&'static str> {
    let mut found = HashSet::new();
    let mut pending = vec![expression];
    let mut visited = HashSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Ok(view) = pool.node(current) else {
            continue;
        };
        if let NodeView::Quantity { unit, .. } = view
            && let Some(symbol) = temperature_difference_symbol(pool, unit)
        {
            found.insert(symbol);
        }
        pending.extend(match view {
            NodeView::Symbol(_) | NodeView::Number(_) | NodeView::Bound(_) => Vec::new(),
            NodeView::Apply {
                head: Head::Operator(Operator::ConvertUnit),
                arguments,
            } => arguments.first().copied().into_iter().collect(),
            NodeView::Apply { arguments, .. } => arguments.to_vec(),
            NodeView::Bind {
                arguments, body, ..
            } => arguments.iter().copied().chain([body]).collect(),
            NodeView::Quantity { value, .. } => vec![value],
            NodeView::Array { elements, .. } => elements.to_vec(),
        });
    }
    found
}

fn converted_alone(pool: &ExprPool, expression: ExprId, quantity: ExprId) -> bool {
    matches!(
        pool.node(expression),
        Ok(NodeView::Apply {
            head: Head::Operator(Operator::ConvertUnit),
            arguments: [value, _],
        }) if *value == quantity
    )
}

fn is_a_finite_operand_note(note: &Diagnostic) -> bool {
    matches!(
        note.code.as_str(),
        "overflowed_to_infinity" | "not_a_number_from_finite_operands"
    )
}

fn holds_only_finite(value: &ResultValue) -> bool {
    match value {
        ResultValue::Number(number) => is_finite(number),
        ResultValue::Complex { real, imaginary } => is_finite(real) && is_finite(imaginary),
        ResultValue::Array { elements, .. } => elements.iter().all(holds_only_finite),
        ResultValue::Expression(_) => true,
    }
}

fn is_finite(number: &Number) -> bool {
    match number {
        Number::F32(value) => value.is_finite(),
        Number::F64(value) => value.is_finite(),
        Number::Integer(_) | Number::Rational(_) => true,
    }
}

fn scalar_number(value: &ResultValue) -> Option<&Number> {
    match value {
        ResultValue::Number(number) => Some(number),
        _ => None,
    }
}

fn bound_is_not_smaller(value: &Number, bound: &Number) -> bool {
    let Ok(value) = value.to_exact() else {
        return false;
    };
    let Ok(bound) = bound.to_exact() else {
        return false;
    };
    let Ok(magnitude) = absolute_exact(&value) else {
        return false;
    };
    let Ok(bound) = absolute_exact(&bound) else {
        return false;
    };
    matches!(
        bound.sub_exact(&magnitude),
        Ok(difference) if !is_negative_number(&difference)
    )
}

fn absolute_exact(number: &Number) -> Result<Number, calc_numbers::ExactArithmeticError> {
    if is_negative_number(number) {
        return number.negate_exact();
    }
    Ok(number.clone())
}

fn is_negative_number(number: &Number) -> bool {
    match number {
        Number::Integer(integer) => integer.is_negative(),
        Number::Rational(rational) => rational.numerator().is_negative(),
        Number::F32(value) => *value < 0.0,
        Number::F64(value) => *value < 0.0,
    }
}

fn written_conversion(pool: &ExprPool, expression: ExprId) -> Option<(ExprId, &'static str)> {
    let Ok(NodeView::Apply {
        head: Head::Operator(operator),
        arguments: [argument],
    }) = pool.node(expression)
    else {
        return None;
    };
    match operator {
        Operator::ToF64 => Some((*argument, "to_f64")),
        Operator::ToF32 => Some((*argument, "to_f32")),
        _ => None,
    }
}

fn conversions_below_root(pool: &ExprPool, root: ExprId) -> Vec<ExprId> {
    let mut found = Vec::new();
    let mut pending = vec![root];
    let mut visited = HashSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Ok(view) = pool.node(current) else {
            continue;
        };
        if current != root
            && matches!(
                view,
                NodeView::Apply {
                    head: Head::Operator(Operator::ToF64 | Operator::ToF32),
                    ..
                }
            )
        {
            found.push(current);
            continue;
        }
        let mut children: Vec<ExprId> = match view {
            NodeView::Symbol(_) | NodeView::Number(_) | NodeView::Bound(_) => Vec::new(),
            NodeView::Apply { arguments, .. } => arguments.to_vec(),
            NodeView::Bind {
                arguments, body, ..
            } => arguments.iter().copied().chain([body]).collect(),
            NodeView::Quantity { value, .. } => vec![value],
            NodeView::Array { elements, .. } => elements.to_vec(),
        };
        children.reverse();
        pending.extend(children);
    }
    found
}

fn machine_conversion_note(
    pool: &mut ExprPool,
    conversion: ExprId,
    machine: ExprId,
) -> Option<Diagnostic> {
    let Ok(NodeView::Apply {
        arguments: [argument],
        ..
    }) = pool.node(conversion)
    else {
        return None;
    };
    let argument = *argument;
    let NodeView::Number(machine) = pool.node(machine).ok()? else {
        return None;
    };
    let machine = pool.number_value(machine).ok()?.clone();
    let exact = calc_core::evaluate_exact(pool, argument)
        .ok()?
        .rational_value()?
        .clone();
    let difference = machine.sub_exact(&exact).ok()?;
    let written = print_expression(pool, conversion, PrintMode::Ascii).ok()?;
    let argument_text = print_expression(pool, argument, PrintMode::Ascii).ok()?;
    let machine_digits = crate::summary::digits_text(&machine);
    let machine_text = crate::summary::shortened_digits(&machine_digits)
        .map_or(machine_digits, |(shortened, _)| shortened);
    let difference_text = crate::summary::value_text(&calc_core::ResultValue::Number(difference));
    Some(diagnostic(
        MACHINE_CONVERSION_NOTE,
        vec![
            ("written", identifier(&written)),
            ("argument", identifier(&argument_text)),
            ("machine", identifier(&machine_text)),
            ("difference", identifier(&difference_text)),
        ],
    ))
}

fn temperature_difference_note(pool: &ExprPool, expression: ExprId) -> Option<Diagnostic> {
    if temperature_role(pool, expression) != Some(TemperatureRole::Difference) {
        return None;
    }
    if difference_symbols_written(pool, expression).len() > 1 {
        return Some(diagnostic(TEMPERATURE_DIFFERENCE_NOTE, vec![]));
    }
    let mut pending = vec![expression];
    let mut visited = HashSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Ok(view) = pool.node(current) else {
            continue;
        };
        if let NodeView::Apply {
            head: Head::Operator(Operator::ConvertUnit),
            arguments: [value, target],
        } = view
            && let Ok(NodeView::Quantity { unit, .. }) = pool.node(*target)
            && let Some(symbol) = temperature_difference_symbol(pool, unit)
        {
            return reading_note(pool, symbol, *value, true, current == expression);
        }
        if let NodeView::Quantity { value, unit } = view
            && let Some(symbol) = temperature_difference_symbol(pool, unit)
        {
            let stands_alone = current == expression || converted_alone(pool, expression, current);
            return reading_note(pool, symbol, value, false, stands_alone);
        }
        let children: Vec<ExprId> = match view {
            NodeView::Symbol(_) | NodeView::Number(_) | NodeView::Bound(_) => Vec::new(),
            NodeView::Apply { arguments, .. } => arguments.to_vec(),
            NodeView::Bind {
                arguments, body, ..
            } => arguments.iter().copied().chain([body]).collect(),
            NodeView::Quantity { value, .. } => vec![value],
            NodeView::Array { elements, .. } => elements.to_vec(),
        };
        pending.extend(children.into_iter().rev());
    }
    None
}

fn temperature_difference_entry_notes(pool: &ExprPool, expression: ExprId) -> Vec<Diagnostic> {
    let mut notes = Vec::new();
    let mut pending = vec![expression];
    let mut visited = HashSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Ok(view) = pool.node(current) else {
            continue;
        };
        if let NodeView::Array { elements, .. } = view {
            notes.extend(list_entry_notes(pool, elements));
        }
        let children: Vec<ExprId> = match view {
            NodeView::Symbol(_) | NodeView::Number(_) | NodeView::Bound(_) => Vec::new(),
            NodeView::Apply { arguments, .. } => arguments.to_vec(),
            NodeView::Bind {
                arguments, body, ..
            } => arguments.iter().copied().chain([body]).collect(),
            NodeView::Quantity { value, .. } => vec![value],
            NodeView::Array { elements, .. } => elements.to_vec(),
        };
        pending.extend(children.into_iter().rev());
    }
    notes
}

fn list_entry_notes(pool: &ExprPool, elements: &[ExprId]) -> Vec<Diagnostic> {
    let differences: Vec<(usize, ExprId, &'static str)> = elements
        .iter()
        .enumerate()
        .filter_map(|(position, element)| match pool.node(*element).ok()? {
            NodeView::Quantity { unit, .. } => {
                temperature_difference_symbol(pool, unit).map(|symbol| (position, *element, symbol))
            }
            _ => None,
        })
        .collect();
    let Some((_, _, first_symbol)) = differences.first() else {
        return Vec::new();
    };
    if differences.len() == elements.len()
        && differences
            .iter()
            .all(|(_, _, symbol)| symbol == first_symbol)
    {
        let (display, operator) = scale_words(first_symbol);
        return vec![diagnostic(
            TEMPERATURE_DIFFERENCE_ENTRIES_NOTE,
            vec![
                (UNIT_SYMBOL_DATA, identifier(display)),
                (OPERATOR_DATA_NAME, identifier(operator)),
            ],
        )];
    }
    differences
        .into_iter()
        .filter_map(|(position, element, symbol)| {
            let (_, number) = written_difference(pool, element)?;
            let (display, operator) = scale_words(symbol);
            Some(diagnostic(
                TEMPERATURE_DIFFERENCE_ENTRY_NOTE,
                vec![
                    (WRITTEN_DATA, identifier(&format!("{number} {display}"))),
                    (POSITION_DATA, identifier(&(position + 1).to_string())),
                    (UNIT_SYMBOL_DATA, identifier(display)),
                    (
                        READING_TEXT_DATA,
                        identifier(&format!("{operator}({number})")),
                    ),
                ],
            ))
        })
        .collect()
}

fn scale_words(symbol: &str) -> (&'static str, &'static str) {
    match symbol {
        CELSIUS_DIFFERENCE_SYMBOL => ("°C", "from_celsius"),
        _ => ("°F", "from_fahrenheit"),
    }
}

fn reads_as_a_number(pool: &mut ExprPool, expression: ExprId) -> bool {
    calc_core::enclose_decimal(pool, expression, READING_SIGNIFICANT_DIGITS, &|| false).is_ok()
}

fn machine_number_node(pool: &mut ExprPool, computed: &ComputedResult) -> Option<ExprId> {
    let ResultValue::Number(number) = computed.value() else {
        return None;
    };
    let node = pool.number(number.to_exact().ok()?).ok()?;
    match computed.unit() {
        Some(unit) => pool.quantity(node, unit).ok(),
        None => Some(node),
    }
}

fn holds_a_conversion(pool: &ExprPool, expression: ExprId) -> bool {
    let mut pending = vec![expression];
    let mut seen: Vec<ExprId> = Vec::new();
    while let Some(current) = pending.pop() {
        if seen.contains(&current) {
            continue;
        }
        seen.push(current);
        match pool.node(current) {
            Ok(NodeView::Apply { head, arguments }) => {
                if matches!(head, Head::Operator(Operator::ToF64 | Operator::ToF32)) {
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
            _ => {}
        }
    }
    false
}

fn substituted_enclosures(pool: &mut ExprPool, expression: ExprId) -> Result<ExprId, Diagnostic> {
    let node = pool
        .node(expression)
        .map_err(|_| diagnostic("expression_not_in_pool", vec![]))?;
    match node {
        NodeView::Apply {
            head: Head::Operator(operator @ (Operator::EnclosureLower | Operator::EnclosureUpper)),
            arguments: [value, digits],
        } => {
            let (value, digits) = (*value, *digits);
            let value = substituted_enclosures(pool, value)?;
            let digits = substituted_enclosures(pool, digits)?;
            enclosure_endpoint_node(pool, operator, value, digits)
        }
        NodeView::Apply { head, arguments } => {
            let arguments = arguments.to_vec();
            let substituted = arguments
                .into_iter()
                .map(|argument| substituted_enclosures(pool, argument))
                .collect::<Result<Vec<ExprId>, Diagnostic>>()?;
            pool.apply(head, &substituted)
                .map_err(|_| diagnostic("expression_pool_full", vec![]))
        }
        NodeView::Quantity { value, unit } => {
            let value = substituted_enclosures(pool, value)?;
            pool.quantity(value, unit)
                .map_err(|_| diagnostic("expression_pool_full", vec![]))
        }
        NodeView::Array { shape, elements } => {
            let shape = shape.to_vec();
            let elements = elements.to_vec();
            let substituted = elements
                .into_iter()
                .map(|element| substituted_enclosures(pool, element))
                .collect::<Result<Vec<ExprId>, Diagnostic>>()?;
            pool.array(&shape, &substituted)
                .map_err(|_| diagnostic("expression_pool_full", vec![]))
        }
        _ => Ok(expression),
    }
}

fn enclosure_endpoint_node(
    pool: &mut ExprPool,
    operator: Operator,
    value: ExprId,
    digits: ExprId,
) -> Result<ExprId, Diagnostic> {
    let significant_digits = written_digits(pool, digits)?;
    let unit = calc_core::to_coherent_units(pool, value)
        .ok()
        .and_then(|coherent| coherent.unit);
    let enclosure = calc_core::enclose_decimal(pool, value, significant_digits, &|| false)
        .map_err(|_| diagnostic("enclosure_not_applicable", vec![]))?;
    let endpoint = match operator {
        Operator::EnclosureUpper => enclosure.upper,
        _ => enclosure.lower,
    };
    let node = pool
        .number(endpoint)
        .map_err(|_| diagnostic("expression_pool_full", vec![]))?;
    match unit {
        Some(unit) => pool
            .quantity(node, unit)
            .map_err(|_| diagnostic("expression_pool_full", vec![])),
        None => Ok(node),
    }
}

fn written_digits(pool: &ExprPool, digits: ExprId) -> Result<u32, Diagnostic> {
    let not_whole = || diagnostic("enclosure_digits_not_whole", vec![]);
    let Ok(NodeView::Number(number)) = pool.node(digits) else {
        return Err(not_whole());
    };
    let Ok(Number::Integer(written)) = pool.number_value(number) else {
        return Err(not_whole());
    };
    let outside = || {
        diagnostic(
            "enclosure_digits_out_of_range",
            vec![(DIGITS_DATA, identifier(&integer_to_decimal(written)))],
        )
    };
    let count = written.to_i64().ok_or_else(outside)?;
    u32::try_from(count)
        .ok()
        .filter(|count| (1..=calc_numbers::SIGNIFICANT_DIGITS_LIMIT).contains(count))
        .ok_or_else(outside)
}

fn root_conversion(
    pool: &ExprPool,
    expression: ExprId,
) -> Option<(ExprId, MachineEvaluator, Domain)> {
    let Ok(NodeView::Apply {
        head: Head::Operator(operator),
        arguments: [argument],
    }) = pool.node(expression)
    else {
        return None;
    };
    match operator {
        Operator::ToF64 => Some((*argument, evaluate_f64 as MachineEvaluator, Domain::F64)),
        Operator::ToF32 => Some((*argument, evaluate_f32 as MachineEvaluator, Domain::F32)),
        _ => None,
    }
}

fn evaluated_conversions(
    pool: &mut ExprPool,
    backends: &[Arc<dyn Backend>],
    settings: Settings,
    expression: ExprId,
    uses: &mut Vec<BackendUse>,
) -> ExprId {
    let rebuilt = |pool: &mut ExprPool, head, arguments: &[ExprId]| {
        pool.apply(head, arguments).unwrap_or(expression)
    };
    match pool.node(expression) {
        Ok(NodeView::Apply { head, arguments }) => {
            let arguments = arguments.to_vec();
            let evaluated: Vec<ExprId> = arguments
                .iter()
                .map(|argument| evaluated_conversions(pool, backends, settings, *argument, uses))
                .collect();
            let node = rebuilt(pool, head, &evaluated);
            let (evaluate, width) = match (head, evaluated.as_slice()) {
                (Head::Operator(Operator::ToF64), [_]) => {
                    (evaluate_f64 as MachineEvaluator, Domain::F64)
                }
                (Head::Operator(Operator::ToF32), [_]) => {
                    (evaluate_f32 as MachineEvaluator, Domain::F32)
                }
                _ => return node,
            };
            let Ok((computed, used)) =
                evaluate_in_machine(pool, backends, settings, evaluated[0], evaluate, width)
            else {
                return node;
            };
            if used.iter().any(|used| used.approximate_operations) {
                return node;
            }
            let Some(number) = machine_number_node(pool, &computed) else {
                return node;
            };
            uses.extend(used);
            number
        }
        Ok(NodeView::Quantity { value, unit }) => {
            let value = evaluated_conversions(pool, backends, settings, value, uses);
            pool.quantity(value, unit).unwrap_or(expression)
        }
        Ok(NodeView::Array { shape, elements }) => {
            let shape = shape.to_vec();
            let elements = elements.to_vec();
            let evaluated: Vec<ExprId> = elements
                .iter()
                .map(|element| evaluated_conversions(pool, backends, settings, *element, uses))
                .collect();
            pool.array(&shape, &evaluated).unwrap_or(expression)
        }
        _ => expression,
    }
}

pub(crate) fn evaluate_exactly(
    pool: &mut ExprPool,
    backends: &[Arc<dyn Backend>],
    settings: Settings,
    expression: ExprId,
) -> ExactOutcome {
    let outcome = evaluate_exactly_unmarked(pool, backends, settings, expression);
    with_generator_notes(pool, expression, outcome)
}

fn evaluate_exactly_unmarked(
    pool: &mut ExprPool,
    backends: &[Arc<dyn Backend>],
    settings: Settings,
    expression: ExprId,
) -> ExactOutcome {
    let expression = match calc_core::reduce_arrays(pool, expression) {
        Ok(expression) => expression,
        Err(error) => return ExactOutcome::Failed(ordering_diagnostic(pool, error)),
    };
    if let Some(generated) = calc_core::philox_block(pool, expression) {
        return generated_outcome(pool, generated);
    }
    if let Some(outcome) = calc_core::named_sort(pool, expression, calc_sort::StepRecording::Skip) {
        return sorted_outcome(pool, outcome);
    }
    if let Some(array) = calc_core::array_expression(pool, expression) {
        return evaluate_array(pool, array);
    }
    let mut uses = Vec::new();
    if let Some((argument, evaluate, width)) = root_conversion(pool, expression) {
        let argument = evaluated_conversions(pool, backends, settings, argument, &mut uses);
        return match evaluate_in_machine(pool, backends, settings, argument, evaluate, width) {
            Ok((computed, used)) => {
                uses.extend(used);
                ExactOutcome::Rational(Box::new((computed, uses)))
            }
            Err(error) => ExactOutcome::NeedsMachine(error),
        };
    }
    let (complex_input, complex_unit) = match calc_core::to_coherent_units(pool, expression) {
        Ok(coherent) if coherent.unit.is_some() => (coherent.expression, coherent.unit),
        _ => (expression, None),
    };
    let expression = match calc_core::exact_complex(pool, complex_input) {
        None => expression,
        Some(Ok(calc_core::ExactComplexValue::Real(real))) => match complex_unit {
            Some(unit) => pool.quantity(real, unit).unwrap_or(expression),
            None => real,
        },
        Some(Ok(calc_core::ExactComplexValue::Rational { real, imaginary })) => {
            return match ComputedResult::new(
                ResultKind::ExactRational,
                ResultValue::Complex { real, imaginary },
                complex_unit,
                Method::named(EXACT_EVALUATION_METHOD),
            ) {
                Ok(computed) => ExactOutcome::Rational(Box::new((computed, uses))),
                Err(_) => ExactOutcome::Failed(diagnostic("result_not_representable", vec![])),
            };
        }
        Some(Ok(calc_core::ExactComplexValue::Algebraic(value))) => {
            let form = calc_core::literal_form(pool, expression);
            let Some(text) = crate::displayed_value::exact_expression_text_in(pool, value, form)
            else {
                return ExactOutcome::Failed(diagnostic("result_not_representable", vec![]));
            };
            return match ComputedResult::new(
                ResultKind::Algebraic,
                ResultValue::Expression(text),
                complex_unit,
                Method::named(EXACT_EVALUATION_METHOD),
            ) {
                Ok(computed) => ExactOutcome::Rational(Box::new((computed, uses))),
                Err(_) => ExactOutcome::Failed(diagnostic("result_not_representable", vec![])),
            };
        }
        Some(Err(error)) => {
            return ExactOutcome::Failed(exact_error_diagnostic(pool, expression, error));
        }
    };
    let expression = evaluated_conversions(pool, backends, settings, expression, &mut uses);
    let evaluation = match evaluate_exact(pool, expression) {
        Ok(evaluation) => evaluation,
        Err(
            error @ (ExactEvaluationError::MachineNumber(_)
            | ExactEvaluationError::UnsupportedNode(_)
            | ExactEvaluationError::UnsupportedOperator { .. }
            | ExactEvaluationError::UnsupportedConstant(_)),
        ) => {
            return ExactOutcome::NeedsMachine(exact_error_diagnostic(pool, expression, error));
        }
        Err(error) => {
            return ExactOutcome::Failed(exact_error_diagnostic(pool, expression, error));
        }
    };
    let Some(number) = evaluation.rational_value() else {
        let kind = evaluation.kind();
        let unit = evaluation.unit();
        let evaluated = evaluation.expression();
        if !reads_as_a_number(pool, evaluated) {
            return ExactOutcome::NeedsMachine(diagnostic(
                "result_kind_not_representable",
                vec![(KIND_DATA, identifier(kind_name(kind)))],
            ));
        }
        let written = match evaluation.pi_power() {
            Some((coefficient, exponent)) => {
                let coefficient = coefficient.clone();
                pi_power_text(pool, &coefficient, exponent)
            }
            None => {
                let form = calc_core::literal_form(pool, expression);
                crate::displayed_value::exact_expression_text_in(pool, evaluated, form)
            }
        };
        let Some(text) = written else {
            return ExactOutcome::Failed(diagnostic("result_not_representable", vec![]));
        };
        return match ComputedResult::new(
            kind,
            ResultValue::Expression(text),
            unit,
            Method::named(EXACT_EVALUATION_METHOD),
        ) {
            Ok(computed) => ExactOutcome::Rational(Box::new((computed, uses.clone()))),
            Err(_) => ExactOutcome::Failed(diagnostic("result_not_representable", vec![])),
        };
    };
    match ComputedResult::new(
        ResultKind::ExactRational,
        ResultValue::Number(number.clone()),
        evaluation.unit(),
        Method::named(EXACT_EVALUATION_METHOD),
    ) {
        Ok(computed) => ExactOutcome::Rational(Box::new((computed, uses.clone()))),
        Err(_) => ExactOutcome::Failed(diagnostic("result_not_representable", vec![])),
    }
}

pub(crate) fn evaluate_in_machine(
    pool: &mut ExprPool,
    backends: &[Arc<dyn Backend>],
    settings: Settings,
    expression: ExprId,
    evaluate: MachineEvaluator,
    width: Domain,
) -> Result<(ComputedResult, Vec<BackendUse>), Diagnostic> {
    let backends: Vec<&dyn Backend> = backends.iter().map(AsRef::as_ref).collect();
    let evaluation = evaluate(pool, expression, &backends, settings.backend)
        .map_err(|error| machine_error_diagnostic(pool, &error))?;
    let selected = evaluation.backend();
    let backend = BackendUse {
        preference: settings.backend,
        width,
        selected: Some(selected),
        skipped: evaluation.skipped().iter().map(Into::into).collect(),
        approximate_operations: evaluation.has_approximate_operations(),
        modes: backends
            .iter()
            .find(|backend| backend.kind() == selected)
            .map_or_else(Vec::new, |backend| BackendUse::modes_of(*backend)),
    };
    Ok((evaluation.result().clone(), vec![backend]))
}

fn is_positive(number: &Number) -> bool {
    match number {
        Number::Integer(integer) => !integer.is_negative() && !integer.is_zero(),
        Number::Rational(rational) => !rational.numerator().is_negative(),
        Number::F32(value) => *value > 0.0,
        Number::F64(value) => *value > 0.0,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::result_record::UtcTimestamp;
    use calc_core::{METHOD_NAMES, RoundingError};
    use calc_exec_cpu::CpuBackend;
    use calc_numbers::{Integer, Number};

    pub(crate) const FIXED_TIME_MILLISECONDS: u64 = 1_789_476_067_312;
    const NANOSECONDS_PER_READING: u64 = 250;

    pub(crate) fn fixed_clock() -> Box<dyn Clock> {
        Box::new(crate::FixedClock::new(
            UtcTimestamp::from_milliseconds_since_unix_epoch(FIXED_TIME_MILLISECONDS),
            NANOSECONDS_PER_READING,
        ))
    }

    #[test]
    fn a_backend_registered_later_runs_a_machine_line() {
        let mut session = Session::new(fixed_clock(), Vec::new());
        session.enter("1/3").expect("a line");
        session.register_backend(Box::new(CpuBackend::new()));

        let entered = session.enter("to_f64(r1)").expect("a line");

        let line = session.line(entered).expect("the line").clone();
        assert!(matches!(
            session.line_summary(&line).state,
            crate::LineState::Result(_)
        ));
    }

    #[test]
    fn a_machine_line_without_a_backend_fails() {
        let mut session = Session::new(fixed_clock(), Vec::new());
        session.enter("1/3").expect("a line");

        let entered = session.enter("to_f64(r1)").expect("a line");

        let line = session.line(entered).expect("the line").clone();
        assert!(matches!(
            session.line_summary(&line).state,
            crate::LineState::Failed(_)
        ));
    }

    pub(crate) fn session() -> Session {
        Session::new(fixed_clock(), vec![Box::new(CpuBackend::new())])
    }

    fn entering_cost_per_line(count: usize) -> std::time::Duration {
        let mut session = session();
        let start = std::time::Instant::now();
        for value in 1..=count {
            session.enter(&format!("{value} + 1")).unwrap();
        }
        start.elapsed() / u32::try_from(count).unwrap()
    }

    #[test]
    fn a_line_entered_before_its_name_is_defined_is_recomputed_when_it_arrives() {
        let mut computing = session();
        let waiting = computing.enter("a + 1").unwrap();
        computing.enter("a = 5").unwrap();

        let Outcome::Result(record) = computing.line(waiting).unwrap().outcome() else {
            panic!("the waiting line has a result once its name is defined");
        };
        assert_eq!(
            record.computed().value(),
            &ResultValue::Number(Number::Integer(Integer::from(6_i64)))
        );
    }

    #[test]
    fn entering_a_line_costs_the_same_however_many_lines_are_already_there() {
        let few = entering_cost_per_line(1000);
        let many = entering_cost_per_line(8000);

        assert!(
            many < few * 2,
            "cost per line grew from {few:?} to {many:?} over four times the lines"
        );
    }

    fn line(number: u64) -> LineId {
        LineId::from_number(number).unwrap()
    }

    #[test]
    fn escape_time_line_has_a_picture_outcome() {
        let mut session = session();

        let line = session
            .enter(r#"escape_time {"form": "quadratic_parameter"}"#)
            .unwrap();

        assert_eq!(session.line(line).unwrap().outcome(), &Outcome::Picture);
    }

    #[test]
    fn escape_time_line_keeps_its_request() {
        let mut session = session();

        let line = session
            .enter(r#"escape_time {"form": "quadratic_parameter"}"#)
            .unwrap();

        assert_eq!(
            session.escape_time_line_of(line),
            Some(&EscapeTimeLine {
                form: calc_viz::EscapeTimeForm::QuadraticParameter
            })
        );
    }

    #[test]
    fn escape_time_line_is_written_in_canonical_json() {
        let mut session = session();

        let line = session
            .enter(r#"escape_time {"form":"quadratic_parameter"}"#)
            .unwrap();

        assert_eq!(
            session.line(line).unwrap().input(),
            r#"escape_time {"form": "quadratic_parameter"}"#
        );
    }

    #[test]
    fn invalid_escape_time_request_is_refused() {
        let mut session = session();

        let result = session.enter(r#"escape_time {"form": "cubic"}"#);

        assert!(matches!(result, Err(SessionError::InvalidRequest(_))));
    }

    #[test]
    fn editing_an_escape_time_line_changes_its_form() {
        let mut session = session();
        let line = session
            .enter(r#"escape_time {"form": "quadratic_parameter"}"#)
            .unwrap();

        session
            .edit(
                line,
                r#"escape_time {"form": "quadratic_initial", "c": {"real": {"type": "integer", "digits": "0"}, "imaginary": {"type": "integer", "digits": "0"}}}"#,
            )
            .unwrap();

        assert_eq!(
            session
                .escape_time_line_of(line)
                .map(|line| line.form.clone()),
            Some(calc_viz::EscapeTimeForm::QuadraticInitial {
                c_real: Number::from(0_i64),
                c_imaginary: Number::from(0_i64),
            })
        );
    }

    #[test]
    fn editing_an_escape_time_line_into_an_expression_leaves_no_request() {
        let mut session = session();
        let line = session
            .enter(r#"escape_time {"form": "quadratic_parameter"}"#)
            .unwrap();

        session.edit(line, "2 + 2").unwrap();

        assert_eq!(session.escape_time_line_of(line), None);
    }

    fn record(session: &Session, id: LineId) -> &ResultRecord {
        match session.line(id).unwrap().outcome() {
            Outcome::Result(record) => record,
            other => panic!("expected a result, found {other:?}"),
        }
    }

    fn value(session: &Session, id: LineId) -> ResultValue {
        record(session, id).computed().value().clone()
    }

    fn f64_value(session: &Session, id: LineId) -> f64 {
        match value(session, id) {
            ResultValue::Number(Number::F64(machine)) => machine,
            other => panic!("expected an f64 value, found {other:?}"),
        }
    }

    fn note_data(session: &Session, id: LineId) -> Vec<(String, String)> {
        let notes = &record(session, id).computed().method().notes;
        let [note] = notes.as_slice() else {
            panic!("expected one note, found {notes:?}")
        };
        assert_eq!(note.code, "temperature_difference");
        note.data
            .iter()
            .map(|(name, value)| match value {
                ParameterValue::Identifier(text) => (name.clone(), text.clone()),
                other => panic!("expected an identifier, found {other:?}"),
            })
            .collect()
    }

    fn error_code(session: &Session, id: LineId) -> String {
        match session.line(id).unwrap().outcome() {
            Outcome::Error(error) => error.code.clone(),
            other => panic!("expected an error, found {other:?}"),
        }
    }

    fn exact(numerator: i64, denominator: i64) -> ResultValue {
        ResultValue::Number(
            Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap(),
        )
    }

    #[test]
    fn new_session_is_empty_with_f64_and_automatic_backend() {
        let session = session();

        assert_eq!(
            (session.lines().len(), session.settings()),
            (0, Settings::default())
        );
    }

    #[test]
    fn entered_expression_gets_the_next_label_and_a_result() {
        let mut session = session();

        let id = session.enter("to_f64(1/3)").unwrap();

        assert_eq!(
            (id, f64_value(&session, id).to_bits()),
            (line(1), (1.0_f64 / 3.0).to_bits())
        );
    }

    #[test]
    fn exact_rational_result_is_kept_under_the_f64_setting() {
        let mut session = session();

        let id = session.enter("1/3 + 1/6").unwrap();

        assert_eq!(
            (value(&session, id), record(&session, id).computed().kind()),
            (exact(1, 2), ResultKind::ExactRational)
        );
    }

    #[test]
    fn decimal_sum_stays_exact_under_the_f64_setting() {
        let mut session = session();

        let id = session.enter("0.1 + 0.2").unwrap();

        assert_eq!(value(&session, id), exact(3, 10));
    }

    #[test]
    fn exact_result_under_the_f64_setting_selects_no_backend() {
        let mut session = session();

        let id = session.enter("2^10").unwrap();

        assert!(record(&session, id).backend().is_empty());
    }

    fn notes_of(session: &Session, id: LineId) -> Vec<String> {
        record(session, id)
            .computed()
            .method()
            .notes
            .iter()
            .map(|note| note.code.clone())
            .collect()
    }

    #[test]
    fn a_bound_that_cannot_separate_the_value_from_zero_names_the_exact_route() {
        let mut session = session();

        let id = session
            .enter("to_f64(sqrt(1e16 + 1) - sqrt(1e16))")
            .unwrap();

        assert_eq!(notes_of(&session, id), vec!["bound_not_smaller_than_value"]);
    }

    #[test]
    fn a_bound_smaller_than_its_value_says_nothing() {
        let mut session = session();

        let id = session.enter("to_f64(1/3)").unwrap();

        assert!(notes_of(&session, id).is_empty());
    }

    #[test]
    fn a_machine_zero_that_is_the_exact_value_gets_no_note() {
        let mut session = session();

        let id = session.enter("to_f64(1 - 1)").unwrap();

        assert!(notes_of(&session, id).is_empty());
    }

    #[test]
    fn a_machine_zero_that_cancelled_away_from_a_value_names_the_exact_route() {
        let mut session = session();

        let id = session.enter("to_f64(1e-300 * 1e-300)").unwrap();

        assert_eq!(notes_of(&session, id), vec!["bound_not_smaller_than_value"]);
    }

    #[test]
    fn a_line_with_no_conversion_gets_no_exact_route_note() {
        let mut session = session();

        let id = session.enter("sqrt(1e16 + 1) - sqrt(1e16)").unwrap();

        assert!(notes_of(&session, id).is_empty());
    }

    #[test]
    fn the_error_of_a_machine_value_is_the_number_the_line_asks_for() {
        let mut session = session();

        let id = session.enter("1/3 - to_f64(1/3)").unwrap();

        assert_eq!(
            (value(&session, id), record(&session, id).computed().kind()),
            (exact(1, 54_043_195_528_445_952), ResultKind::ExactRational)
        );
    }

    #[test]
    fn two_widths_stand_in_one_line_and_the_record_names_both() {
        let mut session = session();

        let id = session.enter("to_f64(1/3) - to_f32(1/3)").unwrap();

        let stored = record(&session, id);
        assert_eq!(stored.computed().kind(), ResultKind::ExactRational);
        assert_eq!(
            stored
                .backend()
                .iter()
                .map(|used| used.width)
                .collect::<Vec<Domain>>(),
            vec![Domain::F64, Domain::F32]
        );
    }

    #[test]
    fn an_approximate_operation_inside_a_conversion_keeps_the_machine_path() {
        let mut session = session();

        let id = session.enter("to_f64(sin(1)) - 1").unwrap();

        assert_eq!(
            record(&session, id).computed().kind(),
            ResultKind::MachineFloat
        );
    }

    #[test]
    fn a_conversion_names_its_own_width_whatever_the_session_says() {
        let mut session = session();

        let id = session.enter("to_f32(1/3)").unwrap();

        let stored = record(&session, id);
        assert_eq!(
            (
                value(&session, id),
                stored.backend().first().map(|used| used.width)
            ),
            (
                ResultValue::Number(Number::F32(1.0f32 / 3.0f32)),
                Some(Domain::F32)
            )
        );
    }

    #[test]
    fn a_line_that_ran_no_plan_carries_no_backend_at_all() {
        let mut session = session();

        let id = session.enter("1/3 + 1/6").unwrap();

        assert!(record(&session, id).backend().is_empty());
    }

    #[test]
    fn a_machine_operand_is_the_rational_it_is() {
        let mut session = session();

        let id = session.enter("f64'0.5' + 1").unwrap();

        assert_eq!(
            (value(&session, id), record(&session, id).computed().kind()),
            (exact(3, 2), ResultKind::ExactRational)
        );
    }

    #[test]
    fn algebraic_exact_outcome_keeps_its_expression() {
        let mut session = session();

        let id = session.enter("sqrt(2) + 1").unwrap();

        assert_eq!(
            (record(&session, id).computed().kind(), value(&session, id)),
            (
                ResultKind::Algebraic,
                ResultValue::Expression("1 + sqrt(2)".to_owned())
            )
        );
    }

    #[test]
    fn symbolic_exact_outcome_keeps_its_expression() {
        let mut session = session();

        let id = session.enter("exp(2)").unwrap();

        assert_eq!(
            (record(&session, id).computed().kind(), value(&session, id)),
            (
                ResultKind::Symbolic,
                ResultValue::Expression("exp(2)".to_owned())
            )
        );
    }

    #[test]
    fn a_sum_over_exact_operands_is_evaluated_exactly() {
        let mut session = session();

        let id = session.enter("sum(i, i, 1, 3)").unwrap();

        assert_eq!(value(&session, id), ResultValue::Number(Number::from(6)));
    }

    #[test]
    fn a_sum_with_a_machine_operand_goes_to_the_f64_plan() {
        let mut session = session();

        let id = session.enter("sum(to_f64(i), i, 1, 3)").unwrap();

        assert_eq!(f64_value(&session, id), 6.0);
    }

    #[test]
    fn a_physical_constant_is_its_value_with_its_unit() {
        let mut session = session();

        let id = session.enter("2 * c_0").unwrap();

        assert_eq!(
            value(&session, id),
            ResultValue::Number(Number::from(599_584_916))
        );
    }

    #[test]
    fn a_physical_constant_carries_its_unit() {
        let mut session = session();

        let id = session.enter("c_0").unwrap();

        let unit = record(&session, id).computed().unit().unwrap();
        let metres_per_second = {
            let metre = session.pool.units_mut().lookup("m").unwrap();
            let second = session.pool.units_mut().lookup("s").unwrap();
            let per_second = session.pool.units_mut().power(second, -1).unwrap();
            session
                .pool
                .units_mut()
                .multiply(metre, per_second)
                .unwrap()
        };
        assert_eq!(unit, metres_per_second);
    }

    #[test]
    fn a_measured_constant_carries_its_standard_uncertainty() {
        let mut session = session();

        let id = session.enter("G_N").unwrap();

        assert!(record(&session, id).computed().uncertainty().is_some());
    }

    #[test]
    fn an_exact_constant_carries_no_uncertainty() {
        let mut session = session();

        let id = session.enter("c_0").unwrap();

        assert!(record(&session, id).computed().uncertainty().is_none());
    }

    #[test]
    fn a_bare_temperature_difference_says_it_is_one_and_names_the_reading() {
        let mut session = session();

        let id = session.enter("20 degC").unwrap();

        assert_eq!(
            note_data(&session, id),
            vec![
                ("operator".to_owned(), "from_celsius".to_owned()),
                ("reading".to_owned(), "from_celsius(20)".to_owned()),
                ("unit".to_owned(), "\u{b0}C".to_owned()),
            ]
        );
    }

    #[test]
    fn a_conversion_into_a_temperature_scale_names_the_other_operator() {
        let mut session = session();

        let id = session.enter("300 K -> degC").unwrap();

        assert_eq!(
            note_data(&session, id),
            vec![
                ("operator".to_owned(), "to_celsius".to_owned()),
                ("reading".to_owned(), "to_celsius(300 K)".to_owned()),
                ("unit".to_owned(), "\u{b0}C".to_owned()),
            ]
        );
    }

    #[test]
    fn a_difference_inside_an_expression_names_no_text_to_write() {
        let mut session = session();

        let id = session.enter("5 degC + 3 degC").unwrap();

        assert_eq!(
            note_data(&session, id),
            vec![
                ("operator".to_owned(), "from_celsius".to_owned()),
                ("unit".to_owned(), "\u{b0}C".to_owned()),
            ]
        );
    }

    #[test]
    fn a_temperature_reading_carries_no_note() {
        let mut session = session();

        let reading = session.enter("from_celsius(20)").unwrap();
        let kelvin = session.enter("20 K").unwrap();

        assert_eq!(
            (
                record(&session, reading).computed().method().notes.len(),
                record(&session, kelvin).computed().method().notes.len()
            ),
            (0, 0)
        );
    }

    #[test]
    fn an_undefined_bare_symbol_names_the_constant_that_carries_it() {
        let mut session = session();

        let id = session.enter("c").unwrap();

        assert_eq!(
            (
                error_code(&session, id),
                error_data(&session, id, CONSTANT_DATA)
            ),
            ("undefined_name".to_string(), "c_0".to_string())
        );
    }

    #[test]
    fn an_undefined_name_that_is_no_constant_symbol_is_an_undefined_name() {
        let mut session = session();

        let id = session.enter("zz(2)").unwrap();

        assert_eq!(error_code(&session, id), "undefined_name");
    }

    #[test]
    fn a_name_a_person_gives_wins_over_a_constant() {
        let mut session = session();
        session.enter("c_0 = 5").unwrap();

        let id = session.enter("c_0 * 2").unwrap();

        assert_eq!(value(&session, id), ResultValue::Number(Number::from(10)));
    }

    #[test]
    fn quantity_line_records_its_value_in_newtons() {
        let mut session = session();

        let id = session.enter("2.50 kg * 9.81 m/s^2").unwrap();

        let unit = record(&session, id).computed().unit().unwrap();
        let newton = session.pool.units_mut().lookup("N").unwrap();
        assert_eq!((value(&session, id), unit), (exact(981, 40), newton));
    }

    #[test]
    fn machine_quantity_line_records_its_unit() {
        let mut session = session();

        let id = session.enter("to_f64((1/3) km -> m)").unwrap();

        let unit = record(&session, id).computed().unit().unwrap();
        let metre = session.pool.units_mut().lookup("m").unwrap();
        assert_eq!(
            (record(&session, id).computed().kind(), unit),
            (ResultKind::MachineFloat, metre)
        );
    }

    #[test]
    fn sine_of_an_angle_in_degrees_converts_through_its_unit() {
        let mut session = session();

        let id = session.enter("sin(30 deg)").unwrap();

        assert_eq!(value(&session, id), exact(1, 2));
    }

    #[test]
    fn a_sine_of_a_machine_number_of_degrees_uses_the_unit_too() {
        let mut session = session();

        let id = session.enter("sin(f64'90' deg)").unwrap();

        assert_eq!(
            value(&session, id),
            ResultValue::Number(Number::from(1_i64))
        );
    }

    #[test]
    fn inhomogeneous_line_fails_with_dimension_mismatch() {
        let mut session = session();

        let id = session.enter("3 km + 2 s").unwrap();

        assert_eq!(error_code(&session, id), "dimension_mismatch");
    }

    #[test]
    fn quantity_from_a_dependency_keeps_its_dimension() {
        let mut session = session();
        session.enter("mass = 2 kg").unwrap();

        let weight = session.enter("mass * 9.81 m/s^2").unwrap();

        let unit = record(&session, weight).computed().unit().unwrap();
        let newton = session.pool.units_mut().lookup("N").unwrap();
        assert_eq!(unit, newton);
    }

    #[test]
    fn exact_domain_failure_is_not_retried_in_f64() {
        let mut session = session();

        let id = session.enter("1/0").unwrap();

        assert_eq!(error_code(&session, id), "division_by_zero");
    }

    #[test]
    fn exact_rational_result_is_kept_under_the_f32_setting() {
        let mut session = session();
        session.set_precision(Precision::F32);

        let id = session.enter("1/4").unwrap();

        assert_eq!(value(&session, id), exact(1, 4));
    }

    #[test]
    fn machine_result_carries_backend_and_rounding_bound() {
        let mut session = session();

        let id = session.enter("to_f64(1/3)").unwrap();

        let stored = record(&session, id);
        assert_eq!(
            stored.backend().first().and_then(|used| used.selected),
            Some(calc_exec::BackendKind::Cpu)
        );
        assert!(matches!(
            stored.computed().rounding_error(),
            RoundingError::Bound(_)
        ));
    }

    fn error_of(session: &Session, id: LineId) -> Diagnostic {
        match session.line(id).unwrap().outcome() {
            Outcome::Error(error) => error.clone(),
            other => panic!("expected an error, found {other:?}"),
        }
    }

    fn data_of(error: &Diagnostic, name: &str) -> ParameterValue {
        error.data.get(name).cloned().unwrap()
    }

    fn expression_data(text: &str) -> ParameterValue {
        ParameterValue::Value(ResultValue::Expression(text.to_owned()))
    }

    fn integer_data(value: Integer) -> ParameterValue {
        ParameterValue::Value(ResultValue::Number(Number::Integer(value)))
    }

    #[test]
    fn division_by_zero_carries_the_division_as_its_reading() {
        let mut computing = session();
        let id = computing.enter("1/(2-2)").unwrap();

        assert_eq!(
            data_of(&error_of(&computing, id), "reading"),
            expression_data("1 / (2 - 2)")
        );
    }

    #[test]
    fn power_with_a_too_large_exponent_carries_reading_argument_and_limit() {
        let mut computing = session();
        let id = computing.enter("10^(100^10)").unwrap();
        let error = error_of(&computing, id);

        assert_eq!(
            (
                data_of(&error, "reading"),
                data_of(&error, "argument"),
                data_of(&error, "limit")
            ),
            (
                expression_data("10^(100^10)"),
                integer_data(Integer::from(10_i64).pow(20)),
                integer_data(Integer::from(4_294_967_295_i64))
            )
        );
    }

    #[test]
    fn too_large_whole_result_is_result_too_large() {
        let mut computing = session();
        let id = computing.enter("2^262144").unwrap();

        assert_eq!(error_code(&computing, id), "result_too_large");
    }

    #[test]
    fn too_large_result_carries_its_digits_and_the_limit() {
        let mut computing = session();
        let id = computing.enter("2^262144").unwrap();
        let error = error_of(&computing, id);

        assert_eq!(
            (
                data_of(&error, "estimated_digits"),
                data_of(&error, "limit_bits")
            ),
            (
                integer_data(Integer::from(78_914_i64)),
                integer_data(Integer::from(262_144_i64))
            )
        );
    }

    #[test]
    fn too_large_intermediate_step_is_named_as_a_step() {
        let mut computing = session();
        let id = computing.enter("2^262144 * 0").unwrap();
        let error = error_of(&computing, id);

        assert_eq!(
            (error.code.as_str(), data_of(&error, "reading")),
            ("intermediate_step_too_large", expression_data("2^262144"))
        );
    }

    #[test]
    fn dimension_mismatch_carries_its_reading() {
        let mut computing = session();
        let id = computing.enter("1 m + 1 s").unwrap();
        let error = error_of(&computing, id);

        assert_eq!(
            (error.code.as_str(), data_of(&error, "reading")),
            ("dimension_mismatch", expression_data("1 m + 1 s"))
        );
    }

    #[test]
    fn negative_uncertainty_carries_its_reading() {
        let mut measuring = session();
        let id = measuring.enter("1 +- -0.1").unwrap();

        assert_eq!(
            data_of(&error_of(&measuring, id), "reading"),
            expression_data("1 +- -0.1")
        );
    }

    fn opened_with_input_replaced(entered: &[&str], stored: &str, replaced: &str) -> Session {
        let mut computing = session();
        for text in entered {
            computing.enter(text).unwrap();
        }
        let saved = String::from_utf8(computing.save_to_bytes().unwrap()).unwrap();
        let written = saved.replace(&format!("\"{stored}\""), &format!("\"{replaced}\""));
        assert_ne!(written, saved);
        Session::open_from_bytes(written.as_bytes(), fixed_clock(), Vec::new()).unwrap()
    }

    fn identifier_data(session: &Session, id: LineId, name: &str) -> String {
        match session.line(id).unwrap().outcome() {
            Outcome::Error(error) => match error.data.get(name) {
                Some(ParameterValue::Identifier(text)) => text.clone(),
                other => panic!("expected an identifier, found {other:?}"),
            },
            other => panic!("expected an error, found {other:?}"),
        }
    }

    #[test]
    fn ambiguous_application_is_refused_on_entry() {
        let mut session = session();

        let error = session.enter("sin pi/2").unwrap_err();

        assert!(matches!(
            error,
            SessionError::Parse(ParseError {
                kind: ParseErrorKind::AmbiguousApplication(_),
                ..
            })
        ));
    }

    #[test]
    fn stored_line_that_is_now_ambiguous_opens_as_refused_with_both_readings() {
        let opened = opened_with_input_replaced(&["sin(pi)/2"], "sin(pi)/2", "sin pi/2");

        assert_eq!(
            (
                error_code(&opened, line(1)),
                identifier_data(&opened, line(1), "written"),
                identifier_data(&opened, line(1), "narrow"),
                identifier_data(&opened, line(1), "wide"),
                identifier_data(&opened, line(1), "column"),
            ),
            (
                "ambiguous_application".to_string(),
                "sin pi/2".to_string(),
                "sin(pi)/2".to_string(),
                "sin(pi/2)".to_string(),
                "7".to_string(),
            )
        );
    }

    #[test]
    fn stored_line_that_no_longer_parses_names_its_kind_and_column() {
        let opened = opened_with_input_replaced(&["2 + 3"], "2 + 3", "2 +");

        assert_eq!(
            (
                error_code(&opened, line(1)),
                identifier_data(&opened, line(1), "kind"),
                identifier_data(&opened, line(1), "column"),
            ),
            (
                "parse_error".to_string(),
                "unexpected_end".to_string(),
                "4".to_string(),
            )
        );
    }

    #[test]
    fn stored_line_with_a_temperature_sign_opens_with_both_readings() {
        let opened =
            opened_with_input_replaced(&["from_celsius(20)"], "from_celsius(20)", "20 \u{00B0}C");

        assert_eq!(
            (
                error_code(&opened, line(1)),
                identifier_data(&opened, line(1), "sign"),
                identifier_data(&opened, line(1), "written"),
                identifier_data(&opened, line(1), "difference"),
                identifier_data(&opened, line(1), "reading"),
                identifier_data(&opened, line(1), "column"),
            ),
            (
                "ambiguous_temperature_sign".to_string(),
                "celsius".to_string(),
                "20 \u{00B0}C".to_string(),
                "20 degC".to_string(),
                "from_celsius(20)".to_string(),
                "4".to_string(),
            )
        );
    }

    #[test]
    fn dependents_of_a_refused_stored_line_fail_on_opening() {
        let opened =
            opened_with_input_replaced(&["sin(pi)/2", "r1 + 1", "r2 * 2"], "sin(pi)/2", "sin pi/2");

        assert_eq!(
            (
                error_code(&opened, line(2)),
                identifier_data(&opened, line(2), "line"),
                error_code(&opened, line(3)),
                identifier_data(&opened, line(3), "line"),
            ),
            (
                "dependency_failed".to_string(),
                "r1".to_string(),
                "dependency_failed".to_string(),
                "r2".to_string(),
            )
        );
    }

    #[test]
    fn stored_line_that_still_parses_keeps_its_outcome_on_opening() {
        let opened = opened_with_input_replaced(&["sin(pi)/2", "3 + 4"], "sin(pi)/2", "sin pi/2");

        assert!(matches!(
            opened.line(line(2)).unwrap().outcome(),
            Outcome::Result(_)
        ));
    }

    #[test]
    fn error_with_a_reading_survives_saving_and_opening() {
        let mut computing = session();
        let id = computing.enter("10^(100^10)").unwrap();
        let bytes = computing.save_to_bytes().unwrap();

        let opened = Session::open_from_bytes(&bytes, fixed_clock(), Vec::new()).unwrap();

        assert_eq!(error_of(&opened, id), error_of(&computing, id));
    }

    fn without_data_member(text: &str, member: &str) -> String {
        let start = text.find(&format!("\"{member}\"")).unwrap();
        let open = start + text[start..].find('{').unwrap();
        let mut depth = 0;
        let close = text[open..]
            .char_indices()
            .find_map(|(offset, character)| {
                match character {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
                (depth == 0).then_some(open + offset)
            })
            .unwrap();
        format!("{}{}", &text[..start], text[close + 1..].trim_start())
    }

    #[test]
    fn stored_error_saved_without_a_reading_opens_with_its_plain_message() {
        let mut computing = session();
        let id = computing.enter("1/(2-2)").unwrap();
        let text = String::from_utf8(computing.save_to_bytes().unwrap()).unwrap();
        let old = without_data_member(&text, "reading");

        let opened = Session::open_from_bytes(old.as_bytes(), fixed_clock(), Vec::new()).unwrap();

        assert_eq!(
            crate::diagnostic_message(&error_of(&opened, id)),
            calc_i18n::Message::ErrorDivisionByZeroPlain
        );
    }

    #[test]
    fn negative_uncertainty_fails_the_line_with_its_own_code() {
        let mut measuring = session();
        let id = measuring.enter("1 +- -0.1").unwrap();

        assert_eq!(error_code(&measuring, id), "negative_uncertainty");
    }

    #[test]
    fn nested_uncertainty_fails_the_line_with_its_own_code() {
        let mut measuring = session();
        let id = measuring.enter("(1 +- (1 +- 1))").unwrap();

        assert_eq!(error_code(&measuring, id), "nested_uncertainty");
    }

    #[test]
    fn coverage_factor_below_one_fails_the_line_with_its_own_code() {
        let mut measuring = session();
        let id = measuring.enter("2 +- 0.1 (k=0.5)").unwrap();

        assert_eq!(error_code(&measuring, id), "coverage_factor_below_one");
    }

    #[test]
    fn other_uncertainty_failure_is_uncertainty_not_propagated() {
        let error = UncertaintyError::UnsupportedOperator(Operator::Less);

        assert_eq!(
            uncertainty_diagnostic(&ExprPool::new(), &error).code,
            "uncertainty_not_propagated"
        );
    }

    #[test]
    fn exact_line_method_is_a_listed_method_name() {
        let mut computing = session();
        let id = computing.enter("1/3").unwrap();

        assert!(METHOD_NAMES.contains(&record(&computing, id).computed().method().name.as_str()));
    }

    #[test]
    fn machine_line_method_is_a_listed_method_name() {
        let mut computing = session();
        let id = computing.enter("sqrt(2)").unwrap();

        assert!(METHOD_NAMES.contains(&record(&computing, id).computed().method().name.as_str()));
    }

    #[test]
    fn measured_value_gets_its_standard_uncertainty() {
        let mut measuring = session();
        let id = measuring.enter("2.5 +- 0.01 m").unwrap();

        let computed = record(&measuring, id).computed();
        let uncertainty = computed.uncertainty().unwrap();

        assert_eq!(
            (
                computed.value(),
                uncertainty.coverage_factor(),
                computed.method().parameters.get("uncertainty_propagation")
            ),
            (
                &ResultValue::Number(
                    Number::fraction(&Integer::from(5_i64), &Integer::from(2_i64)).unwrap()
                ),
                &Number::from(1_i64),
                Some(&ParameterValue::Identifier("first_order_taylor".to_owned()))
            )
        );
    }

    #[test]
    fn product_of_measured_values_propagates_both_uncertainties() {
        let mut measuring = session();
        let id = measuring
            .enter("(2.5 +- 0.01 kg) * (9.81 +- 0.02 m/s^2)")
            .unwrap();

        let computed = record(&measuring, id).computed();

        assert_eq!(
            computed.uncertainty().unwrap().standard(),
            &ResultValue::Number(Number::F64(0.110_107_265_881_957_13))
        );
    }

    #[test]
    fn two_measurements_written_alike_are_two_inputs() {
        let mut measuring = session();
        let id = measuring.enter("(10 +- 1) * (10 +- 1)").unwrap();

        let computed = record(&measuring, id).computed();

        assert_eq!(
            (
                computed.uncertainty().unwrap().standard(),
                computed.method().parameters.get("uncertain_input_count")
            ),
            (
                &ResultValue::Number(Number::F64(14.142_135_623_730_951)),
                Some(&ParameterValue::Value(ResultValue::Number(Number::from(
                    2_i64
                ))))
            )
        );
    }

    #[test]
    fn one_measurement_squared_is_one_input() {
        let mut measuring = session();
        let id = measuring.enter("(10 +- 1)^2").unwrap();

        let computed = record(&measuring, id).computed();

        assert_eq!(
            (
                computed.uncertainty().unwrap().standard(),
                computed.method().parameters.get("uncertain_input_count")
            ),
            (
                &ResultValue::Number(Number::F64(20.0)),
                Some(&ParameterValue::Value(ResultValue::Number(Number::from(
                    1_i64
                ))))
            )
        );
    }

    #[test]
    fn a_measurement_written_as_a_call_keeps_its_unit() {
        let mut measuring = session();
        let id = measuring.enter("uncertain(2 kg, 0.1 kg)").unwrap();

        let computed = record(&measuring, id).computed();

        assert_eq!(
            computed.uncertainty().unwrap().standard(),
            &ResultValue::Number(Number::F64(0.1))
        );
    }

    #[test]
    fn difference_of_two_measurements_is_not_certain() {
        let mut measuring = session();
        let id = measuring.enter("(2 +- 0.1) - (2 +- 0.1)").unwrap();

        let computed = record(&measuring, id).computed();

        assert_eq!(
            computed.uncertainty().unwrap().standard(),
            &ResultValue::Number(Number::F64(0.141_421_356_237_309_53))
        );
    }

    #[test]
    fn one_named_measurement_used_twice_is_one_input() {
        let mut measuring = session();
        measuring.enter("x = 10 +- 1").unwrap();
        let id = measuring.enter("x * x").unwrap();

        let computed = record(&measuring, id).computed();

        assert_eq!(
            (
                computed.uncertainty().unwrap().standard(),
                computed.method().parameters.get("uncertain_input_count")
            ),
            (
                &ResultValue::Number(Number::F64(20.0)),
                Some(&ParameterValue::Value(ResultValue::Number(Number::from(
                    1_i64
                ))))
            )
        );
    }

    #[test]
    fn expanded_uncertainty_is_divided_by_its_coverage_factor() {
        let mut measuring = session();
        let id = measuring.enter("2.50 +- 0.02 (k=2)").unwrap();

        let standard = record(&measuring, id)
            .computed()
            .uncertainty()
            .unwrap()
            .standard()
            .clone();

        assert_eq!(standard, ResultValue::Number(Number::F64(0.01)));
    }

    #[test]
    fn naming_statement_binds_the_name() {
        let mut session = session();

        let id = session.enter("mass = 2.5").unwrap();

        assert_eq!(
            (session.line(id).unwrap().name(), session.resolve("mass")),
            (Some("mass"), Some(id))
        );
    }

    #[test]
    fn dependencies_are_recorded_in_order_of_first_use() {
        let mut session = session();
        let a = session.enter("a = 2").unwrap();
        let b = session.enter("b = 3").unwrap();

        let c = session.enter("c = b * a + b").unwrap();

        assert_eq!(record(&session, c).dependencies(), &[b, a]);
    }

    #[test]
    fn dependencies_follow_the_written_order_from_left_to_right() {
        let mut session = session();
        let a = session.enter("a = 2").unwrap();
        let b = session.enter("b = 3").unwrap();

        let c = session.enter("c = a + b * b").unwrap();

        assert_eq!(record(&session, c).dependencies(), &[a, b]);
    }

    #[test]
    fn label_refers_to_an_unnamed_line() {
        let mut session = session();
        let first = session.enter("20").unwrap();

        let second = session.enter("r1 + 1").unwrap();

        assert_eq!(
            (
                value(&session, second),
                record(&session, second).dependencies()
            ),
            (exact(21, 1), &[first][..])
        );
    }

    #[test]
    fn dependency_is_substituted_as_its_expression_so_exact_values_stay_exact() {
        let mut session = session();
        session.set_precision(Precision::Exact);
        session.enter("third = 1/3").unwrap();

        let whole = session.enter("third * 3").unwrap();

        assert_eq!(value(&session, whole), exact(1, 1));
    }

    #[test]
    fn machine_bound_of_a_dependent_covers_the_whole_chain() {
        let mut session = session();
        session.enter("root = sqrt(2)").unwrap();

        let sum = session.enter("root + 0.2").unwrap();

        let direct = session.enter("sqrt(2) + 0.2").unwrap();
        assert_eq!(
            record(&session, sum).computed().rounding_error(),
            record(&session, direct).computed().rounding_error()
        );
    }

    fn error_data(session: &Session, id: LineId, name: &str) -> String {
        match session.line(id).unwrap().outcome() {
            Outcome::Error(error) => match error.data.get(name) {
                Some(ParameterValue::Identifier(text)) => text.clone(),
                other => panic!("expected an identifier, found {other:?}"),
            },
            other => panic!("expected an error, found {other:?}"),
        }
    }

    #[test]
    fn unit_ended_at_space_before_division_is_a_recognised_attempt() {
        let mut session = session();

        let id = session.enter("9.81 m / s^2").unwrap();

        assert_eq!(
            (
                error_code(&session, id),
                error_data(&session, id, "attempt"),
                error_data(&session, id, "name"),
                error_data(&session, id, "replacement"),
                error_data(&session, id, "corrected"),
                error_data(&session, id, "column"),
            ),
            (
                "recognised_attempt".to_string(),
                "unit_ended_at_space".to_string(),
                "s".to_string(),
                "m/s^2".to_string(),
                "9.81 m/s^2".to_string(),
                "7".to_string(),
            )
        );
    }

    #[test]
    fn unit_written_together_evaluates() {
        let mut session = session();

        let id = session.enter("9.81 m/s^2").unwrap();

        assert!(matches!(
            session.line(id).unwrap().outcome(),
            Outcome::Result(_)
        ));
    }

    #[test]
    fn variable_divided_by_an_undefined_unit_spelling_keeps_undefined_name() {
        let mut session = session();
        session.enter("d = 2 m").unwrap();

        let id = session.enter("d / s").unwrap();

        assert_eq!(error_code(&session, id), "undefined_name");
    }

    #[test]
    fn quantity_times_an_undefined_unit_spelling_keeps_undefined_name() {
        let mut session = session();

        let id = session.enter("v = 3 s * t").unwrap();

        assert_eq!(error_code(&session, id), "undefined_name");
    }

    #[test]
    fn quantity_inside_a_larger_left_operand_keeps_undefined_name() {
        let mut session = session();

        let id = session.enter("2 * 3 m / s").unwrap();

        assert_eq!(error_code(&session, id), "undefined_name");
    }

    #[test]
    fn a_name_after_a_unit_that_is_no_unit_spelling_is_a_free_name() {
        let mut session = session();

        let id = session.enter("9.81 m / speed").unwrap();

        assert_eq!(
            free_names_of_line(&session, id),
            Some(vec!["speed".to_string()])
        );
    }

    #[test]
    fn undefined_name_elsewhere_in_a_line_with_a_unit_ending_keeps_undefined_name() {
        let mut session = session();

        let id = session.enter("9.81 m / 2 + s").unwrap();

        assert_eq!(error_code(&session, id), "undefined_name");
    }

    #[test]
    fn a_reference_to_an_unknown_name_is_answered_over_it_as_a_free_name() {
        let mut session = session();

        let id = session.enter("speed * 2").unwrap();

        assert_eq!(
            free_names_of_line(&session, id),
            Some(vec!["speed".to_string()])
        );
    }

    fn free_names_of_line(session: &Session, id: LineId) -> Option<Vec<String>> {
        match session.line(id).map(Line::outcome) {
            Some(Outcome::Result(record)) => free_names_in(record.computed()),
            _ => None,
        }
    }

    fn symbolic_text(session: &Session, id: LineId) -> Option<String> {
        match session.line(id).map(Line::outcome) {
            Some(Outcome::Result(record)) => match record.computed().value() {
                ResultValue::Expression(text) => Some(text.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    #[test]
    fn a_polynomial_in_a_free_name_is_answered_with_its_normal_form() {
        let mut session = session();

        let id = session.enter("(x + 1)^2").unwrap();

        assert_eq!(
            symbolic_text(&session, id).as_deref(),
            Some("x^2 + 2 * x + 1")
        );
    }

    #[test]
    fn a_named_polynomial_is_differentiated_in_the_name_it_carries() {
        let mut session = session();
        session.enter("p = x^2 - 5*x + 6").unwrap();
        session.enter("q = p*(x + 1)").unwrap();

        let id = session.enter("diff(q, x)").unwrap();

        assert_eq!(
            symbolic_text(&session, id).as_deref(),
            Some("3 * x^2 - 8 * x + 1")
        );
    }

    #[test]
    fn a_free_name_that_a_binder_binds_is_no_longer_free() {
        let mut session = session();
        session.enter("p = x^2").unwrap();

        let id = session.enter("sum(p, x, 1, 3)").unwrap();

        assert_eq!(free_names_of_line(&session, id), None);
        assert!(matches!(
            session.line(id).map(Line::outcome),
            Some(Outcome::Result(_))
        ));
    }

    #[test]
    fn an_unknown_function_is_no_free_name() {
        let mut session = session();

        let id = session.enter("f(x) + 1").unwrap();

        assert_eq!(error_code(&session, id), "undefined_name");
    }

    #[test]
    fn a_missing_line_label_is_no_free_name() {
        let mut session = session();

        let id = session.enter("r9 + 1").unwrap();

        assert_eq!(error_code(&session, id), "undefined_name");
    }

    #[test]
    fn line_recomputes_when_its_missing_name_is_entered() {
        let mut session = session();
        let dependent = session.enter("speed * 2").unwrap();

        session.enter("speed = 4").unwrap();

        assert_eq!(value(&session, dependent), exact(8, 1));
    }

    #[test]
    fn edit_recomputes_dependents_in_dependency_order() {
        let mut session = session();
        let a = session.enter("a = 1").unwrap();
        let c = session.enter("c = b + a").unwrap();
        let b = session.enter("b = a + 1").unwrap();

        let recomputed = session.edit(a, "a = 10").unwrap();

        assert_eq!(
            (recomputed, value(&session, b), value(&session, c)),
            (vec![a, b, c], exact(11, 1), exact(21, 1))
        );
    }

    #[test]
    fn edit_leaves_independent_lines_alone() {
        let mut session = session();
        let a = session.enter("a = 1").unwrap();
        session.enter("z = 5").unwrap();

        let recomputed = session.edit(a, "a = 2").unwrap();

        assert_eq!(recomputed, vec![a]);
    }

    #[test]
    fn edit_with_expression_statement_keeps_the_name() {
        let mut session = session();
        let a = session.enter("a = 1").unwrap();

        session.edit(a, "7").unwrap();

        assert_eq!(session.resolve("a"), Some(a));
    }

    #[test]
    fn edit_that_creates_a_cycle_is_rejected_and_changes_nothing() {
        let mut session = session();
        let a = session.enter("a = 1").unwrap();
        session.enter("b = a + 1").unwrap();
        let before = session.lines().to_vec();

        let result = session.edit(a, "a = b + 1");

        assert_eq!(
            (result, session.lines()),
            (Err(SessionError::ReferenceCycle(a)), &before[..])
        );
    }

    #[test]
    fn entered_line_that_closes_a_cycle_is_rejected() {
        let mut session = session();
        session.enter("y = x + 1").unwrap();

        let result = session.enter("x = y");

        assert_eq!(
            (result, session.lines().len()),
            (Err(SessionError::ReferenceCycle(line(2))), 1)
        );
    }

    #[test]
    fn line_referring_to_itself_is_a_cycle() {
        let mut session = session();

        let result = session.enter("x = x + 1");

        assert_eq!(result, Err(SessionError::ReferenceCycle(line(1))));
    }

    #[test]
    fn rejected_line_does_not_use_up_a_line_number() {
        let mut session = session();
        session.enter("x = x").unwrap_err();

        let id = session.enter("1").unwrap();

        assert_eq!(id, line(1));
    }

    #[test]
    fn existing_name_is_rejected() {
        let mut session = session();
        session.enter("a = 1").unwrap();

        let result = session.enter("a = 2");

        assert_eq!(result, Err(SessionError::NameExists("a".to_string())));
    }

    #[test]
    fn renaming_a_line_that_others_use_is_rejected() {
        let mut session = session();
        let a = session.enter("a = 1").unwrap();
        let b = session.enter("b = a").unwrap();

        let result = session.edit(a, "renamed = 1");

        assert_eq!(
            result,
            Err(SessionError::NameInUse {
                line: a,
                dependents: vec![b]
            })
        );
    }

    #[test]
    fn edit_of_unknown_line_is_rejected() {
        let mut session = session();

        let result = session.edit(line(9), "1");

        assert_eq!(result, Err(SessionError::UnknownLine(line(9))));
    }

    #[test]
    fn parse_error_is_rejected() {
        let mut session = session();

        let result = session.enter("1 +");

        assert!(matches!(result, Err(SessionError::Parse(_))));
    }

    #[test]
    fn failure_of_a_dependency_fails_the_dependent() {
        let mut session = session();
        session.set_precision(Precision::Exact);
        session.enter("broken = 1/0").unwrap();

        let dependent = session.enter("broken + 1").unwrap();

        assert_eq!(
            (
                error_code(&session, line(1)),
                error_code(&session, dependent)
            ),
            (
                "division_by_zero".to_string(),
                "dependency_failed".to_string()
            )
        );
    }

    #[test]
    fn operators_on_literals_identify_their_number() {
        let mut session = session();
        let id = session.enter("10^134 + 5").unwrap();

        assert!(session.identifies_number(id));
    }

    #[test]
    fn named_literal_expression_identifies_its_number() {
        let mut session = session();
        let id = session.enter("big = 2^521 - 1").unwrap();

        assert!(session.identifies_number(id));
    }

    #[test]
    fn built_in_function_and_constant_do_not_count_as_free_symbols() {
        let mut session = session();
        let id = session.enter("abs(-5) + floor(pi)").unwrap();

        assert!(session.identifies_number(id));
    }

    #[test]
    fn reference_to_a_name_does_not_identify_a_number() {
        let mut session = session();
        session.enter("a = 3").unwrap();
        let id = session.enter("a + 1").unwrap();

        assert!(!session.identifies_number(id));
    }

    #[test]
    fn reference_to_a_line_label_does_not_identify_a_number() {
        let mut session = session();
        session.enter("3").unwrap();
        let id = session.enter("r1 * 2").unwrap();

        assert!(!session.identifies_number(id));
    }

    #[test]
    fn free_name_does_not_identify_a_number() {
        let mut session = session();
        let id = session.enter("x + 1").unwrap();

        assert!(!session.identifies_number(id));
    }

    #[test]
    fn solve_line_does_not_identify_a_number() {
        let mut session = session();
        let id = session
            .enter(r#"solve {"phase": "ways", "object": "circle", "wanted": "area"}"#)
            .unwrap();

        assert!(!session.identifies_number(id));
    }

    #[test]
    fn call_of_a_defined_function_does_not_identify_a_number() {
        let mut session = session();
        session.enter("f(x) = x^2").unwrap();
        let id = session.enter("f(3)").unwrap();

        assert!(!session.identifies_number(id));
    }

    #[test]
    fn exact_algebraic_result_is_kept_under_the_exact_setting() {
        let mut session = session();
        session.set_precision(Precision::Exact);

        let id = session.enter("sqrt(2)").unwrap();

        assert_eq!(
            (record(&session, id).computed().kind(), value(&session, id)),
            (
                ResultKind::Algebraic,
                ResultValue::Expression("sqrt(2)".to_owned())
            )
        );
    }

    #[test]
    fn machine_result_under_the_f32_setting_is_an_f32_value() {
        let mut session = session();
        session.set_precision(Precision::F32);

        let id = session.enter("to_f32(1/3)").unwrap();

        assert_eq!(
            value(&session, id),
            ResultValue::Number(Number::F32(1.0f32 / 3.0f32))
        );
    }

    #[test]
    fn machine_result_under_the_f32_setting_carries_a_rounding_bound() {
        let mut session = session();
        session.set_precision(Precision::F32);

        let id = session.enter("to_f32(1/3)").unwrap();

        assert!(matches!(
            record(&session, id).computed().rounding_error(),
            RoundingError::Bound(_)
        ));
    }

    #[test]
    fn function_definition_has_its_own_outcome_and_is_kept() {
        let mut session = session();

        let id = session.enter("f(x) = x^2").unwrap();

        assert_eq!(
            (
                session.line(id).map(Line::outcome).cloned(),
                session.resolve("f")
            ),
            (Some(Outcome::Defined), Some(id))
        );
    }

    #[test]
    fn set_precision_recomputes_every_line() {
        let mut session = session();
        let id = session.enter("1/3").unwrap();

        session.set_precision(Precision::Exact);

        assert_eq!(value(&session, id), exact(1, 3));
    }

    #[test]
    fn record_time_and_duration_come_from_the_clock() {
        let mut session = session();

        let id = session.enter("2 * 3").unwrap();

        let stored = record(&session, id);
        assert_eq!(
            (
                stored.computed_at().milliseconds_since_unix_epoch(),
                stored.duration()
            ),
            (
                FIXED_TIME_MILLISECONDS,
                Duration::from_nanos(NANOSECONDS_PER_READING)
            )
        );
    }

    #[test]
    fn operator_names_become_snake_case_identifiers() {
        assert_eq!(
            operator_identifier(Operator::RoundTiesEven),
            identifier("round_ties_even")
        );
    }

    #[test]
    fn every_operation_is_named_by_its_call_name() {
        for operator in Operator::ALL {
            let expected = calc_syntax::operator_call_name(operator)
                .map_or_else(|| snake_case(&format!("{operator:?}")), str::to_string);
            assert_eq!(
                operator_identifier(operator),
                identifier(&expected),
                "{operator:?}"
            );
            if let Some(call_name) = calc_syntax::operator_call_name(operator) {
                assert_eq!(operator_name(operator), call_name, "{operator:?}");
            }
        }
    }

    #[test]
    fn an_operator_without_a_call_name_keeps_its_snake_case_name() {
        for (operator, name) in [
            (Operator::Add, "add"),
            (Operator::Neg, "neg"),
            (Operator::Pow, "pow"),
            (Operator::NotEqual, "not_equal"),
            (Operator::RoundTiesEven, "round_ties_even"),
        ] {
            assert_eq!(
                operator_identifier(operator),
                identifier(name),
                "{operator:?}"
            );
        }
    }

    #[test]
    fn the_two_operations_whose_call_name_differs_are_named_by_it() {
        assert_eq!(
            operator_identifier(Operator::CopySign),
            identifier("copysign")
        );
        assert_eq!(operator_identifier(Operator::ToExact), identifier("exact"));
    }

    #[test]
    fn entering_solve_line_text_adds_a_solve_line() {
        let mut session = Session::new(fixed_clock(), crate::registered_backends());

        let id = session
            .enter(r#"solve {"phase": "reachable", "object": "circle", "given": [{"name": "radius", "value": "6", "unit": "cm"}]}"#)
            .unwrap();

        assert!(matches!(
            session.line(id).unwrap().outcome(),
            Outcome::Reachable(_)
        ));
    }

    #[test]
    fn entering_invalid_solve_line_text_is_an_invalid_request() {
        let mut session = Session::new(fixed_clock(), crate::registered_backends());

        let result = session.enter(r#"solve {"phase": "reachable", "wanted": "area"}"#);

        assert!(matches!(result, Err(SessionError::InvalidRequest(_))));
    }

    #[test]
    fn solve_as_a_name_still_enters_a_naming_statement() {
        let mut session = Session::new(fixed_clock(), crate::registered_backends());

        let id = session.enter("solve = 3").unwrap();

        assert!(!session.solve_requests.contains_key(&id));
    }

    fn pi_sixth_value_under(precision: Precision) -> ResultValue {
        let mut session = session();
        session.set_precision(precision);
        let id = session.enter("pi/6").unwrap();
        value(&session, id)
    }

    #[test]
    fn pi_multiple_stays_exact_under_the_exact_setting() {
        assert_eq!(
            pi_sixth_value_under(Precision::Exact),
            ResultValue::Expression("pi / 6".to_string())
        );
    }

    #[test]
    fn pi_multiple_stays_exact_under_the_f64_setting() {
        assert_eq!(
            pi_sixth_value_under(Precision::F64),
            ResultValue::Expression("pi / 6".to_string())
        );
    }

    #[test]
    fn pi_multiple_stays_exact_under_the_f32_setting() {
        assert_eq!(
            pi_sixth_value_under(Precision::F32),
            ResultValue::Expression("pi / 6".to_string())
        );
    }

    #[test]
    fn sine_of_one_keeps_its_expression() {
        let mut session = session();
        let id = session.enter("sin(1)").unwrap();

        assert_eq!(
            (record(&session, id).computed().kind(), value(&session, id)),
            (
                ResultKind::Symbolic,
                ResultValue::Expression("sin(1)".to_owned())
            )
        );
    }

    #[test]
    fn stored_pi_multiple_has_its_enclosure_view() {
        let mut session = session();
        let id = session.enter("pi/6").unwrap();
        let ResultValue::Expression(text) = value(&session, id) else {
            panic!("expected an expression");
        };
        let expression = calc_syntax::parse_expression(&mut session.pool, &text).unwrap();

        let enclosure =
            calc_core::enclose_decimal(&mut session.pool, expression, 20, &|| false).unwrap();

        assert_eq!(
            (enclosure.lower, enclosure.upper, enclosure.reached),
            (
                Number::fraction(
                    &Integer::from(52_359_877_559_829_887_307_i128),
                    &Integer::from(10_i64).pow(20)
                )
                .unwrap(),
                Number::fraction(
                    &Integer::from(52_359_877_559_829_887_308_i128),
                    &Integer::from(10_i64).pow(20)
                )
                .unwrap(),
                true
            )
        );
    }

    #[test]
    fn second_power_of_pi_stays_exact_under_the_f32_setting() {
        let mut session = session();
        session.set_precision(Precision::F32);
        let id = session.enter("pi^2/6").unwrap();

        assert_eq!(
            value(&session, id),
            ResultValue::Expression("pi^2 / 6".to_string())
        );
    }
}
