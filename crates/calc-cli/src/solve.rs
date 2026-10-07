use std::ffi::OsString;
use std::io::Write;

use calc_app::{
    Backend, EvaluationContext, ReplyStatus, registered_backends, solve, typed_error_json,
};

use calc_i18n::Message;

use crate::completion::{OptionSpec, flag, free, is_listed};
use crate::run::Context;

pub const SOLVE_COMMAND: &str = "solve";

pub const EXIT_ANSWERED: u8 = 0;

pub const EXIT_FAILED: u8 = 1;

pub const EXIT_NOT_REACHED: u8 = 4;

pub const EXIT_INVALID_REQUEST: u8 = 3;

const JSON_OPTION: &str = "--json";
const REQUEST_OPTION: &str = "--request";
const OPTION_PREFIX: &str = "--";

pub(crate) static OPTIONS: &[OptionSpec] = &[
    flag(JSON_OPTION, Message::CliCompleteJson),
    free(REQUEST_OPTION, Message::CliCompleteRequest),
];
const ARGUMENT_DATA: &str = "argument";
const UNKNOWN_OPTION_CODE: &str = "unknown_option";
const MISSING_OPTION_VALUE_CODE: &str = "missing_option_value";
const UNEXPECTED_ARGUMENT_CODE: &str = "unexpected_argument";
const JSON_OUTPUT_REQUIRED_CODE: &str = "json_output_required";
const INPUT_NOT_READ_CODE: &str = "input_not_read";
const OUTPUT_NOT_WRITTEN_CODE: &str = "output_not_written";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestSource {
    Argument(String),
    StandardInput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolveInvocation {
    pub request: RequestSource,
    pub is_json: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SolveUsageError {
    UnknownOption(String),
    MissingOptionValue(String),
    UnexpectedArgument(String),
}

impl SolveUsageError {
    fn json(&self) -> Vec<u8> {
        let (code, argument) = match self {
            Self::UnknownOption(argument) => (UNKNOWN_OPTION_CODE, argument),
            Self::MissingOptionValue(argument) => (MISSING_OPTION_VALUE_CODE, argument),
            Self::UnexpectedArgument(argument) => (UNEXPECTED_ARGUMENT_CODE, argument),
        };
        typed_error_json(code, &[(ARGUMENT_DATA, argument)])
    }
}

pub fn parse_solve_arguments(arguments: &[OsString]) -> Result<SolveInvocation, SolveUsageError> {
    let mut invocation = SolveInvocation {
        request: RequestSource::StandardInput,
        is_json: false,
    };
    let mut remaining = arguments.iter();
    while let Some(argument) = remaining.next() {
        let text = argument.to_string_lossy().into_owned();
        match text.as_str() {
            option if option.starts_with(OPTION_PREFIX) && !is_listed(OPTIONS, option) => {
                return Err(SolveUsageError::UnknownOption(text));
            }
            JSON_OPTION => invocation.is_json = true,
            REQUEST_OPTION => {
                let request = remaining
                    .next()
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| SolveUsageError::MissingOptionValue(text.clone()))?;
                invocation.request = RequestSource::Argument(request.to_owned());
            }
            option if option.starts_with(OPTION_PREFIX) => {
                return Err(SolveUsageError::UnknownOption(text));
            }
            _ => return Err(SolveUsageError::UnexpectedArgument(text)),
        }
    }
    Ok(invocation)
}

fn fail(standard_error: &mut dyn Write, json: &[u8]) -> u8 {
    let _ = standard_error.write_all(json);
    EXIT_FAILED
}

pub fn run_solve(
    arguments: &[OsString],
    context: &Context<'_>,
    standard_output: &mut dyn Write,
    standard_error: &mut dyn Write,
) -> u8 {
    let invocation = match parse_solve_arguments(arguments) {
        Ok(invocation) => invocation,
        Err(error) => return fail(standard_error, &error.json()),
    };
    if !invocation.is_json {
        return fail(
            standard_error,
            &typed_error_json(JSON_OUTPUT_REQUIRED_CODE, &[]),
        );
    }
    let request = match &invocation.request {
        RequestSource::Argument(text) => text.clone().into_bytes(),
        RequestSource::StandardInput => match (context.read_standard_input)() {
            Ok(bytes) => bytes,
            Err(_) => return fail(standard_error, &typed_error_json(INPUT_NOT_READ_CODE, &[])),
        },
    };
    let backends = registered_backends();
    let backend_references: Vec<&dyn Backend> = backends.iter().map(AsRef::as_ref).collect();
    let clock = (context.clock)();
    let evaluation = EvaluationContext {
        backends: &backend_references,
        clock: clock.as_ref(),
    };
    let reply = match solve(&request, &evaluation) {
        Ok(reply) => reply,
        Err(error) => return fail(standard_error, &typed_error_json(error.code(), &[])),
    };
    if standard_output.write_all(&reply.json).is_err() {
        return fail(
            standard_error,
            &typed_error_json(OUTPUT_NOT_WRITTEN_CODE, &[]),
        );
    }
    match reply.status {
        ReplyStatus::Reachable | ReplyStatus::Ways | ReplyStatus::Front | ReplyStatus::Solved => {
            EXIT_ANSWERED
        }
        ReplyStatus::NotReached => EXIT_NOT_REACHED,
        ReplyStatus::InvalidRequest => EXIT_INVALID_REQUEST,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_app::{Clock, UtcTimestamp};
    use std::io;
    use std::path::Path;

    const WAYS: &str = r#"{"phase": "ways", "object": "circle", "wanted": "circumference"}"#;

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

    fn run_with(arguments: &[&str], standard_input: io::Result<&str>) -> Ran {
        let input = standard_input
            .map(str::to_owned)
            .map_err(|error| error.kind());
        let read_standard_input = move || match &input {
            Ok(text) => Ok(text.clone().into_bytes()),
            Err(kind) => Err(io::Error::from(*kind)),
        };
        let read_file = |_: &Path| Err(io::Error::from(io::ErrorKind::NotFound));
        let clock = || -> Box<dyn Clock> { Box::new(FixedClock) };
        let context = Context {
            posix_locale_values: Vec::new(),
            read_file: &read_file,
            read_standard_input: &read_standard_input,
            write_file: &|_: &Path, _: &[u8]| Ok(()),
            check_output: &|_: &Path| Ok(()),
            clock: &clock,
        };
        let arguments: Vec<OsString> = arguments.iter().map(OsString::from).collect();
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let code = run_solve(&arguments, &context, &mut output, &mut errors);
        Ran {
            code,
            output: String::from_utf8(output).unwrap(),
            errors: String::from_utf8(errors).unwrap(),
        }
    }

    #[test]
    fn request_argument_is_a_request_source() {
        let invocation =
            parse_solve_arguments(&[OsString::from("--request"), OsString::from("{}")]).unwrap();
        assert_eq!(invocation.request, RequestSource::Argument("{}".to_owned()));
    }

    #[test]
    fn without_request_option_the_request_comes_from_standard_input() {
        let invocation = parse_solve_arguments(&[OsString::from("--json")]).unwrap();
        assert_eq!(invocation.request, RequestSource::StandardInput);
    }

    #[test]
    fn request_option_without_value_is_missing_option_value() {
        assert_eq!(
            parse_solve_arguments(&[OsString::from("--request")]),
            Err(SolveUsageError::MissingOptionValue("--request".to_owned()))
        );
    }

    #[test]
    fn unknown_option_is_rejected() {
        assert_eq!(
            parse_solve_arguments(&[OsString::from("--fast")]),
            Err(SolveUsageError::UnknownOption("--fast".to_owned()))
        );
    }

    #[test]
    fn positional_argument_is_unexpected() {
        assert_eq!(
            parse_solve_arguments(&[OsString::from("area")]),
            Err(SolveUsageError::UnexpectedArgument("area".to_owned()))
        );
    }

    #[test]
    fn request_from_standard_input_is_answered_with_exit_zero() {
        let ran = run_with(&["--json"], Ok(WAYS));
        assert_eq!(ran.code, EXIT_ANSWERED);
        assert!(ran.output.starts_with("{\n  \"status\": \"ways\",\n"));
    }

    #[test]
    fn usage_error_is_a_typed_error_on_standard_error_with_exit_one() {
        let ran = run_with(&["--json", "--fast"], Ok(WAYS));
        assert_eq!((ran.code, ran.output.as_str()), (EXIT_FAILED, ""));
        assert!(
            ran.errors
                .starts_with("{\n  \"code\": \"unknown_option\",\n")
        );
    }

    #[test]
    fn missing_json_option_is_a_typed_error() {
        let ran = run_with(&["--request", WAYS], Ok(""));
        assert_eq!(ran.code, EXIT_FAILED);
        assert!(ran.errors.contains("\"code\": \"json_output_required\""));
    }

    #[test]
    fn unreadable_standard_input_is_a_typed_error() {
        let ran = run_with(&["--json"], Err(io::Error::from(io::ErrorKind::BrokenPipe)));
        assert_eq!(ran.code, EXIT_FAILED);
        assert!(ran.errors.contains("\"code\": \"input_not_read\""));
    }

    #[test]
    fn every_exit_code_of_the_table_has_its_own_value() {
        let codes = [
            EXIT_ANSWERED,
            EXIT_FAILED,
            crate::run::EXIT_USAGE,
            EXIT_INVALID_REQUEST,
            EXIT_NOT_REACHED,
        ];
        assert_eq!(codes, [0, 1, 2, 3, 4]);
        let distinct: std::collections::BTreeSet<u8> = codes.into_iter().collect();
        assert_eq!(distinct.len(), codes.len());
    }

    #[test]
    fn invalid_request_exits_with_three() {
        let ran = run_with(&["--json"], Ok("{"));
        assert_eq!(ran.code, EXIT_INVALID_REQUEST);
        assert!(
            ran.output
                .starts_with("{\n  \"status\": \"invalid_request\",\n")
        );
    }
}
