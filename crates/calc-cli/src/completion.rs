use std::ffi::OsString;
use std::io::Write;

use calc_app::{
    ConceptLens, QuantityKind, coherent_kind_unit, concept_identifiers, concept_name,
    shipped_unit_systems, units_of_the_dimension_of,
};
use calc_i18n::{LanguageTag, Locale, Message, render, resolve_locale};

use crate::asm::ASM_COMMAND;
use crate::concept::CONCEPT_COMMAND;
use crate::language::{HELP_COMMAND, LANGUAGE_TOPIC};
use crate::plot::PLOT_COMMAND;
use crate::read::READ_COMMAND;
use crate::run::{Context, EXIT_SUCCESS, locale_sources};
use crate::session_commands::{F32_FORMAT, F64_FORMAT};
use crate::solve::SOLVE_COMMAND;

pub const COMPLETE_OPTION: &str = "--complete";

pub const EXIT_FILES_FIT: u8 = 5;

const WORD_SEPARATOR: &str = "--";
const OPTION_PREFIX: &str = "--";
const LOCALE_OPTION: &str = "--locale";
const KIND_SEPARATOR: char = '=';
const UNIT_LIST_SEPARATOR: char = ',';
const DESCRIPTION_SEPARATOR: char = '\t';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Value {
    Free,
    Locale,
    UnitSystem,
    KindUnit,
    Lens,
    MachineFormat,
    File,
}

pub(crate) struct OptionSpec {
    pub name: &'static str,
    pub values: &'static [Value],
    pub description: Message,
}

pub(crate) const fn flag(name: &'static str, description: Message) -> OptionSpec {
    OptionSpec {
        name,
        values: &[],
        description,
    }
}

pub(crate) const fn free(name: &'static str, description: Message) -> OptionSpec {
    OptionSpec {
        name,
        values: &[Value::Free],
        description,
    }
}

pub(crate) fn is_listed(options: &[OptionSpec], name: &str) -> bool {
    options.iter().any(|option| option.name == name)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Positional {
    Expression,
    Concept,
    Topic,
    File,
    Nothing,
}

struct CommandSpec {
    name: &'static str,
    description: Message,
    options: &'static [&'static [OptionSpec]],
    positional: Positional,
}

static MAIN_OPTIONS: [&[OptionSpec]; 2] =
    [crate::arguments::OPTIONS, crate::session_commands::OPTIONS];

static COMMANDS: [CommandSpec; 6] = [
    CommandSpec {
        name: SOLVE_COMMAND,
        description: Message::CliCompleteCommandSolve,
        options: &[crate::solve::OPTIONS],
        positional: Positional::Nothing,
    },
    CommandSpec {
        name: PLOT_COMMAND,
        description: Message::CliCompleteCommandPlot,
        options: &[crate::plot::OPTIONS],
        positional: Positional::File,
    },
    CommandSpec {
        name: READ_COMMAND,
        description: Message::CliCompleteCommandRead,
        options: &[crate::read::OPTIONS],
        positional: Positional::File,
    },
    CommandSpec {
        name: CONCEPT_COMMAND,
        description: Message::CliCompleteCommandConcept,
        options: &[crate::concept::OPTIONS],
        positional: Positional::Concept,
    },
    CommandSpec {
        name: ASM_COMMAND,
        description: Message::CliCompleteCommandAsm,
        options: &[crate::asm::OPTIONS],
        positional: Positional::File,
    },
    CommandSpec {
        name: HELP_COMMAND,
        description: Message::CliCompleteCommandHelp,
        options: &[crate::language::OPTIONS],
        positional: Positional::Topic,
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub word: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Completion {
    pub candidates: Vec<Candidate>,
    pub files_fit: bool,
}

impl Completion {
    fn files() -> Self {
        Self {
            candidates: Vec::new(),
            files_fit: true,
        }
    }

    fn of(candidates: Vec<Candidate>) -> Self {
        Self {
            candidates,
            files_fit: false,
        }
    }
}

fn candidate(word: &str, description: Option<String>) -> Candidate {
    Candidate {
        word: word.to_owned(),
        description,
    }
}

fn described(word: &str, message: &Message, locale: &Locale) -> Candidate {
    candidate(word, Some(render(message, locale).to_string()))
}

fn option_candidates(options: &[&[OptionSpec]], locale: &Locale) -> Vec<Candidate> {
    options
        .iter()
        .flat_map(|table| table.iter())
        .map(|option| described(option.name, &option.description, locale))
        .collect()
}

fn find_option<'table>(options: &[&'table [OptionSpec]], word: &str) -> Option<&'table OptionSpec> {
    options
        .iter()
        .flat_map(|table| table.iter())
        .find(|option| option.name == word)
}

