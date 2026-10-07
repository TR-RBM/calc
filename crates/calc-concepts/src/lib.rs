mod curriculum;
mod document;
mod error;
mod identifier;
mod learn_text;
mod load;
mod model;
mod patterns;
mod rules;
mod units;
mod version;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_content.rs"));
}

pub use error::{LoadError, LoadErrorKind};
pub use identifier::{is_corpus_id, is_identifier, is_role_identifier};
pub use load::load;
pub use model::{
    ActivityDefinition, ActivityKind, ActivityShape, ActivityVariation, BoundDefinition,
    BoundRelation, ConceptNode, ConceptSet, ConceptText, Curriculum, CurriculumGroup,
    CurriculumText, DisplayUnit, LicenceTier, NotationVersion, ObjectKind, QuantityKind, Role,
    ShippedNotation, Source, UnitDeclaration, UnitSystem, WayDefinition,
};
pub use patterns::RecognitionPattern;
pub use rules::{Bound, is_exact_formula};
pub use units::{DisplayUnitError, check_display_unit};
pub use version::concept_set_version;

pub fn embedded_files() -> &'static [(&'static str, &'static [u8])] {
    embedded::EMBEDDED_CONTENT
}

pub fn load_embedded() -> Result<ConceptSet, LoadError> {
    load(embedded::EMBEDDED_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use calc_core::{SearchOutcome, SearchRequest, search_ways};
    use calc_expr::ExprPool;

    const SHIPPED_WAYS: [&str; 17] = [
        "circle-area/from-radius",
        "circle-area/radius-from-area",
        "circle-chord/from-radius-and-central-angle",
        "circle-chord/radius-from-chord-and-central-angle",
        "circle-circumference/diameter-from-circumference",
        "circle-circumference/from-diameter",
        "circle-circumference/from-radius",
        "circle-circumference/radius-from-circumference",
        "circle-diameter/from-radius",
        "circle-diameter/radius-from-diameter",
        "circle-sagitta/radius-from-chord-and-sagitta",
        "herons-formula/area",
        "law-of-cosines/angle-gamma",
        "law-of-cosines/side-c",
        "law-of-sines/side-b",
        "triangle-area/base-height",
        "triangle-area/two-sides-angle",
    ];

    fn shipped_rules(pool: &mut ExprPool) -> calc_core::RuleSet {
        load_embedded().unwrap().rule_set(pool).unwrap()
    }

    #[test]
    fn shipped_content_loads() {
        assert!(load_embedded().is_ok(), "{:?}", load_embedded().err());
    }

    fn shipped_examples(concept: &str, locale: &str) -> String {
        let set = load_embedded().unwrap();
        let node = set
            .concepts
            .iter()
            .find(|node| node.identifier == concept)
            .unwrap();
        node.texts[locale].examples.clone().unwrap()
    }

    #[test]
    fn shipped_beginnings_are_the_three_first_rungs() {
        let set = load_embedded().unwrap();
        let beginnings: Vec<&str> = set
            .concepts
            .iter()
            .filter(|node| node.beginning)
            .map(|node| node.identifier.as_str())
            .collect();
        assert_eq!(
            beginnings,
            ["counting", "length-comparison", "shape-matching"]
        );
    }

    #[test]
    fn shipped_shape_matching_steps_from_identical_to_resized() {
        let set = load_embedded().unwrap();
        let node = set
            .concepts
            .iter()
            .find(|node| node.identifier == "shape-matching")
            .unwrap();
        let variations: Vec<ActivityVariation> = node
            .activities
            .iter()
            .map(|activity| activity.variation)
            .collect();
        assert_eq!(
            variations,
            [
                ActivityVariation::Identical,
                ActivityVariation::Orientation,
                ActivityVariation::Size
            ]
        );
    }

    #[test]
    fn shipped_rounded_value_takes_the_german_comma() {
        assert!(shipped_examples("circle-area", "de").contains("9π cm² ≈ 28,27 cm²"));
    }

    #[test]
    fn shipped_rounded_value_takes_the_english_point() {
        assert!(shipped_examples("circle-area", "en").contains("9π cm² ≈ 28.27 cm²"));
    }

    #[test]
    fn shipped_content_holds_the_t063_ways() {
        let mut pool = ExprPool::new();
        let rules = shipped_rules(&mut pool);
        let identifiers: Vec<&str> = rules
            .rules
            .iter()
            .map(|rule| rule.identifier.as_str())
            .collect();
        assert_eq!(identifiers, SHIPPED_WAYS);
    }

    #[test]
    fn shipped_heron_rule_has_its_inputs_and_is_exact() {
        let mut pool = ExprPool::new();
        let rules = shipped_rules(&mut pool);
        let heron = rules
            .rules
            .iter()
            .find(|rule| rule.identifier == "herons-formula/area")
            .unwrap();
        assert_eq!(heron.output, "triangle.area");
        assert_eq!(
            heron.inputs,
            ["triangle.side-a", "triangle.side-b", "triangle.side-c"]
        );
        assert!(heron.exact);
    }

    #[test]
    fn shipped_two_sides_angle_rule_is_not_exact() {
        let mut pool = ExprPool::new();
        let rules = shipped_rules(&mut pool);
        let rule = rules
            .rules
            .iter()
            .find(|rule| rule.identifier == "triangle-area/two-sides-angle")
            .unwrap();
        assert!(!rule.exact);
    }

    #[test]
    fn shipped_bound_limits_the_triangle_area() {
        let mut pool = ExprPool::new();
        let bounds = load_embedded().unwrap().bounds(&mut pool).unwrap();
        assert_eq!(bounds.len(), 1);
        assert_eq!(
            (bounds[0].identifier.as_str(), bounds[0].role.as_str()),
            ("triangle-area/two-sides", "triangle.area")
        );
    }

    fn document_owner(file: &str) -> Option<&str> {
        let inside = file.strip_prefix("sources/")?;
        let is_record = inside.ends_with(".md") && !inside.contains('/');
        if is_record {
            return None;
        }
        inside.split(['/', '.']).next()
    }

    #[test]
    fn shipped_sources_with_unknown_licence_are_linked() {
        let set = load_embedded().unwrap();
        assert!(
            set.sources
                .iter()
                .filter(|source| source.licence == "unknown")
                .all(|source| source.tier() == LicenceTier::Link)
        );
    }

    #[test]
    fn shipped_sources_record_a_licence() {
        let set = load_embedded().unwrap();
        assert!(
            set.sources
                .iter()
                .all(|source| !source.licence.trim().is_empty())
        );
    }

    #[test]
    fn shipped_document_text_belongs_to_bundled_sources_only() {
        let set = load_embedded().unwrap();
        let unbundled: Vec<&str> = embedded_files()
            .iter()
            .filter_map(|(file, _)| document_owner(file))
            .filter(|owner| {
                set.sources
                    .iter()
                    .find(|source| source.identifier == *owner)
                    .is_none_or(|source| source.tier() != LicenceTier::Bundle)
            })
            .collect();
        assert!(unbundled.is_empty(), "{unbundled:?}");
    }

    #[test]
    fn record_file_is_not_document_text() {
        assert_eq!(document_owner("sources/nist-sp-811-2008.md"), None);
    }

    #[test]
    fn file_beside_a_record_is_document_text_of_that_source() {
        assert_eq!(
            document_owner("sources/bipm-si-brochure-9/table-8.md"),
            Some("bipm-si-brochure-9")
        );
    }

    #[test]
    fn every_cited_source_has_a_record() {
        let set = load_embedded().unwrap();
        let cited = set.concepts.iter().flat_map(|concept| {
            concept
                .sources
                .iter()
                .chain(concept.ways.iter().flat_map(|way| way.sources.iter()))
                .chain(concept.bounds.iter().flat_map(|bound| bound.sources.iter()))
        });
        let missing: Vec<&String> = cited
            .filter(|cited| {
                !set.sources
                    .iter()
                    .any(|source| &source.identifier == *cited)
            })
            .collect();
        assert!(missing.is_empty(), "{missing:?}");
    }

    #[test]
    fn shipped_version_is_the_hash_of_the_embedded_files() {
        assert_eq!(
            load_embedded().unwrap().version,
            concept_set_version(embedded_files())
        );
    }

    #[test]
    fn search_over_shipped_rules_finds_the_chord_and_sagitta_way() {
        let mut pool = ExprPool::new();
        let rules = shipped_rules(&mut pool);
        let request = SearchRequest {
            wanted: "circle.circumference".to_string(),
            given: Vec::new(),
            obtainable: Vec::new(),
            not_obtainable: vec!["circle.radius".to_string()],
            only_obtainable: false,
            way_cap: 1000,
            work_budget: 1_000_000,
        };
        let SearchOutcome::Reached(list) = search_ways(&mut pool, &rules, &request).unwrap() else {
            panic!("expected ways");
        };
        assert!(
            list.ways
                .iter()
                .any(|way| way.inputs == ["circle.chord", "circle.sagitta"])
        );
    }
}
