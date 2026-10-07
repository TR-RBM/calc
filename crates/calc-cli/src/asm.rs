use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Write;
use std::path::Path;

use calc_app::{
    FunctionReport, assembly_case_text, assembly_input_text, assembly_refusal_text,
    assembly_reports, is_argument_register, is_general_register,
};
use calc_i18n::{LanguageTag, Locale, Localized, Message, render};

use crate::completion::{OptionSpec, Value, free, is_listed};
use crate::output::Output;
use crate::run::{Context, EXIT_FAILURE, EXIT_SUCCESS, EXIT_USAGE, locale_asked_for};

pub const ASM_COMMAND: &str = "asm";

const GIVEN_OPTION: &str = "--given";
const FUNCTION_OPTION: &str = "--function";
const LOCALE_OPTION: &str = "--locale";

pub(crate) static OPTIONS: &[OptionSpec] = &[
    free(GIVEN_OPTION, Message::CliCompleteGiven),
    free(FUNCTION_OPTION, Message::CliCompleteFunction),
    OptionSpec {
        name: LOCALE_OPTION,
        values: &[Value::Locale],
        description: Message::CliCompleteLocale,
    },
];
const OPTION_PREFIX: &str = "--";
const STANDARD_INPUT: &str = "-";
const GIVEN_SEPARATOR: char = '=';
const LIST_SEPARATOR: &str = ", ";
const PADDING: char = ' ';
const HEXADECIMAL_PREFIX: &str = "0x";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsmInvocation {
    pub input: String,
    pub given: BTreeMap<String, i128>,
    pub function: Option<String>,
    pub locale: Option<LanguageTag>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AsmUsageError {
    NoInput,
    Given(String),
    UnknownOption(String),
    MissingOptionValue(String),
    InvalidLocale(String),
    UnexpectedArgument(String),
}

impl AsmUsageError {
    fn message(&self) -> Message {
        match self {
            Self::NoInput => Message::CliErrorAsmNeedsInput,
            Self::Given(value) => Message::CliErrorAsmGiven {
                value: value.clone(),
            },
            Self::UnknownOption(option) => Message::CliErrorUnknownOption {
                option: option.clone(),
            },
            Self::MissingOptionValue(option) => Message::CliErrorMissingOptionValue {
                option: option.clone(),
            },
            Self::InvalidLocale(value) => Message::CliErrorInvalidLocale {
                value: value.clone(),
            },
            Self::UnexpectedArgument(argument) => Message::CliErrorUnexpectedArgument {
                argument: argument.clone(),
            },
        }
    }
}

fn given_value(text: &str) -> Option<i128> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let magnitude = match digits.strip_prefix(HEXADECIMAL_PREFIX) {
        Some(hexadecimal) => i128::from_str_radix(hexadecimal, 16).ok()?,
        None => digits.parse::<i128>().ok()?,
    };
    Some(if negative { -magnitude } else { magnitude })
}

pub fn parse_asm_arguments(arguments: &[OsString]) -> Result<AsmInvocation, AsmUsageError> {
    let mut input = None;
    let mut given = BTreeMap::new();
    let mut function = None;
    let mut locale = None;
    let mut remaining = arguments.iter().skip(1);
    while let Some(argument) = remaining.next() {
        let text = argument.to_string_lossy().into_owned();
        let mut value = || {
            remaining
                .next()
                .map(|value| value.to_string_lossy().into_owned())
                .ok_or_else(|| AsmUsageError::MissingOptionValue(text.clone()))
        };
        match text.as_str() {
            option if option.starts_with(OPTION_PREFIX) && !is_listed(OPTIONS, option) => {
                return Err(AsmUsageError::UnknownOption(text));
            }
            GIVEN_OPTION => {
                let pair = value()?;
                let (name, number) = pair
                    .split_once(GIVEN_SEPARATOR)
                    .ok_or_else(|| AsmUsageError::Given(pair.clone()))?;
                let number =
                    given_value(number.trim()).ok_or_else(|| AsmUsageError::Given(pair.clone()))?;
                if !is_argument_register(name) && !is_general_register(name) {
                    return Err(AsmUsageError::Given(pair.clone()));
                }
                given.insert(name.to_owned(), number);
            }
            FUNCTION_OPTION => function = Some(value()?),
            LOCALE_OPTION => {
                let tag = value()?;
                locale =
                    Some(LanguageTag::parse(&tag).map_err(|_| AsmUsageError::InvalidLocale(tag))?);
            }
            STANDARD_INPUT if input.is_none() => input = Some(text),
            option if option.starts_with(OPTION_PREFIX) => {
                return Err(AsmUsageError::UnknownOption(text));
            }
            _ if input.is_none() => input = Some(text),
            _ => return Err(AsmUsageError::UnexpectedArgument(text)),
        }
    }
    Ok(AsmInvocation {
        input: input.ok_or(AsmUsageError::NoInput)?,
        given,
        function,
        locale,
    })
}

