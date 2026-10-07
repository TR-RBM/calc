use std::ffi::OsString;
use std::io::Write;

use calc_app::{ConceptLens, ConceptOutcome, concept, concept_error_message, concept_json};
use calc_i18n::{LanguageTag, Locale, Localized, Message, render};

use crate::completion::{OptionSpec, Value, flag, is_listed};
use crate::output::Output;
use crate::run::{Context, EXIT_FAILURE, EXIT_SUCCESS, EXIT_USAGE, locale_asked_for};

pub const CONCEPT_COMMAND: &str = "concept";

const LENS_OPTION: &str = "--lens";
const JSON_OPTION: &str = "--json";
const LOCALE_OPTION: &str = "--locale";

pub(crate) static OPTIONS: &[OptionSpec] = &[
    OptionSpec {
        name: LENS_OPTION,
        values: &[Value::Lens],
        description: Message::CliCompleteLens,
    },
    flag(JSON_OPTION, Message::CliCompleteJson),
    OptionSpec {
        name: LOCALE_OPTION,
        values: &[Value::Locale],
        description: Message::CliCompleteLocale,
    },
];
const OPTION_PREFIX: &str = "--";
const LIST_SEPARATOR: &str = ", ";
const TERMINAL_WIDTH: usize = 80;
const ROW_INDENT: usize = 2;
const LABEL_GAP: usize = 2;
const PADDING: char = ' ';

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConceptInvocation {
    pub identifier: String,
    pub lens: Option<ConceptLens>,
    pub is_json: bool,
    pub locale: Option<LanguageTag>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConceptUsageError {
    NoIdentifier,
    UnknownLens(String),
    UnknownOption(String),
    MissingOptionValue(String),
    InvalidLocale(String),
    UnexpectedArgument(String),
}

impl ConceptUsageError {
    fn message(&self) -> Message {
        match self {
            Self::NoIdentifier => Message::CliErrorConceptNeedsIdentifier,
            Self::UnknownLens(value) => Message::CliErrorUnknownLens {
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

pub fn is_concept_command(arguments: &[OsString]) -> bool {
    arguments
        .first()
        .and_then(|first| first.to_str())
        .is_some_and(|first| first == CONCEPT_COMMAND)
}

pub fn parse_concept_arguments(
    arguments: &[OsString],
) -> Result<ConceptInvocation, ConceptUsageError> {
    let mut identifier = None;
    let mut lens = None;
    let mut is_json = false;
    let mut locale = None;
    let mut remaining = arguments.iter().skip(1);
    while let Some(argument) = remaining.next() {
        let text = argument.to_string_lossy().into_owned();
        match text.as_str() {
            option if option.starts_with(OPTION_PREFIX) && !is_listed(OPTIONS, option) => {
                return Err(ConceptUsageError::UnknownOption(text));
            }
            JSON_OPTION => is_json = true,
            LENS_OPTION => {
                let given = remaining
                    .next()
                    .map(|value| value.to_string_lossy().into_owned())
                    .ok_or_else(|| ConceptUsageError::MissingOptionValue(text.clone()))?;
                lens = Some(
                    ConceptLens::from_name(&given).ok_or(ConceptUsageError::UnknownLens(given))?,
                );
            }
            LOCALE_OPTION => {
                let given = remaining
                    .next()
                    .map(|value| value.to_string_lossy().into_owned())
                    .ok_or_else(|| ConceptUsageError::MissingOptionValue(text.clone()))?;
                locale = Some(
                    LanguageTag::parse(&given)
                        .map_err(|_| ConceptUsageError::InvalidLocale(given))?,
                );
            }
            option if option.starts_with(OPTION_PREFIX) => {
                return Err(ConceptUsageError::UnknownOption(text));
            }
            _ if identifier.is_none() => identifier = Some(text),
            _ => return Err(ConceptUsageError::UnexpectedArgument(text)),
        }
    }
    Ok(ConceptInvocation {
        identifier: identifier.ok_or(ConceptUsageError::NoIdentifier)?,
        lens,
        is_json,
        locale,
    })
}

fn wrapped(value: &str, label_cells: usize) -> Vec<String> {
    let room = TERMINAL_WIDTH.saturating_sub(label_cells + ROW_INDENT + LABEL_GAP);
    let mut rows = Vec::new();
    let mut row = String::new();
    for word in calc_app::prose_groups(value) {
        let candidate = if row.is_empty() {
            word.clone()
        } else {
            format!("{row} {word}")
        };
        if !row.is_empty() && candidate.chars().count() > room {
            rows.push(row);
            row = word;
        } else {
            row = candidate;
        }
    }
    rows.push(row);
    rows
}

fn kind_message(kind: calc_app::ActivityKind) -> Message {
    match kind {
        calc_app::ActivityKind::ShapeMatching => Message::CommonActivityShapeMatching,
    }
}

fn variation_message(variation: calc_app::ActivityVariation) -> Message {
    match variation {
        calc_app::ActivityVariation::Identical => Message::CommonActivityVariationIdentical,
        calc_app::ActivityVariation::Orientation => Message::CommonActivityVariationOrientation,
        calc_app::ActivityVariation::Size => Message::CommonActivityVariationSize,
    }
}

fn shape_message(shape: calc_app::ActivityShape) -> Message {
    match shape {
        calc_app::ActivityShape::Circle => Message::CommonActivityShapeCircle,
        calc_app::ActivityShape::Square => Message::CommonActivityShapeSquare,
        calc_app::ActivityShape::Triangle => Message::CommonActivityShapeTriangle,
    }
}

fn concept_rows(outcome: &ConceptOutcome, locale: &Locale) -> Vec<Localized> {
    let text = |message: &Message| render(message, locale).to_string();
    let name = outcome.wording.as_ref().map_or_else(
        || outcome.identifier.clone(),
        |wording| wording.name.clone(),
    );
    let mut rows = vec![render(
        &Message::CliConceptHeading {
            identifier: outcome.identifier.clone(),
            name,
        },
        locale,
    )];
    let mut fields: Vec<(String, String)> = Vec::new();
    if let Some(wording) = &outcome.wording {
        fields.push((
            text(&Message::CommonConceptStatement),
            wording.statement.clone(),
        ));
        for (label, value) in [
            (Message::CommonConceptIntuition, wording.intuition.as_ref()),
            (Message::CommonConceptExamples, wording.examples.as_ref()),
            (
                Message::CommonConceptMisconception,
                wording.misconception.as_ref(),
            ),
        ] {
            if let Some(value) = value {
                fields.push((text(&label), value.clone()));
            }
        }
    }
    fields.push((
        text(&Message::CommonConceptLens),
        text(&outcome.lens.message()),
    ));
    fields.push((
        text(&Message::CommonConceptShownBy),
        if outcome.shown_by.is_empty() {
            text(&Message::CommonConceptShownByNone)
        } else {
            outcome
                .shown_by
                .iter()
                .map(|lens| text(&lens.message()))
                .collect::<Vec<String>>()
                .join(LIST_SEPARATOR)
        },
    ));
    for (label, values) in [
        (Message::CommonConceptPrerequisites, &outcome.prerequisites),
        (Message::CommonConceptSources, &outcome.sources),
        (Message::CommonConceptExercises, &outcome.exercises),
    ] {
        if !values.is_empty() {
            fields.push((text(&label), values.join(LIST_SEPARATOR)));
        }
    }
    if !outcome.activities.is_empty() {
        let activities: Vec<String> = outcome
            .activities
            .iter()
            .map(|activity| {
                let shapes: Vec<String> = activity
                    .shapes
                    .iter()
                    .map(|shape| text(&shape_message(*shape)))
                    .collect();
                text(&Message::CommonConceptActivity {
                    kind: text(&kind_message(activity.kind)),
                    shapes: shapes.join(LIST_SEPARATOR),
                    variation: text(&variation_message(activity.variation)),
                })
            })
            .collect();
        fields.push((
            text(&Message::CommonConceptActivities),
            activities.join("; "),
        ));
    }
    let width = fields
        .iter()
        .map(|(label, _)| label.chars().count())
        .max()
        .unwrap_or_default();
    for (label, value) in fields {
        let padding = PADDING.to_string().repeat(width - label.chars().count());
        let label = format!("{label}{padding}");
        let blank = PADDING.to_string().repeat(label.chars().count());
        for (position, line) in wrapped(&value, label.chars().count())
            .into_iter()
            .enumerate()
        {
            rows.push(render(
                &Message::CliConceptRow {
                    label: if position == 0 {
                        label.clone()
                    } else {
                        blank.clone()
                    },
                    value: line,
                },
                locale,
            ));
        }
    }
    if !outcome.shown_by.contains(&outcome.lens) {
        rows.push(render(
            &Message::CliConceptLensEmpty {
                lens: text(&outcome.lens.message()),
            },
            locale,
        ));
    }
    rows
}

pub fn run_concept(
    arguments: &[OsString],
    context: &Context<'_>,
    standard_output: &mut dyn Write,
    standard_error: &mut dyn Write,
) -> u8 {
    let mut output = Output::new(standard_output);
    let mut errors = Output::new(standard_error);
    let invocation = match parse_concept_arguments(arguments) {
        Ok(invocation) => invocation,
        Err(error) => {
            let locale = match error {
                ConceptUsageError::InvalidLocale(_) => Locale::source(),
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
    let outcome = match concept(&invocation.identifier, invocation.lens, None, &locale) {
        Ok(outcome) => outcome,
        Err(error) => {
            let detail = render(&concept_error_message(&error), &locale).to_string();
            let _ = errors.line(&render(&Message::CliError { detail }, &locale));
            return EXIT_FAILURE;
        }
    };
    let written = if invocation.is_json {
        output.json(&concept_json(&outcome))
    } else {
        concept_rows(&outcome, &locale)
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

    #[test]
    fn a_row_never_begins_with_the_sign_that_would_have_overflowed_the_one_before() {
        let label_cells = 10;
        let room = TERMINAL_WIDTH - label_cells - ROW_INDENT - LABEL_GAP;
        let text = format!("{} b = c", "a".repeat(room - 3));

        let rows = wrapped(&text, label_cells);

        assert_eq!(rows, vec!["a".repeat(room - 3), "b = c".to_string()]);
    }
}
