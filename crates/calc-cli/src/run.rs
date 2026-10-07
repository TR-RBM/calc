use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;

use std::sync::mpsc::channel;

use calc_app::{
    ClaimOutcome, Clock, ComparedState, ExpandOutcome, FactorOutcome, Factorization,
    IDENTITY_FRACTION_BOUND, IDENTITY_FRACTION_DENOMINATORS, IDENTITY_SEARCH_FROM,
    IDENTITY_SEARCH_TO, IdentityOutcome, IdentitySearch, IdentityVerdict, InstrumentFacts,
    IntervalText, Job, JobState, Line, LineId, LineInstrument, LineState, Number, Outcome,
    Primality, PrimeFactor, RangeVerdict, ResultValue, ResultView, SearchOutcome, SearchedValue,
    Session, SessionError, SolveOutcome, SolveRefusal, SystemRefusal, Verdict, ViewEvent,
    applying_instruments, claim_takes_zero_to_the_zero, diagnostic_message,
    diagnostic_message_reading, find_matches, load_error_message, parse_error_json,
    parse_error_json_naming_input, parse_error_message, registered_backends, save_error_message,
    session_error_json_line, session_error_message, unknown_name_json, view_error_message,
    view_outcome_json, view_outcome_json_line,
};
use calc_i18n::{LanguageTag, Locale, LocaleSource, Message, render, resolve_locale};

use crate::arguments::{Input, Invocation, OdeRequest, UsageError, ViewRequest, parse_arguments};
use crate::asm::{ASM_COMMAND, run_asm};
use crate::completion::{COMPLETE_OPTION, run_completion};
use crate::concept::{is_concept_command, run_concept};
use crate::gpu::{ProbeOutcome, Probes, probing_session};
use crate::language::{is_language_help, run_language_help};
use crate::ode::{OdeFault, ode_error, ode_rows};
use crate::output::Output;
use crate::plot::{PLOT_COMMAND, run_plot};
use crate::read::{READ_COMMAND, run_read};
use crate::session_commands::{is_session_command, run_session_command};
use crate::solve::{SOLVE_COMMAND, run_solve};
use crate::text::{
    RowOptions, line_rows, modes_rows, status_row, terse_row, trace_rows, view_rows,
};
use crate::units::{DisplayUnits, UnitsProblem, display_units};

const INSPECT_OPTION: &str = "--inspect";
const METHOD_OPTION: &str = "--method";
const PLOT_OPTION: &str = "calc plot";
const LIST_SEPARATOR: &str = ", ";

pub const EXIT_SUCCESS: u8 = 0;

pub const EXIT_FAILURE: u8 = 1;

pub const EXIT_USAGE: u8 = 2;

pub const EXIT_NOT_A_CLAIM: u8 = 3;

pub const EXIT_NOTHING_REACHED: u8 = 4;

fn status_of(is_success: bool) -> u8 {
    if is_success {
        EXIT_SUCCESS
    } else {
        EXIT_FAILURE
    }
}

fn reached_status(is_success: bool, is_reached: bool) -> u8 {
    match (is_success, is_reached) {
        (false, _) => EXIT_FAILURE,
        (true, false) => EXIT_NOTHING_REACHED,
        (true, true) => EXIT_SUCCESS,
    }
}

fn line_status(state: &LineState) -> u8 {
    match state {
        LineState::Failed(Message::ErrorSortLimitReached { .. }) => EXIT_NOTHING_REACHED,
        LineState::Failed(_) => EXIT_FAILURE,
        _ => EXIT_SUCCESS,
    }
}

fn more_consequential(left: u8, right: u8) -> u8 {
    let rank = |code: u8| match code {
        EXIT_FAILURE => 0,
        EXIT_NOT_A_CLAIM => 1,
        EXIT_NOTHING_REACHED => 2,
        EXIT_SUCCESS | EXIT_USAGE => 3,
        _ => 3,
    };
    if rank(left) <= rank(right) {
        left
    } else {
        right
    }
}

fn verdict_status(outcome: &IdentityOutcome) -> u8 {
    match outcome {
        IdentityOutcome::Decided(IdentityVerdict::HoldsEverywhere)
        | IdentityOutcome::OnlyInstance(Verdict::Holds, _)
        | IdentityOutcome::HoldsWhereDefined(_) => EXIT_SUCCESS,
        IdentityOutcome::Decided(
            IdentityVerdict::FailsEverywhere | IdentityVerdict::NotEverywhere,
        )
        | IdentityOutcome::OnlyInstance(Verdict::Fails, _)
        | IdentityOutcome::Searched(IdentitySearch::Refuted(_))
        | IdentityOutcome::FailsWhereDefined(_) => EXIT_FAILURE,
        IdentityOutcome::Definition => EXIT_SUCCESS,
        IdentityOutcome::UnknownName(_) | IdentityOutcome::Refused(_) => EXIT_FAILURE,
        IdentityOutcome::NotARelation | IdentityOutcome::Unreadable(_) => EXIT_NOT_A_CLAIM,
        IdentityOutcome::Decided(IdentityVerdict::Undecided)
        | IdentityOutcome::OnlyInstance(Verdict::Undecided, _)
        | IdentityOutcome::Searched(_) => EXIT_NOTHING_REACHED,
    }
}

fn claim_status(outcome: &ClaimOutcome) -> u8 {
    match outcome {
        ClaimOutcome::Checked(Verdict::Holds)
        | ClaimOutcome::OverRanges(RangeVerdict::HoldsThroughout)
        | ClaimOutcome::Definition => EXIT_SUCCESS,
        ClaimOutcome::Checked(Verdict::Fails)
        | ClaimOutcome::OverRanges(RangeVerdict::FailsThroughout | RangeVerdict::HoldsOnlyInPart)
        | ClaimOutcome::UnknownName(_)
        | ClaimOutcome::Refused(_) => EXIT_FAILURE,
        ClaimOutcome::Checked(Verdict::Undecided) => EXIT_NOTHING_REACHED,
        ClaimOutcome::NotARelation | ClaimOutcome::Unreadable(_) => EXIT_NOT_A_CLAIM,
    }
}

pub struct Context<'a> {
    pub posix_locale_values: Vec<String>,
    pub read_file: &'a dyn Fn(&Path) -> io::Result<Vec<u8>>,
    pub read_standard_input: &'a dyn Fn() -> io::Result<Vec<u8>>,
    pub write_file: &'a dyn Fn(&Path, &[u8]) -> io::Result<()>,
    pub check_output: &'a dyn Fn(&Path) -> io::Result<()>,
    pub clock: &'a dyn Fn() -> Box<dyn Clock>,
}

enum Failure {
    Reported,
    Output,
}

pub fn run(
    arguments: &[OsString],
    context: &Context<'_>,
    standard_output: &mut dyn Write,
    standard_error: &mut dyn Write,
) -> u8 {
    if arguments
        .first()
        .is_some_and(|first| first == COMPLETE_OPTION)
    {
        return run_completion(arguments, context, standard_output);
    }
    if let Some((first, rest)) = arguments.split_first()
        && first == SOLVE_COMMAND
    {
        return run_solve(rest, context, standard_output, standard_error);
    }
    if let Some((first, rest)) = arguments.split_first()
        && first == PLOT_COMMAND
    {
        return run_plot(rest, context, standard_output, standard_error);
    }
    if let Some((first, rest)) = arguments.split_first()
        && first == READ_COMMAND
    {
        return run_read(rest, context, standard_output, standard_error);
    }
    if arguments.first().is_some_and(|first| first == ASM_COMMAND) {
        return run_asm(arguments, context, standard_output, standard_error);
    }
    if is_concept_command(arguments) {
        return run_concept(arguments, context, standard_output, standard_error);
    }
    if is_language_help(arguments) {
        return run_language_help(arguments, context, standard_output, standard_error);
    }
    if is_session_command(arguments) {
        return run_session_command(arguments, context, standard_output, standard_error);
    }
    let mut output = Output::new(standard_output);
    let mut errors = Output::new(standard_error);
    let invocation = match parse_arguments(arguments) {
        Ok(invocation) => invocation,
        Err(error) => return report_usage_error(&error, arguments, context, &mut errors),
    };
    let locale = locale_asked_for(invocation.locale.as_ref(), context, &mut errors);
    let mut probes = Probes::default();
    let outcome = if invocation.is_version {
        output
            .line(&render(&version_message(), &locale))
            .map(|()| EXIT_SUCCESS)
            .map_err(|_| Failure::Output)
    } else if invocation.is_help {
        output
            .line(&render(&Message::CliHelpPage, &locale))
            .map(|()| EXIT_SUCCESS)
            .map_err(|_| Failure::Output)
    } else {
        match display_units(&invocation.units, &locale) {
            Ok(units) => evaluate(
                &invocation,
                &units,
                context,
                &locale,
                &mut output,
                &mut errors,
                &mut probes,
            ),
            Err(problem) => {
                return report_units_problem(&problem, &locale, &mut errors);
            }
        }
    };
    probes.report(&locale, &mut errors);
    match outcome {
        Ok(code) => code,
        Err(Failure::Reported) => EXIT_FAILURE,
        Err(Failure::Output) => {
            let _ = errors.line(&render(&Message::CliErrorOutputFailed, &locale));
            EXIT_FAILURE
        }
    }
}

pub(crate) fn locale_asked_for(
    requested: Option<&LanguageTag>,
    context: &Context<'_>,
    errors: &mut Output<'_>,
) -> Locale {
    let locale = resolve_locale(&locale_sources(requested, context));
    if let Some(tag) = requested
        && !Locale::is_shipped(tag)
    {
        let shipped = Locale::shipped()
            .map(Locale::tag)
            .collect::<Vec<_>>()
            .join(", ");
        let note = Message::CliNoteUnknownLocale {
            requested: tag.as_str().to_owned(),
            answered: locale.tag().to_owned(),
            shipped,
        };
        let _ = errors.line(&render(&note, &locale));
    }
    locale
}

pub(crate) fn locale_sources<'a>(
    requested: Option<&'a LanguageTag>,
    context: &'a Context<'_>,
) -> Vec<LocaleSource<'a>> {
    requested
        .map(LocaleSource::Tag)
        .into_iter()
        .chain(
            context
                .posix_locale_values
                .iter()
                .map(|value| LocaleSource::Posix(value)),
        )
        .collect()
}