fn units_of_kind(kind: QuantityKind, locale: &Locale) -> Vec<String> {
    let mut units: Vec<String> = coherent_kind_unit(kind)
        .map(|coherent| units_of_the_dimension_of(&coherent))
        .unwrap_or_default();
    for system in shipped_unit_systems(locale) {
        let listed = system
            .displayed
            .get(&kind)
            .map(String::as_str)
            .unwrap_or_default();
        for unit in listed.split(UNIT_LIST_SEPARATOR).map(str::trim) {
            if !unit.is_empty() && !units.iter().any(|known| known == unit) {
                units.push(unit.to_owned());
            }
        }
    }
    units
}

fn kind_unit_candidates(current: &str, locale: &Locale) -> Vec<Candidate> {
    match current.split_once(KIND_SEPARATOR) {
        Some((kind_name, _)) => QuantityKind::from_name(kind_name)
            .map(|kind| units_of_kind(kind, locale))
            .unwrap_or_default()
            .iter()
            .map(|unit| candidate(&format!("{kind_name}{KIND_SEPARATOR}{unit}"), None))
            .collect(),
        None => QuantityKind::ALL
            .iter()
            .map(|kind| candidate(&format!("{}{KIND_SEPARATOR}", kind.name()), None))
            .collect(),
    }
}

fn value_completion(value: Value, current: &str, locale: &Locale) -> Completion {
    match value {
        Value::Free => Completion::default(),
        Value::File => Completion::files(),
        Value::Locale => Completion::of(
            Locale::shipped()
                .map(|shipped| candidate(shipped.tag(), None))
                .collect(),
        ),
        Value::UnitSystem => Completion::of(
            shipped_unit_systems(locale)
                .iter()
                .map(|system| candidate(&system.identifier, Some(system.name.clone())))
                .collect(),
        ),
        Value::KindUnit => Completion::of(kind_unit_candidates(current, locale)),
        Value::Lens => Completion::of(
            ConceptLens::ALL
                .iter()
                .map(|lens| described(lens.name(), &lens.message(), locale))
                .collect(),
        ),
        Value::MachineFormat => Completion::of(vec![
            described(F64_FORMAT, &Message::CliCompleteFormatF64, locale),
            described(F32_FORMAT, &Message::CliCompleteFormatF32, locale),
        ]),
    }
}

fn positional_completion(positional: Positional, locale: &Locale) -> Completion {
    match positional {
        Positional::Expression | Positional::File => Completion::files(),
        Positional::Nothing => Completion::default(),
        Positional::Topic => Completion::of(vec![described(
            LANGUAGE_TOPIC,
            &Message::CliCompleteTopicLanguage,
            locale,
        )]),
        Positional::Concept => Completion::of(
            concept_identifiers()
                .iter()
                .map(|identifier| candidate(identifier, concept_name(identifier, locale)))
                .collect(),
        ),
    }
}

fn first_word_completion(locale: &Locale) -> Completion {
    Completion {
        candidates: COMMANDS
            .iter()
            .map(|command| described(command.name, &command.description, locale))
            .collect(),
        files_fit: true,
    }
}

pub(crate) fn complete(words: &[String], locale: &Locale) -> Completion {
    let (current, before) = match words.split_last() {
        Some((current, before)) => (current.as_str(), before),
        None => ("", words),
    };
    let command = before
        .first()
        .and_then(|first| COMMANDS.iter().find(|command| command.name == first));
    let (options, positional, given): (&[&[OptionSpec]], Positional, &[String]) = match command {
        Some(command) => (command.options, command.positional, &before[1..]),
        None => (&MAIN_OPTIONS, Positional::Expression, before),
    };
    let mut pending: &[Value] = &[];
    let mut positionals = 0_usize;
    for word in given {
        if let Some((_, rest)) = pending.split_first() {
            pending = rest;
            continue;
        }
        match find_option(options, word) {
            Some(option) => pending = option.values,
            None if !word.starts_with(OPTION_PREFIX) => positionals += 1,
            None => {}
        }
    }
    let completion = if let Some(value) = pending.first() {
        value_completion(*value, current, locale)
    } else if current.starts_with('-') {
        Completion::of(option_candidates(options, locale))
    } else if before.is_empty() {
        first_word_completion(locale)
    } else if positionals == 0 {
        positional_completion(positional, locale)
    } else {
        Completion::default()
    };
    Completion {
        candidates: completion
            .candidates
            .into_iter()
            .filter(|candidate| candidate.word.starts_with(current))
            .collect(),
        files_fit: completion.files_fit,
    }
}

