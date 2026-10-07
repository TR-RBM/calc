use std::ffi::OsString;
use std::io::Write;

use calc_app::{
    MachineFormat, Session, SessionError, SolveCriterion, parse_error_json, parse_request,
    registered_backends, typed_error_json,
};

use calc_i18n::Message;

use crate::completion::{OptionSpec, Value, flag, free, is_listed};
use crate::run::Context;
use crate::solve::{EXIT_ANSWERED, EXIT_FAILED, EXIT_INVALID_REQUEST};

const HOLDS_FREE_NAMES_CODE: &str = "holds_free_names";

const NAMES_DATA: &str = "names";

pub const SOLVE_OPTION: &str = "--solve";
pub const CHOOSE_WANTED_OPTION: &str = "--choose-wanted";
pub const MACHINE_LINE_OPTION: &str = "--machine-line";
pub const ENTER_OPTION: &str = "--enter";
pub const BEGIN_OPTION: &str = "--begin";
pub(crate) const F64_FORMAT: &str = "f64";
pub(crate) const F32_FORMAT: &str = "f32";
const MACHINE_LINE_NOT_APPLICABLE_CODE: &str = "machine_line_not_applicable";
const INVALID_FORMAT_CODE: &str = "invalid_machine_format";
const STANDARD_STREAMS_FILE: &str = "-";
const OPTION_PREFIX: &str = "--";

