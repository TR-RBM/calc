use calc_concepts::{ConceptNode, ConceptSet, ConceptText};
use calc_i18n::{Locale, Message};

use crate::json::{self, Json};
use crate::result_record::LineId;
use crate::session_file::line_label;
use crate::solve::concept_set;

const SOURCE_LOCALE: &str = "en";
const CANDIDATE_SEPARATOR: &str = ", ";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConceptLens {
    Explore,
    Learn,
    Train,
    Read,
}

impl ConceptLens {
    pub const ALL: [ConceptLens; 4] = [
        ConceptLens::Explore,
        ConceptLens::Learn,
        ConceptLens::Train,
        ConceptLens::Read,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ConceptLens::Explore => "explore",
            ConceptLens::Learn => "learn",
            ConceptLens::Train => "train",
            ConceptLens::Read => "read",
        }
    }

    pub fn from_name(name: &str) -> Option<ConceptLens> {
        ConceptLens::ALL
            .into_iter()
            .find(|lens| lens.name() == name)
    }

    pub fn message(self) -> Message {
        match self {
            ConceptLens::Explore => Message::CommonLensExplore,
            ConceptLens::Learn => Message::CommonLensLearn,
            ConceptLens::Train => Message::CommonLensTrain,
            ConceptLens::Read => Message::CommonLensRead,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConceptWording {
    pub locale: String,
    pub name: String,
    pub statement: String,
    pub intuition: Option<String>,
    pub examples: Option<String>,
    pub misconception: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConceptOutcome {
    pub identifier: String,
    pub lens: ConceptLens,
    pub shown_by: Vec<ConceptLens>,
    pub origin: Option<LineId>,
    pub wording: Option<ConceptWording>,
    pub level: String,
    pub prerequisites: Vec<String>,
    pub sources: Vec<String>,
    pub exercises: Vec<String>,
    pub activities: Vec<calc_concepts::ActivityDefinition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConceptError {
    UnknownConcept(String),
    AmbiguousName {
        name: String,
        candidates: Vec<String>,
    },
    ConceptSetNotLoaded,
}

pub fn concept_error_message(error: &ConceptError) -> Message {
    match error {
        ConceptError::UnknownConcept(identifier) => Message::ErrorUnknownConcept {
            concept: identifier.clone(),
        },
        ConceptError::AmbiguousName { name, candidates } => Message::ErrorAmbiguousConceptName {
            name: name.clone(),
            candidates: candidates.join(CANDIDATE_SEPARATOR),
        },
        ConceptError::ConceptSetNotLoaded => Message::ErrorConceptSetNotLoaded,
    }
}

fn wording_of(node: &ConceptNode, locale: &Locale) -> Option<ConceptWording> {
    let (tag, text): (&str, &ConceptText) = [locale.tag(), SOURCE_LOCALE]
        .into_iter()
        .find_map(|tag| node.texts.get(tag).map(|text| (tag, text)))?;
    Some(ConceptWording {
        locale: tag.to_owned(),
        name: text.name.clone(),
        statement: text.statement.clone(),
        intuition: text.intuition.clone(),
        examples: text.examples.clone(),
        misconception: text.misconception.clone(),
    })
}

fn shows(node: &ConceptNode, wording: Option<&ConceptWording>, lens: ConceptLens) -> bool {
    match lens {
        ConceptLens::Explore => !node.ways.is_empty() || !node.bounds.is_empty(),
        ConceptLens::Learn => {
            !node.activities.is_empty()
                || wording.is_some_and(|wording| {
                    wording.intuition.is_some()
                        || wording.examples.is_some()
                        || wording.misconception.is_some()
                })
        }
        ConceptLens::Train => !node.exercises.is_empty(),
        ConceptLens::Read => !node.sources.is_empty(),
    }
}

pub fn concept_identifiers() -> Vec<String> {
    concept_set()
        .map(|concepts| {
            concepts
                .concepts
                .iter()
                .map(|node| node.identifier.clone())
                .collect()
        })
        .unwrap_or_default()
}

pub fn concept_name(identifier: &str, locale: &Locale) -> Option<String> {
    let concepts = concept_set().ok()?;
    let node = concepts
        .concepts
        .iter()
        .find(|node| node.identifier == identifier)?;
    wording_of(node, locale).map(|wording| wording.name)
}

fn named_like(node: &ConceptNode, wanted: &str, locale: &Locale) -> bool {
    wording_of(node, locale).is_some_and(|wording| wording.name.to_lowercase() == wanted)
}

fn wanted_node<'set>(
    concepts: &'set ConceptSet,
    wanted: &str,
    locale: &Locale,
) -> Result<&'set ConceptNode, ConceptError> {
    if let Some(node) = concepts
        .concepts
        .iter()
        .find(|node| node.identifier == wanted)
    {
        return Ok(node);
    }
    let folded = wanted.trim().to_lowercase();
    let named: Vec<&ConceptNode> = concepts
        .concepts
        .iter()
        .filter(|node| named_like(node, &folded, locale))
        .collect();
    match named.as_slice() {
        [] => Err(ConceptError::UnknownConcept(wanted.to_owned())),
        [node] => Ok(node),
        several => Err(ConceptError::AmbiguousName {
            name: wanted.to_owned(),
            candidates: several.iter().map(|node| node.identifier.clone()).collect(),
        }),
    }
}

pub fn concept(
    wanted: &str,
    lens: Option<ConceptLens>,
    origin: Option<LineId>,
    locale: &Locale,
) -> Result<ConceptOutcome, ConceptError> {
    let concepts = concept_set().map_err(|_| ConceptError::ConceptSetNotLoaded)?;
    let node = wanted_node(concepts, wanted, locale)?;
    let wording = wording_of(node, locale);
    let shown_by = ConceptLens::ALL
        .into_iter()
        .filter(|lens| shows(node, wording.as_ref(), *lens))
        .collect();
    Ok(ConceptOutcome {
        identifier: node.identifier.clone(),
        lens: lens.unwrap_or(ConceptLens::Explore),
        shown_by,
        origin,
        wording,
        level: node.level.clone(),
        prerequisites: node.prerequisites.clone(),
        sources: node.sources.clone(),
        exercises: node.exercises.clone(),
        activities: node.activities.clone(),
    })
}

pub fn activity_kind_name(kind: calc_concepts::ActivityKind) -> &'static str {
    match kind {
        calc_concepts::ActivityKind::ShapeMatching => "shape_matching",
    }
}

pub fn activity_variation_name(variation: calc_concepts::ActivityVariation) -> &'static str {
    match variation {
        calc_concepts::ActivityVariation::Identical => "identical",
        calc_concepts::ActivityVariation::Orientation => "orientation",
        calc_concepts::ActivityVariation::Size => "size",
    }
}

