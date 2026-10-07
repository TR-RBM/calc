use calc_app::{
    KindUnitProblem, QuantityKind, UnitsChoice, UnitsChoiceError, checked_kind_unit,
    kind_unit_problem_message, shipped_unit_systems,
};
use calc_i18n::{Locale, Message};

use crate::arguments::UnitsRequest;

const LIST_SEPARATOR: &str = ", ";
const OVERRIDE_SEPARATOR: char = '=';

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisplayUnits {
    pub choice: Option<UnitsChoice>,
    pub coherent_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnitsProblem {
    UnknownSystem(String),
    NotAKindAndUnit(String),
    UnknownKind(String),
    KindGivenTwice(QuantityKind),
    UnitNotOfKind {
        kind: QuantityKind,
        problem: KindUnitProblem,
    },
}

impl UnitsProblem {
    pub fn message(&self, locale: &Locale) -> Message {
        match self {
            Self::UnknownSystem(value) => Message::CliErrorUnknownUnitSystem {
                value: value.clone(),
                systems: known_systems(locale),
            },
            Self::NotAKindAndUnit(value) => Message::CliErrorUnitNeedsKindAndUnit {
                value: value.clone(),
            },
            Self::UnknownKind(value) => Message::CliErrorUnknownQuantityKind {
                value: value.clone(),
                kinds: known_kinds(),
            },
            Self::KindGivenTwice(kind) => Message::CliErrorKindGivenTwice {
                kind: kind.name().to_owned(),
            },
            Self::UnitNotOfKind { kind, problem } => {
                kind_unit_problem_message(*kind, problem, locale)
            }
        }
    }
}

fn known_systems(locale: &Locale) -> String {
    shipped_unit_systems(locale)
        .iter()
        .map(|system| system.identifier.clone())
        .collect::<Vec<String>>()
        .join(LIST_SEPARATOR)
}

fn known_kinds() -> String {
    QuantityKind::ALL
        .iter()
        .map(|kind| kind.name())
        .collect::<Vec<&str>>()
        .join(LIST_SEPARATOR)
}

fn kind_text(entry: &str) -> String {
    entry
        .split_once(OVERRIDE_SEPARATOR)
        .map_or(entry, |(kind, _)| kind)
        .trim()
        .to_owned()
}

fn choice_problem(error: &UnitsChoiceError, overrides: &[&str]) -> UnitsProblem {
    let entry = |position: usize| overrides.get(position).copied().unwrap_or_default();
    match error {
        UnitsChoiceError::MissingSeparator { position }
        | UnitsChoiceError::EmptyUnit { position } => {
            UnitsProblem::NotAKindAndUnit(entry(*position).to_owned())
        }
        UnitsChoiceError::UnknownKind { position } => {
            UnitsProblem::UnknownKind(kind_text(entry(*position)))
        }
        UnitsChoiceError::RepeatedKind { position } => {
            match QuantityKind::from_name(&kind_text(entry(*position))) {
                Some(kind) => UnitsProblem::KindGivenTwice(kind),
                None => UnitsProblem::UnknownKind(kind_text(entry(*position))),
            }
        }
    }
}

pub fn display_units(
    request: &UnitsRequest,
    locale: &Locale,
) -> Result<DisplayUnits, UnitsProblem> {
    let (system, overrides) = match request {
        UnitsRequest::Unset => return Ok(DisplayUnits::default()),
        UnitsRequest::Coherent => {
            return Ok(DisplayUnits {
                choice: None,
                coherent_only: true,
            });
        }
        UnitsRequest::Chosen { system, overrides } => (system, overrides),
    };
    if let Some(system) = system
        && !shipped_unit_systems(locale)
            .iter()
            .any(|shipped| shipped.identifier == *system)
    {
        return Err(UnitsProblem::UnknownSystem(system.clone()));
    }
    let entries: Vec<&str> = overrides.iter().map(String::as_str).collect();
    let choice = UnitsChoice::from_settings(system.as_deref(), &entries)
        .map_err(|error| choice_problem(&error, &entries))?;
    for (kind, unit) in &choice.overrides {
        checked_kind_unit(*kind, unit).map_err(|problem| UnitsProblem::UnitNotOfKind {
            kind: *kind,
            problem,
        })?;
    }
    Ok(DisplayUnits {
        choice: Some(choice),
        coherent_only: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chosen(system: Option<&str>, overrides: &[&str]) -> UnitsRequest {
        UnitsRequest::Chosen {
            system: system.map(str::to_owned),
            overrides: overrides.iter().map(|entry| (*entry).to_owned()).collect(),
        }
    }

    fn resolved(request: &UnitsRequest) -> Result<DisplayUnits, UnitsProblem> {
        display_units(request, &Locale::source())
    }

    #[test]
    fn nothing_asked_for_leaves_the_display_alone() {
        assert_eq!(resolved(&UnitsRequest::Unset), Ok(DisplayUnits::default()));
    }

    #[test]
    fn coherent_units_ask_for_the_coherent_unit_and_no_choice() {
        let units = resolved(&UnitsRequest::Coherent).unwrap();

        assert!(units.coherent_only && units.choice.is_none());
    }

    #[test]
    fn an_override_becomes_the_unit_of_its_kind() {
        let units = resolved(&chosen(None, &["length=km"])).unwrap();

        assert_eq!(
            units.choice.unwrap().overrides.get(&QuantityKind::Length),
            Some(&"km".to_owned())
        );
    }

    #[test]
    fn an_unknown_system_names_itself() {
        assert_eq!(
            resolved(&chosen(Some("metric-ish"), &[])),
            Err(UnitsProblem::UnknownSystem("metric-ish".to_owned()))
        );
    }

    #[test]
    fn an_override_without_a_separator_is_not_a_kind_and_unit() {
        assert_eq!(
            resolved(&chosen(None, &["km"])),
            Err(UnitsProblem::NotAKindAndUnit("km".to_owned()))
        );
    }

    #[test]
    fn an_override_with_no_unit_is_not_a_kind_and_unit() {
        assert_eq!(
            resolved(&chosen(None, &["length="])),
            Err(UnitsProblem::NotAKindAndUnit("length=".to_owned()))
        );
    }

    #[test]
    fn an_unknown_kind_names_what_was_written() {
        assert_eq!(
            resolved(&chosen(None, &["distance=km"])),
            Err(UnitsProblem::UnknownKind("distance".to_owned()))
        );
    }

    #[test]
    fn the_same_kind_twice_names_the_kind() {
        assert_eq!(
            resolved(&chosen(None, &["length=km", "length=cm"])),
            Err(UnitsProblem::KindGivenTwice(QuantityKind::Length))
        );
    }

    #[test]
    fn a_unit_of_another_kind_says_which_kind_it_is_not() {
        assert_eq!(
            resolved(&chosen(None, &["length=kg"])),
            Err(UnitsProblem::UnitNotOfKind {
                kind: QuantityKind::Length,
                problem: KindUnitProblem::OtherDimension {
                    unit: "kg".to_owned()
                }
            })
        );
    }
}