pub(crate) static OPTIONS: &[OptionSpec] = &[
    flag(BEGIN_OPTION, Message::CliCompleteBegin),
    free(ENTER_OPTION, Message::CliCompleteEnter),
    OptionSpec {
        name: MACHINE_LINE_OPTION,
        values: &[Value::Free, Value::MachineFormat],
        description: Message::CliCompleteMachineLine,
    },
    free(SOLVE_OPTION, Message::CliCompleteSolve),
    OptionSpec {
        name: CHOOSE_WANTED_OPTION,
        values: &[Value::Free, Value::Free],
        description: Message::CliCompleteChooseWanted,
    },
];
const CLIENT_DEFAULT_CAP: u32 = 5;
const ARGUMENT_DATA: &str = "argument";
const LINE_DATA: &str = "line";
const PATH_DATA: &str = "path";
const UNKNOWN_OPTION_CODE: &str = "unknown_option";
const MISSING_OPTION_VALUE_CODE: &str = "missing_option_value";
const UNEXPECTED_ARGUMENT_CODE: &str = "unexpected_argument";
const MISSING_SESSION_FILE_CODE: &str = "missing_session_file";
const SAVING_NOT_SUPPORTED_CODE: &str = "saving_not_supported";
const INPUT_NOT_READ_CODE: &str = "input_not_read";
const SESSION_NOT_LOADED_CODE: &str = "session_not_loaded";
const SESSION_NOT_SAVED_CODE: &str = "session_not_saved";
const UNKNOWN_LINE_CODE: &str = "unknown_line";
const NOT_A_SOLVE_LINE_CODE: &str = "not_a_solve_line";
const COMMAND_FAILED_CODE: &str = "command_failed";
const OUTPUT_NOT_WRITTEN_CODE: &str = "output_not_written";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionCommand {
    Begin,
    Enter(String),
    Solve(String),
    ChooseWanted { line: String, role: String },
    MachineLine { line: String, format: MachineFormat },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionInvocation {
    pub file: String,
    pub command: SessionCommand,
}

struct Failure {
    code: u8,
    json: Vec<u8>,
}

impl Failure {
    fn typed(code: &str, data: &[(&str, &str)]) -> Self {
        Self {
            code: EXIT_FAILED,
            json: typed_error_json(code, data),
        }
    }
}

pub fn is_session_command(arguments: &[OsString]) -> bool {
    arguments.iter().any(|argument| {
        argument == SOLVE_OPTION
            || argument == CHOOSE_WANTED_OPTION
            || argument == MACHINE_LINE_OPTION
            || argument == ENTER_OPTION
            || argument == BEGIN_OPTION
    })
}

fn next_value(
    remaining: &mut std::slice::Iter<'_, OsString>,
    option: &str,
) -> Result<String, Failure> {
    remaining
        .next()
        .and_then(|value| value.to_str())
        .map(str::to_owned)
        .ok_or_else(|| Failure::typed(MISSING_OPTION_VALUE_CODE, &[(ARGUMENT_DATA, option)]))
}

fn parse_session_arguments(arguments: &[OsString]) -> Result<SessionInvocation, Failure> {
    let mut file = None;
    let mut command = None;
    let mut remaining = arguments.iter();
    while let Some(argument) = remaining.next() {
        let text = argument.to_string_lossy().into_owned();
        let parsed = match text.as_str() {
            option if option.starts_with(OPTION_PREFIX) && !is_listed(OPTIONS, option) => {
                return Err(Failure::typed(
                    UNKNOWN_OPTION_CODE,
                    &[(ARGUMENT_DATA, &text)],
                ));
            }
            BEGIN_OPTION => SessionCommand::Begin,
            ENTER_OPTION => SessionCommand::Enter(next_value(&mut remaining, ENTER_OPTION)?),
            SOLVE_OPTION => SessionCommand::Solve(next_value(&mut remaining, SOLVE_OPTION)?),
            CHOOSE_WANTED_OPTION => SessionCommand::ChooseWanted {
                line: next_value(&mut remaining, CHOOSE_WANTED_OPTION)?,
                role: next_value(&mut remaining, CHOOSE_WANTED_OPTION)?,
            },
            MACHINE_LINE_OPTION => {
                let line = next_value(&mut remaining, MACHINE_LINE_OPTION)?;
                let format_text = next_value(&mut remaining, MACHINE_LINE_OPTION)?;
                let format = match format_text.as_str() {
                    F64_FORMAT => MachineFormat::F64,
                    F32_FORMAT => MachineFormat::F32,
                    _ => {
                        return Err(Failure::typed(
                            INVALID_FORMAT_CODE,
                            &[(ARGUMENT_DATA, &format_text)],
                        ));
                    }
                };
                SessionCommand::MachineLine { line, format }
            }
            option if option.starts_with(OPTION_PREFIX) => {
                return Err(Failure::typed(
                    UNKNOWN_OPTION_CODE,
                    &[(ARGUMENT_DATA, &text)],
                ));
            }
            _ if file.is_none() => {
                file = Some(text);
                continue;
            }
            _ => {
                return Err(Failure::typed(
                    UNEXPECTED_ARGUMENT_CODE,
                    &[(ARGUMENT_DATA, &text)],
                ));
            }
        };
        if command.replace(parsed).is_some() {
            return Err(Failure::typed(
                UNEXPECTED_ARGUMENT_CODE,
                &[(ARGUMENT_DATA, &text)],
            ));
        }
    }
    let file = file.ok_or_else(|| Failure::typed(MISSING_SESSION_FILE_CODE, &[]))?;
    let command = command.ok_or_else(|| Failure::typed(MISSING_SESSION_FILE_CODE, &[]))?;
    Ok(SessionInvocation { file, command })
}

fn session_error(error: &SessionError, line: &str) -> Failure {
    match error {
        SessionError::InvalidRequest(request_error) => Failure {
            code: EXIT_INVALID_REQUEST,
            json: request_error.typed_json(),
        },
        SessionError::UnknownLine(_) => Failure::typed(UNKNOWN_LINE_CODE, &[(LINE_DATA, line)]),
        SessionError::NotASolveLine(_) => {
            Failure::typed(NOT_A_SOLVE_LINE_CODE, &[(LINE_DATA, line)])
        }
        SessionError::MachineLineNotApplicable(_) => {
            Failure::typed(MACHINE_LINE_NOT_APPLICABLE_CODE, &[(LINE_DATA, line)])
        }
        SessionError::HoldsFreeNames { names, .. } => Failure::typed(
            HOLDS_FREE_NAMES_CODE,
            &[(LINE_DATA, line), (NAMES_DATA, &names.join(", "))],
        ),
        _ => Failure::typed(COMMAND_FAILED_CODE, &[]),
    }
}

fn execute(invocation: &SessionInvocation, context: &Context<'_>) -> Result<Vec<u8>, Failure> {
    if invocation.file != STANDARD_STREAMS_FILE {
        return Err(Failure::typed(
            SAVING_NOT_SUPPORTED_CODE,
            &[(PATH_DATA, &invocation.file)],
        ));
    }
    if invocation.command == SessionCommand::Begin {
        return Session::new((context.clock)(), registered_backends())
            .save_to_bytes()
            .map_err(|_| Failure::typed(SESSION_NOT_SAVED_CODE, &[]));
    }
    let bytes =
        (context.read_standard_input)().map_err(|_| Failure::typed(INPUT_NOT_READ_CODE, &[]))?;
    let mut session = Session::open_from_bytes(&bytes, (context.clock)(), registered_backends())
        .map_err(|_| Failure::typed(SESSION_NOT_LOADED_CODE, &[]))?;
    match &invocation.command {
        SessionCommand::Begin => {}
        SessionCommand::Enter(text) => {
            session.enter(text).map_err(|error| match &error {
                SessionError::Parse(parse) => Failure {
                    code: EXIT_FAILED,
                    json: parse_error_json(parse, text),
                },
                error => session_error(error, text),
            })?;
        }
        SessionCommand::Solve(text) => {
            let request = parse_request(text.as_bytes()).map_err(|error| Failure {
                code: EXIT_INVALID_REQUEST,
                json: error.typed_json(),
            })?;
            session
                .enter_solve(&request)
                .map_err(|error| session_error(&error, ""))?;
        }
        SessionCommand::ChooseWanted { line, role } => {
            let id = session
                .resolve(line)
                .ok_or_else(|| Failure::typed(UNKNOWN_LINE_CODE, &[(LINE_DATA, line)]))?;
            session
                .choose_wanted(
                    id,
                    role,
                    SolveCriterion::program_default(),
                    CLIENT_DEFAULT_CAP,
                )
                .map_err(|error| session_error(&error, line))?;
        }
        SessionCommand::MachineLine { line, format } => {
            let id = session
                .resolve(line)
                .ok_or_else(|| Failure::typed(UNKNOWN_LINE_CODE, &[(LINE_DATA, line)]))?;
            session
                .machine_line(id, *format)
                .map_err(|error| session_error(&error, line))?;
        }
    }
    session
        .save_to_bytes()
        .map_err(|_| Failure::typed(SESSION_NOT_SAVED_CODE, &[]))
}

pub fn run_session_command(
    arguments: &[OsString],
    context: &Context<'_>,
    standard_output: &mut dyn Write,
    standard_error: &mut dyn Write,
) -> u8 {
    let result =
        parse_session_arguments(arguments).and_then(|invocation| execute(&invocation, context));
    match result {
        Ok(saved) => match standard_output.write_all(&saved) {
            Ok(()) => EXIT_ANSWERED,
            Err(_) => {
                let _ = standard_error.write_all(&typed_error_json(OUTPUT_NOT_WRITTEN_CODE, &[]));
                EXIT_FAILED
            }
        },
        Err(failure) => {
            let _ = standard_error.write_all(&failure.json);
            failure.code
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_app::{Clock, UtcTimestamp};
    use std::io;
    use std::path::Path;

    const EMPTY_SESSION: &str = "{\"format\": \"calc-session\", \"version\": 2, \"settings\": {\"precision\": \"f64\", \"backend\": \"automatic\"}, \"next_line_number\": 1, \"lines\": []}";
    const REACHABLE: &str =
        r#"{"phase": "reachable", "given": [{"name": "radius", "value": "2", "unit": "m"}]}"#;

    #[test]
    fn every_listed_session_option_is_parsed() {
        for option in OPTIONS {
            let given: Vec<OsString> = std::iter::once(STANDARD_STREAMS_FILE)
                .chain(std::iter::once(option.name))
                .chain(option.values.iter().map(|_| F64_FORMAT))
                .map(OsString::from)
                .collect();
            let outcome = parse_session_arguments(&given);
            let refused_as_unknown = outcome.err().is_some_and(|failure| {
                String::from_utf8_lossy(&failure.json).contains(UNKNOWN_OPTION_CODE)
            });
            assert!(!refused_as_unknown, "{}", option.name);
        }
    }

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

    fn run_with(arguments: &[&str], standard_input: &str) -> Ran {
        let input = standard_input.to_owned();
        let read_standard_input = move || Ok(input.clone().into_bytes());
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
        let code = run_session_command(&arguments, &context, &mut output, &mut errors);
        Ran {
            code,
            output: String::from_utf8(output).unwrap(),
            errors: String::from_utf8(errors).unwrap(),
        }
    }

    #[test]
    fn solve_option_marks_a_session_command() {
        assert!(is_session_command(&[
            OsString::from("-"),
            OsString::from("--solve")
        ]));
    }

    #[test]
    fn solve_on_standard_streams_adds_the_solve_line_to_the_saved_session() {
        let ran = run_with(&["-", "--solve", REACHABLE], EMPTY_SESSION);
        assert_eq!(ran.code, EXIT_ANSWERED);
        assert!(
            ran.output
                .contains("\"input\": \"solve {\\\"phase\\\": \\\"reachable\\\"")
        );
    }

    #[test]
    fn choose_wanted_turns_the_saved_reachable_line_into_ways() {
        let reachable = run_with(&["-", "--solve", REACHABLE], EMPTY_SESSION);
        let ran = run_with(&["-", "--choose-wanted", "r1", "area"], &reachable.output);
        assert_eq!(ran.code, EXIT_ANSWERED, "{}", ran.errors);
        assert!(ran.output.contains("\\\"wanted\\\": \\\"area\\\""));
        assert!(ran.output.contains("\"status\": \"ways\",\n"));
    }

    #[test]
    fn choose_wanted_on_an_unknown_line_is_a_typed_error() {
        let ran = run_with(&["-", "--choose-wanted", "r9", "area"], EMPTY_SESSION);
        assert_eq!((ran.code, ran.output.as_str()), (EXIT_FAILED, ""));
        assert!(ran.errors.contains("\"code\": \"unknown_line\""));
    }

    #[test]
    fn invalid_request_in_solve_exits_with_three() {
        let ran = run_with(&["-", "--solve", r#"{"phase": "ways"}"#], EMPTY_SESSION);
        assert_eq!(ran.code, EXIT_INVALID_REQUEST);
        assert!(ran.errors.contains("\"code\": \"missing_wanted\""));
    }

    fn session_with(input: &str) -> String {
        let mut session = Session::new(Box::new(FixedClock), registered_backends());
        session.enter(input).unwrap();
        String::from_utf8(session.save_to_bytes().unwrap()).unwrap()
    }

    #[test]
    fn begin_writes_a_session_with_no_lines_at_the_current_format() {
        let ran = run_with(&["-", "--begin"], "");

        assert_eq!(ran.code, EXIT_ANSWERED);
        assert!(ran.errors.is_empty());
        assert!(ran.output.contains("\"lines\": []"));
        assert!(
            ran.output
                .contains(&format!("\"version\": {}", calc_app::FORMAT_VERSION))
        );
    }

    #[test]
    fn begin_reads_no_session_from_standard_input() {
        let ran = run_with(&["-", "--begin"], "not a session at all");

        assert_eq!(ran.code, EXIT_ANSWERED);
        assert!(ran.output.contains("\"lines\": []"));
    }

    #[test]
    fn what_begin_writes_is_what_enter_reads() {
        let begun = run_with(&["-", "--begin"], "");
        let entered = run_with(&["-", "--enter", "0.1 + 0.2"], &begun.output);

        assert_eq!(entered.code, EXIT_ANSWERED);
        assert!(entered.output.contains("\"input\": \"0.1 + 0.2\","));
    }

    #[test]
    fn machine_line_adds_the_conversion_line_to_the_saved_session() {
        let ran = run_with(
            &["-", "--machine-line", "r1", "f64"],
            &session_with("0.1 + 0.2"),
        );
        assert!(ran.output.contains("\"input\": \"to_f64(r1)\","));
    }

    #[test]
    fn machine_line_with_an_unknown_format_is_a_typed_error() {
        let ran = run_with(&["-", "--machine-line", "r1", "f16"], &session_with("0.1"));
        assert!(ran.errors.contains("\"code\": \"invalid_machine_format\","));
    }

    #[test]
    fn machine_line_of_a_function_definition_is_a_typed_error() {
        let ran = run_with(
            &["-", "--machine-line", "r1", "f32"],
            &session_with("f(x) = x^2"),
        );
        assert!(
            ran.errors
                .contains("\"code\": \"machine_line_not_applicable\",")
        );
    }

    #[test]
    fn session_path_is_not_saved_yet() {
        let ran = run_with(&["lab.calc", "--solve", REACHABLE], EMPTY_SESSION);
        assert_eq!(ran.code, EXIT_FAILED);
        assert!(ran.errors.contains("\"code\": \"saving_not_supported\""));
    }

    #[test]
    fn choose_wanted_without_its_role_is_missing_option_value() {
        let ran = run_with(&["-", "--choose-wanted", "r1"], EMPTY_SESSION);
        assert!(ran.errors.contains("\"code\": \"missing_option_value\""));
    }
}