pub fn activity_shape_name(shape: calc_concepts::ActivityShape) -> &'static str {
    match shape {
        calc_concepts::ActivityShape::Circle => "circle",
        calc_concepts::ActivityShape::Square => "square",
        calc_concepts::ActivityShape::Triangle => "triangle",
    }
}

pub fn concept_json(outcome: &ConceptOutcome) -> Vec<u8> {
    let strings =
        |values: &[String]| Json::Array(values.iter().map(|value| Json::string(value)).collect());
    let wording = outcome.wording.as_ref().map(|wording| {
        Json::object(vec![
            ("locale", Json::string(&wording.locale)),
            ("name", Json::string(&wording.name)),
            ("statement", Json::string(&wording.statement)),
            (
                "intuition",
                Json::optional(wording.intuition.as_deref().map(Json::string)),
            ),
            (
                "examples",
                Json::optional(wording.examples.as_deref().map(Json::string)),
            ),
            (
                "misconception",
                Json::optional(wording.misconception.as_deref().map(Json::string)),
            ),
        ])
    });
    let lens_json = |lens: ConceptLens| {
        Json::object(vec![
            ("name", Json::string(lens.name())),
            ("name_key", Json::string(lens.message().key())),
        ])
    };
    json::write_canonical(&Json::object(vec![
        ("identifier", Json::string(&outcome.identifier)),
        ("lens", lens_json(outcome.lens)),
        (
            "shown_by",
            Json::Array(
                outcome
                    .shown_by
                    .iter()
                    .map(|lens| lens_json(*lens))
                    .collect(),
            ),
        ),
        (
            "origin",
            Json::optional(outcome.origin.map(|line| Json::string(&line_label(line)))),
        ),
        ("wording", Json::optional(wording)),
        ("level", Json::string(&outcome.level)),
        ("prerequisites", strings(&outcome.prerequisites)),
        ("sources", strings(&outcome.sources)),
        ("exercises", strings(&outcome.exercises)),
        (
            "activities",
            Json::Array(
                outcome
                    .activities
                    .iter()
                    .map(|activity| {
                        Json::object(vec![
                            ("identifier", Json::string(&activity.identifier)),
                            ("kind", Json::string(activity_kind_name(activity.kind))),
                            (
                                "variation",
                                Json::string(activity_variation_name(activity.variation)),
                            ),
                            (
                                "shapes",
                                Json::Array(
                                    activity
                                        .shapes
                                        .iter()
                                        .map(|shape| Json::string(activity_shape_name(*shape)))
                                        .collect(),
                                ),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]))
    .into_bytes()
}

pub const OPERATOR_SIGNS: [&str; 16] = [
    "=", "\u{2260}", "\u{2248}", "<", ">", "\u{2264}", "\u{2265}", "+", "-", "\u{2212}",
    "\u{00B7}", "\u{00D7}", "\u{00F7}", "/", ":", "\u{2192}",
];

pub fn unglued_signs(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter(|word| {
            word.chars().count() == 1
                && !word.chars().all(char::is_alphanumeric)
                && !OPERATOR_SIGNS.contains(word)
        })
        .map(str::to_owned)
        .collect()
}

pub fn prose_groups(text: &str) -> Vec<String> {
    let mut groups: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match groups.last_mut() {
            Some(group) if OPERATOR_SIGNS.contains(&word) => {
                group.push(' ');
                group.push_str(word);
            }
            _ => groups.push(word.to_owned()),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_i18n::{LanguageTag, render};

    fn english() -> Locale {
        Locale::matching(&LanguageTag::parse("en").unwrap())
    }

    fn german() -> Locale {
        Locale::matching(&LanguageTag::parse("de").unwrap())
    }

    fn circumference(lens: Option<ConceptLens>) -> ConceptOutcome {
        concept("circle-circumference", lens, None, &english()).expect("a concept")
    }

    fn node(identifier: &str, name: &str) -> ConceptNode {
        ConceptNode {
            identifier: identifier.to_owned(),
            level: "school".to_owned(),
            prerequisites: Vec::new(),
            beginning: true,
            rests_on: Vec::new(),
            curriculum_references: Vec::new(),
            sources: Vec::new(),
            exercises: Vec::new(),
            texts: std::collections::BTreeMap::from([(
                SOURCE_LOCALE.to_owned(),
                ConceptText {
                    name: name.to_owned(),
                    statement: "a statement".to_owned(),
                    intuition: None,
                    examples: None,
                    misconception: None,
                },
            )]),
            ways: Vec::new(),
            bounds: Vec::new(),
            patterns: Vec::new(),
            activities: Vec::new(),
        }
    }

    fn set_of(nodes: Vec<ConceptNode>) -> ConceptSet {
        ConceptSet {
            version: 1,
            concepts: nodes,
            objects: Vec::new(),
            sources: Vec::new(),
            curricula: Vec::new(),
            unit_systems: Vec::new(),
            retired: Vec::new(),
        }
    }

    #[test]
    fn a_name_that_two_concepts_carry_is_refused_with_both_identifiers() {
        let set = set_of(vec![
            node("plane-circle", "circle"),
            node("unit-circle", "circle"),
        ]);

        let refused = wanted_node(&set, "circle", &english()).err();

        assert_eq!(
            refused,
            Some(ConceptError::AmbiguousName {
                name: "circle".to_owned(),
                candidates: vec!["plane-circle".to_owned(), "unit-circle".to_owned()]
            })
        );
    }

    #[test]
    fn an_identifier_is_taken_before_any_name() {
        let set = set_of(vec![node("circle", "ring"), node("other", "circle")]);

        let found = wanted_node(&set, "circle", &english()).expect("a concept");

        assert_eq!(found.identifier, "circle");
    }

    #[test]
    fn a_name_is_found_whatever_its_capitals() {
        let set = set_of(vec![node("plane-circle", "Circle of a plane")]);

        let found = wanted_node(&set, "  circle OF a plane ", &english()).expect("a concept");

        assert_eq!(found.identifier, "plane-circle");
    }

    #[test]
    fn a_concept_is_named_in_the_locale_asked_for() {
        assert_eq!(
            concept_name("circle-circumference", &german()),
            Some("Umfang eines Kreises".to_owned())
        );
    }

    #[test]
    fn a_concept_the_set_does_not_hold_has_no_name() {
        assert_eq!(concept_name("no-such-concept", &english()), None);
    }

    #[test]
    fn a_concept_carries_its_text_in_the_locale() {
        let outcome = circumference(None);

        let wording = outcome.wording.expect("a wording");

        assert_eq!(
            (wording.locale.as_str(), wording.name.as_str()),
            ("en", "Circumference of a circle")
        );
    }

    #[test]
    fn a_concept_carries_its_statement() {
        let outcome = circumference(None);

        let statement = outcome.wording.expect("a wording").statement;

        assert!(statement.contains("perimeter"), "{statement}");
    }

    #[test]
    fn a_concept_takes_the_german_text_under_german() {
        let outcome = concept("circle-circumference", None, None, &german()).expect("a concept");

        assert_eq!(outcome.wording.expect("a wording").locale, "de");
    }

    #[test]
    fn a_concept_without_a_lens_opens_as_explore() {
        let outcome = circumference(None);

        assert_eq!(outcome.lens, ConceptLens::Explore);
    }

    #[test]
    fn a_concept_keeps_the_lens_it_was_asked_for() {
        let outcome = circumference(Some(ConceptLens::Read));

        assert_eq!(outcome.lens, ConceptLens::Read);
    }

    #[test]
    fn a_concept_with_ways_learning_text_and_sources_is_shown_by_explore_learn_and_read() {
        let outcome = circumference(None);

        assert_eq!(
            outcome.shown_by,
            [ConceptLens::Explore, ConceptLens::Learn, ConceptLens::Read]
        );
    }

    #[test]
    fn a_lens_with_nothing_to_show_is_not_an_error() {
        let outcome = circumference(Some(ConceptLens::Train));

        assert_eq!(
            (outcome.lens, outcome.shown_by.contains(&ConceptLens::Train)),
            (ConceptLens::Train, false)
        );
    }

    #[test]
    fn a_concept_carries_the_line_it_was_recognized_in() {
        let line = LineId::from_number(3);

        let outcome = concept("circle-circumference", None, line, &english()).expect("a concept");

        assert_eq!(outcome.origin, line);
    }

    #[test]
    fn a_concept_carries_its_prerequisites_and_sources() {
        let outcome = circumference(None);

        assert!(
            !outcome.prerequisites.is_empty() && !outcome.sources.is_empty(),
            "{outcome:?}"
        );
    }

    #[test]
    fn an_unknown_concept_is_named_in_its_error() {
        let error =
            concept("circle-perimeter", None, None, &english()).expect_err("no such concept");

        assert_eq!(
            error,
            ConceptError::UnknownConcept("circle-perimeter".to_owned())
        );
    }

    #[test]
    fn the_error_of_an_unknown_concept_names_it_in_words() {
        let error = ConceptError::UnknownConcept("circle-perimeter".to_owned());

        let message = render(&concept_error_message(&error), &english());

        assert_eq!(message.as_str(), "there is no concept circle-perimeter");
    }

    #[test]
    fn a_lens_reads_as_a_word_in_every_locale() {
        for lens in ConceptLens::ALL {
            for locale in Locale::shipped() {
                assert!(!render(&lens.message(), &locale).as_str().is_empty());
            }
        }
    }

    #[test]
    fn a_concept_with_no_text_has_no_wording_and_no_learn_lens() {
        let node = ConceptNode {
            identifier: "wordless".to_owned(),
            level: "isced-2".to_owned(),
            prerequisites: Vec::new(),
            beginning: true,
            rests_on: Vec::new(),
            curriculum_references: Vec::new(),
            sources: Vec::new(),
            exercises: Vec::new(),
            texts: std::collections::BTreeMap::new(),
            ways: Vec::new(),
            bounds: Vec::new(),
            patterns: Vec::new(),
            activities: Vec::new(),
        };

        let wording = wording_of(&node, &english());

        assert_eq!(
            (
                wording.clone(),
                shows(&node, wording.as_ref(), ConceptLens::Learn)
            ),
            (None, false)
        );
    }

    #[test]
    fn a_lens_is_named_for_a_program() {
        assert_eq!(ConceptLens::from_name("train"), Some(ConceptLens::Train));
    }

    #[test]
    fn a_sign_is_glued_to_the_word_before_it_and_never_begins_a_group() {
        assert_eq!(
            prose_groups("A = \u{03c0} \u{00b7} r\u{00b2} \u{2248} 28,27"),
            vec![
                "A =".to_string(),
                "\u{03c0} \u{00b7}".to_string(),
                "r\u{00b2} \u{2248}".to_string(),
                "28,27".to_string()
            ]
        );
    }

    #[test]
    fn a_sign_missing_from_the_glue_list_is_found() {
        assert_eq!(
            unglued_signs("a \u{00B1} b = c"),
            vec!["\u{00B1}".to_owned()]
        );
    }

    #[test]
    fn every_sign_standing_alone_in_the_shipped_learn_text_is_glued() {
        let concepts = calc_concepts::load_embedded().expect("the shipped concepts load");
        let mut found = Vec::new();
        for node in &concepts.concepts {
            for (locale, text) in &node.texts {
                let fields = [
                    Some(&text.statement),
                    text.intuition.as_ref(),
                    text.examples.as_ref(),
                    text.misconception.as_ref(),
                ];
                for field in fields.into_iter().flatten() {
                    for sign in unglued_signs(field) {
                        found.push(format!("{} {locale}: {sign}", node.identifier));
                    }
                }
            }
        }

        assert_eq!(found, Vec::<String>::new());
    }
}
