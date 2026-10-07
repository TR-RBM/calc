use std::ffi::OsString;
use std::path::PathBuf;

use calc_app::LineInstrument;
use calc_i18n::{LanguageTag, Message};

use crate::completion::{OptionSpec, Value, flag, free, is_listed};

const JSON_OPTION: &str = "--json";
pub(crate) const LOCALE_OPTION: &str = "--locale";
const HELP_OPTION: &str = "--help";
const VERSION_OPTION: &str = "--version";
const DIGITS_OPTION: &str = "--digits";
const ENCLOSE_OPTION: &str = "--enclose";
const HOW_IT_RAN_OPTION: &str = "--how-it-ran";
const INSPECT_OPTION: &str = "--inspect";
const UNITS_OPTION: &str = "--units";
const UNIT_OPTION: &str = "--unit";
const COHERENT_UNITS_OPTION: &str = "--coherent-units";
const WORKING_OPTION: &str = "--working";
const REPLAY_OPTION: &str = "--replay";
const BATCH_OPTION: &str = "--batch";
const RECOGNIZE_OPTION: &str = "--recognize";
const CHECK_OPTION: &str = "--check";
const TERSE_OPTION: &str = "--terse";
const FIND_OPTION: &str = "--find";
const COUNTER_OPTION: &str = "--counter";
const IDENTITY_OPTION: &str = "--identity";
const EXPAND_OPTION: &str = "--expand";
const FACTOR_OPTION: &str = "--factor";
const SOLVE_FOR_OPTION: &str = "--solve-for";
const ODE_OPTION: &str = "--ode";
const INITIAL_OPTION: &str = "--initial";
const AT_OPTION: &str = "--at";
const TRACE_OPTION: &str = "--trace";
const PATH_SEPARATOR: char = '.';
const OPTION_PREFIX: &str = "--";

pub(crate) static OPTIONS: &[OptionSpec] = &[
    flag(JSON_OPTION, Message::CliCompleteJson),
    OptionSpec {
        name: LOCALE_OPTION,
        values: &[Value::Locale],
        description: Message::CliCompleteLocale,
    },
    flag(HELP_OPTION, Message::CliCompleteHelp),
    flag(VERSION_OPTION, Message::CliCompleteVersion),
    free(DIGITS_OPTION, Message::CliCompleteDigits),
    free(ENCLOSE_OPTION, Message::CliCompleteEnclose),
    flag(WORKING_OPTION, Message::CliCompleteWorking),
    flag(INSPECT_OPTION, Message::CliCompleteInspect),
    OptionSpec {
        name: UNITS_OPTION,
        values: &[Value::UnitSystem],
        description: Message::CliCompleteUnits,
    },
    OptionSpec {
        name: UNIT_OPTION,
        values: &[Value::KindUnit],
        description: Message::CliCompleteUnit,
    },
    flag(COHERENT_UNITS_OPTION, Message::CliCompleteCoherentUnits),
    flag(HOW_IT_RAN_OPTION, Message::CliCompleteHowItRan),
    flag(REPLAY_OPTION, Message::CliCompleteReplay),
    flag(BATCH_OPTION, Message::CliCompleteBatch),
    flag(RECOGNIZE_OPTION, Message::CliCompleteRecognize),
    flag(CHECK_OPTION, Message::CliCompleteCheck),
    flag(TERSE_OPTION, Message::CliCompleteTerse),
    free(FIND_OPTION, Message::CliCompleteFind),
    flag(COUNTER_OPTION, Message::CliCompleteCounter),
    flag(IDENTITY_OPTION, Message::CliCompleteIdentity),
    flag(EXPAND_OPTION, Message::CliCompleteExpand),
    flag(FACTOR_OPTION, Message::CliCompleteFactor),
    flag(SOLVE_FOR_OPTION, Message::CliCompleteSolveFor),
    free(ODE_OPTION, Message::CliCompleteOde),
    free(INITIAL_OPTION, Message::CliCompleteInitial),
    free(AT_OPTION, Message::CliCompleteOdeAt),
    flag(TRACE_OPTION, Message::CliCompleteTrace),
];
const SESSION_FILE_EXTENSION: &str = ".calc";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    Expression(String),
    SessionFile(PathBuf),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewRequest {
    Digits(u32),
    Enclose(u32),
    Working(Vec<usize>),
}

fn is_path(text: &str) -> bool {
    !text.is_empty()
        && text.split(PATH_SEPARATOR).all(|part| {
            !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
        })
}

