use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::mpsc::Sender;

use calc_core::{
    DecimalEnclosureError, RationalForm, References, ResultKind, ResultValue, StepOperation,
    StepValue, Working, WorkingError, digits_of_expression, enclose_decimal, estimated_digits_of,
    exact_working, machine_working, rational_form,
};
use calc_exec::{Backend, BackendKind, Domain, Preference};
use calc_expr::{ExprId, ExprPool, Head, NodeView, Operator, SymbolId, SymbolKind};
use calc_numbers::{
    DECIMAL_PLACES_LIMIT, DecimalDigits, DecimalEnclosure, DecimalError, DecimalPeriod,
    EnclosureError, Expansion, Number, SIGNIFICANT_DIGITS_LIMIT, decimal_digits,
};
use calc_syntax::{Construct, PrintMode, operator_construct, parse_expression, print_expression};
use calc_units::{DisplayTarget, scale_spread_for_display};

use crate::display_units::{DisplayUnit, UnitsChoice};
use crate::json::{self, Json};
use crate::platform::{Job, JobState};
use crate::registered_backends;
use crate::result_record::{LineId, ResultRecord};
use crate::session::{Line, Outcome, Session, SessionError};
use crate::session_file::{line_label, number_json};
use crate::summary::value_text_in;
use crate::unit_display::{display_text, input_text};
use crate::working_rules::WorkingRule;
use calc_i18n::Message;

const CONVERSION_MARK: &str = "->";

const EXPANSION_ENDS: &str = "ends";
const EXPANSION_RECURS: &str = "recurs";
const EXPANSION_RECURS_BEYOND_LIMIT: &str = "recurs_beyond_limit";
const EXPANSION_NOT_KNOWN_TO_RECUR: &str = "not_known_to_recur";

const UNCERTAINTY_STATED: &str = "stated";
const UNCERTAINTY_BELOW_THE_PLACE: &str = "below_the_place";
const UNCERTAINTY_NOT_A_SCALAR: &str = "not_a_scalar";