pub(crate) fn usage_error_locale(arguments: &[OsString], context: &Context<'_>) -> Locale {
    let named = arguments.windows(2).find_map(|pair| {
        (pair[0] == crate::arguments::LOCALE_OPTION)
            .then(|| LanguageTag::parse(pair[1].to_str()?).ok())
            .flatten()
    });
    resolve_locale(&locale_sources(named.as_ref(), context))
}

fn report_usage_error(
    error: &UsageError,
    arguments: &[OsString],
    context: &Context<'_>,
    errors: &mut Output<'_>,
) -> u8 {
    let locale = match error {
        UsageError::InvalidLocale(_) => Locale::source(),
        _ => usage_error_locale(arguments, context),
    };
    let detail = render(&error.message(), &locale).to_string();
    let _ = errors
        .line(&render(&Message::CliError { detail }, &locale))
        .and_then(|()| errors.line(&render(&Message::CliHelpUsage, &locale)));
    EXIT_USAGE
}

fn report_units_problem(problem: &UnitsProblem, locale: &Locale, errors: &mut Output<'_>) -> u8 {
    let detail = render(&problem.message(locale), locale).to_string();
    let _ = errors
        .line(&render(&Message::CliError { detail }, locale))
        .and_then(|()| errors.line(&render(&Message::CliHelpUsage, locale)));
    EXIT_USAGE
}

fn report(errors: &mut Output<'_>, message: &Message, locale: &Locale) -> Failure {
    let detail = render(message, locale).to_string();
    match errors.line(&render(&Message::CliError { detail }, locale)) {
        Ok(()) => Failure::Reported,
        Err(_) => Failure::Output,
    }
}

fn holds_a_number(value: &ResultValue) -> bool {
    match value {
        ResultValue::Number(number) => is_a_number(number),
        ResultValue::Complex { real, imaginary } => is_a_number(real) && is_a_number(imaginary),
        ResultValue::Array { elements, .. } => elements.iter().all(holds_a_number),
        ResultValue::Expression(_) => true,
    }
}

fn is_a_number(number: &Number) -> bool {
    match number {
        Number::F32(value) => !value.is_nan(),
        Number::F64(value) => !value.is_nan(),
        Number::Integer(_) | Number::Rational(_) => true,
    }
}

fn is_not_a_number(session: &Session, id: LineId) -> bool {
    match session.line(id).map(Line::outcome) {
        Some(Outcome::Result(record)) => !holds_a_number(record.computed().value()),
        _ => false,
    }
}

fn claim_message(text: &str, outcome: &ClaimOutcome, locale: &Locale) -> Message {
    let claim = text.to_owned();
    match outcome {
        ClaimOutcome::Checked(Verdict::Holds) => Message::CliClaimHolds { claim },
        ClaimOutcome::Checked(Verdict::Fails) => Message::CliClaimFails { claim },
        ClaimOutcome::Checked(Verdict::Undecided) => Message::CliClaimUndecided { claim },
        ClaimOutcome::OverRanges(RangeVerdict::HoldsThroughout) => {
            Message::CliClaimHoldsThroughout { claim }
        }
        ClaimOutcome::OverRanges(RangeVerdict::FailsThroughout) => {
            Message::CliClaimFailsThroughout { claim }
        }
        ClaimOutcome::OverRanges(RangeVerdict::HoldsOnlyInPart) => {
            Message::CliClaimHoldsInPart { claim }
        }
        ClaimOutcome::Definition => Message::CliClaimDefinition { claim },
        ClaimOutcome::UnknownName(diagnostic) => Message::CliClaimUnknownName {
            claim,
            detail: render(&diagnostic_message(diagnostic), locale).to_string(),
        },
        ClaimOutcome::Refused(diagnostic) => Message::CliClaimRefused {
            detail: render(&diagnostic_message_reading(diagnostic, &claim), locale).to_string(),
        },
        ClaimOutcome::NotARelation => Message::CliClaimNotARelation { claim },
        ClaimOutcome::Unreadable(error) => Message::CliClaimUnreadable {
            claim,
            detail: render(&parse_error_message(error, text), locale).to_string(),
        },
    }
}

const NO_ERROR: &str = "null";

const REAL_ROOT_CALL: &str = "rootof(";

