use std::time::Duration;

use calc_core::{
    ComputedResult, Diagnostic, Method, ParameterValue, RationalForm, RecognitionTruncation,
    ResultKind, ResultValue, RoundingError,
};
use calc_exec::{BackendKind, Domain, Preference};
use calc_i18n::{Locale, Message, render};
use calc_numbers::{Integer, Number, Rational, power_of_ten_text};
use calc_viz::Orbit;

use crate::decimal_text::{civil_time, integer_to_decimal};
use crate::messages::diagnostic_message;
use crate::readable::{PLAIN_HIGHEST_DECADE, PLAIN_LOWEST_DECADE};
use crate::recognition::{RecognitionState, RecognitionUnavailable};
use crate::replay::ReplayReport;
use crate::result_record::{
    LineId, OperationModeUse, RecordRecognition, ResultRecord, UtcTimestamp,
};
use crate::session::{Line, Outcome, Precision, Session};
use crate::session_file::{line_label, unit_text};
use crate::solve_answer::{AnswerBody, SolveAnswer};

const PROPAGATION_PARAMETER: &str = "uncertainty_propagation";
const FIRST_ORDER_PROPAGATION: &str = "first_order_taylor";
const INPUT_COUNT_PARAMETER: &str = "uncertain_input_count";
const RATIONAL_SEPARATOR: &str = "/";
const DECIMAL_POINT: char = '.';
const NEGATIVE_SIGN: char = '-';
const COMPLEX_IMAGINARY_UNIT: &str = "i";
const COMPLEX_PLUS: &str = " + ";
const COMPLEX_MINUS: &str = " - ";
const COMPLEX_TIMES: &str = " * ";
const MINUS_SIGN: char = '-';
const ARRAY_OPEN: &str = "[";
const ARRAY_CLOSE: &str = "]";
const ARRAY_ELEMENT_SEPARATOR: &str = ", ";
const ARRAY_ROW_SEPARATOR: &str = "; ";
const NOT_A_NUMBER: &str = "nan";
const INFINITY: &str = "inf";
const DECIMAL_BASE: i64 = 10;
const NANOSECONDS_PER_MICROSECOND: u128 = 1_000;
const NANOSECONDS_PER_MILLISECOND: u128 = 1_000_000;
const NANOSECONDS_PER_SECOND: u128 = 1_000_000_000;
const EXPONENT_FORM_SAVING: usize = 3;
const SMALLEST_PLAIN_MAGNITUDE: f64 = 1e-5;
const LARGEST_PLAIN_MAGNITUDE: f64 = 1e16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendRun {
    pub selected: Option<BackendKind>,
    pub width: Domain,
    pub modes: Vec<OperationModeUse>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineSummary {
    pub label: String,
    pub name: Option<String>,
    pub input: String,
    pub identifies_number: bool,
    pub state: LineState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineState {
    Result(Box<RecordSummary>),
    Failed(Message),
    NotEvaluated,
    Solve(SolveLineSummary),
    Picture,
    Orbit(Orbit),
    Defined,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordSummary {
    pub value: String,
    pub unit: Option<String>,
    pub kind: ResultKind,
    pub value_form: Option<ValueForm>,
    pub uncertainty: Option<UncertaintySummary>,
    pub rounding_error: RoundingSummary,
    pub method: String,
    pub runs: Vec<BackendRun>,
    pub measured_run_time: Option<Duration>,
    pub recognition: RecognitionSummary,
    pub dependencies: Vec<String>,
    pub notes: Vec<Diagnostic>,
    pub sort: Option<crate::sort_summary::SortSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecognitionSummary {
    NotRun,
    Running,
    Matched(MatchedSummary),
    NothingMatched,
    CutShort(RecognitionTruncation),
    Unavailable(RecognitionUnavailable),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MatchedSummary {
    pub concepts: Vec<String>,
    pub unnamed: Option<u64>,
    pub truncated: Option<bool>,
    pub truncated_by: Option<RecognitionTruncation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UncertaintySummary {
    pub standard: String,
    pub coverage_factor: String,
    pub propagated_from_inputs: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueForm {
    ExactInteger,
    ExactDecimal,
    ExactFraction,
    Machine32,
    Machine64,
    ProvenRange,
    ExactComplex,
    MachineComplex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolveLineSummary {
    Ways { found: u64 },
    Solved { evaluated: u64 },
    NotReached { further_inputs: u64 },
    Reachable { quantities: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoundingSummary {
    Exact,
    Bound(String),
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayState {
    NotRun,
    Verified { at: UtcTimestamp, today: bool },
    Differs { at: UtcTimestamp, today: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionStatus {
    pub replay: ReplayState,
    pub results: u64,
    pub running: u64,
    pub differing: Option<u64>,
    pub precision: Precision,
    pub backend: Preference,
}

impl Session {
    pub fn line_summary(&self, line: &Line) -> LineSummary {
        let state = match line.outcome() {
            Outcome::Result(record) => {
                let form = self.rational_form_of(line.id());
                let mut summary = self.record_summary_in(record, form);
                if is_decimal_answer_to_a_base(line.input(), record.computed().value(), form) {
                    summary.notes.push(Diagnostic {
                        code: RADIX_WRITTEN_DECIMAL_CODE.to_owned(),
                        data: std::collections::BTreeMap::new(),
                    });
                }
                summary.recognition = self.recognition_summary(line.id(), record);
                LineState::Result(Box::new(summary))
            }
            Outcome::Error(diagnostic) => LineState::Failed(diagnostic_message(diagnostic)),
            Outcome::NotEvaluated => LineState::NotEvaluated,
            Outcome::Defined => LineState::Defined,
            Outcome::Answer(answer) => LineState::Solve(solve_line_summary(answer)),
            Outcome::Picture => LineState::Picture,
            Outcome::EscapeTimeReading(orbit) => LineState::Orbit(*orbit),
            Outcome::Reachable(answer) => LineState::Solve(SolveLineSummary::Reachable {
                quantities: answer.reachable_total,
            }),
        };
        LineSummary {
            label: line_label(line.id()),
            name: line.name().map(str::to_owned),
            input: line.input().to_owned(),
            identifies_number: self.identifies_number(line.id()),
            state,
        }
    }

    pub fn status(&self) -> SessionStatus {
        let settings = self.settings();
        let differing = self.replay_report().map(ReplayReport::differing);
        SessionStatus {
            replay: self.replay_state(),
            results: u64::try_from(self.lines().len()).unwrap_or(u64::MAX),
            running: 0,
            differing,
            precision: settings.precision,
            backend: settings.backend,
        }
    }

    pub fn record_summary(&self, record: &ResultRecord) -> RecordSummary {
        self.record_summary_in(record, RationalForm::Fraction)
    }

    fn record_summary_in(&self, record: &ResultRecord, form: RationalForm) -> RecordSummary {
        RecordSummary {
            dependencies: record
                .dependencies()
                .iter()
                .map(|dependency| line_label(*dependency))
                .collect(),
            runs: record
                .backend()
                .iter()
                .map(|used| BackendRun {
                    selected: used.selected,
                    width: used.width,
                    modes: used.modes.clone(),
                })
                .collect(),
            measured_run_time: self.measured_run_time(record),
            recognition: stored_recognition_summary(record),
            ..self.computed_summary(record.computed(), form)
        }
    }

    fn measured_run_time(&self, record: &ResultRecord) -> Option<Duration> {
        let resolution = Duration::from_nanos(self.clock_resolution_nanoseconds());
        (record.duration() > resolution).then(|| record.duration())
    }

    fn computed_summary(&self, computed: &ComputedResult, form: RationalForm) -> RecordSummary {
        let unit = computed
            .unit()
            .and_then(|unit| unit_text(self.pool(), unit).ok().flatten());
        RecordSummary {
            value: value_text_in(computed.value(), form),
            unit,
            kind: computed.kind(),
            value_form: if computed.method().name == crate::session::WORST_CASE_METHOD {
                Some(ValueForm::ProvenRange)
            } else {
                value_form(computed.value(), form)
            },
            uncertainty: computed
                .uncertainty()
                .map(|uncertainty| UncertaintySummary {
                    standard: value_text(uncertainty.standard()),
                    coverage_factor: number_text(uncertainty.coverage_factor()),
                    propagated_from_inputs: propagated_input_count(computed.method()),
                }),
            rounding_error: match computed.rounding_error() {
                RoundingError::None => RoundingSummary::Exact,
                RoundingError::Bound(bound) => RoundingSummary::Bound(value_text(bound)),
                RoundingError::Unknown => RoundingSummary::Unknown,
            },
            method: computed.method().name.clone(),
            notes: radix_notes(computed.value(), form, &computed.method().notes),
            runs: Vec::new(),
            measured_run_time: None,
            recognition: RecognitionSummary::NotRun,
            dependencies: Vec::new(),
            sort: crate::sort_summary::sort_summary(computed),
        }
    }
}

impl Session {
    fn recognition_summary(&self, id: LineId, record: &ResultRecord) -> RecognitionSummary {
        match self.recognition_state(id) {
            Some(RecognitionState::Running) => RecognitionSummary::Running,
            Some(RecognitionState::Unavailable(reason)) => RecognitionSummary::Unavailable(reason),
            Some(RecognitionState::Ready) | None => {
                if self.is_recognition_pending(id) {
                    return RecognitionSummary::Running;
                }
                stored_recognition_summary(record)
            }
        }
    }
}

fn held_concepts() -> Option<Vec<String>> {
    crate::solve::concept_set().ok().map(|concepts| {
        concepts
            .concepts
            .iter()
            .map(|node| node.identifier.clone())
            .collect()
    })
}

fn stored_recognition_summary(record: &ResultRecord) -> RecognitionSummary {
    match record.recognition() {
        RecordRecognition::NotRun => RecognitionSummary::NotRun,
        RecordRecognition::Ran(recognized) if recognized.matches.is_empty() => {
            match recognized.truncated_by {
                Some(cause) => RecognitionSummary::CutShort(cause),
                None => RecognitionSummary::NothingMatched,
            }
        }
        RecordRecognition::Ran(recognized) => {
            let matched = recognized
                .matches
                .iter()
                .map(|matched| matched.concept.clone());
            let (concepts, unnamed) = match held_concepts() {
                Some(held) => {
                    let (held, missing): (Vec<String>, Vec<String>) =
                        matched.partition(|concept| held.contains(concept));
                    (held, Some(u64::try_from(missing.len()).unwrap_or(u64::MAX)))
                }
                None => (Vec::new(), None),
            };
            RecognitionSummary::Matched(MatchedSummary {
                concepts,
                unnamed,
                truncated: recognized.truncated,
                truncated_by: recognized.truncated_by,
            })
        }
        RecordRecognition::Unavailable(reason) => RecognitionSummary::Unavailable(*reason),
    }
}

pub(crate) fn value_form(value: &ResultValue, form: RationalForm) -> Option<ValueForm> {
    match value {
        ResultValue::Number(Number::Integer(_)) => Some(ValueForm::ExactInteger),
        ResultValue::Number(Number::Rational(rational)) => {
            match (
                form,
                finite_decimal_text(rational.numerator(), rational.denominator()),
            ) {
                (RationalForm::Decimal, Some(_)) => Some(ValueForm::ExactDecimal),
                _ => Some(ValueForm::ExactFraction),
            }
        }
        ResultValue::Number(Number::F64(_)) => Some(ValueForm::Machine64),
        ResultValue::Number(Number::F32(_)) => Some(ValueForm::Machine32),
        ResultValue::Complex { real, .. } => Some(if real.is_exact() {
            ValueForm::ExactComplex
        } else {
            ValueForm::MachineComplex
        }),
        ResultValue::Array { .. } | ResultValue::Expression(_) => None,
    }
}

fn propagated_input_count(method: &Method) -> Option<u64> {
    let is_first_order = matches!(
        method.parameters.get(PROPAGATION_PARAMETER),
        Some(ParameterValue::Identifier(name)) if name == FIRST_ORDER_PROPAGATION
    );
    if !is_first_order {
        return None;
    }
    match method.parameters.get(INPUT_COUNT_PARAMETER) {
        Some(ParameterValue::Value(ResultValue::Number(Number::Integer(count)))) => {
            count.to_i128().and_then(|count| u64::try_from(count).ok())
        }
        _ => None,
    }
}

fn solve_line_summary(answer: &SolveAnswer) -> SolveLineSummary {
    let count = |length: usize| u64::try_from(length).unwrap_or(u64::MAX);
    match &answer.body {
        AnswerBody::Ways { listing, .. } | AnswerBody::Front { listing, .. } => {
            SolveLineSummary::Ways {
                found: listing.ways_found,
            }
        }
        AnswerBody::Solved { ways, .. } => SolveLineSummary::Solved {
            evaluated: count(ways.iter().filter(|way| way.record.is_some()).count()),
        },
        AnswerBody::NotReached { missing_any_of, .. } => SolveLineSummary::NotReached {
            further_inputs: count(missing_any_of.len()),
        },
    }
}

pub fn solve_line_message(summary: SolveLineSummary) -> Message {
    match summary {
        SolveLineSummary::Ways { found } => Message::CommonSolveLineWays { count: found },
        SolveLineSummary::Solved { evaluated } => {
            Message::CommonSolveLineSolved { count: evaluated }
        }
        SolveLineSummary::NotReached { further_inputs } => Message::CommonSolveLineNotReached {
            count: further_inputs,
        },
        SolveLineSummary::Reachable { quantities } => {
            Message::CommonSolveLineReachable { count: quantities }
        }
    }
}

pub fn record_kind_message(record: &RecordSummary) -> Message {
    if let Some(inputs) = record
        .uncertainty
        .as_ref()
        .and_then(|uncertainty| uncertainty.propagated_from_inputs)
    {
        return Message::CommonResultKindDerivedFromMeasured { count: inputs };
    }
    value_kind_message(record.kind, record.value_form)
}

pub fn value_kind_message(kind: ResultKind, form: Option<ValueForm>) -> Message {
    match (kind, form) {
        (ResultKind::MachineFloat, Some(ValueForm::Machine64)) => {
            Message::CommonResultKindMachineFloat64
        }
        (ResultKind::MachineFloat, Some(ValueForm::Machine32)) => {
            Message::CommonResultKindMachineFloat32
        }
        (ResultKind::ExactRational, Some(ValueForm::ExactInteger)) => {
            Message::CommonResultKindExactInteger
        }
        (ResultKind::ExactRational, Some(ValueForm::ExactDecimal)) => {
            Message::CommonResultKindExactDecimal
        }
        (ResultKind::ExactRational, Some(ValueForm::ExactFraction)) => {
            Message::CommonResultKindExactFraction
        }
        (_, Some(ValueForm::ProvenRange)) => Message::CommonResultKindProvenRange,
        (_, Some(ValueForm::ExactComplex)) => Message::CommonResultKindExactComplex,
        (_, Some(ValueForm::MachineComplex)) => Message::CommonResultKindMachineComplex,
        _ => kind_message(kind),
    }
}

pub fn run_time_message(run_time: Duration) -> Message {
    let nanoseconds = run_time.as_nanos();
    let (divisor, message): (u128, fn(String) -> Message) = match nanoseconds {
        0..NANOSECONDS_PER_MICROSECOND => {
            (1, |time| Message::CommonRecordRunTimeNanoseconds { time })
        }
        NANOSECONDS_PER_MICROSECOND..NANOSECONDS_PER_MILLISECOND => {
            (NANOSECONDS_PER_MICROSECOND, |time| {
                Message::CommonRecordRunTimeMicroseconds { time }
            })
        }
        NANOSECONDS_PER_MILLISECOND..NANOSECONDS_PER_SECOND => {
            (NANOSECONDS_PER_MILLISECOND, |time| {
                Message::CommonRecordRunTimeMilliseconds { time }
            })
        }
        _ => (NANOSECONDS_PER_SECOND, |time| {
            Message::CommonRecordRunTimeSeconds { time }
        }),
    };
    message(rounded_to_three_figures(nanoseconds, divisor))
}

fn rounded_to_three_figures(nanoseconds: u128, divisor: u128) -> String {
    let whole = nanoseconds / divisor;
    let decimals = match whole {
        0..10 => 2,
        10..100 => 1,
        _ => 0,
    };
    let scale = 10_u128.pow(decimals);
    let scaled = (nanoseconds * scale + divisor / 2) / divisor;
    if decimals == 0 {
        return scaled.to_string();
    }
    let places = usize::try_from(decimals).unwrap_or(0);
    format!(
        "{}{DECIMAL_POINT}{:0places$}",
        scaled / scale,
        scaled % scale
    )
}

pub fn rounding_bound_message(bound: String) -> Message {
    Message::CommonRecordRoundingAtMost { bound }
}

pub fn propagation_message(inputs: u64) -> Message {
    Message::CommonRecordPropagatedFirstOrder { count: inputs }
}

pub fn replay_state_message(state: ReplayState, locale: &Locale) -> Message {
    match state {
        ReplayState::NotRun => Message::CommonStatusReplayNotRun,
        ReplayState::Verified { at, today } => Message::CommonStatusReplayVerified {
            time: replay_time_text(at, today, locale),
        },
        ReplayState::Differs { at, today } => Message::CommonStatusReplayDiffers {
            time: replay_time_text(at, today, locale),
        },
    }
}

fn replay_time_text(at: UtcTimestamp, today: bool, locale: &Locale) -> String {
    let civil = civil_time(at.milliseconds_since_unix_epoch());
    let two_digits = |value: u64| format!("{value:02}");
    let time = render(
        &Message::CommonTimeOfDay {
            hour: two_digits(civil.hour),
            minute: two_digits(civil.minute),
            second: two_digits(civil.second),
        },
        locale,
    )
    .to_string();
    if today {
        return time;
    }
    render(
        &Message::CommonDateAndTime {
            year: format!("{:04}", civil.year),
            month: two_digits(civil.month),
            day: two_digits(civil.day),
            time,
        },
        locale,
    )
    .to_string()
}

pub fn rounding_message(rounding: &RoundingSummary) -> Option<Message> {
    match rounding {
        RoundingSummary::Exact => None,
        RoundingSummary::Unknown => Some(Message::CommonRecordRoundingUnknown),
        RoundingSummary::Bound(_) => None,
    }
}

pub fn kind_message(kind: ResultKind) -> Message {
    match kind {
        ResultKind::ExactRational => Message::CommonResultKindExactRational,
        ResultKind::Algebraic => Message::CommonResultKindAlgebraic,
        ResultKind::Symbolic => Message::CommonResultKindSymbolic,
        ResultKind::MachineFloat => Message::CommonResultKindMachineFloat,
    }
}

pub fn value_text(value: &ResultValue) -> String {
    match value {
        ResultValue::Number(number) => number_text(number),
        ResultValue::Complex { real, imaginary } => complex_text(real, imaginary, number_text),
        ResultValue::Array { shape, elements } => array_text(shape, elements, value_text),
        ResultValue::Expression(text) => text.clone(),
    }
}

fn complex_text(real: &Number, imaginary: &Number, text: impl Fn(&Number) -> String) -> String {
    let is_exact_integer = |number: &Number, written: &str| {
        matches!(number, Number::Integer(_)) && number_text(number) == written
    };
    let imaginary_term = if is_exact_integer(imaginary, "1") {
        COMPLEX_IMAGINARY_UNIT.to_owned()
    } else if is_exact_integer(imaginary, "-1") {
        format!("{MINUS_SIGN}{COMPLEX_IMAGINARY_UNIT}")
    } else {
        format!("{}{COMPLEX_TIMES}{COMPLEX_IMAGINARY_UNIT}", text(imaginary))
    };
    if is_exact_integer(real, "0") {
        return imaginary_term;
    }
    match imaginary_term.strip_prefix(MINUS_SIGN) {
        Some(magnitude) => format!("{}{COMPLEX_MINUS}{magnitude}", text(real)),
        None => format!("{}{COMPLEX_PLUS}{imaginary_term}", text(real)),
    }
}

pub fn value_text_in(value: &ResultValue, form: RationalForm) -> String {
    let number_in = |number: &Number| match (number, form) {
        (Number::Rational(rational), RationalForm::Fraction | RationalForm::Radix(_)) => {
            fraction_text(rational)
        }
        (Number::Integer(integer), RationalForm::Radix(radix)) => {
            radix_text(integer, radix).unwrap_or_else(|| number_text(number))
        }
        _ => number_text(number),
    };
    match value {
        ResultValue::Number(number) => number_in(number),
        ResultValue::Complex { real, imaginary } => complex_text(real, imaginary, number_in),
        ResultValue::Array { shape, elements } => {
            array_text(shape, elements, |element| value_text_in(element, form))
        }
        ResultValue::Expression(text) => text.clone(),
    }
}

pub(crate) const RADIX_DECIMAL_CODE: &str = "radix_decimal";

pub(crate) const RADIX_WRITTEN_DECIMAL_CODE: &str = "radix_written_decimal";

fn is_decimal_answer_to_a_base(input: &str, value: &ResultValue, form: RationalForm) -> bool {
    calc_syntax::written_radix(input).is_some()
        && !matches!(
            (value, form),
            (
                ResultValue::Number(Number::Integer(_)),
                RationalForm::Radix(_)
            )
        )
}

fn radix_notes(value: &ResultValue, form: RationalForm, notes: &[Diagnostic]) -> Vec<Diagnostic> {
    let mut notes = notes.to_vec();
    if let (ResultValue::Number(Number::Integer(integer)), RationalForm::Radix(radix)) =
        (value, form)
        && radix.base != 10
    {
        notes.push(Diagnostic {
            code: RADIX_DECIMAL_CODE.to_owned(),
            data: std::collections::BTreeMap::from([(
                "value".to_owned(),
                calc_core::ParameterValue::Identifier(integer_to_decimal(integer)),
            )]),
        });
    }
    notes
}

const RADIX_DIGITS: &str = "0123456789ABCDEF";
const BINARY_GROUP: usize = 4;
const GROUP_SEPARATOR: char = '_';
const BYTE_SEPARATOR: &str = " ";

fn digits_in(integer: &Integer, base: u32) -> String {
    let divisor = Integer::from(u64::from(base));
    let mut rest = integer.absolute();
    let mut digits = Vec::new();
    while !rest.is_zero() {
        let Ok((quotient, remainder)) = rest.div_rem_euclid(&divisor) else {
            break;
        };
        let index = usize::try_from(remainder.to_i64().unwrap_or(0)).unwrap_or(0);
        digits.push(RADIX_DIGITS.as_bytes()[index] as char);
        rest = quotient;
    }
    if digits.is_empty() {
        digits.push('0');
    }
    digits.iter().rev().collect()
}

pub(crate) fn radix_text(integer: &Integer, form: calc_core::RadixForm) -> Option<String> {
    let span = (form.bits > 0).then(|| Integer::one().shifted_left(form.bits as usize));
    let pattern = match &span {
        Some(span) => integer.div_rem_euclid(span).ok()?.1,
        None => integer.clone(),
    };
    if let Some(big_endian) = form.big_endian {
        if pattern.is_negative() {
            return None;
        }
        let mut hex = digits_in(&pattern, 16);
        let width = if form.bits > 0 {
            usize::try_from(form.bits / 4).ok()?
        } else {
            hex.len() + hex.len() % 2
        };
        while hex.len() < width {
            hex.insert(0, '0');
        }
        let mut bytes: Vec<String> = hex
            .as_bytes()
            .chunks(2)
            .map(|pair| String::from_utf8_lossy(pair).into_owned())
            .collect();
        if !big_endian {
            bytes.reverse();
        }
        return Some(bytes.join(BYTE_SEPARATOR));
    }
    let (prefix, bits_per_digit) = match form.base {
        16 => ("0x", 4),
        2 => ("0b", 1),
        8 => ("0o", 3),
        _ => return Some(integer_to_decimal(integer)),
    };
    let mut digits = digits_in(&pattern, form.base);
    if form.bits > 0 {
        let width = usize::try_from(form.bits.div_ceil(bits_per_digit)).ok()?;
        while digits.len() < width {
            digits.insert(0, '0');
        }
    }
    if form.base == 2 {
        let characters: Vec<char> = digits.chars().collect();
        let first = characters.len() % BINARY_GROUP;
        let mut grouped = String::new();
        for (index, character) in characters.iter().enumerate() {
            if index > 0 && (index + BINARY_GROUP - first).is_multiple_of(BINARY_GROUP) {
                grouped.push(GROUP_SEPARATOR);
            }
            grouped.push(*character);
        }
        digits = grouped;
    }
    let sign = if pattern.is_negative() { "-" } else { "" };
    Some(format!("{sign}{prefix}{digits}"))
}

fn fraction_text(rational: &Rational) -> String {
    format!(
        "{}{RATIONAL_SEPARATOR}{}",
        integer_to_decimal(rational.numerator()),
        integer_to_decimal(rational.denominator())
    )
}

fn array_text(
    shape: &[usize],
    elements: &[ResultValue],
    element_text: impl Fn(&ResultValue) -> String,
) -> String {
    let texts: Vec<String> = elements.iter().map(element_text).collect();
    let body = match shape {
        [_, columns] if *columns > 0 => texts
            .chunks(*columns)
            .map(|row| row.join(ARRAY_ELEMENT_SEPARATOR))
            .collect::<Vec<_>>()
            .join(ARRAY_ROW_SEPARATOR),
        _ => texts.join(ARRAY_ELEMENT_SEPARATOR),
    };
    format!("{ARRAY_OPEN}{body}{ARRAY_CLOSE}")
}

pub const LONGEST_SHOWN_DIGITS: usize = 40;
const KEPT_DIGITS: usize = 12;
const ELISION: char = '\u{2026}';

pub fn shortened_digits(text: &str) -> Option<(String, Vec<u64>)> {
    if let Some(whole) = shortened_whole_number_in_exponent_form(text) {
        return Some(whole);
    }
    let mut shortened = String::with_capacity(text.len());
    let mut counts: Vec<u64> = Vec::new();
    let mut digits = String::new();
    for character in text.chars() {
        if character.is_ascii_digit() {
            digits.push(character);
            continue;
        }
        write_digits(&digits, &mut shortened, &mut counts);
        digits.clear();
        shortened.push(character);
    }
    write_digits(&digits, &mut shortened, &mut counts);
    (!counts.is_empty()).then_some((shortened, counts))
}

fn shortened_whole_number_in_exponent_form(text: &str) -> Option<(String, Vec<u64>)> {
    let (significand, after) = text.split_once('e')?;
    let (leading, fraction) = significand.split_once('.')?;
    let leading_digits = leading.strip_prefix('-').unwrap_or(leading);
    let exponent_text: String = after.chars().take_while(char::is_ascii_digit).collect();
    let exponent: u64 = exponent_text.parse().ok()?;
    let is_whole = leading_digits.len() == 1
        && leading_digits
            .chars()
            .all(|character| character.is_ascii_digit())
        && fraction.chars().all(|character| character.is_ascii_digit())
        && u64::try_from(fraction.len()).ok()? <= exponent;
    if !is_whole || fraction.len() <= LONGEST_SHOWN_DIGITS {
        return None;
    }
    let run: Vec<char> = fraction.chars().collect();
    let mut shortened = format!("{leading}.");
    shortened.extend(run.iter().take(KEPT_DIGITS));
    shortened.push(ELISION);
    shortened.extend(run.iter().skip(run.len() - KEPT_DIGITS));
    shortened.push('e');
    shortened.push_str(after);
    Some((shortened, vec![exponent + 1]))
}

fn write_digits(digits: &str, shortened: &mut String, counts: &mut Vec<u64>) {
    let run: Vec<char> = digits.chars().collect();
    if run.len() <= LONGEST_SHOWN_DIGITS {
        shortened.push_str(digits);
        return;
    }
    shortened.extend(run.iter().take(KEPT_DIGITS));
    shortened.push(ELISION);
    shortened.extend(run.iter().skip(run.len() - KEPT_DIGITS));
    counts.push(run.len() as u64);
}

pub fn exact_number_text(number: &Number) -> String {
    number_text(number)
}

pub fn digits_text(number: &Number) -> String {
    match number {
        Number::Integer(integer) => integer_to_decimal(integer),
        Number::Rational(rational) => {
            finite_decimal_text(rational.numerator(), rational.denominator()).unwrap_or_else(|| {
                format!(
                    "{}{RATIONAL_SEPARATOR}{}",
                    integer_to_decimal(rational.numerator()),
                    integer_to_decimal(rational.denominator())
                )
            })
        }
        machine => number_text(machine),
    }
}

pub(crate) fn number_text(number: &Number) -> String {
    match number {
        Number::Integer(integer) => written_by_magnitude(integer_to_decimal(integer), true),
        Number::Rational(rational) => {
            match finite_decimal_text(rational.numerator(), rational.denominator()) {
                Some(decimal) => written_by_magnitude(decimal, false),
                None => format!(
                    "{}{RATIONAL_SEPARATOR}{}",
                    integer_to_decimal(rational.numerator()),
                    integer_to_decimal(rational.denominator())
                ),
            }
        }
        Number::F32(value) => {
            machine_text(f64::from(*value), value.to_string(), format!("{value:e}"))
        }
        Number::F64(value) => machine_text(*value, value.to_string(), format!("{value:e}")),
    }
}

fn machine_text(magnitude_of: f64, plain: String, scientific: String) -> String {
    if magnitude_of.is_nan() {
        return NOT_A_NUMBER.to_owned();
    }
    if magnitude_of.is_infinite() {
        let sign = if magnitude_of.is_sign_negative() {
            NEGATIVE_SIGN.to_string()
        } else {
            String::new()
        };
        return format!("{sign}{INFINITY}");
    }
    let magnitude = magnitude_of.abs();
    if magnitude == 0.0 || (SMALLEST_PLAIN_MAGNITUDE..LARGEST_PLAIN_MAGNITUDE).contains(&magnitude)
    {
        plain
    } else {
        scientific
    }
}

fn written_by_magnitude(plain: String, is_an_integer: bool) -> String {
    let Some((significand, decade)) = significand_and_decade(&plain) else {
        return plain;
    };
    if (PLAIN_LOWEST_DECADE..=PLAIN_HIGHEST_DECADE).contains(&decade) {
        return plain;
    }
    let sign = if plain.starts_with(NEGATIVE_SIGN) {
        NEGATIVE_SIGN.to_string()
    } else {
        String::new()
    };
    let written = format!(
        "{sign}{}",
        power_of_ten_text(&significand, decade, DECIMAL_POINT)
    );
    if is_an_integer && plain.len() < written.len() + EXPONENT_FORM_SAVING {
        return plain;
    }
    written
}

fn significand_and_decade(plain: &str) -> Option<(String, i64)> {
    let magnitude = plain.strip_prefix(NEGATIVE_SIGN).unwrap_or(plain);
    let (whole, fraction) = match magnitude.split_once(DECIMAL_POINT) {
        Some((whole, fraction)) => (whole, fraction),
        None => (magnitude, ""),
    };
    let digits = format!("{whole}{fraction}");
    let first = digits.find(|digit| digit != '0')?;
    let decade = i64::try_from(whole.len()).ok()? - 1 - i64::try_from(first).ok()?;
    let significand = digits.get(first..)?.trim_end_matches('0');
    let significand = if significand.is_empty() {
        digits.get(first..first + 1)?
    } else {
        significand
    };
    Some((significand.to_owned(), decade))
}

fn finite_decimal_text(numerator: &Integer, denominator: &Integer) -> Option<String> {
    let two = Integer::from(2i64);
    let five = Integer::from(5i64);
    let ten = Integer::from(DECIMAL_BASE);
    let mut rest = denominator.clone();
    let mut digits_after_point = 0u32;
    let mut scale = Integer::one();
    for factor in [&two, &five] {
        let mut count = 0u32;
        while let Ok((quotient, remainder)) = rest.div_rem_euclid(factor) {
            if !remainder.is_zero() {
                break;
            }
            rest = quotient;
            count += 1;
        }
        digits_after_point = digits_after_point.max(count);
    }
    if !rest.is_one() {
        return None;
    }
    for _ in 0..digits_after_point {
        scale = &scale * &ten;
    }
    let (scaled, _) = (&numerator.absolute() * &scale)
        .div_rem_euclid(denominator)
        .ok()?;
    let digits = integer_to_decimal(&scaled);
    let point = usize::try_from(digits_after_point).ok()?;
    let padded = format!("{digits:0>width$}", width = point + 1);
    let (whole, fraction) = padded.split_at(padded.len() - point);
    let sign = if numerator.is_negative() {
        NEGATIVE_SIGN.to_string()
    } else {
        String::new()
    };
    Some(format!("{sign}{whole}{DECIMAL_POINT}{fraction}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rational(numerator: i64, denominator: i64) -> ResultValue {
        let value =
            Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap();
        ResultValue::Number(value)
    }

    fn session_measuring(step_nanoseconds: u64, resolution_nanoseconds: u64) -> Session {
        let clock = crate::FixedClock::with_resolution(
            crate::result_record::UtcTimestamp::from_milliseconds_since_unix_epoch(0),
            step_nanoseconds,
            resolution_nanoseconds,
        );
        Session::new(
            Box::new(clock),
            vec![Box::new(calc_exec_cpu::CpuBackend::new())],
        )
    }

    fn measured_run_time(step_nanoseconds: u64, resolution_nanoseconds: u64) -> Option<Duration> {
        let mut session = session_measuring(step_nanoseconds, resolution_nanoseconds);
        let id = session.enter("2 + 3").unwrap();
        let line = session.line(id).unwrap();
        match session.line_summary(line).state {
            LineState::Result(record) => record.measured_run_time,
            _ => panic!("expected a result"),
        }
    }

    #[test]
    fn a_run_time_above_the_clock_resolution_is_measured() {
        assert_eq!(
            measured_run_time(1_000_000, 30),
            Some(Duration::from_nanos(1_000_000))
        );
    }

    #[test]
    fn a_run_time_at_the_clock_resolution_is_left_out() {
        assert_eq!(measured_run_time(30, 30), None);
    }

    fn run_time_text(nanoseconds: u64) -> String {
        let locale = calc_i18n::Locale::source();
        calc_i18n::render(
            &run_time_message(Duration::from_nanos(nanoseconds)),
            &locale,
        )
        .to_string()
    }

    #[test]
    fn a_run_time_below_a_microsecond_is_shown_in_nanoseconds() {
        assert_eq!(run_time_text(420), "420 ns, measured on this run");
    }

    #[test]
    fn a_run_time_below_a_millisecond_is_shown_in_microseconds() {
        assert_eq!(run_time_text(83_400), "83.4 µs, measured on this run");
    }

    #[test]
    fn a_run_time_below_a_second_is_shown_in_milliseconds() {
        assert_eq!(run_time_text(1_235_000), "1.24 ms, measured on this run");
    }

    #[test]
    fn a_run_time_of_seconds_is_shown_in_seconds() {
        assert_eq!(run_time_text(2_050_000_000), "2.05 s, measured on this run");
    }

    #[test]
    fn a_run_time_keeps_three_figures_without_a_point() {
        assert_eq!(run_time_text(123_456), "123 µs, measured on this run");
    }

    fn fraction_of(numerator: &str, denominator: &str) -> ResultValue {
        let numerator = crate::decimal_text::integer_from_decimal(numerator).unwrap();
        let denominator = crate::decimal_text::integer_from_decimal(denominator).unwrap();
        ResultValue::Number(Number::fraction(&numerator, &denominator).unwrap())
    }

    fn whole(digits: &str) -> ResultValue {
        let integer = crate::decimal_text::integer_from_decimal(digits).unwrap();
        ResultValue::Number(Number::Integer(integer))
    }

    #[test]
    fn a_decimal_below_the_window_is_written_in_the_exponent_form() {
        let photon_energy = fraction_of("3313035075", "10000000000000000000000000000");

        assert_eq!(value_text(&photon_energy), "3.313035075e-19");
    }

    #[test]
    fn a_decimal_at_the_bottom_of_the_window_keeps_its_point() {
        assert_eq!(value_text(&rational(1, 1000)), "0.001");
    }

    #[test]
    fn a_decimal_below_the_bottom_of_the_window_takes_the_exponent_form() {
        assert_eq!(value_text(&rational(1, 10_000)), "1e-4");
    }

    #[test]
    fn a_decimal_at_the_top_of_the_window_keeps_its_point() {
        assert_eq!(value_text(&rational(1_999_999, 2)), "999999.5");
    }

    #[test]
    fn an_integer_below_a_million_keeps_its_digits() {
        assert_eq!(
            (value_text(&whole("100000")), value_text(&whole("100001"))),
            ("100000".to_owned(), "100001".to_owned())
        );
    }

    #[test]
    fn a_million_is_written_in_the_exponent_form() {
        assert_eq!(value_text(&whole("1000000")), "1e6");
    }

    #[test]
    fn an_integer_whose_digits_cost_little_keeps_them() {
        assert_eq!(
            (value_text(&whole("1234567")), value_text(&whole("2500000"))),
            ("1234567".to_owned(), "2500000".to_owned())
        );
    }

    #[test]
    fn an_integer_that_is_mostly_zeros_takes_the_exponent_form() {
        assert_eq!(
            value_text(&whole("602214076000000000000000")),
            "6.02214076e23"
        );
    }

    #[test]
    fn a_negative_integer_of_large_magnitude_keeps_its_sign() {
        assert_eq!(value_text(&whole("-1000000")), "-1e6");
    }

    #[test]
    fn zero_is_written_as_zero() {
        assert_eq!(value_text(&whole("0")), "0");
    }

    #[test]
    fn a_fraction_of_small_magnitude_stays_a_fraction() {
        assert_eq!(value_text(&rational(1, 3_000_000)), "1/3000000");
    }

    #[test]
    fn the_digits_view_keeps_the_digits_of_a_small_value() {
        let photon_energy = fraction_of("3313035075", "10000000000000000000000000000");
        let ResultValue::Number(number) = &photon_energy else {
            panic!("expected a number");
        };

        assert_eq!(digits_text(number), "0.0000000000000000003313035075");
    }

    #[test]
    fn a_value_within_the_row_is_not_shortened() {
        assert_eq!(shortened_digits("1267650600228229401496703205376"), None);
    }

    #[test]
    fn a_long_integer_keeps_its_ends_and_names_its_digit_count() {
        let digits = "9".repeat(41);

        let (shortened, counts) = shortened_digits(&digits).unwrap();

        assert_eq!(
            (shortened, counts),
            (
                format!("{}\u{2026}{}", "9".repeat(12), "9".repeat(12)),
                vec![41]
            )
        );
    }

    #[test]
    fn a_whole_number_in_exponent_form_counts_all_its_digits() {
        let text = format!("4.{}e2567", "2".repeat(2318));

        let (_, counts) = shortened_digits(&text).unwrap();

        assert_eq!(counts, vec![2568]);
    }

    #[test]
    fn a_long_fraction_shortens_each_part_and_counts_both() {
        let fraction = format!("{}/{}", "1".repeat(50), "2".repeat(45));

        let (shortened, counts) = shortened_digits(&fraction).unwrap();

        assert_eq!(
            (shortened, counts),
            (
                format!(
                    "{}\u{2026}{}/{}\u{2026}{}",
                    "1".repeat(12),
                    "1".repeat(12),
                    "2".repeat(12),
                    "2".repeat(12)
                ),
                vec![50, 45]
            )
        );
    }

    #[test]
    fn a_short_part_of_a_long_fraction_is_left_whole() {
        let fraction = format!("{}/7", "1".repeat(50));

        let (shortened, counts) = shortened_digits(&fraction).unwrap();

        assert_eq!((shortened.ends_with("/7"), counts), (true, vec![50]));
    }

    #[test]
    fn integer_prints_its_digits() {
        let value = ResultValue::Number(Number::Integer(Integer::from(-42i64)));
        assert_eq!(value_text(&value), "-42");
    }

    #[test]
    fn rational_with_finite_decimal_prints_as_decimal() {
        assert_eq!(value_text(&rational(-1, 8)), "-0.125");
    }

    #[test]
    fn rational_without_finite_decimal_prints_as_fraction() {
        assert_eq!(value_text(&rational(1, 3)), "1/3");
    }

    #[test]
    fn f64_prints_shortest_plain_decimal() {
        let value = ResultValue::Number(Number::F64(24.525000000000002));
        assert_eq!(value_text(&value), "24.525000000000002");
    }

    #[test]
    fn large_f64_prints_in_scientific_form() {
        let value = ResultValue::Number(Number::F64(1e300));
        assert_eq!(value_text(&value), "1e300");
    }

    #[test]
    fn negative_zero_keeps_its_sign() {
        let value = ResultValue::Number(Number::F64(-0.0));
        assert_eq!(value_text(&value), "-0");
    }

    #[test]
    fn negative_infinity_prints_as_minus_inf() {
        let value = ResultValue::Number(Number::F64(f64::NEG_INFINITY));
        assert_eq!(value_text(&value), "-inf");
    }

    #[test]
    fn nan_prints_as_nan() {
        let value = ResultValue::Number(Number::F64(f64::NAN));
        assert_eq!(value_text(&value), "nan");
    }

    #[test]
    fn matrix_prints_rows_separated_by_semicolons() {
        let element = |value: i64| ResultValue::Number(Number::Integer(Integer::from(value)));
        let value = ResultValue::Array {
            shape: vec![2, 2],
            elements: vec![element(1), element(2), element(3), element(4)],
        };
        assert_eq!(value_text(&value), "[1, 2; 3, 4]");
    }

    #[test]
    fn machine_float_kind_maps_to_its_word() {
        assert_eq!(
            kind_message(ResultKind::MachineFloat),
            Message::CommonResultKindMachineFloat
        );
    }

    #[test]
    fn machine_value_of_sixty_four_bits_has_that_form() {
        assert_eq!(
            value_form(
                &ResultValue::Number(Number::F64(1.5)),
                RationalForm::Fraction
            ),
            Some(ValueForm::Machine64)
        );
    }

    #[test]
    fn rational_in_the_fraction_form_is_an_exact_fraction() {
        assert_eq!(
            value_form(&rational(3, 8), RationalForm::Fraction),
            Some(ValueForm::ExactFraction)
        );
    }

    #[test]
    fn rational_in_the_decimal_form_is_an_exact_decimal() {
        assert_eq!(
            value_form(&rational(3, 8), RationalForm::Decimal),
            Some(ValueForm::ExactDecimal)
        );
    }

    #[test]
    fn terminating_rational_in_the_fraction_form_is_written_as_a_fraction() {
        assert_eq!(
            value_text_in(&rational(1, 4), RationalForm::Fraction),
            "1/4"
        );
    }

    #[test]
    fn rational_in_the_decimal_form_is_written_as_its_decimal() {
        assert_eq!(
            value_text_in(&rational(3, 4), RationalForm::Decimal),
            "0.75"
        );
    }

    #[test]
    fn negative_fraction_has_its_sign_first() {
        assert_eq!(
            value_text_in(&rational(-1, 3), RationalForm::Fraction),
            "-1/3"
        );
    }

    #[test]
    fn thirty_two_bit_machine_float_kind_names_its_width() {
        assert_eq!(
            value_kind_message(ResultKind::MachineFloat, Some(ValueForm::Machine32)),
            Message::CommonResultKindMachineFloat32
        );
    }

    #[test]
    fn exact_integer_kind_says_integer() {
        assert_eq!(
            value_kind_message(ResultKind::ExactRational, Some(ValueForm::ExactInteger)),
            Message::CommonResultKindExactInteger
        );
    }

    #[test]
    fn first_order_propagation_names_its_input_count() {
        let mut method = Method::named("exact_evaluation");
        method.parameters.insert(
            PROPAGATION_PARAMETER.to_owned(),
            ParameterValue::Identifier(FIRST_ORDER_PROPAGATION.to_owned()),
        );
        method.parameters.insert(
            INPUT_COUNT_PARAMETER.to_owned(),
            ParameterValue::Value(ResultValue::Number(Number::from(2_i64))),
        );
        assert_eq!(propagated_input_count(&method), Some(2));
    }

    #[test]
    fn exact_rounding_has_no_row_message() {
        assert_eq!(rounding_message(&RoundingSummary::Exact), None);
    }

    #[test]
    fn saved_solve_line_is_summarised_by_what_it_found() {
        let mut solving = crate::session::tests::session();
        let id = solving
            .enter(r#"solve {"phase": "ways", "object": "circle", "wanted": "circumference", "cap": 2}"#)
            .unwrap();
        let summary = solving.line_summary(solving.line(id).unwrap());
        assert!(matches!(
            summary.state,
            LineState::Solve(SolveLineSummary::Ways { found }) if found >= 2
        ));
    }
}