fn parse_range(text: &str) -> Option<(String, i64, i64)> {
    let (name, bounds) = text.split_once('=')?;
    let (from, to) = bounds.split_once("..")?;
    let from = from.trim().parse::<i64>().ok()?;
    let to = to.trim().parse::<i64>().ok()?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    Some((name.to_owned(), from, to))
}

fn path_of(text: &str) -> Option<Vec<usize>> {
    text.split(PATH_SEPARATOR)
        .map(|part| part.parse::<usize>().ok())
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum UnitsRequest {
    #[default]
    Unset,
    Chosen {
        system: Option<String>,
        overrides: Vec<String>,
    },
    Coherent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invocation {
    pub input: Option<Input>,
    pub line: Option<String>,
    pub view: Option<ViewRequest>,
    pub instrument: Option<LineInstrument>,
    pub units: UnitsRequest,
    pub is_json: bool,
    pub is_how_it_ran: bool,
    pub is_trace: bool,
    pub is_replay: bool,
    pub is_batch: bool,
    pub is_recognize: bool,
    pub is_check: bool,
    pub is_terse: bool,
    pub find: Vec<(String, i64, i64)>,
    pub is_counter: bool,
    pub is_identity: bool,
    pub is_expand: bool,
    pub is_factor: bool,
    pub is_solve_for: bool,
    pub ode: Option<OdeRequest>,
    pub is_help: bool,
    pub is_version: bool,
    pub locale: Option<LanguageTag>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OdeRequest {
    pub system: String,
    pub initial: String,
    pub at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UsageError {
    InvalidPath(String),
    NoInput,
    UnknownOption(String),
    MissingOptionValue(String),
    UnexpectedArgument(String),
    InvalidLocale(String),
    InvalidCount { option: String, value: String },
    ViewWithoutLine,
    OptionGivenTwice(String),
    TwoFormsOfOneLine,
    CoherentUnitsWithChosenUnits,
    DescendingRange { value: String, ascending: String },
    OdeWithout(String),
}

impl UsageError {
    pub fn message(&self) -> Message {
        match self {
            Self::InvalidPath(value) => Message::CliErrorInvalidPath {
                value: value.clone(),
            },
            Self::NoInput => Message::CliErrorNoInput,
            Self::UnknownOption(option) => Message::CliErrorUnknownOption {
                option: option.clone(),
            },
            Self::MissingOptionValue(option) => Message::CliErrorMissingOptionValue {
                option: option.clone(),
            },
            Self::UnexpectedArgument(argument) => Message::CliErrorUnexpectedArgument {
                argument: argument.clone(),
            },
            Self::InvalidLocale(value) => Message::CliErrorInvalidLocale {
                value: value.clone(),
            },
            Self::InvalidCount { option, value } => Message::CliErrorInvalidCount {
                option: option.clone(),
                value: value.clone(),
            },
            Self::ViewWithoutLine => Message::CliErrorViewWithoutLine,
            Self::OptionGivenTwice(option) => Message::CliErrorOptionGivenTwice {
                option: option.clone(),
            },
            Self::TwoFormsOfOneLine => Message::CliErrorTwoFormsOfOneLine,
            Self::CoherentUnitsWithChosenUnits => Message::CliErrorCoherentUnitsWithChosenUnits,
            Self::DescendingRange { value, ascending } => Message::CliErrorDescendingRange {
                value: value.clone(),
                ascending: ascending.clone(),
            },
            Self::OdeWithout(option) => Message::CliErrorOdeWithout {
                option: option.clone(),
            },
        }
    }
}

pub fn parse_arguments(arguments: &[OsString]) -> Result<Invocation, UsageError> {
    let mut invocation = Invocation {
        input: None,
        line: None,
        view: None,
        instrument: None,
        units: UnitsRequest::Unset,
        is_json: false,
        is_how_it_ran: false,
        is_trace: false,
        is_replay: false,
        is_batch: false,
        is_recognize: false,
        is_check: false,
        is_terse: false,
        find: Vec::new(),
        is_counter: false,
        is_identity: false,
        is_expand: false,
        is_factor: false,
        is_solve_for: false,
        ode: None,
        is_help: false,
        is_version: false,
        locale: None,
    };
    let mut system: Option<String> = None;
    let mut overrides: Vec<String> = Vec::new();
    let mut is_coherent = false;
    let mut ode_parts: [Option<String>; 3] = [None, None, None];
    let mut remaining = arguments.iter().peekable();
    while let Some(argument) = remaining.next() {
        let text = argument.to_string_lossy().into_owned();
        match text.as_str() {
            option if option.starts_with(OPTION_PREFIX) && !is_listed(OPTIONS, option) => {
                return Err(UsageError::UnknownOption(text));
            }
            JSON_OPTION => invocation.is_json = true,
            HOW_IT_RAN_OPTION => invocation.is_how_it_ran = true,
            TRACE_OPTION => invocation.is_trace = true,
            REPLAY_OPTION => invocation.is_replay = true,
            BATCH_OPTION => invocation.is_batch = true,
            RECOGNIZE_OPTION => invocation.is_recognize = true,
            CHECK_OPTION => invocation.is_check = true,
            TERSE_OPTION => invocation.is_terse = true,
            COUNTER_OPTION => invocation.is_counter = true,
            IDENTITY_OPTION => invocation.is_identity = true,
            EXPAND_OPTION => invocation.is_expand = true,
            FACTOR_OPTION => invocation.is_factor = true,
            SOLVE_FOR_OPTION => invocation.is_solve_for = true,
            FIND_OPTION => {
                let value = remaining
                    .next()
                    .ok_or_else(|| UsageError::MissingOptionValue(text.clone()))?
                    .to_string_lossy()
                    .into_owned();
                let range = parse_range(&value).ok_or_else(|| UsageError::InvalidCount {
                    option: text.clone(),
                    value: value.clone(),
                })?;
                let (name, from, to) = &range;
                if from > to {
                    return Err(UsageError::DescendingRange {
                        value,
                        ascending: format!("{name}={to}..{from}"),
                    });
                }
                invocation.find.push(range);
            }
            ODE_OPTION | INITIAL_OPTION | AT_OPTION => {
                let value = remaining
                    .next()
                    .ok_or_else(|| UsageError::MissingOptionValue(text.clone()))?
                    .to_string_lossy()
                    .into_owned();
                let slot = match text.as_str() {
                    ODE_OPTION => 0,
                    INITIAL_OPTION => 1,
                    _ => 2,
                };
                if ode_parts[slot].replace(value).is_some() {
                    return Err(UsageError::OptionGivenTwice(text));
                }
            }
            HELP_OPTION => invocation.is_help = true,
            VERSION_OPTION => invocation.is_version = true,
            LOCALE_OPTION => {
                let value = remaining
                    .next()
                    .ok_or_else(|| UsageError::MissingOptionValue(text.clone()))?
                    .to_string_lossy()
                    .into_owned();
                let tag =
                    LanguageTag::parse(&value).map_err(|_| UsageError::InvalidLocale(value))?;
                invocation.locale = Some(tag);
            }
            INSPECT_OPTION => {
                if invocation
                    .instrument
                    .replace(LineInstrument::Inspect)
                    .is_some()
                {
                    return Err(UsageError::OptionGivenTwice(text));
                }
            }
            COHERENT_UNITS_OPTION => is_coherent = true,
            UNITS_OPTION => {
                let value = remaining
                    .next()
                    .ok_or_else(|| UsageError::MissingOptionValue(text.clone()))?
                    .to_string_lossy()
                    .into_owned();
                if system.replace(value).is_some() {
                    return Err(UsageError::OptionGivenTwice(text));
                }
            }
            UNIT_OPTION => {
                let value = remaining
                    .next()
                    .ok_or_else(|| UsageError::MissingOptionValue(text.clone()))?
                    .to_string_lossy()
                    .into_owned();
                overrides.push(value);
            }
            WORKING_OPTION => {
                let path = match remaining.peek().and_then(|next| next.to_str()) {
                    Some(next) if !next.starts_with(OPTION_PREFIX) && is_path(next) => {
                        let given = next.to_owned();
                        remaining.next();
                        path_of(&given).ok_or(UsageError::InvalidPath(given))?
                    }
                    _ => Vec::new(),
                };
                if invocation
                    .view
                    .replace(ViewRequest::Working(path))
                    .is_some()
                {
                    return Err(UsageError::UnexpectedArgument(text));
                }
            }
            DIGITS_OPTION | ENCLOSE_OPTION => {
                let value = remaining
                    .next()
                    .ok_or_else(|| UsageError::MissingOptionValue(text.clone()))?
                    .to_string_lossy()
                    .into_owned();
                let count = value.parse::<u32>().map_err(|_| UsageError::InvalidCount {
                    option: text.clone(),
                    value: value.clone(),
                })?;
                let view = if text == DIGITS_OPTION {
                    ViewRequest::Digits(count)
                } else {
                    ViewRequest::Enclose(count)
                };
                if invocation.view.replace(view).is_some() {
                    return Err(UsageError::UnexpectedArgument(text));
                }
            }
            option if option.starts_with(OPTION_PREFIX) => {
                return Err(UsageError::UnknownOption(text));
            }
            _ if matches!(invocation.input, Some(Input::SessionFile(_)))
                && invocation.line.is_none() =>
            {
                invocation.line = Some(text);
            }
            _ if invocation.input.is_some() => return Err(UsageError::UnexpectedArgument(text)),
            _ if argument.to_str().is_none() => return Err(UsageError::UnexpectedArgument(text)),
            _ if text.ends_with(SESSION_FILE_EXTENSION) => {
                invocation.input = Some(Input::SessionFile(PathBuf::from(argument)));
            }
            _ => invocation.input = Some(Input::Expression(text)),
        }
    }
    invocation.ode = match ode_parts {
        [None, None, None] => None,
        [Some(system), Some(initial), Some(at)] => Some(OdeRequest {
            system,
            initial,
            at,
        }),
        [system, initial, _] => {
            let missing = if system.is_none() {
                ODE_OPTION
            } else if initial.is_none() {
                INITIAL_OPTION
            } else {
                AT_OPTION
            };
            return Err(UsageError::OdeWithout(missing.to_owned()));
        }
    };
    let chooses_units = system.is_some() || !overrides.is_empty();
    invocation.units = match (is_coherent, chooses_units) {
        (true, true) => return Err(UsageError::CoherentUnitsWithChosenUnits),
        (true, false) => UnitsRequest::Coherent,
        (false, true) => UnitsRequest::Chosen { system, overrides },
        (false, false) => UnitsRequest::Unset,
    };
    if invocation.input.is_none()
        && !invocation.is_help
        && !invocation.is_version
        && !invocation.is_batch
        && invocation.find.is_empty()
        && invocation.ode.is_none()
    {
        return Err(UsageError::NoInput);
    }
    if invocation.view.is_some() && invocation.instrument.is_some() {
        return Err(UsageError::TwoFormsOfOneLine);
    }
    let is_file = matches!(invocation.input, Some(Input::SessionFile(_)));
    let shows_one_line = invocation.view.is_some() || invocation.instrument.is_some();
    if is_file && invocation.line.is_some() != shows_one_line {
        return Err(match &invocation.line {
            Some(line) => UsageError::UnexpectedArgument(line.clone()),
            None => UsageError::ViewWithoutLine,
        });
    }
    Ok(invocation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(texts: &[&str]) -> Vec<OsString> {
        texts.iter().map(OsString::from).collect()
    }

    #[test]
    fn plain_argument_is_an_expression() {
        let invocation = parse_arguments(&arguments(&["1 + 2"])).unwrap();
        assert_eq!(
            invocation.input,
            Some(Input::Expression("1 + 2".to_owned()))
        );
    }

    #[test]
    fn argument_ending_in_calc_is_a_session_file() {
        let invocation = parse_arguments(&arguments(&["lab.calc"])).unwrap();
        assert_eq!(
            invocation.input,
            Some(Input::SessionFile(PathBuf::from("lab.calc")))
        );
    }

    #[test]
    fn negative_number_is_an_expression_not_an_option() {
        let invocation = parse_arguments(&arguments(&["-2"])).unwrap();
        assert_eq!(invocation.input, Some(Input::Expression("-2".to_owned())));
    }

    #[test]
    fn json_option_sets_json_output() {
        let invocation = parse_arguments(&arguments(&["2", "--json"])).unwrap();
        assert!(invocation.is_json);
    }

    #[test]
    fn locale_option_takes_a_language_tag() {
        let invocation = parse_arguments(&arguments(&["--locale", "de-DE", "2"])).unwrap();
        assert_eq!(invocation.locale.unwrap().as_str(), "de-DE");
    }

    #[test]
    fn help_needs_no_input() {
        let invocation = parse_arguments(&arguments(&["--help"])).unwrap();
        assert!(invocation.is_help);
    }

    #[test]
    fn no_arguments_is_no_input_error() {
        assert_eq!(parse_arguments(&[]), Err(UsageError::NoInput));
    }

    #[test]
    fn unknown_double_dash_option_is_unknown_option_error() {
        assert_eq!(
            parse_arguments(&arguments(&["--fast", "2"])),
            Err(UsageError::UnknownOption("--fast".to_owned()))
        );
    }

    #[test]
    fn locale_without_value_is_missing_option_value_error() {
        assert_eq!(
            parse_arguments(&arguments(&["2", "--locale"])),
            Err(UsageError::MissingOptionValue("--locale".to_owned()))
        );
    }

    #[test]
    fn second_input_is_unexpected_argument_error() {
        assert_eq!(
            parse_arguments(&arguments(&["2", "3"])),
            Err(UsageError::UnexpectedArgument("3".to_owned()))
        );
    }

    #[test]
    fn digits_option_asks_for_the_digits_view() {
        let invocation = parse_arguments(&arguments(&["1/3", "--digits", "5"])).unwrap();
        assert_eq!(invocation.view, Some(ViewRequest::Digits(5)));
    }

    #[test]
    fn enclose_option_asks_for_the_enclosure_view() {
        let invocation = parse_arguments(&arguments(&["sqrt(2)", "--enclose", "30"])).unwrap();
        assert_eq!(invocation.view, Some(ViewRequest::Enclose(30)));
    }

    #[test]
    fn session_file_view_names_its_line() {
        let invocation = parse_arguments(&arguments(&["lab.calc", "r2", "--digits", "3"])).unwrap();
        assert_eq!(invocation.line.as_deref(), Some("r2"));
    }

    #[test]
    fn view_count_that_is_not_a_number_is_invalid_count_error() {
        assert_eq!(
            parse_arguments(&arguments(&["1/3", "--digits", "five"])),
            Err(UsageError::InvalidCount {
                option: "--digits".to_owned(),
                value: "five".to_owned()
            })
        );
    }

    #[test]
    fn session_file_view_without_a_line_is_an_error() {
        assert_eq!(
            parse_arguments(&arguments(&["lab.calc", "--enclose", "3"])),
            Err(UsageError::ViewWithoutLine)
        );
    }

    #[test]
    fn session_file_line_without_a_view_is_unexpected() {
        assert_eq!(
            parse_arguments(&arguments(&["lab.calc", "r2"])),
            Err(UsageError::UnexpectedArgument("r2".to_owned()))
        );
    }

    #[test]
    fn units_option_names_the_system() {
        let invocation = parse_arguments(&arguments(&["2 m", "--units", "si"])).unwrap();
        assert_eq!(
            invocation.units,
            UnitsRequest::Chosen {
                system: Some("si".to_owned()),
                overrides: Vec::new()
            }
        );
    }

    #[test]
    fn unit_option_collects_one_entry_per_kind() {
        let invocation = parse_arguments(&arguments(&[
            "2 m",
            "--unit",
            "length=km",
            "--unit",
            "mass=g",
        ]))
        .unwrap();
        assert_eq!(
            invocation.units,
            UnitsRequest::Chosen {
                system: None,
                overrides: vec!["length=km".to_owned(), "mass=g".to_owned()]
            }
        );
    }

    #[test]
    fn coherent_units_option_asks_for_the_coherent_unit() {
        let invocation = parse_arguments(&arguments(&["2 m", "--coherent-units"])).unwrap();
        assert_eq!(invocation.units, UnitsRequest::Coherent);
    }

    #[test]
    fn no_unit_option_leaves_the_request_unset() {
        let invocation = parse_arguments(&arguments(&["2 m"])).unwrap();
        assert_eq!(invocation.units, UnitsRequest::Unset);
    }

    #[test]
    fn coherent_units_beside_a_chosen_unit_is_an_error() {
        assert_eq!(
            parse_arguments(&arguments(&[
                "2 m",
                "--coherent-units",
                "--unit",
                "length=km"
            ])),
            Err(UsageError::CoherentUnitsWithChosenUnits)
        );
    }

    #[test]
    fn units_option_given_twice_is_an_error() {
        assert_eq!(
            parse_arguments(&arguments(&[
                "2 m",
                "--units",
                "si",
                "--units",
                "us-customary"
            ])),
            Err(UsageError::OptionGivenTwice("--units".to_owned()))
        );
    }

    #[test]
    fn unit_option_without_a_value_is_missing_option_value_error() {
        assert_eq!(
            parse_arguments(&arguments(&["2 m", "--unit"])),
            Err(UsageError::MissingOptionValue("--unit".to_owned()))
        );
    }

    #[test]
    fn malformed_locale_is_invalid_locale_error() {
        assert_eq!(
            parse_arguments(&arguments(&["--locale", "de_DE", "2"])),
            Err(UsageError::InvalidLocale("de_DE".to_owned()))
        );
    }
}