fn claim_json(text: &str, outcome: &ClaimOutcome, _locale: &Locale) -> String {
    let verdict = match outcome {
        ClaimOutcome::Checked(Verdict::Holds) => "holds",
        ClaimOutcome::Checked(Verdict::Fails) => "fails",
        ClaimOutcome::Checked(Verdict::Undecided) => "undecided",
        ClaimOutcome::OverRanges(RangeVerdict::HoldsThroughout) => "holds_throughout",
        ClaimOutcome::OverRanges(RangeVerdict::FailsThroughout) => "fails_throughout",
        ClaimOutcome::OverRanges(RangeVerdict::HoldsOnlyInPart) => "holds_in_part",
        ClaimOutcome::Definition => "definition",
        ClaimOutcome::UnknownName(_) => "unknown_name",
        ClaimOutcome::Refused(_) => "refused",
        ClaimOutcome::NotARelation => "not_a_relation",
        ClaimOutcome::Unreadable(_) => "unreadable",
    };
    let escaped: String = text
        .chars()
        .flat_map(|character| match character {
            '"' => vec!['\\', '"'],
            '\\' => vec!['\\', '\\'],
            other => vec![other],
        })
        .collect();
    let error = match outcome {
        ClaimOutcome::Unreadable(error) => {
            String::from_utf8(parse_error_json(error, text)).unwrap_or_else(|_| NO_ERROR.to_owned())
        }
        ClaimOutcome::UnknownName(diagnostic) | ClaimOutcome::Refused(diagnostic) => {
            String::from_utf8(unknown_name_json(diagnostic)).unwrap_or_else(|_| NO_ERROR.to_owned())
        }
        _ => NO_ERROR.to_owned(),
    };
    let error = error.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{{\"claim\": \"{escaped}\", \"verdict\": \"{verdict}\", \"error\": {error}}}")
}

fn version_message() -> Message {
    Message::CliVersion {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        commit: env!("CALC_COMMIT").to_owned(),
        target: env!("CALC_TARGET").to_owned(),
        format: calc_app::FORMAT_VERSION.to_string(),
    }
}

fn quoted(text: &str) -> String {
    let escaped: String = text
        .chars()
        .flat_map(|character| match character {
            '"' => vec!['\\', '"'],
            '\\' => vec!['\\', '\\'],
            other => vec![other],
        })
        .collect();
    format!("\"{escaped}\"")
}

fn flattened(written: &[u8]) -> String {
    String::from_utf8_lossy(written)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn witness_json(witness: &[(String, SearchedValue)]) -> String {
    let members: Vec<String> = witness
        .iter()
        .map(|(name, value)| format!("{}: {}", quoted(name), quoted(&value.to_string())))
        .collect();
    format!("{{{}}}", members.join(", "))
}

pub(crate) fn where_not_zero(excluding: &[String], locale: &Locale) -> Message {
    Message::CommonWhereNotZero {
        count: excluding.len() as u64,
        denominators: denominators_text(excluding, locale),
    }
}

fn denominators_text(denominators: &[String], locale: &Locale) -> String {
    let Some((last, head)) = denominators.split_last() else {
        return String::new();
    };
    if head.is_empty() {
        return last.clone();
    }
    render(
        &Message::CliIdentityExclusionList {
            head: head.join(LIST_SEPARATOR),
            last: last.clone(),
        },
        locale,
    )
    .to_string()
}

fn substitution_text(substituted: &[(String, String)]) -> String {
    substituted
        .iter()
        .map(|(name, value)| format!("{name} = {value}"))
        .collect::<Vec<String>>()
        .join(LIST_SEPARATOR)
}

fn witness_text(witness: &[(String, SearchedValue)]) -> String {
    witness
        .iter()
        .map(|(name, value)| format!("{name} = {value}"))
        .collect::<Vec<String>>()
        .join(", ")
}

fn searched_range() -> String {
    format!("{IDENTITY_SEARCH_FROM}..{IDENTITY_SEARCH_TO}")
}

fn fraction_denominators() -> String {
    IDENTITY_FRACTION_DENOMINATORS
        .iter()
        .map(i64::to_string)
        .collect::<Vec<String>>()
        .join(", ")
}

fn denominator_at(position: Option<&i64>) -> String {
    position.map(i64::to_string).unwrap_or_default()
}

fn searched_message(with_fractions: bool) -> Message {
    let from = IDENTITY_SEARCH_FROM.to_string();
    let to = IDENTITY_SEARCH_TO.to_string();
    if with_fractions {
        Message::CliIdentityUndecidedSearchedFractions {
            from,
            to,
            smallest: denominator_at(IDENTITY_FRACTION_DENOMINATORS.first()),
            largest: denominator_at(IDENTITY_FRACTION_DENOMINATORS.last()),
            bound: IDENTITY_FRACTION_BOUND.to_string(),
        }
    } else {
        Message::CliIdentityUndecidedSearched { from, to }
    }
}

fn identity_json(claim: &str, outcome: &IdentityOutcome) -> String {
    let (verdict, member) = match outcome {
        IdentityOutcome::Decided(IdentityVerdict::HoldsEverywhere) => {
            ("holds_everywhere", String::new())
        }
        IdentityOutcome::Decided(IdentityVerdict::FailsEverywhere) => {
            ("fails_everywhere", String::new())
        }
        IdentityOutcome::Decided(IdentityVerdict::NotEverywhere) => {
            ("not_an_identity", String::new())
        }
        IdentityOutcome::Decided(IdentityVerdict::Undecided) => ("undecided", String::new()),
        IdentityOutcome::Searched(IdentitySearch::Refuted(witness)) => (
            "not_an_identity",
            format!(", \"witness\": {}", witness_json(witness)),
        ),
        IdentityOutcome::Searched(IdentitySearch::NothingFound) => (
            "undecided",
            format!(", \"searched\": {}", quoted(&searched_range())),
        ),
        IdentityOutcome::Searched(IdentitySearch::NothingFoundWithFractions) => (
            "undecided",
            format!(
                ", \"searched\": {}, \"fractions\": {{\"denominators\": [{}], \"bound\": {}}}",
                quoted(&searched_range()),
                fraction_denominators(),
                IDENTITY_FRACTION_BOUND
            ),
        ),
        IdentityOutcome::Searched(IdentitySearch::Stopped(at)) => (
            "undecided",
            format!(
                ", \"searched\": {}, \"stopped_at\": {}",
                quoted("stopped"),
                witness_json(at)
            ),
        ),
        IdentityOutcome::Searched(IdentitySearch::NotRun) => (
            "undecided",
            format!(", \"searched\": {}", quoted("not_run")),
        ),
        IdentityOutcome::Searched(IdentitySearch::NothingToVary) => ("undecided", String::new()),
        IdentityOutcome::OnlyInstance(Verdict::Holds, _) => ("holds_everywhere", String::new()),
        IdentityOutcome::OnlyInstance(..) => ("fails_everywhere", String::new()),
        IdentityOutcome::FailsWhereDefined(denominators) => (
            "fails_where_defined",
            format!(
                ", \"excluding\": [{}]",
                denominators
                    .iter()
                    .map(|denominator| quoted(denominator))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        ),
        IdentityOutcome::HoldsWhereDefined(denominators) => (
            "holds_where_defined",
            format!(
                ", \"excluding\": [{}]",
                denominators
                    .iter()
                    .map(|denominator| quoted(denominator))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        ),
        IdentityOutcome::Definition => ("definition", String::new()),
        IdentityOutcome::UnknownName(diagnostic) => (
            "unknown_name",
            format!(", \"error\": {}", flattened(&unknown_name_json(diagnostic))),
        ),
        IdentityOutcome::Refused(diagnostic) => (
            "refused",
            format!(", \"error\": {}", flattened(&unknown_name_json(diagnostic))),
        ),
        IdentityOutcome::NotARelation => ("not_a_relation", String::new()),
        IdentityOutcome::Unreadable(_) => ("unreadable", String::new()),
    };
    format!(
        "{{\"claim\": {}, \"verdict\": {}{member}}}",
        quoted(claim),
        quoted(verdict)
    )
}

fn expansion_json(text: &str, outcome: &ExpandOutcome) -> String {
    match outcome {
        ExpandOutcome::Expanded(expression) => format!(
            "{{\"entry\": {}, \"expanded\": {}}}",
            quoted(text),
            quoted(expression)
        ),
        ExpandOutcome::ExpandedWhere(expression, excluding) => format!(
            "{{\"entry\": {}, \"expanded\": {}, \"excluding\": [{}]}}",
            quoted(text),
            quoted(expression),
            excluding
                .iter()
                .map(|denominator| quoted(denominator))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ExpandOutcome::DividesByZero(reading) => format!(
            "{{\"entry\": {}, \"refused\": {}, \"reading\": {}}}",
            quoted(text),
            quoted("division_by_zero"),
            quoted(reading)
        ),
        ExpandOutcome::Definition(name) => format!(
            "{{\"entry\": {}, \"definition\": {}}}",
            quoted(text),
            quoted(name)
        ),
        ExpandOutcome::NotAlgebraic => format!(
            "{{\"entry\": {}, \"refused\": {}}}",
            quoted(text),
            quoted("not_algebraic")
        ),
        ExpandOutcome::Unreadable(_) => format!(
            "{{\"entry\": {}, \"refused\": {}}}",
            quoted(text),
            quoted("unreadable")
        ),
    }
}

fn solution_json(equation: &str, outcome: &SolveOutcome) -> String {
    match outcome {
        SolveOutcome::Solved(roots) => {
            let listed: Vec<String> = roots.iter().map(|root| quoted(root)).collect();
            format!(
                "{{\"equation\": {}, \"solutions\": [{}]}}",
                quoted(equation),
                listed.join(", ")
            )
        }
        SolveOutcome::SolvedRealOnly(roots) => {
            let listed: Vec<String> = roots.iter().map(|root| quoted(root)).collect();
            format!(
                "{{\"equation\": {}, \"solutions\": [{}], \"non_real_left_out\": true}}",
                quoted(equation),
                listed.join(", ")
            )
        }
        SolveOutcome::SolvedSystem { values, free } => {
            let listed: Vec<String> = values
                .iter()
                .map(|(name, value)| format!("{}: {}", quoted(name), quoted(value)))
                .collect();
            let free: Vec<String> = free.iter().map(|name| quoted(name)).collect();
            format!(
                "{{\"equation\": {}, \"solutions\": {{{}}}, \"free\": [{}]}}",
                quoted(equation),
                listed.join(", "),
                free.join(", ")
            )
        }
        SolveOutcome::SolvedInequality { intervals, .. } => {
            let end = |end: &Option<(String, bool)>| match end {
                Some((value, included)) => (quoted(value), included.to_string()),
                None => ("null".to_string(), "null".to_string()),
            };
            let listed: Vec<String> = intervals
                .iter()
                .map(|interval| {
                    let (from, from_included) = end(&interval.from);
                    let (to, to_included) = end(&interval.to);
                    format!(
                        "{{\"from\": {from}, \"from_included\": {from_included}, \"to\": {to}, \"to_included\": {to_included}}}"
                    )
                })
                .collect();
            format!(
                "{{\"equation\": {}, \"intervals\": [{}]}}",
                quoted(equation),
                listed.join(", ")
            )
        }
        SolveOutcome::SystemWithoutSolution => format!(
            "{{\"equation\": {}, \"solutions\": null, \"free\": []}}",
            quoted(equation)
        ),
        SolveOutcome::RefusedSystem(refusal) => format!(
            "{{\"equation\": {}, \"refused\": {}, \"detail\": {}}}",
            quoted(equation),
            quoted(match refusal {
                SystemRefusal::NotLinear => "not_linear",
                SystemRefusal::NotAnEquation => "not_an_equation",
            }),
            quoted("")
        ),
        SolveOutcome::Definition(name) => format!(
            "{{\"equation\": {}, \"definition\": {}}}",
            quoted(equation),
            quoted(name)
        ),
        SolveOutcome::Refused(SolveRefusal::EveryNumber) => format!(
            "{{\"equation\": {}, \"every_number\": true}}",
            quoted(equation)
        ),
        SolveOutcome::Refused(refusal) => {
            let (reason, detail) = match refusal {
                SolveRefusal::NoName => ("no_name", String::new()),
                SolveRefusal::SeveralNames(names) => ("several_names", names.join(", ")),
                SolveRefusal::EveryNumber => ("every_number", String::new()),
                SolveRefusal::DegreeTooHigh(degree) => ("degree_too_high", degree.to_string()),
                SolveRefusal::RadicalCoefficients(degree) => {
                    ("radical_coefficients", degree.to_string())
                }
                SolveRefusal::NotAPolynomial => ("not_a_polynomial", String::new()),
                SolveRefusal::SquarePartUnknown(number) => {
                    ("square_part_unknown", integer_text(number))
                }
                SolveRefusal::ConstantCoefficients(names) => {
                    ("constant_coefficients", names.join(", "))
                }
                SolveRefusal::ConstantCoefficientDegree(degree) => {
                    ("constant_coefficient_degree", degree.to_string())
                }
                SolveRefusal::UnprovenCoefficient => ("unproven_coefficient", String::new()),
                SolveRefusal::CoefficientOutsideTheField(_) => ("not_a_polynomial", String::new()),
            };
            format!(
                "{{\"equation\": {}, \"refused\": {}, \"detail\": {}}}",
                quoted(equation),
                quoted(reason),
                quoted(&detail)
            )
        }
        SolveOutcome::Unreadable(_) => format!(
            "{{\"equation\": {}, \"refused\": {}, \"detail\": {}}}",
            quoted(equation),
            quoted("unreadable"),
            quoted("")
        ),
    }
}

fn assignment_text(ranges: &[(String, i64, i64)], values: &[i64]) -> String {
    ranges
        .iter()
        .zip(values)
        .map(|((name, _, _), value)| format!("{name} = {value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn claim_lines(
    invocation: &Invocation,
    locale: &Locale,
    errors: &mut Output<'_>,
) -> Result<Vec<String>, Failure> {
    if invocation.is_batch {
        return Ok(std::io::read_to_string(std::io::stdin())
            .map_err(|_| Failure::Output)?
            .lines()
            .map(|line| line.trim().to_owned())
            .filter(|line| !line.is_empty())
            .collect());
    }
    match &invocation.input {
        Some(Input::Expression(text)) => Ok(vec![text.clone()]),
        _ => Err(report(errors, &Message::CliErrorNoInput, locale)),
    }
}

fn asking_session(context: &Context<'_>, probes: &mut Probes) -> Session {
    let (session, probe) = probing_session(Session::new((context.clock)(), registered_backends()));
    probes.watch(probe);
    session
}

fn run_identity(
    invocation: &Invocation,
    context: &Context<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
    errors: &mut Output<'_>,
    probes: &mut Probes,
) -> Result<u8, Failure> {
    let lines = claim_lines(invocation, locale, errors)?;
    let mut session = asking_session(context, probes);
    let mut status = EXIT_SUCCESS;
    for claim in lines {
        let outcome = calc_app::identity_of(&mut session, &claim);
        status = more_consequential(status, verdict_status(&outcome));
        let noted = claim_takes_zero_to_the_zero(&claim);
        if invocation.is_json {
            output
                .json_record(with_claim_notes(identity_json(&claim, &outcome), noted).as_bytes())
                .map_err(|_| Failure::Output)?;
            continue;
        }
        let message = match outcome {
            IdentityOutcome::Decided(IdentityVerdict::HoldsEverywhere) => {
                Message::CliIdentityHoldsEverywhere
            }
            IdentityOutcome::Decided(IdentityVerdict::FailsEverywhere) => {
                Message::CliIdentityFailsEverywhere
            }
            IdentityOutcome::Decided(IdentityVerdict::NotEverywhere) => {
                Message::CliIdentityNotEverywhere
            }
            IdentityOutcome::Decided(IdentityVerdict::Undecided) => searched_message(false),
            IdentityOutcome::Searched(IdentitySearch::Refuted(witness)) => {
                Message::CliIdentityRefuted {
                    witness: witness_text(&witness),
                }
            }
            IdentityOutcome::Searched(IdentitySearch::NothingFound) => searched_message(false),
            IdentityOutcome::Searched(IdentitySearch::NothingFoundWithFractions) => {
                searched_message(true)
            }
            IdentityOutcome::Searched(IdentitySearch::Stopped(at)) => {
                Message::CliIdentityUndecidedStopped {
                    witness: witness_text(&at),
                }
            }
            IdentityOutcome::Searched(IdentitySearch::NotRun) => {
                Message::CliIdentityUndecidedNotSearched
            }
            IdentityOutcome::Searched(IdentitySearch::NothingToVary) => {
                Message::CliIdentityUndecided
            }
            IdentityOutcome::FailsWhereDefined(denominators) => {
                Message::CliIdentityFailsWhereDefined {
                    count: denominators.len() as u64,
                    denominators: denominators_text(&denominators, locale),
                }
            }
            IdentityOutcome::HoldsWhereDefined(denominators) => {
                Message::CliIdentityHoldsWhereDefined {
                    count: denominators.len() as u64,
                    denominators: denominators_text(&denominators, locale),
                }
            }
            IdentityOutcome::Definition => Message::CliClaimDefinition {
                claim: claim.clone(),
            },
            IdentityOutcome::UnknownName(diagnostic) => Message::CliClaimUnknownName {
                claim: claim.clone(),
                detail: render(&diagnostic_message(&diagnostic), locale).to_string(),
            },
            IdentityOutcome::Refused(diagnostic) => Message::CliClaimRefused {
                detail: render(&diagnostic_message_reading(&diagnostic, &claim), locale)
                    .to_string(),
            },
            IdentityOutcome::OnlyInstance(Verdict::Holds, substituted)
                if substituted.is_empty() =>
            {
                Message::CliIdentityHoldsOnlyInstance
            }
            IdentityOutcome::OnlyInstance(Verdict::Holds, substituted) => {
                Message::CliIdentityHoldsWith {
                    substitution: substitution_text(&substituted),
                }
            }
            IdentityOutcome::OnlyInstance(_, substituted) if substituted.is_empty() => {
                Message::CliIdentityFailsOnlyInstance
            }
            IdentityOutcome::OnlyInstance(_, substituted) => Message::CliIdentityFailsWith {
                substitution: substitution_text(&substituted),
            },
            IdentityOutcome::NotARelation => Message::CliClaimNotARelation {
                claim: claim.clone(),
            },
            IdentityOutcome::Unreadable(error) => Message::CliClaimUnreadable {
                claim: claim.clone(),
                detail: render(&parse_error_message(&error, &claim), locale).to_string(),
            },
        };
        output
            .line(&render(&message, locale))
            .map_err(|_| Failure::Output)?;
        write_claim_note(output, noted, locale)?;
    }
    Ok(status)
}

fn run_ode(
    request: &OdeRequest,
    is_json: bool,
    locale: &Locale,
    output: &mut Output<'_>,
    errors: &mut Output<'_>,
) -> Result<u8, Failure> {
    match calc_app::solve_ode(&request.system, &request.initial, &request.at) {
        Ok(report) => {
            if is_json {
                output
                    .json(&calc_app::ode_report_json(&request.system, &report))
                    .map_err(|_| Failure::Output)?;
            } else {
                for row in ode_rows(&report, locale) {
                    output.line(&row).map_err(|_| Failure::Output)?;
                }
            }
            Ok(EXIT_SUCCESS)
        }
        Err(error) => {
            let (message, fault) = ode_error(&error, locale);
            let failure = report(errors, &message, locale);
            match (fault, failure) {
                (_, Failure::Output) => Err(Failure::Output),
                (OdeFault::Input, _) => Ok(EXIT_NOT_A_CLAIM),
                (OdeFault::Method, _) => Ok(EXIT_NOTHING_REACHED),
                (OdeFault::Calc, _) => Ok(EXIT_FAILURE),
            }
        }
    }
}

fn run_solve_for(
    invocation: &Invocation,
    context: &Context<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
    errors: &mut Output<'_>,
    probes: &mut Probes,
) -> Result<u8, Failure> {
    let lines = claim_lines(invocation, locale, errors)?;
    let mut session = asking_session(context, probes);
    let mut all_solved = true;
    for equation in lines {
        let outcome = calc_app::solutions_of(&mut session, &equation);
        if invocation.is_json {
            if !matches!(
                outcome,
                SolveOutcome::Solved(_)
                    | SolveOutcome::SolvedRealOnly(_)
                    | SolveOutcome::Refused(SolveRefusal::EveryNumber)
                    | SolveOutcome::SolvedInequality { .. }
                    | SolveOutcome::SolvedSystem { .. }
                    | SolveOutcome::SystemWithoutSolution
                    | SolveOutcome::Definition(_)
            ) {
                all_solved = false;
            }
            output
                .json_record(solution_json(&equation, &outcome).as_bytes())
                .map_err(|_| Failure::Output)?;
            continue;
        }
        let message = match outcome {
            SolveOutcome::Definition(_) => Message::CliClaimDefinition {
                claim: equation.clone(),
            },
            SolveOutcome::SolvedSystem { values, free } => {
                let values = values
                    .iter()
                    .map(|(name, value)| format!("{name} = {value}"))
                    .collect::<Vec<String>>()
                    .join(", ");
                if free.is_empty() {
                    Message::CliSolvedSystem { values }
                } else {
                    Message::CliSolvedSystemFree {
                        values,
                        free: free.join(", "),
                    }
                }
            }
            SolveOutcome::SystemWithoutSolution => Message::CliSolvedSystemNothing,
            SolveOutcome::SolvedInequality { name, intervals } => {
                inequality_message(&name, &intervals, locale)
            }
            SolveOutcome::RefusedSystem(refusal) => {
                all_solved = false;
                match refusal {
                    SystemRefusal::NotLinear => Message::CliSolveSystemNotLinear,
                    SystemRefusal::NotAnEquation => Message::CliSolveSystemNotEquations,
                }
            }
            SolveOutcome::Solved(roots) if roots.is_empty() => Message::CliSolvedNothing,
            SolveOutcome::Solved(roots) => Message::CliSolved {
                solutions: roots.join(", "),
            },
            SolveOutcome::SolvedRealOnly(roots) if roots.is_empty() => {
                Message::CliSolvedNothingReal
            }
            SolveOutcome::SolvedRealOnly(roots) => Message::CliSolvedRealOnly {
                solutions: roots.join(", "),
            },
            SolveOutcome::Refused(refusal) => {
                all_solved &= refusal == SolveRefusal::EveryNumber;
                match refusal {
                    SolveRefusal::NoName => Message::CliSolveNoName,
                    SolveRefusal::SeveralNames(names) => Message::CliSolveSeveralNames {
                        names: names.join(", "),
                    },
                    SolveRefusal::EveryNumber => Message::CliSolveEveryNumber,
                    SolveRefusal::DegreeTooHigh(degree) => Message::CliSolveDegreeTooHigh {
                        degree: degree.to_string(),
                    },
                    SolveRefusal::RadicalCoefficients(degree) => {
                        Message::CliSolveRadicalCoefficients {
                            degree: degree.to_string(),
                        }
                    }
                    SolveRefusal::NotAPolynomial => Message::CliSolveNotAPolynomial,
                    SolveRefusal::ConstantCoefficients(names) => {
                        Message::CliSolveConstantCoefficients {
                            names: names.join(", "),
                        }
                    }
                    SolveRefusal::ConstantCoefficientDegree(degree) => {
                        Message::CliSolveConstantCoefficientDegree {
                            degree: degree.to_string(),
                        }
                    }
                    SolveRefusal::UnprovenCoefficient => Message::CliSolveUnprovenCoefficient,
                    SolveRefusal::CoefficientOutsideTheField(_) => Message::CliSolveNotAPolynomial,
                    SolveRefusal::SquarePartUnknown(number) => Message::CliSolveSquarePartUnknown {
                        number: integer_text(&number),
                    },
                }
            }
            SolveOutcome::Unreadable(error) => {
                all_solved = false;
                Message::CliClaimUnreadable {
                    claim: equation.clone(),
                    detail: render(&parse_error_message(&error, &equation), locale).to_string(),
                }
            }
        };
        output
            .line(&render(&message, locale))
            .map_err(|_| Failure::Output)?;
        if names_a_real_root(&message) {
            output
                .line(&render(&Message::CliSolvedRootOfNote, locale))
                .map_err(|_| Failure::Output)?;
        }
    }
    Ok(status_of(all_solved))
}

fn names_a_real_root(message: &Message) -> bool {
    match message {
        Message::CliSolved { solutions } | Message::CliSolvedRealOnly { solutions } => {
            solutions.contains(REAL_ROOT_CALL)
        }
        _ => false,
    }
}

fn interval_text(name: &str, interval: &IntervalText) -> String {
    let relation = |included: bool, below: bool| match (included, below) {
        (true, true) => "<=",
        (false, true) => "<",
        (true, false) => ">=",
        (false, false) => ">",
    };
    match (&interval.from, &interval.to) {
        (Some((from, true)), Some((to, true))) if from == to => format!("{name} = {from}"),
        (None, Some((to, included))) => format!("{name} {} {to}", relation(*included, true)),
        (Some((from, included)), None) => format!("{name} {} {from}", relation(*included, false)),
        (Some((from, from_included)), Some((to, to_included))) => format!(
            "{from} {} {name} {} {to}",
            relation(*from_included, true),
            relation(*to_included, true)
        ),
        (None, None) => name.to_string(),
    }
}

fn inequality_message(name: &str, intervals: &[IntervalText], locale: &Locale) -> Message {
    match intervals {
        [] => Message::CliSolvedInequalityNone,
        [only] if only.from.is_none() && only.to.is_none() => Message::CliSolvedInequalityEvery,
        _ => {
            let joining = format!(" {} ", render(&Message::CliInequalityOr, locale));
            Message::CliSolvedInequality {
                intervals: intervals
                    .iter()
                    .map(|interval| interval_text(name, interval))
                    .collect::<Vec<_>>()
                    .join(&joining),
            }
        }
    }
}

fn integer_text(value: &calc_app::Integer) -> String {
    calc_app::value_text(&ResultValue::Number(Number::Integer(value.clone())))
}

fn factor_power(factor: &PrimeFactor) -> String {
    match factor.exponent.abs() {
        1 => integer_text(&factor.base),
        power => format!("{}^{power}", integer_text(&factor.base)),
    }
}

fn factored_text(factorization: &Factorization) -> String {
    if factorization.is_zero {
        return "0".to_string();
    }
    let above: Vec<String> = factorization
        .factors
        .iter()
        .filter(|factor| factor.exponent > 0)
        .map(factor_power)
        .collect();
    let below: Vec<String> = factorization
        .factors
        .iter()
        .filter(|factor| factor.exponent < 0)
        .map(factor_power)
        .collect();
    let mut text = if above.is_empty() {
        "1".to_string()
    } else {
        above.join(" * ")
    };
    match below.len() {
        0 => {}
        1 => text = format!("{text} / {}", below[0]),
        _ => text = format!("{text} / ({})", below.join(" * ")),
    }
    if factorization.is_negative {
        text = format!("-{text}");
    }
    text
}

fn factor_json(text: &str, outcome: &FactorOutcome) -> String {
    match outcome {
        FactorOutcome::Factored(factorization) => {
            let factors: Vec<String> = factorization
                .factors
                .iter()
                .map(|factor| {
                    format!(
                        "{{\"base\": {}, \"exponent\": {}, \"status\": {}}}",
                        quoted(&integer_text(&factor.base)),
                        factor.exponent,
                        quoted(match factor.primality {
                            Primality::Proven => "prime",
                            Primality::Probable => "probably_prime",
                            Primality::NotSplit => "not_split",
                        })
                    )
                })
                .collect();
            format!(
                "{{\"entry\": {}, \"negative\": {}, \"zero\": {}, \"factors\": [{}]}}",
                quoted(text),
                factorization.is_negative,
                factorization.is_zero,
                factors.join(", ")
            )
        }
        FactorOutcome::FactoredPolynomial(factored) => format!(
            "{{\"entry\": {}, \"factored\": {}}}",
            quoted(text),
            quoted(factored)
        ),
        FactorOutcome::Definition(name) => format!(
            "{{\"entry\": {}, \"definition\": {}}}",
            quoted(text),
            quoted(name)
        ),
        FactorOutcome::NotRational => format!(
            "{{\"entry\": {}, \"refused\": {}}}",
            quoted(text),
            quoted("not_rational")
        ),
        FactorOutcome::Unreadable(_) => format!(
            "{{\"entry\": {}, \"refused\": {}}}",
            quoted(text),
            quoted("unreadable")
        ),
    }
}

fn run_factor(
    invocation: &Invocation,
    context: &Context<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
    errors: &mut Output<'_>,
    probes: &mut Probes,
) -> Result<u8, Failure> {
    let lines = claim_lines(invocation, locale, errors)?;
    let mut session = asking_session(context, probes);
    let mut all_factored = true;
    for text in lines {
        let outcome = calc_app::factorization_of(&mut session, &text);
        let factored = match &outcome {
            FactorOutcome::Factored(factorization) => factorization
                .factors
                .iter()
                .all(|factor| factor.primality != Primality::NotSplit),
            FactorOutcome::Definition(_) | FactorOutcome::FactoredPolynomial(_) => true,
            FactorOutcome::NotRational | FactorOutcome::Unreadable(_) => false,
        };
        all_factored &= factored;
        if invocation.is_json {
            output
                .json_record(factor_json(&text, &outcome).as_bytes())
                .map_err(|_| Failure::Output)?;
            continue;
        }
        let messages = match outcome {
            FactorOutcome::Factored(factorization) => {
                let mut messages = vec![Message::CliFactored {
                    factors: factored_text(&factorization),
                }];
                for factor in &factorization.factors {
                    match factor.primality {
                        Primality::Proven => {}
                        Primality::Probable => messages.push(Message::CliFactorProbable {
                            factor: integer_text(&factor.base),
                        }),
                        Primality::NotSplit => messages.push(Message::CliFactorNotSplit {
                            factor: integer_text(&factor.base),
                        }),
                    }
                }
                messages
            }
            FactorOutcome::FactoredPolynomial(factors) => vec![Message::CliFactored { factors }],
            FactorOutcome::Definition(_) => vec![Message::CliClaimDefinition {
                claim: text.clone(),
            }],
            FactorOutcome::NotRational => vec![Message::CliFactorNotRational],
            FactorOutcome::Unreadable(error) => vec![Message::CliClaimUnreadable {
                claim: text.clone(),
                detail: render(&parse_error_message(&error, &text), locale).to_string(),
            }],
        };
        for message in messages {
            output
                .line(&render(&message, locale))
                .map_err(|_| Failure::Output)?;
        }
    }
    Ok(status_of(all_factored))
}

fn run_expand(
    invocation: &Invocation,
    context: &Context<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
    errors: &mut Output<'_>,
    probes: &mut Probes,
) -> Result<u8, Failure> {
    let lines = claim_lines(invocation, locale, errors)?;
    let mut session = asking_session(context, probes);
    let mut all_expanded = true;
    for text in lines {
        let outcome = calc_app::expansion_of(&mut session, &text);
        if invocation.is_json {
            if !matches!(
                outcome,
                ExpandOutcome::Expanded(_)
                    | ExpandOutcome::ExpandedWhere(..)
                    | ExpandOutcome::Definition(_)
            ) {
                all_expanded = false;
            }
            output
                .json_record(expansion_json(&text, &outcome).as_bytes())
                .map_err(|_| Failure::Output)?;
            continue;
        }
        let message = match outcome {
            ExpandOutcome::Definition(_) => Message::CliClaimDefinition {
                claim: text.clone(),
            },
            ExpandOutcome::Expanded(expression) => Message::CliExpanded { expression },
            ExpandOutcome::ExpandedWhere(expression, excluding) => Message::CliExpandedWhere {
                expression,
                condition: render(&where_not_zero(&excluding, locale), locale).to_string(),
            },
            ExpandOutcome::DividesByZero(reading) => {
                all_expanded = false;
                Message::ErrorDivisionByZero { reading }
            }
            ExpandOutcome::NotAlgebraic => {
                all_expanded = false;
                Message::CliNotAlgebraic
            }
            ExpandOutcome::Unreadable(error) => {
                all_expanded = false;
                Message::CliClaimUnreadable {
                    claim: text.clone(),
                    detail: render(&parse_error_message(&error, &text), locale).to_string(),
                }
            }
        };
        output
            .line(&render(&message, locale))
            .map_err(|_| Failure::Output)?;
    }
    Ok(status_of(all_expanded))
}

fn assignment_json(ranges: &[(String, i64, i64)], values: &[i64]) -> String {
    let members: Vec<String> = ranges
        .iter()
        .zip(values)
        .map(|((name, _, _), value)| format!("{}: {}", quoted(name), quoted(&value.to_string())))
        .collect();
    format!("{{{}}}", members.join(", "))
}

fn find_json(
    claim: &str,
    ranges: &[(String, i64, i64)],
    found: &[Vec<i64>],
    undecided: Option<&calc_app::UndecidedPoints>,
    is_counter: bool,
) -> String {
    let listed: Vec<String> = found
        .iter()
        .map(|assignment| assignment_json(ranges, assignment))
        .collect();
    let searched: Vec<String> = ranges
        .iter()
        .map(|(name, from, to)| quoted(&format!("{name}={from}..{to}")))
        .collect();
    let undecided = match undecided {
        Some(points) => format!(
            "{{\"count\": {}, \"first\": {}}}",
            points.count,
            assignment_json(ranges, &points.first)
        ),
        None => "null".to_owned(),
    };
    format!(
        "{{\"claim\": {}, \"searched\": [{}], \"asked\": {}, \"found\": [{}], \"undecided\": {}}}",
        quoted(claim),
        searched.join(", "),
        quoted(if is_counter { "fails" } else { "holds" }),
        listed.join(", "),
        undecided
    )
}

fn find_status(found_any: bool, undecided_any: bool, is_counter: bool) -> u8 {
    match (is_counter, found_any, undecided_any) {
        (true, true, _) => EXIT_FAILURE,
        (true, false, false) | (false, true, _) => EXIT_SUCCESS,
        (true, false, true) | (false, false, _) => EXIT_NOTHING_REACHED,
    }
}

fn run_find(
    invocation: &Invocation,
    locale: &Locale,
    output: &mut Output<'_>,
    errors: &mut Output<'_>,
) -> Result<u8, Failure> {
    let ranges = invocation.find.clone();
    let Some(Input::Expression(claim)) = &invocation.input else {
        return Err(report(errors, &Message::CliErrorNoInput, locale));
    };
    match find_matches(&ranges, claim, invocation.is_counter) {
        SearchOutcome::Found { found, undecided } if invocation.is_json => {
            output
                .json_record(
                    find_json(
                        claim,
                        &ranges,
                        &found,
                        undecided.as_ref(),
                        invocation.is_counter,
                    )
                    .as_bytes(),
                )
                .map_err(|_| Failure::Output)?;
            Ok(find_status(
                !found.is_empty(),
                undecided.is_some(),
                invocation.is_counter,
            ))
        }
        SearchOutcome::Found { found, undecided } => {
            let nothing_found = match (&undecided, invocation.is_counter) {
                (None, true) => Some(Message::CliFoundNoCounter),
                (None, false) => Some(Message::CliFoundNone),
                (Some(_), true) => Some(Message::CliFoundNoCounterDecided),
                (Some(_), false) => Some(Message::CliFoundNoneDecided),
            }
            .filter(|_| found.is_empty());
            let found_lines = found.iter().map(|assignment| Message::CliFound {
                assignment: assignment_text(&ranges, assignment),
            });
            let undecided_line = undecided.as_ref().map(|points| Message::CliFoundUndecided {
                count: points.count,
                assignment: assignment_text(&ranges, &points.first),
            });
            for message in found_lines.chain(nothing_found).chain(undecided_line) {
                output
                    .line(&render(&message, locale))
                    .map_err(|_| Failure::Output)?;
            }
            Ok(find_status(
                !found.is_empty(),
                undecided.is_some(),
                invocation.is_counter,
            ))
        }
        SearchOutcome::TooManyCandidates(count) => {
            output
                .line(&render(
                    &Message::CliFoundTooMany {
                        count: count.to_string(),
                        limit: calc_app::SEARCH_LIMIT.to_string(),
                    },
                    locale,
                ))
                .map_err(|_| Failure::Output)?;
            Ok(EXIT_NOT_A_CLAIM)
        }
        other => {
            let outcome = match other {
                SearchOutcome::Unreadable(error) => ClaimOutcome::Unreadable(error),
                _ => ClaimOutcome::NotARelation,
            };
            output
                .line(&render(&claim_message(claim, &outcome, locale), locale))
                .map_err(|_| Failure::Output)?;
            Ok(EXIT_NOT_A_CLAIM)
        }
    }
}

fn run_check(
    invocation: &Invocation,
    context: &Context<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
    errors: &mut Output<'_>,
    probes: &mut Probes,
) -> Result<u8, Failure> {
    let claims: Vec<String> = if invocation.is_batch {
        std::io::read_to_string(std::io::stdin())
            .map_err(|_| Failure::Output)?
            .lines()
            .map(|line| line.trim().to_owned())
            .filter(|line| !line.is_empty())
            .collect()
    } else {
        match &invocation.input {
            Some(Input::Expression(text)) => vec![text.clone()],
            _ => return Err(report(errors, &Message::CliErrorNoInput, locale)),
        }
    };
    let mut session = asking_session(context, probes);
    let mut status = EXIT_SUCCESS;
    for claim in claims {
        let outcome = calc_app::claim_of(&mut session, &claim);
        status = more_consequential(status, claim_status(&outcome));
        let noted = claim_takes_zero_to_the_zero(&claim);
        if invocation.is_json {
            output
                .json_record(
                    with_claim_notes(claim_json(&claim, &outcome, locale), noted).as_bytes(),
                )
                .map_err(|_| Failure::Output)?;
        } else {
            output
                .line(&render(&claim_message(&claim, &outcome, locale), locale))
                .map_err(|_| Failure::Output)?;
            write_claim_note(output, noted, locale)?;
        }
    }
    Ok(status)
}

const ZERO_TO_THE_ZERO_CODE: &str = "zero_to_the_zero";

fn with_claim_notes(json: String, noted: bool) -> String {
    match json.strip_suffix('}') {
        Some(open) if noted => format!("{open}, \"notes\": [{}]}}", quoted(ZERO_TO_THE_ZERO_CODE)),
        _ => json,
    }
}

fn write_claim_note(output: &mut Output<'_>, noted: bool, locale: &Locale) -> Result<(), Failure> {
    if !noted {
        return Ok(());
    }
    let note = render(&Message::CommonNoteZeroToTheZero, locale).to_string();
    output
        .line(&render(&Message::CliClaimNote { note }, locale))
        .map_err(|_| Failure::Output)
}

enum BatchError {
    Json(Vec<u8>),
    Text(String),
}

fn run_batch(
    invocation: &Invocation,
    units: &DisplayUnits,
    context: &Context<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
    errors: &mut Output<'_>,
    probes: &mut Probes,
) -> Result<u8, Failure> {
    let (mut session, probe) =
        probing_session(Session::new((context.clock)(), registered_backends()));
    probes.watch(probe);
    let mut entered: Vec<Result<LineId, BatchError>> = Vec::new();
    let mut is_success = true;
    let mut is_reached = true;
    for (index, line) in std::io::read_to_string(std::io::stdin())
        .map_err(|_| Failure::Output)?
        .lines()
        .enumerate()
    {
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        let number = index + 1;
        match session.enter(text) {
            Ok(id) => entered.push(Ok(id)),
            Err(error) => {
                is_success = false;
                let reported = if invocation.is_json {
                    BatchError::Json(session_error_json_line(&error, text, number))
                } else {
                    let detail = render(&session_error_message(&error, text), locale).to_string();
                    BatchError::Text(
                        render(
                            &Message::CliBatchLineRefused {
                                number: number.to_string(),
                                input: text.to_owned(),
                                detail,
                            },
                            locale,
                        )
                        .to_string(),
                    )
                };
                entered.push(Err(reported));
            }
        }
    }
    if invocation.is_recognize {
        session.run_pending_recognition();
    }
    for entry in entered {
        let id = match entry {
            Ok(id) => id,
            Err(BatchError::Json(reported)) => {
                output.json_record(&reported).map_err(|_| Failure::Output)?;
                continue;
            }
            Err(BatchError::Text(detail)) => {
                errors
                    .line(&render(&Message::CliError { detail }, locale))
                    .map_err(|_| Failure::Output)?;
                continue;
            }
        };
        let Some((summary, display)) =
            session.displayed_line(id, units.choice.as_ref(), None, units.coherent_only)
        else {
            continue;
        };
        if matches!(
            summary.state,
            LineState::Failed(Message::ErrorSortLimitReached { .. })
        ) {
            is_reached = false;
        } else if matches!(summary.state, LineState::Failed(_)) || is_not_a_number(&session, id) {
            is_success = false;
        }
        if let Some(instrument) = invocation.instrument {
            let asked = InstrumentRequest {
                instrument,
                units,
                is_json: invocation.is_json,
                is_how_it_ran: invocation.is_how_it_ran,
            };
            match show_instrument_in(&mut session, id, &asked, locale, output, errors, true) {
                Ok(EXIT_SUCCESS) => {}
                Ok(_) | Err(Failure::Reported) => is_success = false,
                Err(other) => return Err(other),
            }
            continue;
        }
        if let Some(view) = invocation.view.clone() {
            let shown = ViewInvocation {
                view,
                units,
                is_json: invocation.is_json,
            };
            if !show_view_of_line(&mut session, id, &shown, locale, output, errors)? {
                is_success = false;
            }
            continue;
        }
        let Some(line) = session.line(id) else {
            continue;
        };
        if invocation.is_json {
            let json = session
                .line_json_compact(line)
                .map_err(|error| report(errors, &save_error_message(&error), locale))?;
            output.json_record(&json).map_err(|_| Failure::Output)?;
        } else if invocation.is_terse {
            output
                .line(&terse_row(&summary, locale))
                .map_err(|_| Failure::Output)?;
        } else {
            let reading = display.and_then(|(_, displayed)| {
                displayed
                    .reading
                    .map(|text| (text, displayed.reading_distance))
            });
            for row in line_rows(
                &summary,
                reading
                    .as_ref()
                    .map(|(text, distance)| (text.as_str(), distance.as_ref())),
                RowOptions {
                    how_it_ran: invocation.is_how_it_ran,
                    offer: false,
                },
                locale,
            ) {
                output.line(&row).map_err(|_| Failure::Output)?;
            }
            if invocation.is_trace {
                for row in trace_rows(session.sort_trace(id).as_deref(), locale) {
                    output.line(&row).map_err(|_| Failure::Output)?;
                }
            }
        }
    }
    Ok(reached_status(is_success, is_reached))
}

fn evaluate(
    invocation: &Invocation,
    units: &DisplayUnits,
    context: &Context<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
    errors: &mut Output<'_>,
    probes: &mut Probes,
) -> Result<u8, Failure> {
    if let Some(request) = &invocation.ode {
        return run_ode(request, invocation.is_json, locale, output, errors);
    }
    if invocation.is_solve_for {
        return run_solve_for(invocation, context, locale, output, errors, probes);
    }
    if invocation.is_factor {
        return run_factor(invocation, context, locale, output, errors, probes);
    }
    if invocation.is_expand {
        return run_expand(invocation, context, locale, output, errors, probes);
    }
    if invocation.is_identity {
        return run_identity(invocation, context, locale, output, errors, probes);
    }
    if !invocation.find.is_empty() {
        return run_find(invocation, locale, output, errors);
    }
    if invocation.is_check {
        return run_check(invocation, context, locale, output, errors, probes);
    }
    if invocation.is_batch {
        return run_batch(invocation, units, context, locale, output, errors, probes);
    }
    let (mut session, lines_to_print, is_session_file) = match &invocation.input {
        Some(Input::Expression(text)) => {
            let (mut session, probe) =
                probing_session(Session::new((context.clock)(), registered_backends()));
            probes.watch(probe);
            let line = session.enter(text).map_err(|error| match &error {
                SessionError::Parse(parse) if invocation.is_json => errors
                    .json(&parse_error_json_naming_input(parse, text))
                    .map_or(Failure::Output, |()| Failure::Reported),
                _ => report(errors, &session_error_message(&error, text), locale),
            })?;
            (session, vec![line], false)
        }
        Some(Input::SessionFile(path)) => {
            let (session, probe) = open_session(path, context, locale, errors)?;
            probes.watch(probe);
            let lines = session.lines().iter().map(Line::id).collect();
            (session, lines, true)
        }
        None => return Ok(EXIT_SUCCESS),
    };
    session.run_pending_recognition();
    let mut differing_lines = 0_usize;
    if invocation.is_replay && is_session_file {
        differing_lines = session
            .replay()
            .lines
            .iter()
            .filter(|line| line.state == ComparedState::Differs)
            .count();
    }
    if let Some(instrument) = invocation.instrument {
        let target = one_line(&mut session, invocation, &lines_to_print, locale, errors)?;
        return show_instrument(
            &mut session,
            target,
            &InstrumentRequest {
                instrument,
                units,
                is_json: invocation.is_json,
                is_how_it_ran: invocation.is_how_it_ran,
            },
            locale,
            output,
            errors,
        );
    }
    if let Some(view) = invocation.view.clone() {
        let mut session = session;
        let target = one_line(&mut session, invocation, &lines_to_print, locale, errors)?;
        return show_view(
            &mut session,
            target,
            &ViewInvocation {
                view,
                units,
                is_json: invocation.is_json,
            },
            locale,
            output,
            errors,
        );
    }
    let mut is_success = true;
    let mut is_reached = true;
    for id in lines_to_print {
        let Some((summary, display)) =
            session.displayed_line(id, units.choice.as_ref(), None, units.coherent_only)
        else {
            continue;
        };
        let reading = display.and_then(|(_, displayed)| {
            displayed
                .reading
                .map(|text| (text, displayed.reading_distance))
        });
        if matches!(
            summary.state,
            LineState::Failed(Message::ErrorSortLimitReached { .. })
        ) {
            is_reached = false;
        } else if matches!(summary.state, LineState::Failed(_)) || is_not_a_number(&session, id) {
            is_success = false;
        }
        let Some(line) = session.line(id) else {
            continue;
        };
        if invocation.is_json {
            let json = session
                .line_json(line)
                .map_err(|error| report(errors, &save_error_message(&error), locale))?;
            output.json(&json).map_err(|_| Failure::Output)?;
        } else if invocation.is_terse {
            output
                .line(&terse_row(&summary, locale))
                .map_err(|_| Failure::Output)?;
        } else {
            for row in line_rows(
                &summary,
                reading
                    .as_ref()
                    .map(|(text, distance)| (text.as_str(), distance.as_ref())),
                RowOptions {
                    how_it_ran: invocation.is_how_it_ran,
                    offer: false,
                },
                locale,
            ) {
                output.line(&row).map_err(|_| Failure::Output)?;
            }
            if invocation.is_how_it_ran
                && let LineState::Result(record) = &summary.state
            {
                for row in modes_rows(record, locale) {
                    output.line(&row).map_err(|_| Failure::Output)?;
                }
            }
            if invocation.is_trace {
                for row in trace_rows(session.sort_trace(id).as_deref(), locale) {
                    output.line(&row).map_err(|_| Failure::Output)?;
                }
            }
        }
    }
    if is_session_file && !invocation.is_json {
        output
            .line(&status_row(&session.status(), locale))
            .map_err(|_| Failure::Output)?;
    }
    Ok(reached_status(
        is_success && differing_lines == 0,
        is_reached,
    ))
}

struct ViewInvocation<'a> {
    view: ViewRequest,
    units: &'a DisplayUnits,
    is_json: bool,
}

fn one_line(
    session: &mut Session,
    invocation: &Invocation,
    lines: &[LineId],
    locale: &Locale,
    errors: &mut Output<'_>,
) -> Result<LineId, Failure> {
    let target = match (&invocation.line, lines.first()) {
        (Some(name), _) => session.resolve(name),
        (None, first) => first.copied(),
    };
    target.ok_or_else(|| {
        let line = invocation.line.clone().unwrap_or_default();
        report(errors, &Message::ErrorUnknownLabel { line }, locale)
    })
}

fn instrument_option(instrument: LineInstrument) -> &'static str {
    match instrument {
        LineInstrument::Inspect => INSPECT_OPTION,
        LineInstrument::Method => METHOD_OPTION,
        LineInstrument::Plot => PLOT_OPTION,
    }
}

struct InstrumentRequest<'a> {
    instrument: LineInstrument,
    units: &'a DisplayUnits,
    is_json: bool,
    is_how_it_ran: bool,
}

fn show_instrument(
    session: &mut Session,
    target: LineId,
    asked: &InstrumentRequest<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
    errors: &mut Output<'_>,
) -> Result<u8, Failure> {
    show_instrument_in(session, target, asked, locale, output, errors, false)
}

#[allow(clippy::too_many_arguments)]
fn show_instrument_in(
    session: &mut Session,
    target: LineId,
    asked: &InstrumentRequest<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
    errors: &mut Output<'_>,
    one_line_json: bool,
) -> Result<u8, Failure> {
    let InstrumentRequest {
        instrument,
        units,
        is_json,
        is_how_it_ran,
    } = asked;
    let instrument = *instrument;
    let facts = InstrumentFacts {
        plottable: session.is_plottable(target),
        naming_shortened: false,
    };
    let Some(line) = session.line(target) else {
        return Err(report(errors, &Message::CliErrorViewNotFinished, locale));
    };
    let applying = applying_instruments(line.outcome(), facts);
    if !applying.contains(&instrument) {
        let options: Vec<&str> = applying
            .iter()
            .copied()
            .filter(|applying| *applying != LineInstrument::Method)
            .map(instrument_option)
            .collect();
        let detail = render(
            &Message::CliErrorInstrumentNotApplicable {
                option: instrument_option(instrument).to_owned(),
                line: session.line_summary(line).label,
                options: if options.is_empty() {
                    render(&Message::CliInstrumentNone, locale).to_string()
                } else {
                    options.join(LIST_SEPARATOR)
                },
            },
            locale,
        )
        .to_string();
        return Err(
            match errors.line(&render(&Message::CliError { detail }, locale)) {
                Ok(()) => Failure::Reported,
                Err(_) => Failure::Output,
            },
        );
    }
    let Some((summary, display)) =
        session.displayed_line(target, units.choice.as_ref(), None, units.coherent_only)
    else {
        return Err(report(errors, &Message::CliErrorViewNotFinished, locale));
    };
    let reading = display.and_then(|(_, displayed)| {
        displayed
            .reading
            .map(|text| (text, displayed.reading_distance))
    });
    if *is_json {
        let Some(line) = session.line(target) else {
            return Err(report(errors, &Message::CliErrorViewNotFinished, locale));
        };
        if one_line_json {
            let json = session
                .line_json_compact(line)
                .map_err(|error| report(errors, &save_error_message(&error), locale))?;
            output.json_record(&json).map_err(|_| Failure::Output)?;
        } else {
            let json = session
                .line_json(line)
                .map_err(|error| report(errors, &save_error_message(&error), locale))?;
            output.json(&json).map_err(|_| Failure::Output)?;
        }
        return Ok(line_status(&summary.state));
    }
    let shown = RowOptions {
        how_it_ran: *is_how_it_ran,
        offer: true,
    };
    for row in line_rows(
        &summary,
        reading
            .as_ref()
            .map(|(text, distance)| (text.as_str(), distance.as_ref())),
        shown,
        locale,
    ) {
        output.line(&row).map_err(|_| Failure::Output)?;
    }
    if let (true, LineState::Result(record)) = (*is_how_it_ran, &summary.state) {
        for row in modes_rows(record, locale) {
            output.line(&row).map_err(|_| Failure::Output)?;
        }
    }
    Ok(line_status(&summary.state))
}

fn show_view_of_line(
    session: &mut Session,
    target: LineId,
    asked: &ViewInvocation<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
    errors: &mut Output<'_>,
) -> Result<bool, Failure> {
    Ok(show_view_in(session, target, asked, locale, output, errors, true)? == EXIT_SUCCESS)
}

fn show_view(
    session: &mut Session,
    target: LineId,
    asked: &ViewInvocation<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
    errors: &mut Output<'_>,
) -> Result<u8, Failure> {
    show_view_in(session, target, asked, locale, output, errors, false)
}

#[allow(clippy::too_many_arguments)]
fn show_view_in(
    session: &mut Session,
    target: LineId,
    asked: &ViewInvocation<'_>,
    locale: &Locale,
    output: &mut Output<'_>,
    errors: &mut Output<'_>,
    one_line_json: bool,
) -> Result<u8, Failure> {
    let ViewInvocation {
        view,
        units,
        is_json,
    } = asked;
    let is_json = *is_json;
    let request = match view {
        ViewRequest::Digits(places) => ResultView::Digits { places: *places },
        ViewRequest::Enclose(significant_digits) => ResultView::Enclose {
            significant_digits: *significant_digits,
        },
        ViewRequest::Working(path) => ResultView::Working { path: path.clone() },
    };
    let (events, received) = channel();
    let mut job = session
        .view(
            target,
            request,
            units.choice.as_ref(),
            units.coherent_only,
            events,
        )
        .map_err(|error| report(errors, &view_error_message(&error), locale))?;
    while job.step() == JobState::Pending {}
    let event = received
        .try_recv()
        .map_err(|_| report(errors, &Message::CliErrorViewNotFinished, locale))?;
    let outcome = match event {
        ViewEvent::Finished { outcome, .. } => outcome,
        ViewEvent::Failed { error, .. } => {
            return Err(report(errors, &view_error_message(&error), locale));
        }
    };
    if is_json {
        if one_line_json {
            output
                .json_record(&view_outcome_json_line(&outcome))
                .map_err(|_| Failure::Output)?;
        } else {
            output
                .json(&view_outcome_json(&outcome))
                .map_err(|_| Failure::Output)?;
        }
    } else {
        if let Some(line) = session.line(target) {
            let header = session.line_summary(line);
            for row in line_rows(&header, None, RowOptions::default(), locale)
                .into_iter()
                .take(1)
            {
                output.line(&row).map_err(|_| Failure::Output)?;
            }
        }
        for row in view_rows(&outcome, locale) {
            output.line(&row).map_err(|_| Failure::Output)?;
        }
    }
    Ok(EXIT_SUCCESS)
}

pub(crate) fn open_file(
    path: &Path,
    context: &Context<'_>,
) -> Result<(Session, ProbeOutcome), Box<Message>> {
    let path_text = path.display().to_string();
    let bytes = (context.read_file)(path).map_err(|error| {
        Box::new(match error.kind() {
            io::ErrorKind::NotFound => Message::ErrorFileNotFound {
                path: path_text.clone(),
            },
            io::ErrorKind::PermissionDenied => Message::ErrorFilePermissionDenied {
                path: path_text.clone(),
            },
            _ => Message::ErrorFileUnreadable {
                path: path_text.clone(),
            },
        })
    })?;
    Session::open_from_bytes(&bytes, (context.clock)(), registered_backends())
        .map(probing_session)
        .map_err(|error| Box::new(load_error_message(&error, &path_text)))
}

fn open_session(
    path: &Path,
    context: &Context<'_>,
    locale: &Locale,
    errors: &mut Output<'_>,
) -> Result<(Session, ProbeOutcome), Failure> {
    open_file(path, context).map_err(|message| report(errors, &message, locale))
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_app::{SystemClock, UtcTimestamp};

    struct FixedClock;

    impl Clock for FixedClock {
        fn now_utc(&self) -> UtcTimestamp {
            UtcTimestamp::from_milliseconds_since_unix_epoch(0)
        }

        fn monotonic_nanoseconds(&self) -> u64 {
            0
        }

        fn resolution_nanoseconds(&self) -> u64 {
            1
        }
    }

    struct Ran {
        code: u8,
        output: String,
        errors: String,
    }

    fn run_with(arguments: &[&str], posix: &[&str], file: io::Result<&str>) -> Ran {
        let file = file.map(str::to_owned).map_err(|error| error.kind());
        let read_file = move |_: &Path| match &file {
            Ok(text) => Ok(text.clone().into_bytes()),
            Err(kind) => Err(io::Error::from(*kind)),
        };
        let clock = || -> Box<dyn Clock> { Box::new(FixedClock) };
        let read_standard_input = || Ok(Vec::new());
        let context = Context {
            posix_locale_values: posix.iter().map(|value| (*value).to_owned()).collect(),
            read_file: &read_file,
            read_standard_input: &read_standard_input,
            write_file: &|_: &Path, _: &[u8]| Ok(()),
            check_output: &|_: &Path| Ok(()),
            clock: &clock,
        };
        let arguments: Vec<OsString> = arguments.iter().map(OsString::from).collect();
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let code = run(&arguments, &context, &mut output, &mut errors);
        Ran {
            code,
            output: String::from_utf8(output).unwrap(),
            errors: String::from_utf8(errors).unwrap(),
        }
    }

    #[test]
    fn expression_prints_its_line_and_succeeds() {
        let ran = run_with(&["2 + 3"], &[], Err(io::ErrorKind::NotFound.into()));
        assert_eq!(ran.code, EXIT_SUCCESS);
        assert!(ran.output.starts_with("r1  2 + 3\n  value "));
    }

    #[test]
    fn expression_output_follows_the_posix_locale() {
        let ran = run_with(
            &["2 + 3"],
            &["de_DE.UTF-8"],
            Err(io::ErrorKind::NotFound.into()),
        );
        assert!(ran.output.contains("\n  Wert "));
    }

    #[test]
    fn locale_option_wins_over_the_posix_locale() {
        let ran = run_with(
            &["2 + 3", "--locale", "en"],
            &["de_DE.UTF-8"],
            Err(io::ErrorKind::NotFound.into()),
        );
        assert!(ran.output.contains("\n  value "));
    }

    #[test]
    fn parse_error_is_reported_on_standard_error() {
        let ran = run_with(&["2 +"], &[], Err(io::ErrorKind::NotFound.into()));
        assert_eq!(ran.code, EXIT_FAILURE);
        assert_eq!(ran.output, "");
        assert!(ran.errors.starts_with("error: "));
    }

    #[test]
    fn failed_line_exits_with_failure() {
        let ran = run_with(&["r9 + 1"], &[], Err(io::ErrorKind::NotFound.into()));
        assert_eq!(ran.code, EXIT_FAILURE);
        assert!(ran.output.contains("  r9 is not defined\n"));
    }

    #[test]
    fn json_output_is_the_line_object() {
        let ran = run_with(
            &["2 + 3", "--json"],
            &[],
            Err(io::ErrorKind::NotFound.into()),
        );
        assert!(
            ran.output
                .starts_with("{\n  \"id\": \"r1\",\n  \"name\": null,\n  \"input\": \"2 + 3\",")
        );
        assert!(ran.output.ends_with("}\n"));
    }

    #[test]
    fn json_parse_error_is_the_typed_error_on_standard_error() {
        let ran = run_with(&["2 +", "--json"], &[], Err(io::ErrorKind::NotFound.into()));
        assert_eq!(
            (ran.code, ran.output.as_str(), ran.errors.as_str()),
            (
                EXIT_FAILURE,
                "",
                "{\n  \"code\": \"parse_error\",\n  \"input\": \"2 +\",\n  \"data\": {\n    \"kind\": \"unexpected_end\",\n    \"column\": \"4\"\n  }\n}\n"
            )
        );
    }

    #[test]
    fn json_ambiguous_application_keeps_its_readings() {
        let ran = run_with(
            &["sin pi/2", "--json"],
            &[],
            Err(io::ErrorKind::NotFound.into()),
        );

        assert_eq!(
            (ran.code, ran.errors.as_str()),
            (
                EXIT_FAILURE,
                "{\n  \"code\": \"ambiguous_application\",\n  \"input\": \"sin pi/2\",\n  \"data\": {\n    \"column\": \"7\",\n    \"function\": \"sin\",\n    \"narrow\": \"sin(pi)/2\",\n    \"wide\": \"sin(pi/2)\",\n    \"written\": \"sin pi/2\"\n  }\n}\n"
            )
        );
    }

    #[test]
    fn json_ambiguous_temperature_sign_keeps_both_readings() {
        let ran = run_with(
            &["20 \u{00B0}C", "--json"],
            &[],
            Err(io::ErrorKind::NotFound.into()),
        );

        assert_eq!(
            (ran.code, ran.errors.as_str()),
            (
                EXIT_FAILURE,
                "{\n  \"code\": \"ambiguous_temperature_sign\",\n  \"input\": \"20 \u{00B0}C\",\n  \"data\": {\n    \"column\": \"4\",\n    \"difference\": \"20 degC\",\n    \"reading\": \"from_celsius(20)\",\n    \"sign\": \"celsius\",\n    \"written\": \"20 \u{00B0}C\"\n  }\n}\n"
            )
        );
    }

    #[test]
    fn json_recognised_attempt_is_the_typed_error_on_standard_error() {
        let ran = run_with(
            &["10**6", "--json"],
            &[],
            Err(io::ErrorKind::NotFound.into()),
        );
        assert_eq!(
            (ran.code, ran.output.as_str(), ran.errors.as_str()),
            (
                EXIT_FAILURE,
                "",
                "{\n  \"code\": \"recognised_attempt\",\n  \"input\": \"10**6\",\n  \"data\": {\n    \"attempt\": \"double_star_power\",\n    \"column\": \"3\",\n    \"corrected\": \"10^6\",\n    \"replacement\": \"^\"\n  }\n}\n"
            )
        );
    }

    #[test]
    fn text_parse_error_stays_prose() {
        let ran = run_with(&["2 +"], &[], Err(io::ErrorKind::NotFound.into()));
        assert!(ran.errors.starts_with("error: "));
    }

    #[test]
    fn usage_error_exits_with_usage_code() {
        let ran = run_with(&[], &[], Err(io::ErrorKind::NotFound.into()));
        assert_eq!(ran.code, EXIT_USAGE);
        assert!(
            ran.errors
                .starts_with("error: no expression or session file given\n")
        );
    }

    #[test]
    fn invalid_locale_is_reported_in_english() {
        let ran = run_with(
            &["--locale", "de_DE", "2"],
            &["de_DE.UTF-8"],
            Err(io::ErrorKind::NotFound.into()),
        );
        assert_eq!(ran.code, EXIT_USAGE);
        assert!(
            ran.errors
                .starts_with("error: de_DE is not a BCP 47 language tag\n")
        );
    }

    #[test]
    fn help_prints_the_manual_page_on_standard_output() {
        let ran = run_with(&["--help"], &[], Err(io::ErrorKind::NotFound.into()));
        assert_eq!(ran.code, EXIT_SUCCESS);
        assert!(ran.output.starts_with("calc — "));
        assert!(ran.output.contains("\nExit status:\n"));
    }

    #[test]
    fn missing_session_file_is_reported() {
        let ran = run_with(&["lab.calc"], &[], Err(io::ErrorKind::NotFound.into()));
        assert_eq!(ran.code, EXIT_FAILURE);
        assert_eq!(ran.errors, "error: lab.calc does not exist\n");
    }

    #[test]
    fn invalid_session_file_is_reported() {
        let ran = run_with(&["lab.calc"], &[], Ok("{}"));
        assert_eq!(ran.code, EXIT_FAILURE);
        assert!(
            ran.errors
                .starts_with("error: lab.calc is not a valid session file at ")
        );
    }

    #[test]
    fn session_file_prints_lines_and_status() {
        let mut session = Session::new(Box::new(SystemClock::new()), registered_backends());
        session.enter("a = 1/4").unwrap();
        session.enter("a * 2").unwrap();
        let file = String::from_utf8(session.save_to_bytes().unwrap()).unwrap();
        let ran = run_with(&["lab.calc"], &[], Ok(&file));
        assert_eq!(ran.code, EXIT_SUCCESS);
        assert!(ran.output.starts_with("r1  a = 1/4\n"));
        assert!(ran.output.contains("\nr2  a * 2\n"));
        assert!(ran.output.ends_with(
            "replay: not run  2 results  0 running  en  precision: f64  backend: automatic\n"
        ));
    }

    #[test]
    fn session_file_view_shows_the_named_line() {
        let mut session = Session::new(Box::new(SystemClock::new()), registered_backends());
        session.enter("a = 1/4").unwrap();
        session.enter("a / 3").unwrap();
        let file = String::from_utf8(session.save_to_bytes().unwrap()).unwrap();
        let ran = run_with(
            &["lab.calc", "r2", "--digits", "3", "--locale", "en"],
            &[],
            Ok(&file),
        );
        assert_eq!(ran.code, EXIT_SUCCESS);
        assert!(ran.output.starts_with("r2  a / 3\n  decimal places  3\n"));
    }

    #[test]
    fn session_file_view_of_an_unknown_line_is_reported() {
        let mut session = Session::new(Box::new(SystemClock::new()), registered_backends());
        session.enter("1/4").unwrap();
        let file = String::from_utf8(session.save_to_bytes().unwrap()).unwrap();
        let ran = run_with(
            &["lab.calc", "r9", "--digits", "3", "--locale", "en"],
            &[],
            Ok(&file),
        );
        assert_eq!(ran.code, EXIT_FAILURE);
    }

    #[test]
    fn f32_session_file_prints_the_f32_value_and_bound_of_a_machine_line() {
        let mut session = Session::new(Box::new(SystemClock::new()), registered_backends());
        session.set_precision(calc_app::Precision::F32);
        session.enter("to_f32(1/3)").unwrap();
        let file = String::from_utf8(session.save_to_bytes().unwrap()).unwrap();
        let ran = run_with(&["lab.calc", "--locale", "en"], &[], Ok(&file));
        assert_eq!(ran.code, EXIT_SUCCESS);
        assert!(ran.output.starts_with(
            "r1  to_f32(1/3)\n  value           0.33333334\n  number          machine float, 32-bit (f32)\n  rounding error  at most 9.934107536579974e-9, absolute\n  computed        in machine arithmetic\n  ran on          CPU\n"
        ));
    }
}