fn function_rows(report: &FunctionReport, locale: &Locale) -> Vec<Localized> {
    let text = |message: &Message| render(message, locale).to_string();
    let mut fields: Vec<(String, String)> = Vec::new();
    match &report.outcome {
        Ok(cases) => {
            for case in cases {
                let label = if case.conditions.is_empty() {
                    text(&Message::CliAsmLabelAlways)
                } else {
                    case.conditions.join(LIST_SEPARATOR)
                };
                fields.push((label, assembly_case_text(case, locale)));
            }
        }
        Err(refusal) => fields.push((
            text(&Message::CliAsmLabelCount),
            assembly_refusal_text(refusal, locale),
        )),
    }
    for input in &report.inputs {
        fields.push((input.clone(), assembly_input_text(input, locale)));
    }
    if !report.outside.is_empty() {
        let names: Vec<String> = report
            .outside
            .iter()
            .map(|name| {
                if name.is_empty() {
                    text(&Message::CliAsmIndirectCall)
                } else if name == calc_app::UNRESOLVED_CALL {
                    text(&Message::CliAsmUnresolvedCall)
                } else {
                    name.clone()
                }
            })
            .collect();
        fields.push((
            text(&Message::CliAsmLabelOutside),
            text(&Message::CliAsmOutside {
                names: names.join(LIST_SEPARATOR),
            }),
        ));
    }
    if report.has_loops {
        fields.push((
            text(&Message::CliAsmLabelAssumes),
            text(&Message::CliAsmAssumesNoOverflow),
        ));
    }
    if report.assumes_separate_stack {
        fields.push((
            text(&Message::CliAsmLabelAssumes),
            text(&Message::CliAsmAssumesSeparateStack),
        ));
    }
    fields.push((
        text(&Message::CliAsmLabelCycles),
        text(&Message::CliAsmCyclesX8664),
    ));
    let width = fields
        .iter()
        .map(|(label, _)| label.chars().count())
        .max()
        .unwrap_or_default();
    let mut rows = vec![render(
        &Message::CliAsmHeading {
            name: report.name.clone(),
        },
        locale,
    )];
    for (label, value) in fields {
        let padding = PADDING.to_string().repeat(width - label.chars().count());
        rows.push(render(
            &Message::CliAsmRow {
                label: format!("{label}{padding}"),
                value,
            },
            locale,
        ));
    }
    rows
}

pub fn run_asm(
    arguments: &[OsString],
    context: &Context<'_>,
    standard_output: &mut dyn Write,
    standard_error: &mut dyn Write,
) -> u8 {
    let mut output = Output::new(standard_output);
    let mut errors = Output::new(standard_error);
    let invocation = match parse_asm_arguments(arguments) {
        Ok(invocation) => invocation,
        Err(error) => {
            let locale = match error {
                AsmUsageError::InvalidLocale(_) => Locale::source(),
                _ => crate::run::usage_error_locale(arguments, context),
            };
            let detail = render(&error.message(), &locale).to_string();
            let _ = errors
                .line(&render(&Message::CliError { detail }, &locale))
                .and_then(|()| errors.line(&render(&Message::CliHelpUsage, &locale)));
            return EXIT_USAGE;
        }
    };
    let locale = locale_asked_for(invocation.locale.as_ref(), context, &mut errors);
    let fail = |errors: &mut Output<'_>, message: Message| {
        let detail = render(&message, &locale).to_string();
        let _ = errors.line(&render(&Message::CliError { detail }, &locale));
        EXIT_FAILURE
    };
    let bytes = if invocation.input == STANDARD_INPUT {
        (context.read_standard_input)()
    } else {
        (context.read_file)(Path::new(&invocation.input))
    };
    let source = match bytes.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()) {
        Ok(source) => source,
        Err(_) => {
            return fail(
                &mut errors,
                Message::ErrorFileUnreadable {
                    path: invocation.input.clone(),
                },
            );
        }
    };
    let mut reports = assembly_reports(&source, &invocation.given);
    if reports.is_empty() {
        return fail(
            &mut errors,
            Message::CliErrorAsmNoFunctions {
                path: invocation.input.clone(),
            },
        );
    }
    if let Some(name) = &invocation.function {
        reports.retain(|report| report.name == *name);
        if reports.is_empty() {
            return fail(
                &mut errors,
                Message::CliErrorAsmNoFunction {
                    name: name.clone(),
                    path: invocation.input.clone(),
                },
            );
        }
    }
    let counted = reports.iter().all(|report| {
        report
            .outcome
            .as_ref()
            .is_ok_and(|cases| cases.iter().all(|case| case.outcome.is_ok()))
    });
    let written = reports
        .iter()
        .flat_map(|report| function_rows(report, &locale))
        .try_for_each(|row| output.line(&row));
    match (written, counted) {
        (Ok(()), true) => EXIT_SUCCESS,
        _ => EXIT_FAILURE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn a_given_register_takes_a_whole_number() {
        let invocation =
            parse_asm_arguments(&arguments(&["asm", "f.s", "--given", "edx=14"])).unwrap();
        assert_eq!(invocation.given.get("edx"), Some(&14));
    }

    #[test]
    fn a_given_that_is_not_a_register_is_a_usage_error() {
        let outcome = parse_asm_arguments(&arguments(&["asm", "f.s", "--given", "n=14"]));
        assert_eq!(outcome, Err(AsmUsageError::Given("n=14".to_owned())));
    }

    #[test]
    fn asm_needs_an_input() {
        assert_eq!(
            parse_asm_arguments(&arguments(&["asm"])),
            Err(AsmUsageError::NoInput)
        );
    }
}
