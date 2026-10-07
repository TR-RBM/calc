use std::collections::BTreeMap;

use calc_syntax::{AnswerNotation, NotationMode};
use calc_units::{Dimension, TemperatureScale};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QuantityKind {
    Length,
    Area,
    Volume,
    Angle,
    Count,
    Ratio,
    Mass,
    Time,
    Speed,
    Force,
    Energy,
    Temperature,
    TemperatureDifference,
}

impl QuantityKind {
    pub const ALL: [QuantityKind; 13] = [
        QuantityKind::Length,
        QuantityKind::Area,
        QuantityKind::Volume,
        QuantityKind::Angle,
        QuantityKind::Count,
        QuantityKind::Ratio,
        QuantityKind::Mass,
        QuantityKind::Time,
        QuantityKind::Speed,
        QuantityKind::Force,
        QuantityKind::Energy,
        QuantityKind::Temperature,
        QuantityKind::TemperatureDifference,
    ];

    pub fn name(self) -> &'static str {
        match self {
            QuantityKind::Length => "length",
            QuantityKind::Area => "area",
            QuantityKind::Volume => "volume",
            QuantityKind::Angle => "angle",
            QuantityKind::Count => "count",
            QuantityKind::Ratio => "ratio",
            QuantityKind::Mass => "mass",
            QuantityKind::Time => "time",
            QuantityKind::Speed => "speed",
            QuantityKind::Force => "force",
            QuantityKind::Energy => "energy",
            QuantityKind::Temperature => "temperature",
            QuantityKind::TemperatureDifference => "temperature-difference",
        }
    }

    pub fn dimension(self) -> Dimension {
        let exponents = match self {
            QuantityKind::Length => [1, 0, 0, 0, 0, 0, 0, 0],
            QuantityKind::Area => [2, 0, 0, 0, 0, 0, 0, 0],
            QuantityKind::Volume => [3, 0, 0, 0, 0, 0, 0, 0],
            QuantityKind::Angle | QuantityKind::Count | QuantityKind::Ratio => [0; 8],
            QuantityKind::Mass => [0, 1, 0, 0, 0, 0, 0, 0],
            QuantityKind::Time => [0, 0, 1, 0, 0, 0, 0, 0],
            QuantityKind::Speed => [1, 0, -1, 0, 0, 0, 0, 0],
            QuantityKind::Force => [1, 1, -2, 0, 0, 0, 0, 0],
            QuantityKind::Energy => [2, 1, -2, 0, 0, 0, 0, 0],
            QuantityKind::Temperature | QuantityKind::TemperatureDifference => {
                [0, 0, 0, 0, 1, 0, 0, 0]
            }
        };
        Dimension::from_exponents(exponents)
    }

    pub fn dimension_determines_it(self) -> bool {
        match self {
            QuantityKind::Length
            | QuantityKind::Area
            | QuantityKind::Volume
            | QuantityKind::Mass
            | QuantityKind::Time
            | QuantityKind::Speed
            | QuantityKind::Force => true,
            QuantityKind::Angle
            | QuantityKind::Count
            | QuantityKind::Ratio
            | QuantityKind::Energy
            | QuantityKind::Temperature
            | QuantityKind::TemperatureDifference => false,
        }
    }

    pub fn allows_compound(self) -> bool {
        matches!(
            self,
            QuantityKind::Time | QuantityKind::Length | QuantityKind::Mass
        )
    }

    pub fn from_name(name: &str) -> Option<QuantityKind> {
        QuantityKind::ALL
            .iter()
            .copied()
            .find(|kind| kind.name() == name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LicenceTier {
    Bundle,
    Fetch,
    Link,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub identifier: String,
    pub title: String,
    pub authors: Vec<String>,
    pub year: u32,
    pub reference: String,
    pub licence: String,
    pub retrieved: Option<String>,
}

const BUNDLE_LICENCE_PREFIXES: [&str; 3] = ["CC0-", "CC-BY-SA-", "CC-PDDC"];
const FETCH_LICENCE_PREFIXES: [&str; 2] = ["CC-BY-NC", "CC-BY-ND"];
const ATTRIBUTION_ONLY_PREFIX: &str = "CC-BY-";

impl Source {
    pub fn tier(&self) -> LicenceTier {
        let licence = self.licence.as_str();
        if FETCH_LICENCE_PREFIXES
            .iter()
            .any(|prefix| licence.starts_with(prefix))
        {
            return LicenceTier::Fetch;
        }
        let attribution_only = licence
            .strip_prefix(ATTRIBUTION_ONLY_PREFIX)
            .is_some_and(|rest| rest.chars().next().is_some_and(|c| c.is_ascii_digit()));
        if attribution_only
            || BUNDLE_LICENCE_PREFIXES
                .iter()
                .any(|prefix| licence.starts_with(prefix))
        {
            return LicenceTier::Bundle;
        }
        LicenceTier::Link
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Role {
    pub identifier: String,
    pub dimension: Dimension,
    pub quantity: QuantityKind,
    pub names: BTreeMap<String, String>,
    pub running_names: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectKind {
    pub identifier: String,
    pub names: BTreeMap<String, String>,
    pub running_names: BTreeMap<String, String>,
    pub roles: Vec<Role>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WayDefinition {
    pub identifier: String,
    pub output: String,
    pub inputs: Vec<String>,
    pub relation: String,
    pub sources: Vec<String>,
    pub rests_on: Vec<String>,
    pub permutations: Vec<Vec<String>>,
    pub assumes: Vec<String>,
    pub formula: String,
    pub conditions: Vec<String>,
    pub recognized: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActivityKind {
    ShapeMatching,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActivityShape {
    Circle,
    Square,
    Triangle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActivityVariation {
    Identical,
    Orientation,
    Size,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityDefinition {
    pub identifier: String,
    pub kind: ActivityKind,
    pub shapes: Vec<ActivityShape>,
    pub variation: ActivityVariation,
    pub sources: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternDefinition {
    pub identifier: String,
    pub relation: Option<String>,
    pub body: String,
    pub guard: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundRelation {
    AtMost,
    AtLeast,
    StrictlyBetween,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundDefinition {
    pub identifier: String,
    pub role: String,
    pub inputs: Vec<String>,
    pub relation: BoundRelation,
    pub sources: Vec<String>,
    pub expressions: Vec<String>,
    pub conditions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConceptText {
    pub name: String,
    pub statement: String,
    pub intuition: Option<String>,
    pub examples: Option<String>,
    pub misconception: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConceptNode {
    pub identifier: String,
    pub level: String,
    pub prerequisites: Vec<String>,
    pub beginning: bool,
    pub rests_on: Vec<String>,
    pub curriculum_references: Vec<String>,
    pub sources: Vec<String>,
    pub exercises: Vec<String>,
    pub texts: BTreeMap<String, ConceptText>,
    pub ways: Vec<WayDefinition>,
    pub bounds: Vec<BoundDefinition>,
    pub patterns: Vec<PatternDefinition>,
    pub activities: Vec<ActivityDefinition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurriculumGroup {
    pub identifier: String,
    pub concepts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotationVersion {
    pub identifier: String,
    pub version: u32,
    pub from: String,
    pub notation: AnswerNotation,
    pub sources: Vec<String>,
    pub mode_sources: Vec<(NotationMode, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShippedNotation {
    pub identifier: String,
    pub version: u32,
    pub notation: AnswerNotation,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CurriculumText {
    pub name: String,
    pub groups: BTreeMap<String, String>,
    pub concepts: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisplayUnit {
    Units(Vec<String>),
    Scale(TemperatureScale),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitDeclaration {
    pub kind: QuantityKind,
    pub from: Option<String>,
    pub posed: Option<DisplayUnit>,
    pub displayed: Vec<DisplayUnit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitSystem {
    pub identifier: String,
    pub names: BTreeMap<String, String>,
    pub units: Vec<UnitDeclaration>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Curriculum {
    pub identifier: String,
    pub locale: String,
    pub documents: Vec<String>,
    pub groups: Vec<CurriculumGroup>,
    pub prerequisites: BTreeMap<String, Vec<String>>,
    pub notations: Vec<NotationVersion>,
    pub shipped: Vec<ShippedNotation>,
    pub units: Vec<UnitDeclaration>,
    pub texts: BTreeMap<String, CurriculumText>,
}

impl Curriculum {
    pub fn concept_order(&self) -> Vec<&str> {
        self.groups
            .iter()
            .flat_map(|group| group.concepts.iter().map(String::as_str))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConceptSet {
    pub version: u64,
    pub concepts: Vec<ConceptNode>,
    pub objects: Vec<ObjectKind>,
    pub sources: Vec<Source>,
    pub curricula: Vec<Curriculum>,
    pub unit_systems: Vec<UnitSystem>,
    pub retired: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_with_licence(licence: &str) -> Source {
        Source {
            identifier: "s".to_string(),
            title: "t".to_string(),
            authors: Vec::new(),
            year: 2020,
            reference: "https://example.org".to_string(),
            licence: licence.to_string(),
            retrieved: None,
        }
    }

    #[test]
    fn attribution_licence_is_bundled() {
        assert_eq!(source_with_licence("CC-BY-4.0").tier(), LicenceTier::Bundle);
    }

    #[test]
    fn share_alike_licence_is_bundled() {
        assert_eq!(
            source_with_licence("CC-BY-SA-4.0").tier(),
            LicenceTier::Bundle
        );
    }

    #[test]
    fn public_domain_dedication_is_bundled() {
        assert_eq!(source_with_licence("CC0-1.0").tier(), LicenceTier::Bundle);
    }

    #[test]
    fn non_commercial_share_alike_licence_is_fetched() {
        assert_eq!(
            source_with_licence("CC-BY-NC-SA-4.0").tier(),
            LicenceTier::Fetch
        );
    }

    #[test]
    fn no_derivatives_licence_is_fetched() {
        assert_eq!(
            source_with_licence("CC-BY-ND-4.0").tier(),
            LicenceTier::Fetch
        );
    }

    #[test]
    fn unknown_licence_is_linked() {
        assert_eq!(source_with_licence("unknown").tier(), LicenceTier::Link);
    }

    #[test]
    fn proprietary_licence_is_linked() {
        assert_eq!(
            source_with_licence("LicenseRef-Proprietary").tier(),
            LicenceTier::Link
        );
    }

    #[test]
    fn every_quantity_kind_name_reads_back() {
        for kind in QuantityKind::ALL {
            assert_eq!(QuantityKind::from_name(kind.name()), Some(kind));
        }
    }
}
