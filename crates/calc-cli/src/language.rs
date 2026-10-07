use std::ffi::OsString;
use std::io::Write;

use calc_app::{
    Associativity, LocalizedReference, PrecedenceRow, PrefixEntry, ReferenceEntry, ReferenceWords,
    UnitEntry, construct_words, language_reference_json, localized_language_reference,
};
use calc_i18n::{LanguageTag, Locale, Localized, Message, render};

use crate::completion::{OptionSpec, Value, flag, is_listed};
use crate::output::Output;
use crate::run::{Context, EXIT_FAILURE, EXIT_SUCCESS, EXIT_USAGE, locale_asked_for};

pub const HELP_COMMAND: &str = "help";
pub const LANGUAGE_TOPIC: &str = "language";

const JSON_OPTION: &str = "--json";
const LOCALE_OPTION: &str = "--locale";
const OPTION_PREFIX: &str = "--";

pub(crate) static OPTIONS: &[OptionSpec] = &[
    flag(JSON_OPTION, Message::CliCompleteJson),
    OptionSpec {
        name: LOCALE_OPTION,
        values: &[Value::Locale],
        description: Message::CliCompleteLocale,
    },
];
const PADDING: char = ' ';
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageInvocation {
    pub is_json: bool,
    pub locale: Option<LanguageTag>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LanguageUsageError {
    UnknownTopic(String),
    UnknownOption(String),
    MissingOptionValue(String),
    InvalidLocale(String),
}

impl LanguageUsageError {
    fn message(&self) -> Message {
        match self {
            Self::UnknownTopic(topic) => Message::CliErrorUnknownHelpTopic {
                topic: topic.clone(),
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
        }
    }
}

pub fn is_language_help(arguments: &[OsString]) -> bool {
    let mut texts = arguments.iter().filter_map(|argument| argument.to_str());
    texts.next() == Some(HELP_COMMAND) && texts.next() == Some(LANGUAGE_TOPIC)
}

pub fn parse_language_arguments(
    arguments: &[OsString],
) -> Result<LanguageInvocation, LanguageUsageError> {
    let mut is_json = false;
    let mut locale = None;
    let mut remaining = arguments.iter().skip(2);
    while let Some(argument) = remaining.next() {
        let text = argument.to_string_lossy().into_owned();
        match text.as_str() {
            option if option.starts_with(OPTION_PREFIX) && !is_listed(OPTIONS, option) => {
                return Err(LanguageUsageError::UnknownOption(text));
            }
            JSON_OPTION => is_json = true,
            LOCALE_OPTION => {
                let given = remaining
                    .next()
                    .map(|value| value.to_string_lossy().into_owned())
                    .ok_or_else(|| {
                        LanguageUsageError::MissingOptionValue(LOCALE_OPTION.to_owned())
                    })?;
                locale = Some(
                    LanguageTag::parse(&given)
                        .map_err(|_| LanguageUsageError::InvalidLocale(given))?,
                );
            }
            option if option.starts_with(OPTION_PREFIX) => {
                return Err(LanguageUsageError::UnknownOption(text));
            }
            _ => return Err(LanguageUsageError::UnknownTopic(text)),
        }
    }
    Ok(LanguageInvocation { is_json, locale })
}

fn padded(text: &str, width: usize) -> String {
    let mut padded = text.to_owned();
    for _ in text.chars().count()..width {
        padded.push(PADDING);
    }
    padded
}

fn entry_rows(
    entry: &ReferenceEntry,
    words: Option<&ReferenceWords>,
    locale: &Locale,
) -> Vec<Localized> {
    let name = words.map_or_else(
        || entry.construct.name.to_owned(),
        |words| render(&words.name, locale).to_string(),
    );
    let heading = match words {
        Some(words) => render(
            &Message::CliLanguageEntry {
                name,
                meaning: render(&words.meaning, locale).to_string(),
            },
            locale,
        ),
        None => render(&Message::CliLanguageEntryWithoutWords { name }, locale),
    };
    let mut rows = vec![heading];
    if !entry.is_computed {
        rows.push(render(&Message::CliLanguageNotComputed, locale));
    }
    let mode_width = entry
        .spellings
        .iter()
        .map(|spelling| spelling.mode.name().chars().count())
        .max()
        .unwrap_or_default();
    let pattern_width = entry
        .spellings
        .iter()
        .map(|spelling| spelling.pattern.chars().count())
        .max()
        .unwrap_or_default();
    for spelling in &entry.spellings {
        rows.push(render(
            &Message::CliLanguageSpelling {
                mode: padded(spelling.mode.name(), mode_width),
                pattern: padded(spelling.pattern, pattern_width),
                example: spelling.example.to_owned(),
            },
            locale,
        ));
    }
    if let Some(precedence) = entry.precedence {
        let level = precedence.level.to_string();
        rows.push(render(
            &match precedence.associativity {
                Associativity::Left => Message::CliLanguagePrecedenceLeft { level },
                Associativity::Right => Message::CliLanguagePrecedenceRight { level },
                Associativity::None => Message::CliLanguagePrecedenceNone { level },
            },
            locale,
        ));
    }
    if let Some(arguments) = entry.arguments {
        rows.push(render(
            &Message::CliLanguageArguments {
                least: arguments.least.to_string(),
                largest: arguments.largest.to_string(),
            },
            locale,
        ));
        for keyword in arguments.keywords {
            rows.push(render(
                &Message::CliLanguageKeyword {
                    name: keyword.name.to_owned(),
                    value: keyword.value.name().to_owned(),
                },
                locale,
            ));
        }
    }
    rows
}

fn unit_row(unit: &UnitEntry, locale: &Locale) -> Localized {
    let symbol = unit.symbol.clone();
    let display_symbol = unit.display_symbol.clone();
    render(
        &if unit.accepts_prefix {
            Message::CliLanguageUnitWithPrefixes {
                symbol,
                display_symbol,
            }
        } else {
            Message::CliLanguageUnit {
                symbol,
                display_symbol,
            }
        },
        locale,
    )
}

fn precedence_row(
    row: &PrecedenceRow,
    words: Option<&ReferenceWords>,
    locale: &Locale,
) -> Localized {
    let name = words.map_or_else(
        || row.construct.name.to_owned(),
        |words| render(&words.name, locale).to_string(),
    );
    let level = row.level.to_string();
    render(
        &match row.associativity {
            Associativity::Left => Message::CliLanguageLevelLeft { level, name },
            Associativity::Right => Message::CliLanguageLevelRight { level, name },
            Associativity::None => Message::CliLanguageLevelNone { level, name },
        },
        locale,
    )
}

fn prefix_row(prefix: &PrefixEntry, locale: &Locale) -> Localized {
    render(
        &Message::CliLanguagePrefix {
            symbol: prefix.symbol.to_owned(),
            exponent: prefix.exponent.to_string(),
        },
        locale,
    )
}

pub fn reference_rows(reference: &LocalizedReference, locale: &Locale) -> Vec<Localized> {
    let mut rows = Vec::new();
    for section in &reference.sections {
        if section.entries.is_empty() {
            continue;
        }
        rows.push(render(&section.heading, locale));
        for (entry, words) in &section.entries {
            rows.extend(entry_rows(entry, words.as_ref(), locale));
        }
    }
    rows.push(render(&Message::CliLanguagePrecedenceTable, locale));
    for row in &reference.reference.precedence {
        rows.push(precedence_row(
            row,
            construct_words(&row.construct).as_ref(),
            locale,
        ));
    }
    rows.push(render(&Message::CliLanguageUnits, locale));
    for unit in &reference.reference.units {
        rows.push(unit_row(unit, locale));
    }
    rows.push(render(&Message::CliLanguagePrefixes, locale));
    for prefix in &reference.reference.prefixes {
        rows.push(prefix_row(prefix, locale));
    }
    rows
}

pub fn run_language_help(
    arguments: &[OsString],
    context: &Context<'_>,
    standard_output: &mut dyn Write,
    standard_error: &mut dyn Write,
) -> u8 {
    let mut output = Output::new(standard_output);
    let mut errors = Output::new(standard_error);
    let invocation = match parse_language_arguments(arguments) {
        Ok(invocation) => invocation,
        Err(error) => {
            let locale = match error {
                LanguageUsageError::InvalidLocale(_) => Locale::source(),
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
    let reference = localized_language_reference();
    let written = if invocation.is_json {
        output.json(&language_reference_json(&reference))
    } else {
        reference_rows(&reference, &locale)
            .iter()
            .try_for_each(|row| output.line(row))
    };
    match written {
        Ok(()) => EXIT_SUCCESS,
        Err(_) => {
            let _ = errors.line(&render(&Message::CliErrorOutputFailed, &locale));
            EXIT_FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(texts: &[&str]) -> Vec<OsString> {
        texts.iter().map(OsString::from).collect()
    }

    #[test]
    fn help_language_is_the_language_reference() {
        assert!(is_language_help(&arguments(&["help", "language"])));
    }

    #[test]
    fn help_without_a_topic_is_not_the_language_reference() {
        assert!(!is_language_help(&arguments(&["help"])));
    }

    #[test]
    fn the_json_option_is_read() {
        let invocation = parse_language_arguments(&arguments(&["help", "language", "--json"]))
            .expect("the arguments parse");

        assert!(invocation.is_json);
    }

    #[test]
    fn a_second_topic_is_a_usage_error() {
        let outcome = parse_language_arguments(&arguments(&["help", "language", "units"])).err();

        assert_eq!(
            outcome,
            Some(LanguageUsageError::UnknownTopic("units".to_owned()))
        );
    }

    #[test]
    fn an_unknown_option_is_a_usage_error() {
        let outcome = parse_language_arguments(&arguments(&["help", "language", "--rows"])).err();

        assert_eq!(
            outcome,
            Some(LanguageUsageError::UnknownOption("--rows".to_owned()))
        );
    }

    #[test]
    fn a_locale_without_a_value_is_a_usage_error() {
        let outcome = parse_language_arguments(&arguments(&["help", "language", "--locale"])).err();

        assert_eq!(
            outcome,
            Some(LanguageUsageError::MissingOptionValue(
                "--locale".to_owned()
            ))
        );
    }
}