fn requested_locale(words: &[String]) -> Option<LanguageTag> {
    words
        .windows(2)
        .find(|pair| pair[0] == LOCALE_OPTION)
        .and_then(|pair| LanguageTag::parse(&pair[1]).ok())
}

pub fn run_completion(
    arguments: &[OsString],
    context: &Context<'_>,
    standard_output: &mut dyn Write,
) -> u8 {
    let words: Vec<String> = arguments
        .iter()
        .skip(1)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let words = match words.split_first() {
        Some((first, rest)) if first == WORD_SEPARATOR => rest.to_vec(),
        _ => words,
    };
    let requested = requested_locale(&words);
    let locale = resolve_locale(&locale_sources(requested.as_ref(), context));
    let completion = complete(&words, &locale);
    for candidate in &completion.candidates {
        let line = match &candidate.description {
            Some(description) => format!(
                "{}{DESCRIPTION_SEPARATOR}{}",
                candidate.word,
                description.replace(DESCRIPTION_SEPARATOR, " ")
            ),
            None => candidate.word.clone(),
        };
        if writeln!(standard_output, "{line}").is_err() {
            return crate::run::EXIT_FAILURE;
        }
    }
    if completion.files_fit {
        EXIT_FILES_FIT
    } else {
        EXIT_SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(texts: &[&str]) -> Vec<String> {
        texts.iter().map(|text| (*text).to_owned()).collect()
    }

    fn offered(texts: &[&str]) -> Vec<String> {
        complete(&words(texts), &Locale::source())
            .candidates
            .into_iter()
            .map(|candidate| candidate.word)
            .collect()
    }

    #[test]
    fn the_first_word_offers_every_subcommand_and_files() {
        let completion = complete(&words(&[""]), &Locale::source());
        let offered: Vec<&str> = completion
            .candidates
            .iter()
            .map(|candidate| candidate.word.as_str())
            .collect();

        assert_eq!(offered, ["solve", "plot", "read", "concept", "asm", "help"]);
        assert!(completion.files_fit);
    }

    #[test]
    fn a_subcommand_offers_only_its_own_options() {
        let offered = offered(&["plot", "x^2", "--"]);

        assert!(offered.contains(&"--output".to_owned()));
        assert!(!offered.contains(&"--digits".to_owned()));
    }

    #[test]
    fn the_main_command_offers_its_options_by_prefix() {
        assert_eq!(offered(&["--dig"]), ["--digits"]);
    }

    #[test]
    fn a_session_command_is_offered_beside_the_main_options() {
        assert_eq!(offered(&["-", "--ent"]), ["--enter"]);
    }

    #[test]
    fn the_locale_option_offers_the_shipped_locales() {
        let offered = offered(&["--locale", ""]);

        assert!(offered.contains(&"en".to_owned()));
        assert!(offered.contains(&"de".to_owned()));
    }

    #[test]
    fn the_units_option_offers_the_unit_systems() {
        assert_eq!(offered(&["--units", ""]), ["si", "us-customary"]);
    }

    #[test]
    fn the_unit_option_offers_kinds_and_then_their_units() {
        assert!(offered(&["--unit", "spe"]).contains(&"speed=".to_owned()));
        assert!(offered(&["--unit", "speed="]).contains(&"speed=m/s".to_owned()));
    }

    #[test]
    fn a_system_listing_several_units_offers_each_of_them() {
        let offered = offered(&["--unit", "length="]);

        assert!(offered.contains(&"length=ft".to_owned()), "{offered:?}");
        assert!(
            offered.iter().all(|word| !word.contains(',')),
            "{offered:?}"
        );
    }

    #[test]
    fn the_lens_option_offers_the_four_lenses() {
        assert_eq!(
            offered(&["concept", "x", "--lens", ""]),
            ["explore", "learn", "train", "read"]
        );
    }

    #[test]
    fn the_second_value_of_a_machine_line_is_a_format() {
        assert_eq!(offered(&["-", "--machine-line", "r1", ""]), ["f64", "f32"]);
    }

    #[test]
    fn help_offers_the_language_topic() {
        assert_eq!(offered(&["help", ""]), ["language"]);
    }

    #[test]
    fn a_concept_offers_its_identifiers() {
        assert!(!offered(&["concept", ""]).is_empty());
    }

    #[test]
    fn a_value_that_takes_any_text_offers_nothing() {
        let completion = complete(&words(&["--digits", ""]), &Locale::source());

        assert_eq!(completion, Completion::default());
    }

    #[test]
    fn an_expression_position_lets_files_fit() {
        assert!(complete(&words(&["--json", ""]), &Locale::source()).files_fit);
    }

    fn arguments(texts: &[&str]) -> Vec<OsString> {
        texts.iter().map(OsString::from).collect()
    }

    fn each_option_with_values(options: &[OptionSpec]) -> Vec<Vec<&'static str>> {
        options
            .iter()
            .map(|option| {
                std::iter::once(option.name)
                    .chain(option.values.iter().map(|_| "1"))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn every_listed_main_option_is_parsed() {
        for option in each_option_with_values(crate::arguments::OPTIONS) {
            let given = arguments(&[&["1"], option.as_slice()].concat());
            let outcome = crate::arguments::parse_arguments(&given);
            assert!(
                !matches!(outcome, Err(crate::arguments::UsageError::UnknownOption(_))),
                "{option:?}"
            );
        }
    }

    #[test]
    fn every_listed_plot_option_is_parsed() {
        for option in each_option_with_values(crate::plot::OPTIONS) {
            let given = arguments(&[&["x"], option.as_slice()].concat());
            let outcome = crate::plot::parse_plot_arguments(&given);
            assert!(
                !matches!(outcome, Err(crate::plot::PlotUsageError::UnknownOption(_))),
                "{option:?}"
            );
        }
    }

    #[test]
    fn every_listed_read_option_is_parsed() {
        for option in each_option_with_values(crate::read::OPTIONS) {
            let given = arguments(&[&["lab.calc", "r1"], option.as_slice()].concat());
            let outcome = crate::read::parse_read_arguments(&given);
            assert!(
                !matches!(outcome, Err(crate::plot::PlotUsageError::UnknownOption(_))),
                "{option:?}"
            );
        }
    }

    #[test]
    fn every_listed_solve_option_is_parsed() {
        for option in each_option_with_values(crate::solve::OPTIONS) {
            let outcome = crate::solve::parse_solve_arguments(&arguments(&option));
            assert!(
                !matches!(
                    outcome,
                    Err(crate::solve::SolveUsageError::UnknownOption(_))
                ),
                "{option:?}"
            );
        }
    }

    #[test]
    fn every_listed_asm_option_is_parsed() {
        for option in each_option_with_values(crate::asm::OPTIONS) {
            let given = arguments(&[&["asm", "f.s"], option.as_slice()].concat());
            let outcome = crate::asm::parse_asm_arguments(&given);
            assert!(
                !matches!(outcome, Err(crate::asm::AsmUsageError::UnknownOption(_))),
                "{option:?}"
            );
        }
    }

    #[test]
    fn every_listed_concept_option_is_parsed() {
        for option in each_option_with_values(crate::concept::OPTIONS) {
            let given = arguments(&[&["concept", "circle"], option.as_slice()].concat());
            let outcome = crate::concept::parse_concept_arguments(&given);
            assert!(
                !matches!(
                    outcome,
                    Err(crate::concept::ConceptUsageError::UnknownOption(_))
                ),
                "{option:?}"
            );
        }
    }

    #[test]
    fn every_listed_language_option_is_parsed() {
        for option in each_option_with_values(crate::language::OPTIONS) {
            let given = arguments(&[&["help", "language"], option.as_slice()].concat());
            let outcome = crate::language::parse_language_arguments(&given);
            assert!(
                !matches!(
                    outcome,
                    Err(crate::language::LanguageUsageError::UnknownOption(_))
                ),
                "{option:?}"
            );
        }
    }
}
