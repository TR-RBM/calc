use std::collections::BTreeMap;

use calc_concepts::{
    DisplayUnit as ContentUnit, QuantityKind, UnitSystem as ContentSystem, is_identifier,
};
use calc_expr::ExprPool;
use calc_i18n::{Locale, Message, render};

use crate::application_place::check_kind_unit;
use crate::display_units::{UnitSystem, UnitsChoice};
use crate::session::{KindUnit, UnitOverride};

const COMPOUND_SEPARATOR: &str = " ";
const LIST_SEPARATOR: &str = ", ";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverrideProblem {
    UnknownSystem { system: String },
    UnknownKind { kind: String },
    UnresolvedUnit { kind: QuantityKind, unit: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppliedOverride {
    pub choice: UnitsChoice,
    pub problems: Vec<OverrideProblem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnitOverrideError {
    Empty,
    SystemNotAnIdentifier,
    KindNotAnIdentifier { position: usize },
    KindsNotInOrder { position: usize },
    EmptyUnit { position: usize },
}

fn content_unit_text(unit: &ContentUnit) -> String {
    match unit {
        ContentUnit::Units(parts) => parts.join(COMPOUND_SEPARATOR),
        ContentUnit::Scale(scale) => scale.name().to_owned(),
    }
}

fn content_list_text(units: &[ContentUnit]) -> String {
    units
        .iter()
        .map(content_unit_text)
        .collect::<Vec<String>>()
        .join(LIST_SEPARATOR)
}

pub(crate) fn display_systems(systems: &[ContentSystem]) -> Vec<UnitSystem> {
    systems
        .iter()
        .map(|system| UnitSystem {
            identifier: system.identifier.clone(),
            displayed: system
                .units
                .iter()
                .map(|declaration| (declaration.kind, content_list_text(&declaration.displayed)))
                .collect(),
        })
        .collect()
}

pub(crate) fn check_structure(unit_override: &UnitOverride) -> Result<(), UnitOverrideError> {
    if unit_override.system.is_none() && unit_override.overrides.is_empty() {
        return Err(UnitOverrideError::Empty);
    }
    if unit_override
        .system
        .as_deref()
        .is_some_and(|system| !is_identifier(system))
    {
        return Err(UnitOverrideError::SystemNotAnIdentifier);
    }
    let mut previous: Option<&str> = None;
    for (position, KindUnit { kind, unit }) in unit_override.overrides.iter().enumerate() {
        if !is_identifier(kind) {
            return Err(UnitOverrideError::KindNotAnIdentifier { position });
        }
        if previous.is_some_and(|previous| previous >= kind.as_str()) {
            return Err(UnitOverrideError::KindsNotInOrder { position });
        }
        if unit.is_empty() {
            return Err(UnitOverrideError::EmptyUnit { position });
        }
        previous = Some(kind);
    }
    Ok(())
}

pub(crate) fn apply_override(
    pool: &mut ExprPool,
    unit_override: &UnitOverride,
    systems: &[ContentSystem],
) -> AppliedOverride {
    let mut problems = Vec::new();
    let system = unit_override.system.as_ref().and_then(|system| {
        let known = systems.iter().any(|content| &content.identifier == system);
        if !known {
            problems.push(OverrideProblem::UnknownSystem {
                system: system.clone(),
            });
        }
        known.then(|| system.clone())
    });
    let mut overrides = BTreeMap::new();
    for KindUnit { kind, unit } in &unit_override.overrides {
        let Some(quantity_kind) = QuantityKind::from_name(kind) else {
            problems.push(OverrideProblem::UnknownKind { kind: kind.clone() });
            continue;
        };
        if check_kind_unit(pool, quantity_kind, unit).is_ok() {
            overrides.insert(quantity_kind, unit.clone());
        } else {
            problems.push(OverrideProblem::UnresolvedUnit {
                kind: quantity_kind,
                unit: unit.clone(),
            });
        }
    }
    AppliedOverride {
        choice: UnitsChoice { system, overrides },
        problems,
    }
}

pub fn quantity_kind_message(kind: QuantityKind) -> Message {
    match kind {
        QuantityKind::Length => Message::CommonQuantityKindLength,
        QuantityKind::Area => Message::CommonQuantityKindArea,
        QuantityKind::Volume => Message::CommonQuantityKindVolume,
        QuantityKind::Angle => Message::CommonQuantityKindAngle,
        QuantityKind::Count => Message::CommonQuantityKindCount,
        QuantityKind::Ratio => Message::CommonQuantityKindRatio,
        QuantityKind::Mass => Message::CommonQuantityKindMass,
        QuantityKind::Time => Message::CommonQuantityKindTime,
        QuantityKind::Speed => Message::CommonQuantityKindSpeed,
        QuantityKind::Force => Message::CommonQuantityKindForce,
        QuantityKind::Energy => Message::CommonQuantityKindEnergy,
        QuantityKind::Temperature => Message::CommonQuantityKindTemperature,
        QuantityKind::TemperatureDifference => Message::CommonQuantityKindTemperatureDifference,
    }
}

pub fn override_problem_message(problem: &OverrideProblem, locale: &Locale) -> Message {
    match problem {
        OverrideProblem::UnknownSystem { system } => Message::CommonUnitOverrideSystemNotApplied {
            system: system.clone(),
        },
        OverrideProblem::UnknownKind { kind } => {
            Message::CommonUnitOverrideKindNotApplied { kind: kind.clone() }
        }
        OverrideProblem::UnresolvedUnit { kind, unit } => {
            Message::CommonUnitOverrideUnitNotApplied {
                unit: unit.clone(),
                kind: render(&quantity_kind_message(*kind), locale)
                    .as_str()
                    .to_owned(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display_units::{Area, DecidedBy, DisplayQuestion, ValuePlace};
    use crate::session::Session;
    use crate::session::tests::session;
    use calc_i18n::{LanguageTag, Locale, render};

    fn kind_unit(kind: &str, unit: &str) -> KindUnit {
        KindUnit {
            kind: kind.to_owned(),
            unit: unit.to_owned(),
        }
    }

    fn override_of(system: Option<&str>, overrides: &[(&str, &str)]) -> UnitOverride {
        UnitOverride {
            system: system.map(str::to_owned),
            overrides: overrides
                .iter()
                .map(|(kind, unit)| kind_unit(kind, unit))
                .collect(),
        }
    }

    fn session_with(unit_override: UnitOverride) -> Session {
        let mut session = session();
        session.set_unit_override(unit_override).unwrap();
        session
    }

    fn speed_question() -> DisplayQuestion {
        DisplayQuestion {
            area: Area::Calculate,
            place: ValuePlace::Derived,
            dimension: QuantityKind::Speed.dimension(),
            stored: None,
            kind: Some(QuantityKind::Speed),
            written: None,
            stated: None,
            readable: None,
        }
    }

    fn unit_text_of(
        session: &mut Session,
        decision: &crate::display_units::DisplayDecision,
    ) -> Option<String> {
        let unit = match decision.unit.as_ref()? {
            crate::display_units::DisplayUnit::Unit(unit) => *unit,
            _ => return None,
        };
        crate::session_file::unit_text(session.pool(), unit)
            .ok()
            .flatten()
    }

    #[test]
    fn set_stores_the_override_in_the_session() {
        let unit_override = override_of(Some("si"), &[("speed", "km/h")]);

        let session = session_with(unit_override.clone());

        assert_eq!(session.unit_override(), Some(&unit_override));
    }

    #[test]
    fn clear_removes_the_override() {
        let mut session = session_with(override_of(Some("si"), &[]));

        session.clear_unit_override();

        assert_eq!(session.unit_override(), None);
    }

    #[test]
    fn empty_override_is_rejected_by_set() {
        let mut session = session();

        let result = session.set_unit_override(override_of(None, &[]));

        assert_eq!(result, Err(UnitOverrideError::Empty));
        assert_eq!(session.unit_override(), None);
    }

    #[test]
    fn kinds_out_of_order_are_rejected_by_set() {
        let result =
            session().set_unit_override(override_of(None, &[("time", "h"), ("speed", "km/h")]));

        assert_eq!(
            result,
            Err(UnitOverrideError::KindsNotInOrder { position: 1 })
        );
    }

    #[test]
    fn kind_that_is_not_an_identifier_is_rejected_by_set() {
        let result = session().set_unit_override(override_of(None, &[("Speed!", "km/h")]));

        assert_eq!(
            result,
            Err(UnitOverrideError::KindNotAnIdentifier { position: 0 })
        );
    }

    #[test]
    fn empty_unit_is_rejected_by_set() {
        let result = session().set_unit_override(override_of(None, &[("speed", "")]));

        assert_eq!(result, Err(UnitOverrideError::EmptyUnit { position: 0 }));
    }

    #[test]
    fn system_that_is_not_an_identifier_is_rejected_by_set() {
        let result = session().set_unit_override(override_of(Some("US Customary"), &[]));

        assert_eq!(result, Err(UnitOverrideError::SystemNotAnIdentifier));
    }

    #[test]
    fn scale_for_a_temperature_difference_is_a_problem() {
        let session = session_with(override_of(None, &[("temperature-difference", "celsius")]));

        let applied = session.applied_unit_override().cloned().unwrap();

        assert_eq!(
            applied.problems,
            [OverrideProblem::UnresolvedUnit {
                kind: QuantityKind::TemperatureDifference,
                unit: "celsius".to_owned(),
            }]
        );
    }

    #[test]
    fn override_that_resolves_is_applied_without_problems() {
        let session = session_with(override_of(Some("us-customary"), &[("speed", "km/h")]));

        let applied = session.applied_unit_override().cloned().unwrap();

        assert_eq!(
            applied,
            AppliedOverride {
                choice: UnitsChoice {
                    system: Some("us-customary".to_owned()),
                    overrides: BTreeMap::from([(QuantityKind::Speed, "km/h".to_owned())]),
                },
                problems: Vec::new(),
            }
        );
    }

    #[test]
    fn unknown_system_is_not_applied_and_reported() {
        let session = session_with(override_of(Some("imperial"), &[("speed", "km/h")]));

        let applied = session.applied_unit_override().cloned().unwrap();

        assert_eq!(
            (applied.choice.system, applied.problems),
            (
                None,
                vec![OverrideProblem::UnknownSystem {
                    system: "imperial".to_owned()
                }]
            )
        );
    }

    #[test]
    fn unknown_kind_is_not_applied_and_reported() {
        let session = session_with(override_of(None, &[("warmth", "K")]));

        let applied = session.applied_unit_override().cloned().unwrap();

        assert_eq!(
            applied.problems,
            [OverrideProblem::UnknownKind {
                kind: "warmth".to_owned()
            }]
        );
    }

    #[test]
    fn unit_that_does_not_resolve_is_not_applied_and_reported() {
        let session = session_with(override_of(None, &[("length", "furlong")]));

        let applied = session.applied_unit_override().cloned().unwrap();

        assert_eq!(
            (applied.choice.overrides.len(), applied.problems),
            (
                0,
                vec![OverrideProblem::UnresolvedUnit {
                    kind: QuantityKind::Length,
                    unit: "furlong".to_owned()
                }]
            )
        );
    }

    #[test]
    fn unit_of_the_wrong_dimension_for_its_kind_is_not_applied() {
        let session = session_with(override_of(None, &[("speed", "kg")]));

        let applied = session.applied_unit_override().cloned().unwrap();

        assert_eq!(applied.problems.len(), 1);
    }

    #[test]
    fn session_without_an_override_applies_none() {
        assert_eq!(session().applied_unit_override(), None);
    }

    #[test]
    fn content_unit_system_of_the_override_decides_the_displayed_unit() {
        let mut session = session_with(override_of(Some("us-customary"), &[]));

        let decision = session.display_unit(&speed_question(), None, None, false);

        assert_eq!(
            (
                decision.decided_by,
                unit_text_of(&mut session, &decision).as_deref()
            ),
            (DecidedBy::SessionOverride, Some("h^-1*mi"))
        );
    }

    #[test]
    fn session_override_comes_before_the_preference() {
        let mut session = session_with(override_of(None, &[("speed", "km/h")]));
        let preference = UnitsChoice {
            system: None,
            overrides: BTreeMap::from([(QuantityKind::Speed, "mi/h".to_owned())]),
        };

        let decision = session.display_unit(&speed_question(), Some(&preference), None, false);

        assert_eq!(decision.decided_by, DecidedBy::SessionOverride);
    }

    #[test]
    fn override_that_is_not_applied_leaves_the_decision_to_the_preference() {
        let mut session = session_with(override_of(Some("imperial"), &[("speed", "furlong")]));
        let preference = UnitsChoice {
            system: None,
            overrides: BTreeMap::from([(QuantityKind::Speed, "mi/h".to_owned())]),
        };

        let decision = session.display_unit(&speed_question(), Some(&preference), None, false);

        assert_eq!(decision.decided_by, DecidedBy::Preference);
    }

    #[test]
    fn unresolved_override_is_kept_through_saving_and_opening() {
        let unit_override = override_of(Some("imperial"), &[("warmth", "K")]);
        let session = session_with(unit_override.clone());

        let bytes = session.save_to_bytes().unwrap();
        let reopened =
            Session::open_from_bytes(&bytes, crate::session::tests::fixed_clock(), Vec::new())
                .unwrap();

        assert_eq!(reopened.unit_override(), Some(&unit_override));
    }

    #[test]
    fn english_message_names_the_system_that_was_not_found() {
        let message = override_problem_message(
            &OverrideProblem::UnknownSystem {
                system: "imperial".to_owned(),
            },
            &Locale::source(),
        );

        let text = render(&message, &Locale::source());

        assert_eq!(
            text.as_str(),
            "the unit system imperial was not found, so it is not applied"
        );
    }

    #[test]
    fn german_message_names_the_unit_and_kind_that_were_not_found() {
        let german = Locale::matching(&LanguageTag::parse("de").unwrap());
        let message = override_problem_message(
            &OverrideProblem::UnresolvedUnit {
                kind: QuantityKind::Length,
                unit: "furlong".to_owned(),
            },
            &german,
        );

        let text = render(&message, &german);

        assert_eq!(
            text.as_str(),
            "die Einheit furlong für Länge wurde nicht gefunden und wird nicht angewendet"
        );
    }

    #[test]
    fn english_unit_message_names_the_kind_in_words_not_its_identifier() {
        let message = override_problem_message(
            &OverrideProblem::UnresolvedUnit {
                kind: QuantityKind::TemperatureDifference,
                unit: "mK/s".to_owned(),
            },
            &Locale::source(),
        );

        let text = render(&message, &Locale::source());

        assert_eq!(
            text.as_str(),
            "the unit mK/s for temperature difference was not found, so it is not applied"
        );
    }

    #[test]
    fn applied_override_is_computed_when_set_and_read_without_resolving_again() {
        let mut session = session_with(override_of(None, &[("speed", "km/h")]));
        let before = session.applied_unit_override().cloned();

        session.clear_unit_override();

        assert_eq!(
            (
                before.map(|applied| applied.problems.len()),
                session.applied_unit_override()
            ),
            (Some(0), None)
        );
    }

    #[test]
    fn opened_session_has_its_override_applied() {
        let session = session_with(override_of(Some("imperial"), &[]));
        let bytes = session.save_to_bytes().unwrap();

        let reopened =
            Session::open_from_bytes(&bytes, crate::session::tests::fixed_clock(), Vec::new())
                .unwrap();

        assert_eq!(
            reopened
                .applied_unit_override()
                .map(|applied| applied.problems.len()),
            Some(1)
        );
    }

    #[test]
    fn content_systems_are_given_as_unit_texts_per_kind() {
        let concepts = crate::solve::concept_set().unwrap();

        let systems = display_systems(&concepts.unit_systems);

        let us = systems
            .iter()
            .find(|system| system.identifier == "us-customary")
            .unwrap();
        assert_eq!(
            us.displayed
                .get(&QuantityKind::Temperature)
                .map(String::as_str),
            Some("fahrenheit")
        );
    }
}