const EXACT_VALUE: &str = "exact_value";
const MACHINE_VALUE: &str = "machine_value";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ViewKind {
    Digits,
    Enclose,
    Working,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResultView {
    Digits { places: u32 },
    Enclose { significant_digits: u32 },
    Working { path: Vec<usize> },
}

impl ResultView {
    fn kind(&self) -> ViewKind {
        match self {
            Self::Digits { .. } => ViewKind::Digits,
            Self::Enclose { .. } => ViewKind::Enclose,
            Self::Working { .. } => ViewKind::Working,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkingOperation {
    Operator(Construct),
    Reference(LineId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkingOperand {
    pub value: String,
    pub path: Option<Vec<usize>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkingStepLine {
    pub path: Vec<usize>,
    pub operation: WorkingOperation,
    pub operands: Vec<WorkingOperand>,
    pub result: String,
    pub format: Option<MachineFormat>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkingRefusal {
    BackendUnavailable(Option<BackendKind>),
    ApproximateOperations,
    ResultDiffers { path: Vec<usize>, shown: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkingOutcome {
    Steps {
        rules: Vec<WorkingRule>,
        steps: Vec<WorkingStepLine>,
        further_steps: usize,
        further_paths: Vec<Vec<usize>>,
    },
    Refused(WorkingRefusal),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineFormat {
    F32,
    F64,
}

impl MachineFormat {
    pub fn name(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }

    fn conversion(self) -> &'static str {
        match self {
            Self::F32 => "to_f32",
            Self::F64 => "to_f64",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DigitsOf {
    ExactValue,
    MachineValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DigitsOutcome {
    pub of: DigitsOf,
    pub digits: DecimalDigits,
    pub unit: Option<String>,
    pub uncertainty: Option<DigitsUncertainty>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnclosureOutcome {
    pub enclosure: DecimalEnclosure,
    pub unit: Option<String>,
    pub of: DigitsOf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewOutcome {
    Digits(DigitsOutcome),
    Enclosure(EnclosureOutcome),
    Working(WorkingOutcome),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewError {
    UnknownLine(LineId),
    LineRefused {
        line: LineId,
        diagnostic: calc_core::Diagnostic,
    },
    NotApplicable {
        line: LineId,
        view: ViewKind,
    },
    HoldsFreeNames {
        line: LineId,
        names: Vec<String>,
    },
    PlacesAboveLimit {
        places: u32,
        limit: u32,
    },
    SignificantDigitsOutOfRange {
        digits: u32,
        limit: u32,
    },
    OverBudget {
        budget_bits: usize,
    },
    Undetermined {
        places: u32,
        digits: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewEvent {
    Finished {
        line: LineId,
        generation: u64,
        outcome: Box<ViewOutcome>,
    },
    Failed {
        line: LineId,
        generation: u64,
        error: ViewError,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AvailableViews {
    pub digits: bool,
    pub enclose: bool,
    pub machine_line: bool,
    pub working: bool,
}

#[derive(Default)]
pub(crate) struct ViewJobs {
    cancellations: HashMap<(LineId, ViewKind), Arc<AtomicBool>>,
    latest_generations: HashMap<(LineId, ViewKind), u64>,
    next_generation: u64,
}

enum ViewWork {
    Digits {
        value: Number,
        of: DigitsOf,
        places: u32,
        unit: Option<String>,
        uncertainty: Option<ShownSpread>,
    },
    RefinedDigits {
        pool: Box<ExprPool>,
        expression: ExprId,
        of: DigitsOf,
        places: u32,
        unit: Option<String>,
        uncertainty: Option<ShownSpread>,
    },
    Enclose {
        pool: Box<ExprPool>,
        expression: ExprId,
        significant_digits: u32,
        unit: Option<String>,
        of: DigitsOf,
    },
    Working(Box<WorkingWork>),
}

struct WorkingWork {
    rules: Vec<WorkingRule>,
    pool: Box<ExprPool>,
    expression: ExprId,
    path: Vec<usize>,
    references: References,
    labels: HashMap<SymbolId, LineId>,
    machine: Option<(Domain, BackendKind)>,
    stored: String,
    form: RationalForm,
    refusal: Option<WorkingRefusal>,
}

pub struct ViewJob {
    work: ViewWork,
    cancellation: Arc<AtomicBool>,
    events: Sender<ViewEvent>,
    line: LineId,
    generation: u64,
}

fn scalar_number(record: &ResultRecord) -> Option<&Number> {
    match record.computed().value() {
        ResultValue::Number(number) => Some(number),
        ResultValue::Complex { .. } | ResultValue::Array { .. } | ResultValue::Expression(_) => {
            None
        }
    }
}

struct ShownUnit {
    unit: Option<String>,
    number: Option<Number>,
    conversion: Option<String>,
    uncertainty: Option<ShownSpread>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ShownSpread {
    Standard(Number),
    NotAScalar,
}

fn digits_uncertainty(spread: Option<&ShownSpread>, places: u32) -> Option<DigitsUncertainty> {
    match spread? {
        ShownSpread::NotAScalar => Some(DigitsUncertainty::NotAScalar),
        ShownSpread::Standard(standard) => match decimal_digits(standard, places) {
            Ok(shown) if is_zero(&shown.digits) && !shown.terminates => {
                Some(DigitsUncertainty::BelowThePlace)
            }
            Ok(shown) => Some(DigitsUncertainty::Standard {
                standard: shown.digits,
                terminates: shown.terminates,
            }),
            Err(_) => Some(DigitsUncertainty::NotAScalar),
        },
    }
}

fn budget_bits_as_u64(budget_bits: usize) -> u64 {
    u64::try_from(budget_bits).unwrap_or(u64::MAX)
}

fn is_zero(number: &Number) -> bool {
    matches!(number, Number::Integer(value) if value.is_zero())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DigitsUncertainty {
    Standard { standard: Number, terminates: bool },
    BelowThePlace,
    NotAScalar,
}

fn encloses_a_machine_value(pool: &ExprPool, expression: ExprId) -> bool {
    match pool.node(expression) {
        Ok(NodeView::Number(number)) => matches!(
            pool.number_value(number),
            Ok(Number::F32(_) | Number::F64(_))
        ),
        Ok(NodeView::Apply {
            head: Head::Operator(Operator::ToF32 | Operator::ToF64 | Operator::ToExact),
            ..
        }) => true,
        _ => false,
    }
}

fn digits_of(record: &ResultRecord) -> Option<(Number, DigitsOf)> {
    let number = scalar_number(record)?;
    let of = match (record.computed().kind(), number) {
        (ResultKind::ExactRational, Number::Integer(_) | Number::Rational(_)) => {
            DigitsOf::ExactValue
        }
        (ResultKind::MachineFloat, Number::F64(value)) if value.is_finite() => {
            DigitsOf::MachineValue
        }
        (ResultKind::MachineFloat, Number::F32(value)) if value.is_finite() => {
            DigitsOf::MachineValue
        }
        _ => return None,
    };
    Some((number.clone(), of))
}

fn is_real_scalar(record: &ResultRecord) -> bool {
    match record.computed().value() {
        ResultValue::Number(number) => is_finite_number(number),
        ResultValue::Expression(_) => true,
        ResultValue::Complex { .. } | ResultValue::Array { .. } => false,
    }
}

fn is_finite_number(number: &Number) -> bool {
    match number {
        Number::F32(value) => value.is_finite(),
        Number::F64(value) => value.is_finite(),
        Number::Integer(_) | Number::Rational(_) => true,
    }
}

fn is_enclosable(record: &ResultRecord) -> bool {
    let computed = record.computed();
    is_real_scalar(record) && computed.uncertainty().is_none() && computed.seed().is_none()
}

fn machine_domain(record: &ResultRecord) -> Option<Domain> {
    match (record.computed().kind(), record.computed().value()) {
        (ResultKind::MachineFloat, ResultValue::Number(Number::F64(_))) => Some(Domain::F64),
        (ResultKind::MachineFloat, ResultValue::Number(Number::F32(_))) => Some(Domain::F32),
        _ => None,
    }
}

fn working_refusal(
    record: &ResultRecord,
    machine: Option<(Domain, Option<BackendKind>)>,
) -> Option<WorkingRefusal> {
    let (_, kind) = machine?;
    if record
        .backend()
        .iter()
        .any(|used| used.approximate_operations)
    {
        return Some(WorkingRefusal::ApproximateOperations);
    }
    kind.is_none()
        .then_some(WorkingRefusal::BackendUnavailable(None))
}

fn refused(refusal: WorkingRefusal) -> WorkingOutcome {
    WorkingOutcome::Refused(refusal)
}

#[derive(Clone, Copy)]
struct LineShape {
    root: ExprId,
    form: RationalForm,
}

fn step_unit_at(pool: &mut ExprPool, root: ExprId, path: &[usize]) -> Option<String> {
    let node = calc_core::node_at(pool, root, path).ok()?;
    let dimension = calc_core::to_coherent_units(pool, node).ok()?.dimension;
    let coherent = pool.units_mut().coherent_unit(&dimension).ok()?;
    display_text(pool, coherent)
}

fn step_text(
    pool: &mut ExprPool,
    shape: LineShape,
    path: &[usize],
    unit_path: Option<&[usize]>,
    value: &StepValue,
) -> Option<String> {
    match value {
        StepValue::Number(number) => {
            let form = if path.is_empty() {
                shape.form
            } else {
                match calc_core::node_at(pool, shape.root, path) {
                    Ok(node) => rational_form(pool, node, None),
                    Err(_) => RationalForm::Fraction,
                }
            };
            let text = value_text_in(&ResultValue::Number(number.clone()), form);
            match unit_path.and_then(|path| step_unit_at(pool, shape.root, path)) {
                Some(unit) => Some(format!("{text} {unit}")),
                None => Some(text),
            }
        }
        StepValue::Expression(expression) => {
            print_expression(pool, *expression, PrintMode::Ascii).ok()
        }
    }
}

fn step_line(
    pool: &mut ExprPool,
    shape: LineShape,
    step: &calc_core::WorkingStep,
    labels: &HashMap<SymbolId, LineId>,
    format: Option<MachineFormat>,
) -> Option<WorkingStepLine> {
    let operation = match step.operation {
        StepOperation::Operator(operator) => {
            WorkingOperation::Operator(operator_construct(operator))
        }
        StepOperation::Reference(symbol) => WorkingOperation::Reference(*labels.get(&symbol)?),
    };
    let mut operands = Vec::new();
    for (index, operand) in step.operands.iter().enumerate() {
        let child = matches!(step.operation, StepOperation::Operator(_)).then(|| {
            let mut path = step.path.clone();
            path.push(index);
            path
        });
        let unit_path = operand.path.as_deref().or(child.as_deref());
        operands.push(WorkingOperand {
            value: step_text(
                pool,
                shape,
                operand.path.as_deref().unwrap_or(&step.path),
                unit_path,
                &operand.value,
            )?,
            path: operand.path.clone(),
        });
    }
    Some(WorkingStepLine {
        path: step.path.clone(),
        operation,
        operands,
        result: step_text(pool, shape, &step.path, Some(&step.path), &step.result)?,
        format,
    })
}

fn steps_outcome(
    pool: &mut ExprPool,
    shape: LineShape,
    working: &Working,
    labels: &HashMap<SymbolId, LineId>,
    machine: Option<(Domain, BackendKind)>,
    stored: &str,
    rules: &[WorkingRule],
) -> Option<WorkingOutcome> {
    let format = machine.map(|(domain, _)| match domain {
        Domain::F32 => MachineFormat::F32,
        Domain::F64 => MachineFormat::F64,
    });
    let mut steps = Vec::new();
    for step in &working.steps {
        steps.push(step_line(pool, shape, step, labels, format)?);
    }
    let stored = match step_unit_at(pool, shape.root, &[]) {
        Some(unit) => format!("{stored} {unit}"),
        None => stored.to_owned(),
    };
    let last = steps.last()?;
    if rules.is_empty() && last.path.is_empty() && last.result != stored {
        return Some(refused(WorkingRefusal::ResultDiffers {
            path: last.path.clone(),
            shown: last.result.clone(),
        }));
    }
    Some(WorkingOutcome::Steps {
        rules: rules.to_vec(),
        steps,
        further_steps: working.further_steps,
        further_paths: working.further_paths.clone(),
    })
}

fn computed_working(work: &mut WorkingWork, line: LineId) -> Result<WorkingOutcome, ViewError> {
    let WorkingWork {
        rules,
        pool,
        expression,
        path,
        references,
        labels,
        machine,
        stored,
        form,
        ..
    } = work;
    let (expression, machine) = (*expression, *machine);
    let path = path.as_slice();
    let not_applicable = ViewError::NotApplicable {
        line,
        view: ViewKind::Working,
    };
    let backends = registered_backends();
    let chosen: Vec<&dyn Backend> = match machine {
        None => Vec::new(),
        Some((_, kind)) => backends
            .iter()
            .map(AsRef::as_ref)
            .filter(|backend| backend.kind() == kind)
            .collect(),
    };
    if machine.is_some() && chosen.is_empty() {
        return Ok(refused(WorkingRefusal::BackendUnavailable(
            machine.map(|(_, kind)| kind),
        )));
    }
    let computed = match machine {
        None => exact_working(pool, expression, path, references),
        Some((domain, _)) => machine_working(
            pool,
            expression,
            path,
            references,
            domain,
            &chosen,
            Preference::Automatic,
        ),
    };
    match computed {
        Ok(working) => steps_outcome(
            pool,
            LineShape {
                root: expression,
                form: *form,
            },
            &working,
            labels,
            machine,
            stored,
            rules,
        )
        .ok_or(not_applicable),
        Err(WorkingError::ApproximateOperations) => {
            Ok(refused(WorkingRefusal::ApproximateOperations))
        }
        Err(_) => Err(not_applicable),
    }
}

impl Job for ViewJob {
    fn step(&mut self) -> JobState {
        if self.cancellation.load(AtomicOrdering::SeqCst) {
            return JobState::Finished;
        }
        let result =
            match &mut self.work {
                ViewWork::Digits {
                    value,
                    of,
                    places,
                    unit,
                    uncertainty,
                } => match decimal_digits(value, *places) {
                    Ok(digits) => Ok(ViewOutcome::Digits(DigitsOutcome {
                        of: *of,
                        digits,
                        unit: unit.clone(),
                        uncertainty: digits_uncertainty(uncertainty.as_ref(), *places),
                    })),
                    Err(DecimalError::PlacesAboveLimit { places, limit }) => {
                        Err(ViewError::PlacesAboveLimit { places, limit })
                    }
                    Err(DecimalError::NotFinite) => Err(ViewError::NotApplicable {
                        line: self.line,
                        view: ViewKind::Digits,
                    }),
                },
                ViewWork::Enclose {
                    pool,
                    expression,
                    significant_digits,
                    unit,
                    of,
                } => {
                    let cancellation = Arc::clone(&self.cancellation);
                    let is_cancelled = move || cancellation.load(AtomicOrdering::SeqCst);
                    match enclose_decimal(pool, *expression, *significant_digits, &is_cancelled) {
                        Ok(enclosure) => Ok(ViewOutcome::Enclosure(EnclosureOutcome {
                            enclosure,
                            unit: unit.clone(),
                            of: *of,
                        })),
                        Err(DecimalEnclosureError::Enclosure(EnclosureError::Cancelled)) => {
                            return JobState::Finished;
                        }
                        Err(DecimalEnclosureError::Enclosure(
                            EnclosureError::DigitsOutOfRange { digits, limit },
                        )) => Err(ViewError::SignificantDigitsOutOfRange { digits, limit }),
                        Err(DecimalEnclosureError::Enclosure(
                            EnclosureError::OverBudget { budget_bits }
                            | EnclosureError::NoEnclosure { budget_bits },
                        )) => Err(ViewError::OverBudget { budget_bits }),
                        Err(
                            DecimalEnclosureError::Access(_)
                            | DecimalEnclosureError::Quantity(_)
                            | DecimalEnclosureError::NotApplicable { .. }
                            | DecimalEnclosureError::Enclosure(EnclosureError::NotFinite),
                        ) => Err(ViewError::NotApplicable {
                            line: self.line,
                            view: ViewKind::Enclose,
                        }),
                    }
                }
                ViewWork::RefinedDigits {
                    pool,
                    expression,
                    of,
                    places,
                    unit,
                    uncertainty,
                } => {
                    let cancellation = Arc::clone(&self.cancellation);
                    let is_cancelled = move || cancellation.load(AtomicOrdering::SeqCst);
                    match digits_of_expression(pool, *expression, *places, &is_cancelled) {
                        Ok(digits) => Ok(ViewOutcome::Digits(DigitsOutcome {
                            of: *of,
                            digits,
                            unit: unit.clone(),
                            uncertainty: digits_uncertainty(uncertainty.as_ref(), *places),
                        })),
                        Err(DecimalEnclosureError::Enclosure(EnclosureError::Cancelled)) => {
                            return JobState::Finished;
                        }
                        Err(DecimalEnclosureError::Enclosure(
                            EnclosureError::DigitsOutOfRange { digits, limit },
                        )) => Err(ViewError::SignificantDigitsOutOfRange { digits, limit }),
                        Err(DecimalEnclosureError::Enclosure(
                            EnclosureError::OverBudget { budget_bits }
                            | EnclosureError::NoEnclosure { budget_bits },
                        )) => Err(ViewError::Undetermined {
                            places: *places,
                            digits: estimated_digits_of(budget_bits_as_u64(budget_bits)),
                        }),
                        Err(
                            DecimalEnclosureError::Access(_)
                            | DecimalEnclosureError::Quantity(_)
                            | DecimalEnclosureError::NotApplicable { .. }
                            | DecimalEnclosureError::Enclosure(EnclosureError::NotFinite),
                        ) => Err(ViewError::NotApplicable {
                            line: self.line,
                            view: ViewKind::Digits,
                        }),
                    }
                }
                ViewWork::Working(work) => match &work.refusal {
                    Some(refusal) => Ok(ViewOutcome::Working(WorkingOutcome::Refused(
                        refusal.clone(),
                    ))),
                    None => computed_working(work, self.line).map(ViewOutcome::Working),
                },
            };
        if self.cancellation.load(AtomicOrdering::SeqCst) {
            return JobState::Finished;
        }
        let event = match result {
            Ok(outcome) => ViewEvent::Finished {
                line: self.line,
                generation: self.generation,
                outcome: Box::new(outcome),
            },
            Err(error) => ViewEvent::Failed {
                line: self.line,
                generation: self.generation,
                error,
            },
        };
        self.events.send(event).ok();
        JobState::Finished
    }
}

pub fn working_operation_message(operation: &WorkingOperation) -> Message {
    match operation {
        WorkingOperation::Operator(construct) => match crate::construct_words(construct) {
            Some(words) => words.name,
            None => Message::CommonWorkingSteps,
        },
        WorkingOperation::Reference(line) => Message::CommonWorkingStepReference {
            line: line_label(*line),
        },
    }
}

pub fn working_refusal_message(refusal: &WorkingRefusal) -> Message {
    match refusal {
        WorkingRefusal::BackendUnavailable(None) => Message::CommonWorkingBackendUnavailable,
        WorkingRefusal::BackendUnavailable(Some(kind)) => {
            Message::CommonWorkingBackendUnavailableNamed {
                backend: machine_backend_name(*kind).to_owned(),
            }
        }
        WorkingRefusal::ApproximateOperations => Message::CommonWorkingApproximateOperations,
        WorkingRefusal::ResultDiffers { shown, .. } => Message::CommonWorkingResultDiffers {
            shown: shown.clone(),
        },
    }
}

fn machine_backend_name(kind: BackendKind) -> &'static str {
    match kind {
        BackendKind::Cpu => "cpu",
        BackendKind::Simd => "simd",
        BackendKind::Gpu => "gpu",
    }
}

fn path_json(path: &[usize]) -> Json {
    Json::Array(
        path.iter()
            .map(|index| Json::Count(u64::try_from(*index).unwrap_or(u64::MAX)))
            .collect(),
    )
}

fn working_json(outcome: &WorkingOutcome) -> Json {
    match outcome {
        WorkingOutcome::Refused(refusal) => {
            let (cause, named) = match refusal {
                WorkingRefusal::BackendUnavailable(kind) => (
                    "backend_unavailable",
                    Json::optional(kind.map(|kind| Json::string(machine_backend_name(kind)))),
                ),
                WorkingRefusal::ApproximateOperations => ("approximate_operations", Json::Null),
                WorkingRefusal::ResultDiffers { path, shown } => (
                    "result_differs",
                    Json::object(vec![
                        ("path", path_json(path)),
                        ("shown", Json::string(shown)),
                    ]),
                ),
            };
            Json::object(vec![
                ("steps", Json::Array(Vec::new())),
                ("refused", Json::string(cause)),
                ("detail", named),
            ])
        }
        WorkingOutcome::Steps {
            rules,
            steps,
            further_steps,
            further_paths,
        } => Json::object(vec![
            ("rules", Json::Array(rules.iter().map(rule_json).collect())),
            ("steps", Json::Array(steps.iter().map(step_json).collect())),
            (
                "further_steps",
                Json::Count(u64::try_from(*further_steps).unwrap_or(u64::MAX)),
            ),
            (
                "further_paths",
                Json::Array(further_paths.iter().map(|path| path_json(path)).collect()),
            ),
        ]),
    }
}

fn rule_json(rule: &WorkingRule) -> Json {
    Json::object(vec![
        ("rule", Json::string(&rule.rule)),
        ("output", Json::string(&rule.output)),
        (
            "inputs",
            Json::Array(
                rule.inputs
                    .iter()
                    .map(|input| Json::string(input))
                    .collect(),
            ),
        ),
        (
            "values",
            Json::Array(
                rule.values
                    .iter()
                    .map(|(role, value)| {
                        Json::object(vec![
                            ("role", Json::string(role)),
                            ("value", Json::string(value)),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("result", Json::string(&rule.result)),
        (
            "conditions",
            Json::Array(
                rule.conditions
                    .iter()
                    .map(|condition| {
                        Json::object(vec![
                            ("text", Json::string(&condition.text)),
                            ("holds", Json::optional(condition.holds.map(Json::Boolean))),
                            (
                                "could_have_failed",
                                Json::Boolean(condition.could_have_failed),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "sources",
            Json::Array(
                rule.sources
                    .iter()
                    .map(|source| Json::string(source))
                    .collect(),
            ),
        ),
        (
            "corpus_references",
            Json::Array(
                rule.corpus_references
                    .iter()
                    .map(|reference| Json::string(reference))
                    .collect(),
            ),
        ),
    ])
}

fn step_json(step: &WorkingStepLine) -> Json {
    let operation = match &step.operation {
        WorkingOperation::Operator(construct) => Json::object(vec![
            ("kind", Json::string(construct.kind.name())),
            ("name", Json::string(construct.name)),
        ]),
        WorkingOperation::Reference(line) => Json::object(vec![
            ("kind", Json::string("reference")),
            ("name", Json::string(&line_label(*line))),
        ]),
    };
    Json::object(vec![
        ("path", path_json(&step.path)),
        ("operation", operation),
        (
            "operands",
            Json::Array(
                step.operands
                    .iter()
                    .map(|operand| {
                        Json::object(vec![
                            ("value", Json::string(&operand.value)),
                            (
                                "path",
                                Json::optional(operand.path.as_deref().map(path_json)),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("result", Json::string(&step.result)),
        (
            "format",
            Json::optional(step.format.map(|format| Json::string(format.name()))),
        ),
    ])
}

fn expansion_json(expansion: Expansion) -> Json {
    let period = |period: DecimalPeriod| {
        Json::object(vec![
            ("start", Json::Count(u64::from(period.start))),
            ("length", Json::Count(u64::from(period.length))),
        ])
    };
    match expansion {
        Expansion::Ends => Json::object(vec![
            ("state", Json::string(EXPANSION_ENDS)),
            ("period", Json::Null),
        ]),
        Expansion::Recurs(found) => Json::object(vec![
            ("state", Json::string(EXPANSION_RECURS)),
            ("period", period(found)),
        ]),
        Expansion::RecursBeyondLimit => Json::object(vec![
            ("state", Json::string(EXPANSION_RECURS_BEYOND_LIMIT)),
            ("period", Json::Null),
        ]),
        Expansion::NotKnownToRecur => Json::object(vec![
            ("state", Json::string(EXPANSION_NOT_KNOWN_TO_RECUR)),
            ("period", Json::Null),
        ]),
    }
}

fn uncertainty_json(uncertainty: &DigitsUncertainty) -> Json {
    match uncertainty {
        DigitsUncertainty::Standard {
            standard,
            terminates,
        } => Json::object(vec![
            ("state", Json::string(UNCERTAINTY_STATED)),
            ("standard", number_json(standard)),
            ("terminates", Json::Boolean(*terminates)),
        ]),
        DigitsUncertainty::BelowThePlace => Json::object(vec![
            ("state", Json::string(UNCERTAINTY_BELOW_THE_PLACE)),
            ("standard", Json::Null),
            ("terminates", Json::Null),
        ]),
        DigitsUncertainty::NotAScalar => Json::object(vec![
            ("state", Json::string(UNCERTAINTY_NOT_A_SCALAR)),
            ("standard", Json::Null),
            ("terminates", Json::Null),
        ]),
    }
}

pub fn view_outcome_json(outcome: &ViewOutcome) -> Vec<u8> {
    json::write_canonical(&view_outcome_value(outcome)).into_bytes()
}

pub fn view_outcome_json_line(outcome: &ViewOutcome) -> Vec<u8> {
    json::write_one_line(&view_outcome_value(outcome)).into_bytes()
}

fn view_outcome_value(outcome: &ViewOutcome) -> Json {
    let unit_json = |unit: &Option<String>| Json::optional(unit.as_deref().map(Json::string));
    match outcome {
        ViewOutcome::Working(outcome) => working_json(outcome),
        ViewOutcome::Digits(outcome) => {
            let digits = &outcome.digits;
            Json::object(vec![
                ("places", Json::Count(u64::from(digits.places))),
                ("digits", number_json(&digits.digits)),
                ("terminates", Json::Boolean(digits.terminates)),
                (
                    "remainder",
                    Json::optional(digits.remainder.as_ref().map(number_json)),
                ),
                ("expansion", expansion_json(digits.expansion)),
                (
                    "of",
                    Json::string(match outcome.of {
                        DigitsOf::ExactValue => EXACT_VALUE,
                        DigitsOf::MachineValue => MACHINE_VALUE,
                    }),
                ),
                ("unit", unit_json(&outcome.unit)),
                (
                    "uncertainty",
                    Json::optional(outcome.uncertainty.as_ref().map(uncertainty_json)),
                ),
            ])
        }
        ViewOutcome::Enclosure(outcome) => {
            let enclosure = &outcome.enclosure;
            Json::object(vec![
                (
                    "significant_digits",
                    Json::Count(u64::from(enclosure.significant_digits)),
                ),
                ("lower", number_json(&enclosure.lower)),
                ("upper", number_json(&enclosure.upper)),
                ("width", number_json(&enclosure.width)),
                ("reached", Json::Boolean(enclosure.reached)),
                ("unit", unit_json(&outcome.unit)),
                (
                    "of",
                    Json::string(match outcome.of {
                        DigitsOf::ExactValue => EXACT_VALUE,
                        DigitsOf::MachineValue => MACHINE_VALUE,
                    }),
                ),
            ])
        }
    }
}

impl Session {
    fn result_record(&self, id: LineId) -> Option<&ResultRecord> {
        match self.line(id)?.outcome() {
            Outcome::Result(record) => Some(record),
            _ => None,
        }
    }

    fn has_rule_chain(&self, id: LineId) -> bool {
        matches!(
            self.line(id).map(Line::outcome),
            Some(Outcome::Answer(answer))
                if crate::working_rules::evaluated_way(answer)
                    .is_some_and(|way| !way.derivation.is_empty())
        )
    }

    fn line_has_operation(&self, id: LineId) -> bool {
        let Some(root) = self.definition_value(id) else {
            return false;
        };
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            match self.pool().node(node) {
                Ok(NodeView::Apply { head, arguments }) => {
                    if matches!(head, Head::Operator(_)) {
                        return true;
                    }
                    pending.extend(arguments.iter().copied());
                }
                Ok(NodeView::Quantity { value, .. }) => pending.push(value),
                _ => {}
            }
        }
        false
    }

    fn free_names_of_line(&self, id: LineId) -> Option<Vec<String>> {
        crate::session::free_names_in(self.result_record(id)?.computed())
    }

    pub fn available_views(&self, id: LineId) -> AvailableViews {
        if self.free_names_of_line(id).is_some() {
            return AvailableViews {
                digits: false,
                enclose: false,
                machine_line: false,
                working: false,
            };
        }
        let record = self.result_record(id);
        let is_expression_line = self.is_expression_line(id);
        AvailableViews {
            digits: record.and_then(digits_of).is_some()
                || (is_expression_line && record.is_some_and(is_enclosable)),
            enclose: is_expression_line && record.is_some_and(is_enclosable),
            machine_line: is_expression_line && record.is_some(),
            working: (is_expression_line && record.is_some() && self.line_has_operation(id))
                || self.has_rule_chain(id),
        }
    }

    fn shown_unit(
        &mut self,
        id: LineId,
        record: Option<&ResultRecord>,
        preference: Option<&UnitsChoice>,
        coherent_only: bool,
    ) -> ShownUnit {
        let stored = record
            .and_then(|record| record.computed().unit())
            .and_then(|unit| input_text(self.pool(), unit));
        let spread = record
            .and_then(|record| record.computed().uncertainty())
            .map(|uncertainty| match uncertainty.standard() {
                ResultValue::Number(number) => ShownSpread::Standard(number.clone()),
                _ => ShownSpread::NotAScalar,
            });
        let as_stored = |unit: Option<String>, spread: Option<ShownSpread>| ShownUnit {
            unit,
            number: None,
            conversion: None,
            uncertainty: spread,
        };
        let Some((decision, displayed)) = self
            .displayed_line(id, preference, None, coherent_only)
            .and_then(|(_, display)| display)
        else {
            return as_stored(stored, spread);
        };
        let Some(number) = displayed.number else {
            return as_stored(stored, spread);
        };
        match decision.unit {
            Some(DisplayUnit::Unit(unit)) => {
                let target = input_text(self.pool(), unit);
                ShownUnit {
                    unit: target.clone(),
                    number: Some(number),
                    conversion: target,
                    uncertainty: self.shown_spread(record, spread, DisplayTarget::Unit(unit)),
                }
            }
            _ => as_stored(stored, spread),
        }
    }

    fn view_pool(
        &mut self,
        id: LineId,
        conversion: Option<&str>,
    ) -> Result<(Box<ExprPool>, ExprId), ViewError> {
        let not_applicable = ViewError::NotApplicable {
            line: id,
            view: ViewKind::Enclose,
        };
        let expression = self
            .view_expression(id)
            .ok_or_else(|| not_applicable.clone())?;
        let text = print_expression(self.pool(), expression, PrintMode::Ascii)
            .map_err(|_| not_applicable.clone())?;
        let text = match conversion {
            Some(target) => format!("({text}) {CONVERSION_MARK} {target}"),
            None => text,
        };
        let mut pool = ExprPool::new();
        let expression = parse_expression(&mut pool, &text).map_err(|_| not_applicable)?;
        Ok((Box::new(pool), expression))
    }

    fn shown_spread(
        &self,
        record: Option<&ResultRecord>,
        spread: Option<ShownSpread>,
        target: DisplayTarget,
    ) -> Option<ShownSpread> {
        let coherent = record
            .and_then(|record| record.computed().unit())
            .unwrap_or_else(|| self.pool().units().dimensionless());
        match spread? {
            ShownSpread::Standard(standard) => {
                match scale_spread_for_display(self.pool().units(), &standard, coherent, target) {
                    Ok(scaled) => Some(ShownSpread::Standard(scaled)),
                    Err(_) => Some(ShownSpread::NotAScalar),
                }
            }
            ShownSpread::NotAScalar => Some(ShownSpread::NotAScalar),
        }
    }

    pub fn view(
        &mut self,
        id: LineId,
        view: ResultView,
        preference: Option<&UnitsChoice>,
        coherent_only: bool,
        events: Sender<ViewEvent>,
    ) -> Result<ViewJob, ViewError> {
        if self.line(id).is_none() {
            return Err(ViewError::UnknownLine(id));
        }
        if let Some(names) = self.free_names_of_line(id) {
            return Err(ViewError::HoldsFreeNames { line: id, names });
        }
        if let Some(crate::session::Outcome::Error(diagnostic)) =
            self.line(id).map(|line| line.outcome())
        {
            return Err(ViewError::LineRefused {
                line: id,
                diagnostic: diagnostic.clone(),
            });
        }
        let not_applicable = ViewError::NotApplicable {
            line: id,
            view: view.kind(),
        };
        let available = self.available_views(id);
        let record = self.result_record(id).cloned();
        let shown = self.shown_unit(id, record.as_ref(), preference, coherent_only);
        let unit = shown.unit.clone();
        let kind = view.kind();
        let work = match view {
            ResultView::Digits { places } => match record.as_ref().and_then(digits_of) {
                Some((value, of)) => {
                    if places > DECIMAL_PLACES_LIMIT {
                        return Err(ViewError::PlacesAboveLimit {
                            places,
                            limit: DECIMAL_PLACES_LIMIT,
                        });
                    }
                    ViewWork::Digits {
                        value: shown.number.unwrap_or(value),
                        of,
                        places,
                        unit,
                        uncertainty: shown.uncertainty,
                    }
                }
                None => {
                    if !available.digits {
                        return Err(not_applicable);
                    }
                    if places > SIGNIFICANT_DIGITS_LIMIT {
                        return Err(ViewError::SignificantDigitsOutOfRange {
                            digits: places,
                            limit: SIGNIFICANT_DIGITS_LIMIT,
                        });
                    }
                    let record = record.as_ref().ok_or_else(|| not_applicable.clone())?;
                    let of = match record.computed().kind() {
                        ResultKind::MachineFloat => DigitsOf::MachineValue,
                        _ => DigitsOf::ExactValue,
                    };
                    let (pool, expression) = self.view_pool(id, shown.conversion.as_deref())?;
                    ViewWork::RefinedDigits {
                        pool,
                        expression,
                        of,
                        places,
                        unit,
                        uncertainty: shown.uncertainty,
                    }
                }
            },
            ResultView::Enclose { significant_digits } => {
                if significant_digits == 0 || significant_digits > SIGNIFICANT_DIGITS_LIMIT {
                    return Err(ViewError::SignificantDigitsOutOfRange {
                        digits: significant_digits,
                        limit: SIGNIFICANT_DIGITS_LIMIT,
                    });
                }
                if !available.enclose {
                    return Err(not_applicable);
                }
                let (pool, expression) = self.view_pool(id, shown.conversion.as_deref())?;
                let of = if encloses_a_machine_value(&pool, expression) {
                    DigitsOf::MachineValue
                } else {
                    DigitsOf::ExactValue
                };
                ViewWork::Enclose {
                    pool,
                    expression,
                    significant_digits,
                    unit,
                    of,
                }
            }
            ResultView::Working { path } => {
                if !available.working {
                    return Err(not_applicable);
                }
                if let Some(working) = self.solve_working(id) {
                    let text = print_expression(self.pool(), working.expression, PrintMode::Ascii)
                        .map_err(|_| not_applicable.clone())?;
                    let mut pool = ExprPool::new();
                    let expression =
                        parse_expression(&mut pool, &text).map_err(|_| not_applicable.clone())?;
                    return self.started_working(
                        id,
                        kind,
                        WorkingWork {
                            rules: working.rules,
                            pool: Box::new(pool),
                            expression,
                            path,
                            references: References::new(),
                            labels: HashMap::new(),
                            machine: None,
                            stored: String::new(),
                            form: RationalForm::Fraction,
                            refusal: None,
                        },
                        events,
                    );
                }
                let record = record.as_ref().ok_or_else(|| not_applicable.clone())?;
                let own = self
                    .definition_value(id)
                    .ok_or_else(|| not_applicable.clone())?;
                let text = print_expression(self.pool(), own, PrintMode::Ascii)
                    .map_err(|_| not_applicable.clone())?;
                let mut pool = ExprPool::new();
                let expression =
                    parse_expression(&mut pool, &text).map_err(|_| not_applicable.clone())?;
                let mut references = References::new();
                let mut labels = HashMap::new();
                for dependency in record.dependencies() {
                    let name = self
                        .line(*dependency)
                        .and_then(|line| line.name().map(str::to_string))
                        .unwrap_or_else(|| line_label(*dependency));
                    let Some(ResultValue::Number(number)) = self
                        .result_record(*dependency)
                        .map(|record| record.computed().value())
                    else {
                        continue;
                    };
                    let number = number.clone();
                    let symbol = pool
                        .intern_symbol(&name, SymbolKind::Variable)
                        .map_err(|_| not_applicable.clone())?;
                    references = references.with(symbol, number);
                    labels.insert(symbol, *dependency);
                }
                let machine = machine_domain(record).map(|domain| {
                    (
                        domain,
                        record.backend().first().and_then(|used| used.selected),
                    )
                });
                let refusal = working_refusal(record, machine);
                ViewWork::Working(Box::new(WorkingWork {
                    rules: Vec::new(),
                    pool: Box::new(pool),
                    expression,
                    path,
                    references,
                    labels,
                    machine: machine.and_then(|(domain, kind)| kind.map(|kind| (domain, kind))),
                    stored: value_text_in(record.computed().value(), self.rational_form_of(id)),
                    form: self.rational_form_of(id),
                    refusal,
                }))
            }
        };
        let cancellation = Arc::new(AtomicBool::new(false));
        let jobs = self.view_jobs_mut();
        if let Some(previous) = jobs
            .cancellations
            .insert((id, kind), Arc::clone(&cancellation))
        {
            previous.store(true, AtomicOrdering::SeqCst);
        }
        let generation = jobs.next_generation;
        jobs.next_generation = jobs.next_generation.saturating_add(1);
        jobs.latest_generations.insert((id, kind), generation);
        Ok(ViewJob {
            work,
            cancellation,
            events,
            line: id,
            generation,
        })
    }

    fn started_working(
        &mut self,
        id: LineId,
        kind: ViewKind,
        work: WorkingWork,
        events: Sender<ViewEvent>,
    ) -> Result<ViewJob, ViewError> {
        let cancellation = Arc::new(AtomicBool::new(false));
        let jobs = self.view_jobs_mut();
        if let Some(previous) = jobs
            .cancellations
            .insert((id, kind), Arc::clone(&cancellation))
        {
            previous.store(true, AtomicOrdering::SeqCst);
        }
        let generation = jobs.next_generation;
        jobs.next_generation = jobs.next_generation.saturating_add(1);
        jobs.latest_generations.insert((id, kind), generation);
        Ok(ViewJob {
            work: ViewWork::Working(Box::new(work)),
            cancellation,
            events,
            line: id,
            generation,
        })
    }

    pub fn latest_view_generation(&self, id: LineId, kind: ViewKind) -> Option<u64> {
        self.view_jobs()
            .latest_generations
            .get(&(id, kind))
            .copied()
    }

    pub fn cancel_view(&mut self, id: LineId, kind: ViewKind) {
        let jobs = self.view_jobs_mut();
        if let Some(cancellation) = jobs.cancellations.remove(&(id, kind)) {
            cancellation.store(true, AtomicOrdering::SeqCst);
        }
        jobs.latest_generations.remove(&(id, kind));
    }

    pub fn machine_line(
        &mut self,
        id: LineId,
        format: MachineFormat,
    ) -> Result<LineId, SessionError> {
        let line = self.line(id).ok_or(SessionError::UnknownLine(id))?;
        if let Some(names) = self.free_names_of_line(id) {
            return Err(SessionError::HoldsFreeNames { line: id, names });
        }
        if !self.available_views(id).machine_line {
            return Err(SessionError::MachineLineNotApplicable(id));
        }
        let reference = line.name().map_or_else(|| line_label(id), str::to_owned);
        self.enter(&format!("{}({reference})", format.conversion()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Precision;
    use crate::session::tests::session;
    use calc_numbers::{DecimalPeriod, Integer};
    use std::sync::mpsc::{Receiver, channel};

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    fn run(session: &mut Session, id: LineId, view: ResultView) -> ViewEvent {
        run_with(session, id, view, None, false)
    }

    fn run_with(
        session: &mut Session,
        id: LineId,
        view: ResultView,
        preference: Option<&UnitsChoice>,
        coherent_only: bool,
    ) -> ViewEvent {
        let (sender, receiver): (Sender<ViewEvent>, Receiver<ViewEvent>) = channel();
        let mut job = session
            .view(id, view, preference, coherent_only, sender)
            .unwrap();
        assert_eq!(job.step(), JobState::Finished);
        receiver.try_recv().unwrap()
    }

    fn finished(event: ViewEvent) -> ViewOutcome {
        match event {
            ViewEvent::Finished { outcome, .. } => *outcome,
            ViewEvent::Failed { error, .. } => panic!("expected an outcome, found {error:?}"),
        }
    }

    fn digits(session: &mut Session, id: LineId, places: u32) -> DigitsOutcome {
        match finished(run(session, id, ResultView::Digits { places })) {
            ViewOutcome::Digits(outcome) => outcome,
            other => panic!("expected digits, found {other:?}"),
        }
    }

    fn working(session: &mut Session, id: LineId, path: &[usize]) -> WorkingOutcome {
        match finished(run(
            session,
            id,
            ResultView::Working {
                path: path.to_vec(),
            },
        )) {
            ViewOutcome::Working(outcome) => outcome,
            other => panic!("expected a working, found {other:?}"),
        }
    }

    fn steps(outcome: &WorkingOutcome) -> &[WorkingStepLine] {
        match outcome {
            WorkingOutcome::Steps { steps, .. } => steps,
            other => panic!("expected steps, found {other:?}"),
        }
    }

    #[test]
    fn a_working_holds_one_step_per_operation_from_the_inside_out() {
        let mut computing = session();
        let id = computing.enter("(2 + 3) * 4").unwrap();

        let outcome = working(&mut computing, id, &[]);

        let shown: Vec<(&str, &str)> = steps(&outcome)
            .iter()
            .map(|step| {
                (
                    match &step.operation {
                        WorkingOperation::Operator(construct) => construct.name,
                        WorkingOperation::Reference(_) => "reference",
                    },
                    step.result.as_str(),
                )
            })
            .collect();
        assert_eq!(shown, [("add", "5"), ("mul", "20")]);
    }

    #[test]
    fn a_working_step_carries_the_coherent_unit_of_its_operands_and_result() {
        let mut computing = session();
        let id = computing.enter("100 km / 2 h").unwrap();

        let outcome = working(&mut computing, id, &[]);

        let step = &steps(&outcome)[0];
        let operands: Vec<&str> = step
            .operands
            .iter()
            .map(|operand| operand.value.as_str())
            .collect();
        assert_eq!(operands, ["100000 m", "7200 s"]);
        assert_eq!(step.result, "125/9 m/s");
    }

    #[test]
    fn a_working_step_of_a_dimensionless_line_names_no_unit() {
        let mut computing = session();
        let id = computing.enter("(2 + 3) * 4").unwrap();

        let outcome = working(&mut computing, id, &[]);

        assert!(
            steps(&outcome)
                .iter()
                .all(|step| !step.result.contains(' '))
        );
    }

    #[test]
    fn a_working_step_converts_a_mixed_unit_sum_to_one_coherent_unit() {
        let mut computing = session();
        let id = computing.enter("5 m + 300 cm").unwrap();

        let outcome = working(&mut computing, id, &[]);

        let step = &steps(&outcome)[0];
        let operands: Vec<&str> = step
            .operands
            .iter()
            .map(|operand| operand.value.as_str())
            .collect();
        assert_eq!(operands, ["5 m", "3 m"]);
        assert_eq!(step.result, "8 m");
    }

    #[test]
    fn a_working_of_an_exact_line_names_no_machine_format() {
        let mut computing = session();
        let id = computing.enter("(2 + 3) * 4").unwrap();

        let outcome = working(&mut computing, id, &[]);

        assert!(steps(&outcome).iter().all(|step| step.format.is_none()));
    }

    #[test]
    fn a_line_that_uses_another_line_shows_it_as_a_step_of_its_own() {
        let mut computing = session();
        let first = computing.enter("6 * 7").unwrap();
        let second = computing.enter("r1 + 1").unwrap();

        let outcome = working(&mut computing, second, &[]);

        let first_step = steps(&outcome).first().expect("a step");
        assert_eq!(
            (first_step.operation.clone(), first_step.result.as_str()),
            (WorkingOperation::Reference(first), "42")
        );
    }

    #[test]
    fn an_operand_that_has_its_own_working_carries_its_path() {
        let mut computing = session();
        let id = computing.enter("(2 + 3) * 4").unwrap();

        let outcome = working(&mut computing, id, &[]);

        let last = steps(&outcome).last().expect("a step");
        let paths: Vec<Option<Vec<usize>>> = last
            .operands
            .iter()
            .map(|operand| operand.path.clone())
            .collect();
        assert_eq!(paths, [Some(vec![0]), None]);
    }

    #[test]
    fn a_working_of_a_path_holds_that_subexpression_only() {
        let mut computing = session();
        let id = computing.enter("(2 + 3) * 4").unwrap();

        let outcome = working(&mut computing, id, &[0]);

        assert_eq!(steps(&outcome).len(), 1);
    }

    #[test]
    fn a_line_without_an_operation_offers_no_working() {
        let mut computing = session();
        let id = computing.enter("42").unwrap();

        assert!(!computing.available_views(id).working);
    }

    #[test]
    fn a_working_that_does_not_apply_is_refused_with_its_line() {
        let mut computing = session();
        let id = computing.enter("42").unwrap();

        let error = match computing.view(
            id,
            ResultView::Working { path: Vec::new() },
            None,
            false,
            channel().0,
        ) {
            Ok(_) => panic!("expected no working"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            ViewError::NotApplicable {
                line: id,
                view: ViewKind::Working
            }
        );
    }

    #[test]
    fn a_machine_line_names_the_format_of_its_steps() {
        let mut computing = session();
        computing.set_precision(Precision::F64);
        let id = computing.enter("to_f64(0.1 + 0.2)").unwrap();

        let outcome = working(&mut computing, id, &[]);

        assert!(
            steps(&outcome)
                .iter()
                .all(|step| step.format == Some(MachineFormat::F64))
        );
    }

    #[test]
    fn a_line_with_an_approximate_operation_says_why_it_has_no_steps() {
        let mut computing = session();
        computing.set_precision(Precision::F64);
        let id = computing.enter("to_f64(sin(1)) + 1").unwrap();

        let outcome = working(&mut computing, id, &[]);

        assert_eq!(
            outcome,
            WorkingOutcome::Refused(WorkingRefusal::ApproximateOperations)
        );
    }

    #[test]
    fn a_working_beyond_the_limit_says_how_many_steps_it_left() {
        let mut computing = session();
        let text = format!("1 {}", " + 1".repeat(250));
        let id = computing.enter(&text).unwrap();

        let outcome = working(&mut computing, id, &[]);

        match outcome {
            WorkingOutcome::Steps {
                steps,
                further_steps,
                further_paths,
                ..
            } => assert_eq!(
                (steps.len(), further_steps, further_paths.is_empty()),
                (200, 50, false)
            ),
            other => panic!("expected steps, found {other:?}"),
        }
    }

    #[test]
    fn the_json_of_a_working_names_its_steps_and_what_is_left() {
        let mut computing = session();
        let id = computing.enter("2 + 3").unwrap();

        let outcome = working(&mut computing, id, &[]);
        let json = String::from_utf8(view_outcome_json(&ViewOutcome::Working(outcome))).unwrap();

        assert!(
            json.contains("\"name\": \"add\"") && json.contains("\"further_steps\": 0"),
            "{json}"
        );
    }

    #[test]
    fn cancelled_view_reports_nothing() {
        let mut computing = session();
        let id = computing.enter("1/3").unwrap();
        let (sender, receiver) = channel();
        let mut job = computing
            .view(id, ResultView::Digits { places: 5 }, None, false, sender)
            .unwrap();

        computing.cancel_view(id, ViewKind::Digits);
        job.step();

        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn cancelled_view_has_no_latest_generation() {
        let mut computing = session();
        let id = computing.enter("1/3").unwrap();
        let (sender, _receiver) = channel();
        let _job = computing
            .view(id, ResultView::Digits { places: 5 }, None, false, sender)
            .unwrap();

        computing.cancel_view(id, ViewKind::Digits);

        assert_eq!(computing.latest_view_generation(id, ViewKind::Digits), None);
    }

    #[test]
    fn digits_of_a_third_are_truncated_with_their_remainder_and_period() {
        let mut computing = session();
        let id = computing.enter("1/3").unwrap();

        let outcome = digits(&mut computing, id, 5);

        assert_eq!(
            (
                outcome.of,
                outcome.digits.digits,
                outcome.digits.remainder,
                outcome.digits.expansion
            ),
            (
                DigitsOf::ExactValue,
                fraction(33_333, 100_000),
                Some(fraction(1, 300_000)),
                Expansion::Recurs(DecimalPeriod {
                    start: 1,
                    length: 1
                })
            )
        );
    }

    #[test]
    fn digits_of_a_machine_line_are_the_digits_of_the_machine_number() {
        let mut computing = session();
        let id = computing.enter("to_f64(0.1)").unwrap();

        let outcome = digits(&mut computing, id, 20);

        assert_eq!(
            (outcome.of, outcome.digits.terminates),
            (DigitsOf::MachineValue, false)
        );
    }

    #[test]
    fn a_symbolic_value_gives_the_digits_of_its_true_value() {
        let mut computing = session();
        let id = computing.enter("pi/3").unwrap();

        let outcome = digits(&mut computing, id, 10);

        assert_eq!(
            (outcome.of, outcome.digits.digits),
            (
                DigitsOf::ExactValue,
                fraction(10_471_975_511, 10_000_000_000)
            )
        );
    }

    #[test]
    fn a_refined_expansion_is_not_known_to_recur() {
        let mut computing = session();
        let id = computing.enter("pi/3").unwrap();

        let outcome = digits(&mut computing, id, 10);

        assert_eq!(
            (outcome.digits.expansion, outcome.digits.remainder),
            (Expansion::NotKnownToRecur, None)
        );
    }

    #[test]
    fn a_refined_answer_does_not_claim_the_value_ends() {
        let mut computing = session();
        let id = computing.enter("pi/3").unwrap();

        assert!(!digits(&mut computing, id, 10).digits.terminates);
    }

    #[test]
    fn a_symbolic_line_can_be_enclosed() {
        let mut computing = session();
        let id = computing.enter("pi").unwrap();

        assert!(computing.available_views(id).enclose);
    }

    #[test]
    fn refined_places_above_the_significant_digit_limit_are_refused() {
        let mut computing = session();
        let id = computing.enter("pi/3").unwrap();
        let (sender, _receiver) = channel();

        assert_eq!(
            computing
                .view(
                    id,
                    ResultView::Digits { places: 5_001 },
                    None,
                    false,
                    sender
                )
                .err(),
            Some(ViewError::SignificantDigitsOutOfRange {
                digits: 5_001,
                limit: SIGNIFICANT_DIGITS_LIMIT
            })
        );
    }

    #[test]
    fn an_exact_rational_keeps_the_places_limit_of_its_own_size() {
        let mut computing = session();
        let id = computing.enter("1/3").unwrap();
        let (sender, _receiver) = channel();

        assert_eq!(
            computing
                .view(
                    id,
                    ResultView::Digits { places: 5_001 },
                    None,
                    false,
                    sender
                )
                .err(),
            None
        );
    }

    #[test]
    fn the_digits_of_a_measured_value_state_its_uncertainty() {
        let mut computing = session();
        let id = computing.enter("(2 +- 0.1) * 3").unwrap();

        let outcome = digits(&mut computing, id, 2);

        assert_eq!(
            outcome.uncertainty,
            Some(DigitsUncertainty::Standard {
                standard: fraction(3, 10),
                terminates: false
            })
        );
    }

    #[test]
    fn an_uncertainty_below_the_last_place_is_not_stated_as_zero() {
        let mut computing = session();
        let id = computing.enter("(1000 +- 0.001) * 1").unwrap();

        let outcome = digits(&mut computing, id, 1);

        assert_eq!(outcome.uncertainty, Some(DigitsUncertainty::BelowThePlace));
    }

    #[test]
    fn an_uncertainty_cut_off_says_that_it_was() {
        let mut computing = session();
        let id = computing.enter("(2 +- 0.125) * 1").unwrap();

        let outcome = digits(&mut computing, id, 2);

        assert_eq!(
            outcome.uncertainty,
            Some(DigitsUncertainty::Standard {
                standard: fraction(12, 100),
                terminates: false
            })
        );
    }

    #[test]
    fn the_uncertainty_takes_the_unit_the_value_is_shown_in() {
        let mut computing = session();
        let id = computing.enter("(1.5 +- 0.1) km").unwrap();

        let outcome = digits(&mut computing, id, 2);

        assert_eq!(
            (outcome.unit.as_deref(), outcome.uncertainty),
            (
                Some("km"),
                Some(DigitsUncertainty::Standard {
                    standard: fraction(1, 10),
                    terminates: false
                })
            )
        );
    }

    #[test]
    fn a_value_without_an_uncertainty_states_none() {
        let mut computing = session();
        let id = computing.enter("1/3").unwrap();

        let outcome = digits(&mut computing, id, 3);

        assert_eq!(outcome.uncertainty, None);
    }

    #[test]
    fn digits_keep_the_unit_the_value_is_shown_in() {
        let mut computing = session();
        let id = computing.enter("1.5 km").unwrap();

        assert_eq!(digits(&mut computing, id, 2).unit.as_deref(), Some("km"));
    }

    #[test]
    fn digits_above_their_limit_are_refused_before_a_job_starts() {
        let mut computing = session();
        let id = computing.enter("1/3").unwrap();
        let (sender, _receiver) = channel();

        assert_eq!(
            computing
                .view(
                    id,
                    ResultView::Digits { places: 78_914 },
                    None,
                    false,
                    sender,
                )
                .err(),
            Some(ViewError::PlacesAboveLimit {
                places: 78_914,
                limit: 78_913
            })
        );
    }

    #[test]
    fn digits_of_a_refused_line_give_its_refusal() {
        let mut computing = session();
        let id = computing.enter("1/0").unwrap();
        let (sender, _receiver) = channel();

        let refused = computing
            .view(id, ResultView::Digits { places: 3 }, None, false, sender)
            .err();

        assert!(
            matches!(&refused, Some(ViewError::LineRefused { diagnostic, .. }) if diagnostic.code == "division_by_zero"),
            "{refused:?}"
        );
    }

    #[test]
    fn enclosure_of_a_square_root_proves_its_significant_digits() {
        let mut computing = session();
        let id = computing.enter("sqrt(2)").unwrap();

        let ViewOutcome::Enclosure(outcome) = finished(run(
            &mut computing,
            id,
            ResultView::Enclose {
                significant_digits: 10,
            },
        )) else {
            panic!("expected an enclosure");
        };

        assert_eq!(
            (
                outcome.enclosure.lower,
                outcome.enclosure.upper,
                outcome.enclosure.reached
            ),
            (
                fraction(1_414_213_562, 1_000_000_000),
                fraction(1_414_213_563, 1_000_000_000),
                true
            )
        );
    }

    #[test]
    fn digits_are_of_the_number_the_value_row_shows() {
        let mut computing = session();
        let id = computing.enter("1.5 km").unwrap();

        let outcome = digits(&mut computing, id, 3);

        assert_eq!(outcome.digits.digits, fraction(3, 2));
    }

    #[test]
    fn the_enclosure_rows_take_the_unit_the_value_is_shown_in() {
        let mut computing = session();
        let id = computing.enter("1.5 km").unwrap();

        let ViewOutcome::Enclosure(outcome) = finished(run(
            &mut computing,
            id,
            ResultView::Enclose {
                significant_digits: 3,
            },
        )) else {
            panic!("expected an enclosure");
        };

        assert_eq!(
            (outcome.unit.as_deref(), outcome.enclosure.lower),
            (Some("km"), fraction(3, 2))
        );
    }

    #[test]
    fn enclosure_of_a_measured_line_does_not_apply() {
        let mut computing = session();
        let id = computing.enter("2.5 +- 0.1").unwrap();

        assert!(!computing.available_views(id).enclose);
    }

    #[test]
    fn enclosure_outside_its_digit_range_is_refused() {
        let mut computing = session();
        let id = computing.enter("sqrt(2)").unwrap();
        let (sender, _receiver) = channel();

        assert_eq!(
            computing
                .view(
                    id,
                    ResultView::Enclose {
                        significant_digits: 5_001
                    },
                    None,
                    false,
                    sender
                )
                .err(),
            Some(ViewError::SignificantDigitsOutOfRange {
                digits: 5_001,
                limit: 5_000
            })
        );
    }

    #[test]
    fn newer_enclosure_of_the_same_line_cancels_the_older_one() {
        let mut computing = session();
        let id = computing.enter("sqrt(2)").unwrap();
        let (sender, receiver) = channel();
        let view = ResultView::Enclose {
            significant_digits: 10,
        };
        let mut older = computing
            .view(id, view.clone(), None, false, sender.clone())
            .unwrap();
        let _newer = computing.view(id, view, None, false, sender).unwrap();

        older.step();

        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn latest_view_generation_is_the_generation_of_the_newest_job() {
        let mut computing = session();
        let id = computing.enter("sqrt(2)").unwrap();
        let (sender, receiver) = channel();
        let view = ResultView::Digits { places: 2 };
        let _older = computing
            .view(id, view.clone(), None, false, sender.clone())
            .unwrap();
        let mut newer = computing.view(id, view, None, false, sender).unwrap();
        newer.step();

        let ViewEvent::Finished { generation, .. } = receiver.try_recv().unwrap() else {
            panic!("expected a finished view");
        };
        assert_eq!(
            computing.latest_view_generation(id, ViewKind::Digits),
            Some(generation)
        );
    }

    #[test]
    fn line_without_a_view_has_no_latest_generation() {
        let mut computing = session();
        let id = computing.enter("1/3").unwrap();

        assert_eq!(
            computing.latest_view_generation(id, ViewKind::Enclose),
            None
        );
    }

    #[test]
    fn machine_line_enters_the_conversion_of_the_line_label() {
        let mut computing = session();
        let id = computing.enter("0.1 + 0.2").unwrap();

        let machine = computing.machine_line(id, MachineFormat::F64).unwrap();

        assert_eq!(computing.line(machine).unwrap().input(), "to_f64(r1)");
    }

    #[test]
    fn machine_line_evaluates_the_written_expression_in_machine_arithmetic() {
        let mut computing = session();
        let id = computing.enter("0.1 + 0.2").unwrap();

        let machine = computing.machine_line(id, MachineFormat::F64).unwrap();

        let Some(Outcome::Result(record)) = computing.line(machine).map(|line| line.outcome())
        else {
            panic!("expected a result");
        };
        assert_eq!(
            record.computed().value(),
            &ResultValue::Number(Number::F64(0.1 + 0.2))
        );
    }

    #[test]
    fn machine_line_of_a_named_line_uses_its_name() {
        let mut computing = session();
        let id = computing.enter("x = 1/3").unwrap();

        let machine = computing.machine_line(id, MachineFormat::F32).unwrap();

        assert_eq!(computing.line(machine).unwrap().input(), "to_f32(x)");
    }

    #[test]
    fn a_line_holding_a_free_name_offers_no_view() {
        let mut computing = session();
        let id = computing.enter("(x + 1)^2").unwrap();

        assert_eq!(
            computing.available_views(id),
            AvailableViews {
                digits: false,
                enclose: false,
                machine_line: false,
                working: false,
            }
        );
    }

    #[test]
    fn a_machine_line_of_a_line_holding_a_free_name_names_the_name() {
        let mut computing = session();
        let id = computing.enter("(x + 1)^2").unwrap();

        assert_eq!(
            computing.machine_line(id, MachineFormat::F64).err(),
            Some(SessionError::HoldsFreeNames {
                line: id,
                names: vec!["x".to_string()],
            })
        );
    }

    #[test]
    fn machine_line_of_a_function_definition_does_not_apply() {
        let mut computing = session();
        let id = computing.enter("f(x) = x^2").unwrap();

        assert_eq!(
            computing.machine_line(id, MachineFormat::F64).err(),
            Some(SessionError::MachineLineNotApplicable(id))
        );
    }

    #[test]
    fn digits_outcome_is_written_with_the_members_of_a_125() {
        let mut computing = session();
        let id = computing.enter("1/3").unwrap();
        let outcome = finished(run(&mut computing, id, ResultView::Digits { places: 5 }));

        let text = String::from_utf8(view_outcome_json(&outcome)).unwrap();

        assert!(
            text.starts_with("{\n  \"places\": 5,\n  \"digits\":")
                && text.contains("\"expansion\": {\n    \"state\": \"recurs\",\n")
                && text.contains("\"of\": \"exact_value\",\n"),
            "{text}"
        );
    }

    #[test]
    fn enclosure_outcome_is_written_with_the_members_of_a_062() {
        let mut computing = session();
        let id = computing.enter("sqrt(2)").unwrap();
        let outcome = finished(run(
            &mut computing,
            id,
            ResultView::Enclose {
                significant_digits: 3,
            },
        ));

        let text = String::from_utf8(view_outcome_json(&outcome)).unwrap();

        assert!(
            text.starts_with("{\n  \"significant_digits\": 3,\n")
                && text.contains("\"reached\": true,\n")
        );
    }

    #[test]
    fn exact_line_offers_all_three() {
        let mut computing = session();
        let id = computing.enter("1/3").unwrap();

        assert_eq!(
            computing.available_views(id),
            AvailableViews {
                working: true,
                digits: true,
                enclose: true,
                machine_line: true
            }
        );
    }
}

#[cfg(test)]
mod form_tests {
    use crate::session::tests::{fixed_clock, session};
    use crate::{RationalForm, Session};

    #[test]
    fn decimal_form_is_decided_again_when_a_session_is_opened() {
        let mut computing = session();
        let id = computing.enter("0.25 + 0.125").unwrap();
        let bytes = computing.save_to_bytes().unwrap();

        let opened = Session::open_from_bytes(&bytes, fixed_clock(), Vec::new()).unwrap();

        assert_eq!(opened.rational_form_of(id), RationalForm::Decimal);
    }

    #[test]
    fn division_gives_the_fraction_form() {
        let mut computing = session();
        let id = computing.enter("1/4").unwrap();

        assert_eq!(computing.rational_form_of(id), RationalForm::Fraction);
    }

    #[test]
    fn editing_a_line_into_a_division_changes_its_form() {
        let mut computing = session();
        let id = computing.enter("0.25").unwrap();
        computing.edit(id, "1/4").unwrap();

        assert_eq!(computing.rational_form_of(id), RationalForm::Fraction);
    }
}
