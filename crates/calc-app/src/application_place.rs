use std::collections::BTreeMap;

use calc_concepts::{
    ConceptSet, DisplayUnit as ContentUnit, DisplayUnitError, QuantityKind, check_display_unit,
};
use calc_expr::ExprPool;
use calc_i18n::{Locale, Message, render};

use crate::display_units::{display_dimension, resolve_display_unit};
use crate::solve::concept_set;
use crate::unit_display::input_text;
use crate::unit_override::quantity_kind_message;

const SOURCE_LOCALE: &str = "en";
const COMPOUND_SEPARATOR: &str = " ";
const LIST_SEPARATOR: &str = ", ";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerDoor {
    SessionBarSelectors,
    SessionBarMenuUnitOverride,
    LearnLandingCurriculumChip,
}

impl PointerDoor {
    pub fn needs_current_session(self) -> bool {
        matches!(
            self,
            PointerDoor::SessionBarSelectors | PointerDoor::SessionBarMenuUnitOverride
        )
    }

    pub fn message(self) -> Message {
        match self {
            PointerDoor::SessionBarSelectors => Message::UiPointerDoorSessionBar,
            PointerDoor::SessionBarMenuUnitOverride => Message::UiPointerDoorSessionBarMenu,
            PointerDoor::LearnLandingCurriculumChip => Message::UiPointerDoorLearnLanding,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pointer {
    PrecisionAndBackend,
    UnitsForThisSession,
    Curriculum,
    DegreesOrRadians,
    DecimalPlaces,
    DefaultPrecision,
    DecimalComma,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerEntry {
    pub pointer: Pointer,
    pub door: Option<PointerDoor>,
}

impl Pointer {
    pub const ALL: [Pointer; 7] = [
        Pointer::PrecisionAndBackend,
        Pointer::UnitsForThisSession,
        Pointer::Curriculum,
        Pointer::DegreesOrRadians,
        Pointer::DecimalPlaces,
        Pointer::DefaultPrecision,
        Pointer::DecimalComma,
    ];

    pub fn door(self) -> Option<PointerDoor> {
        match self {
            Pointer::PrecisionAndBackend => Some(PointerDoor::SessionBarSelectors),
            Pointer::UnitsForThisSession => Some(PointerDoor::SessionBarMenuUnitOverride),
            Pointer::Curriculum => Some(PointerDoor::LearnLandingCurriculumChip),
            Pointer::DegreesOrRadians
            | Pointer::DecimalPlaces
            | Pointer::DefaultPrecision
            | Pointer::DecimalComma => None,
        }
    }

    pub fn is_answer(self) -> bool {
        self.door().is_none()
    }

    pub fn name(self) -> Message {
        match self {
            Pointer::PrecisionAndBackend => Message::UiPointerPrecisionAndBackend,
            Pointer::UnitsForThisSession => Message::UiPointerUnitsForThisSession,
            Pointer::Curriculum => Message::UiPointerCurriculum,
            Pointer::DegreesOrRadians => Message::UiPointerDegreesOrRadians,
            Pointer::DecimalPlaces => Message::UiPointerDecimalPlaces,
            Pointer::DefaultPrecision => Message::UiPointerDefaultPrecision,
            Pointer::DecimalComma => Message::UiPointerDecimalComma,
        }
    }

    pub fn words(self) -> Message {
        match self {
            Pointer::PrecisionAndBackend => Message::UiPointerPrecisionAndBackendWords,
            Pointer::UnitsForThisSession => Message::UiPointerUnitsForThisSessionWords,
            Pointer::Curriculum => Message::UiPointerCurriculumWords,
            Pointer::DegreesOrRadians => Message::UiPointerDegreesOrRadiansWords,
            Pointer::DecimalPlaces => Message::UiPointerDecimalPlacesWords,
            Pointer::DefaultPrecision => Message::UiPointerDefaultPrecisionWords,
            Pointer::DecimalComma => Message::UiPointerDecimalCommaWords,
        }
    }

    pub fn sentence(self) -> Message {
        match self {
            Pointer::PrecisionAndBackend => Message::UiPointerPrecisionAndBackendSentence,
            Pointer::UnitsForThisSession => Message::UiPointerUnitsForThisSessionSentence,
            Pointer::Curriculum => Message::UiPointerCurriculumSentence,
            Pointer::DegreesOrRadians => Message::UiPointerDegreesOrRadiansSentence,
            Pointer::DecimalPlaces => Message::UiPointerDecimalPlacesSentence,
            Pointer::DefaultPrecision => Message::UiPointerDefaultPrecisionSentence,
            Pointer::DecimalComma => Message::UiPointerDecimalCommaSentence,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceContext {
    pub session_current: bool,
    pub exam: bool,
}

pub fn pointer_block(context: PlaceContext) -> Vec<PointerEntry> {
    if context.exam {
        return Vec::new();
    }
    Pointer::ALL
        .into_iter()
        .map(|pointer| PointerEntry {
            pointer,
            door: pointer
                .door()
                .filter(|door| context.session_current || !door.needs_current_session()),
        })
        .collect()
}

pub fn search_answers() -> Vec<Pointer> {
    Pointer::ALL
        .into_iter()
        .filter(|pointer| pointer.is_answer())
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShippedUnitSystem {
    pub identifier: String,
    pub name: String,
    pub displayed: BTreeMap<QuantityKind, String>,
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

fn shipped_systems_of(concepts: &ConceptSet, locale: &Locale) -> Vec<ShippedUnitSystem> {
    let mut systems: Vec<ShippedUnitSystem> = concepts
        .unit_systems
        .iter()
        .map(|system| ShippedUnitSystem {
            identifier: system.identifier.clone(),
            name: [locale.tag(), SOURCE_LOCALE]
                .into_iter()
                .find_map(|tag| system.names.get(tag))
                .unwrap_or(&system.identifier)
                .clone(),
            displayed: system
                .units
                .iter()
                .map(|declaration| (declaration.kind, content_list_text(&declaration.displayed)))
                .collect(),
        })
        .collect();
    systems.sort_by(|left, right| left.identifier.cmp(&right.identifier));
    systems
}

pub fn shipped_unit_systems(locale: &Locale) -> Vec<ShippedUnitSystem> {
    concept_set()
        .map(|concepts| shipped_systems_of(concepts, locale))
        .unwrap_or_default()
}

pub fn override_kinds(systems: &[ShippedUnitSystem], locale: &Locale) -> Vec<QuantityKind> {
    let mut kinds: Vec<(String, QuantityKind)> = QuantityKind::ALL
        .into_iter()
        .filter(|kind| {
            systems
                .iter()
                .any(|system| system.displayed.contains_key(kind))
        })
        .map(|kind| {
            let name = render(&quantity_kind_message(kind), locale)
                .as_str()
                .to_owned();
            (name, kind)
        })
        .collect();
    kinds.sort();
    kinds.into_iter().map(|(_, kind)| kind).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KindUnitProblem {
    NotAUnit { unit: String },
    OtherDimension { unit: String },
    CompoundNotAllowed,
    CompoundNotDescending { unit: String },
    ScaleRequired,
    NotADisplayUnit { unit: String },
}

pub fn check_kind_unit(
    pool: &mut ExprPool,
    kind: QuantityKind,
    text: &str,
) -> Result<String, KindUnitProblem> {
    let normalized = text
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(COMPOUND_SEPARATOR);
    check_display_unit(pool, kind, &normalized).map_err(|error| match error {
        DisplayUnitError::Invalid(_) if kind == QuantityKind::Temperature => {
            KindUnitProblem::ScaleRequired
        }
        DisplayUnitError::Invalid(unit) => KindUnitProblem::NotAUnit { unit },
        DisplayUnitError::OtherDimension(unit) => KindUnitProblem::OtherDimension { unit },
        DisplayUnitError::CompoundNotAllowed => KindUnitProblem::CompoundNotAllowed,
        DisplayUnitError::CompoundNotDescending(unit) => {
            KindUnitProblem::CompoundNotDescending { unit }
        }
    })?;
    let applies = resolve_display_unit(pool, &normalized, Some(kind))
        .is_some_and(|resolved| display_dimension(pool, &resolved) == Some(kind.dimension()));
    if applies {
        Ok(normalized)
    } else {
        Err(KindUnitProblem::NotADisplayUnit { unit: normalized })
    }
}

pub fn checked_kind_unit(kind: QuantityKind, text: &str) -> Result<String, KindUnitProblem> {
    check_kind_unit(&mut ExprPool::new(), kind, text)
}

pub fn coherent_kind_unit(kind: QuantityKind) -> Option<String> {
    let mut pool = ExprPool::new();
    let unit = pool.units_mut().coherent_unit(&kind.dimension()).ok()?;
    input_text(&pool, unit)
}

pub fn kind_unit_problem_message(
    kind: QuantityKind,
    problem: &KindUnitProblem,
    locale: &Locale,
) -> Message {
    let kind = render(&quantity_kind_message(kind), locale)
        .as_str()
        .to_owned();
    match problem {
        KindUnitProblem::NotAUnit { unit } => {
            Message::CommonKindUnitNotAUnit { unit: unit.clone() }
        }
        KindUnitProblem::OtherDimension { unit } => Message::CommonKindUnitOtherDimension {
            unit: unit.clone(),
            kind,
        },
        KindUnitProblem::CompoundNotAllowed => Message::CommonKindUnitCompoundNotAllowed { kind },
        KindUnitProblem::CompoundNotDescending { unit } => {
            Message::CommonKindUnitCompoundNotDescending { unit: unit.clone() }
        }
        KindUnitProblem::ScaleRequired => Message::CommonKindUnitScaleRequired { kind },
        KindUnitProblem::NotADisplayUnit { unit } => Message::CommonKindUnitNotADisplayUnit {
            unit: unit.clone(),
            kind,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_i18n::LanguageTag;

    fn german() -> Locale {
        Locale::matching(&LanguageTag::parse("de").unwrap())
    }

    #[test]
    fn the_coherent_unit_of_a_kind_is_the_product_of_its_base_units() {
        assert_eq!(
            coherent_kind_unit(QuantityKind::Force),
            Some("kg*m/s^2".to_owned())
        );
    }

    #[test]
    fn the_coherent_unit_of_a_base_kind_is_its_base_unit() {
        assert_eq!(
            coherent_kind_unit(QuantityKind::Length),
            Some("m".to_owned())
        );
    }

    #[test]
    fn a_dimensionless_kind_has_no_coherent_unit_to_name() {
        assert_eq!(coherent_kind_unit(QuantityKind::Angle), None);
    }

    fn check(kind: QuantityKind, text: &str) -> Result<String, KindUnitProblem> {
        check_kind_unit(&mut ExprPool::new(), kind, text)
    }

    fn system(identifier: &str, kinds: &[QuantityKind]) -> ShippedUnitSystem {
        ShippedUnitSystem {
            identifier: identifier.to_owned(),
            name: identifier.to_owned(),
            displayed: kinds
                .iter()
                .map(|kind| (*kind, kind.name().to_owned()))
                .collect(),
        }
    }

    #[test]
    fn pointers_are_the_doors_then_the_answers_of_the_table() {
        let kinds: Vec<bool> = Pointer::ALL
            .iter()
            .map(|pointer| pointer.is_answer())
            .collect();

        assert_eq!(kinds, [false, false, false, true, true, true, true]);
    }

    #[test]
    fn door_pointers_lead_to_their_controls() {
        let doors: Vec<Option<PointerDoor>> = Pointer::ALL[..3]
            .iter()
            .map(|pointer| pointer.door())
            .collect();

        assert_eq!(
            doors,
            [
                Some(PointerDoor::SessionBarSelectors),
                Some(PointerDoor::SessionBarMenuUnitOverride),
                Some(PointerDoor::LearnLandingCurriculumChip),
            ]
        );
    }

    #[test]
    fn block_without_a_session_keeps_every_pointer_and_only_the_curriculum_door() {
        let block = pointer_block(PlaceContext {
            session_current: false,
            exam: false,
        });

        let doors: Vec<Option<PointerDoor>> = block.iter().map(|entry| entry.door).collect();
        assert_eq!(
            doors,
            [
                None,
                None,
                Some(PointerDoor::LearnLandingCurriculumChip),
                None,
                None,
                None,
                None
            ]
        );
    }

    #[test]
    fn block_with_a_session_has_every_door() {
        let block = pointer_block(PlaceContext {
            session_current: true,
            exam: false,
        });

        let doors: Vec<Option<PointerDoor>> = block.iter().map(|entry| entry.door).collect();
        assert_eq!(doors, Pointer::ALL.map(Pointer::door).to_vec());
    }

    #[test]
    fn exam_layer_has_no_block() {
        let block = pointer_block(PlaceContext {
            session_current: true,
            exam: true,
        });

        assert!(block.is_empty());
    }

    #[test]
    fn search_holds_the_answer_pointers_only() {
        assert_eq!(
            search_answers(),
            [
                Pointer::DegreesOrRadians,
                Pointer::DecimalPlaces,
                Pointer::DefaultPrecision,
                Pointer::DecimalComma
            ]
        );
    }

    #[test]
    fn angle_answer_says_the_unit_is_written_with_the_number() {
        let sentence = render(&Pointer::DegreesOrRadians.sentence(), &Locale::source());

        assert!(sentence.as_str().contains("sin(30 deg)"));
    }

    #[test]
    fn every_pointer_has_distinct_name_words_and_sentence_in_every_locale() {
        for locale in Locale::shipped() {
            for pointer in Pointer::ALL {
                let texts = [pointer.name(), pointer.words(), pointer.sentence()]
                    .map(|message| render(&message, &locale).as_str().to_owned());
                assert!(texts[0] != texts[1] && texts[1] != texts[2] && texts[0] != texts[2]);
            }
        }
    }

    #[test]
    fn door_names_where_it_leads() {
        let name = render(
            &PointerDoor::LearnLandingCurriculumChip.message(),
            &german(),
        );

        assert_eq!(name.as_str(), "zur Startseite von Lernen");
    }

    #[test]
    fn shipped_unit_systems_are_in_file_name_order() {
        let identifiers: Vec<String> = shipped_unit_systems(&Locale::source())
            .into_iter()
            .map(|system| system.identifier)
            .collect();

        assert_eq!(identifiers, ["si", "us-customary"]);
    }

    #[test]
    fn unit_system_is_named_in_the_locale() {
        let systems = shipped_unit_systems(&german());

        assert_eq!(systems[1].name, "US-amerikanische Maßeinheiten");
    }

    #[test]
    fn unit_system_lists_its_displayed_unit_per_kind() {
        let systems = shipped_unit_systems(&Locale::source());

        assert_eq!(
            systems[1]
                .displayed
                .get(&QuantityKind::Speed)
                .map(String::as_str),
            Some("mi/h")
        );
    }

    #[test]
    fn override_kinds_are_those_of_the_systems_ordered_by_name_in_the_locale() {
        let systems = [
            system("a", &[QuantityKind::Angle, QuantityKind::Mass]),
            system("b", &[QuantityKind::Force, QuantityKind::Angle]),
        ];

        assert_eq!(
            override_kinds(&systems, &german()),
            [QuantityKind::Force, QuantityKind::Mass, QuantityKind::Angle]
        );
    }

    #[test]
    fn unit_of_the_kind_is_accepted_with_single_spaces() {
        assert_eq!(
            check(QuantityKind::Time, " h   min "),
            Ok("h min".to_owned())
        );
    }

    #[test]
    fn unit_of_another_dimension_is_named() {
        assert_eq!(
            check(QuantityKind::Speed, "kg"),
            Err(KindUnitProblem::OtherDimension {
                unit: "kg".to_owned()
            })
        );
    }

    #[test]
    fn compound_is_refused_for_a_kind_without_compounds() {
        assert_eq!(
            check(QuantityKind::Speed, "km/h m/s"),
            Err(KindUnitProblem::CompoundNotAllowed)
        );
    }

    #[test]
    fn compound_must_descend() {
        assert_eq!(
            check(QuantityKind::Time, "min h"),
            Err(KindUnitProblem::CompoundNotDescending {
                unit: "min h".to_owned()
            })
        );
    }

    #[test]
    fn temperature_reading_needs_a_scale() {
        assert_eq!(
            check(QuantityKind::Temperature, "K"),
            Err(KindUnitProblem::ScaleRequired)
        );
    }

    #[test]
    fn temperature_difference_refuses_a_scale() {
        assert_eq!(
            check(QuantityKind::TemperatureDifference, "celsius"),
            Err(KindUnitProblem::NotAUnit {
                unit: "celsius".to_owned()
            })
        );
    }

    #[test]
    fn text_that_is_no_unit_is_named() {
        assert_eq!(
            check(QuantityKind::Length, "furlongs"),
            Err(KindUnitProblem::NotAUnit {
                unit: "furlongs".to_owned()
            })
        );
    }

    #[test]
    fn dimensionless_one_is_not_offered_as_a_display_unit() {
        assert_eq!(
            check(QuantityKind::Count, "1"),
            Err(KindUnitProblem::NotADisplayUnit {
                unit: "1".to_owned()
            })
        );
    }

    #[test]
    fn problem_message_names_the_unit_and_the_kind() {
        let problem = KindUnitProblem::OtherDimension {
            unit: "kg".to_owned(),
        };

        let message = kind_unit_problem_message(QuantityKind::Speed, &problem, &Locale::source());

        assert_eq!(
            render(&message, &Locale::source()).as_str(),
            "kg is not a unit of speed"
        );
    }
}
