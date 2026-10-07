use std::collections::{BTreeMap, BTreeSet};

use calc_syntax::{NotationMode, NotationValueError, NotationValues};

use crate::document::{Document, FieldReader, Section, list};
use crate::error::{LoadError, LoadErrorKind};
use crate::identifier::{is_identifier, is_locale};
use crate::load::{check_title, no_blocks};
use crate::model::{
    ConceptNode, Curriculum, CurriculumGroup, CurriculumText, NotationVersion, ShippedNotation,
    Source, UnitDeclaration,
};
use crate::units::{check_unique_stages, read_unit_section};
use calc_expr::ExprPool;

const GROUP_SECTION: &str = "Group";
const CONCEPT_SECTION: &str = "Concept";
const NOTATION_SECTION: &str = "Notation";
const UNIT_SECTION: &str = "Unit";
const MODE_SOURCE_PREFIX: &str = "Source ";

pub(crate) struct CurriculumFiles {
    pub(crate) curriculum: Option<Document>,
    pub(crate) shipped: Option<Document>,
    pub(crate) texts: Vec<(String, Document)>,
}

pub(crate) struct Known<'set> {
    pub(crate) concepts: &'set [ConceptNode],
    pub(crate) sources: &'set [Source],
    pub(crate) retired: &'set [String],
}

impl Known<'_> {
    fn has_concept(&self, identifier: &str) -> bool {
        self.concepts
            .iter()
            .any(|concept| concept.identifier == identifier)
    }

    fn has_source(&self, identifier: &str) -> bool {
        self.sources
            .iter()
            .any(|source| source.identifier == identifier)
    }

    fn is_retired(&self, identifier: &str) -> bool {
        self.retired.iter().any(|retired| retired == identifier)
    }
}

fn notation_label(identifier: &str, version: u32) -> String {
    format!("{identifier} {version}")
}

fn check_sources(
    document: &Document,
    line: usize,
    known: &Known,
    sources: &[String],
) -> Result<(), LoadError> {
    match sources.iter().find(|source| !known.has_source(source)) {
        Some(unknown) => Err(document.error(line, LoadErrorKind::UnknownSource(unknown.clone()))),
        None => Ok(()),
    }
}

fn parse_notation_heading(
    document: &Document,
    section: &Section,
) -> Result<(String, u32), LoadError> {
    let invalid = || {
        document.error(
            section.line,
            LoadErrorKind::InvalidNotationHeading(section.name.clone()),
        )
    };
    let (identifier, version) = section.name.rsplit_once(' ').ok_or_else(invalid)?;
    let version = version
        .parse::<u32>()
        .ok()
        .filter(|number| *number > 0 && number.to_string() == version)
        .ok_or_else(invalid)?;
    if !is_identifier(identifier) {
        return Err(invalid());
    }
    Ok((identifier.to_string(), version))
}

fn read_modes(
    document: &Document,
    section: &Section,
    reader: &mut FieldReader,
) -> Result<calc_syntax::AnswerNotation, LoadError> {
    let mut values = NotationValues::new();
    let mut lines = BTreeMap::new();
    for mode in NotationMode::ALL {
        let field = reader.required(mode.name())?;
        values.set(mode, &field.value);
        lines.insert(mode, field.line);
    }
    values.notation().map_err(|error| match error {
        NotationValueError::MissingMode(mode) => document.error(
            section.line,
            LoadErrorKind::MissingKey(mode.name().to_string()),
        ),
        NotationValueError::InvalidValue(mode, value) => document.error(
            lines.get(&mode).copied().unwrap_or(section.line),
            LoadErrorKind::InvalidModeValue { mode, value },
        ),
    })
}

fn read_notation(
    document: &Document,
    section: &Section,
    known: &Known,
) -> Result<NotationVersion, LoadError> {
    let (identifier, version) = parse_notation_heading(document, section)?;
    no_blocks(document, section, 0)?;
    let mut reader = FieldReader::new(document, &section.fields, section.line);
    let from = reader.required_value("From")?.value.clone();
    let notation = read_modes(document, section, &mut reader)?;
    let mut mode_sources = Vec::new();
    for mode in NotationMode::ALL {
        let key = format!("{MODE_SOURCE_PREFIX}{}", mode.name());
        let field = reader.required_value(&key)?;
        check_sources(
            document,
            field.line,
            known,
            std::slice::from_ref(&field.value),
        )?;
        mode_sources.push((mode, field.value.clone()));
    }
    let sources_field = reader.required_value("Sources")?;
    let sources = list(&sources_field.value);
    reader.finish()?;
    check_sources(document, sources_field.line, known, &sources)?;
    if let Some(conflict) = notation.check().first() {
        return Err(document.error(section.line, LoadErrorKind::NotationConflict(*conflict)));
    }
    let listed: BTreeSet<&str> = sources.iter().map(String::as_str).collect();
    let per_mode: BTreeSet<&str> = mode_sources
        .iter()
        .map(|(_, source)| source.as_str())
        .collect();
    if listed != per_mode || listed.len() != sources.len() {
        return Err(document.error(
            sources_field.line,
            LoadErrorKind::SourcesDiffer(notation_label(&identifier, version)),
        ));
    }
    Ok(NotationVersion {
        identifier,
        version,
        from,
        notation,
        sources,
        mode_sources,
    })
}

struct Sections<'document> {
    groups: BTreeMap<String, (&'document Section, Vec<String>)>,
    prerequisites: Vec<(&'document Section, Vec<String>)>,
    notations: Vec<(&'document Section, NotationVersion)>,
    units: Vec<(&'document Section, UnitDeclaration)>,
}

fn read_sections<'document>(
    document: &'document Document,
    known: &Known,
    group_order: &[String],
) -> Result<Sections<'document>, LoadError> {
    let mut sections = Sections {
        groups: BTreeMap::new(),
        prerequisites: Vec::new(),
        notations: Vec::new(),
        units: Vec::new(),
    };
    let mut pool = ExprPool::new();
    let mut seen_concepts = BTreeSet::new();
    for section in &document.sections {
        match section.kind.as_str() {
            GROUP_SECTION => {
                if !group_order.contains(&section.name) {
                    return Err(document.error(
                        section.line,
                        LoadErrorKind::UnknownGroup(section.name.clone()),
                    ));
                }
                no_blocks(document, section, 0)?;
                let mut reader = FieldReader::new(document, &section.fields, section.line);
                let concepts_field = reader.required_value("Concepts")?;
                let concepts = list(&concepts_field.value);
                reader.finish()?;
                for concept in &concepts {
                    if !known.has_concept(concept) {
                        return Err(document.error(
                            concepts_field.line,
                            LoadErrorKind::UnknownConcept(concept.clone()),
                        ));
                    }
                    if !seen_concepts.insert(concept.clone()) {
                        return Err(document.error(
                            concepts_field.line,
                            LoadErrorKind::DuplicateIdentifier(concept.clone()),
                        ));
                    }
                }
                sections
                    .groups
                    .insert(section.name.clone(), (section, concepts));
            }
            CONCEPT_SECTION => {
                no_blocks(document, section, 0)?;
                let mut reader = FieldReader::new(document, &section.fields, section.line);
                let prerequisites = list(&reader.required_value("Prerequisites")?.value);
                reader.finish()?;
                sections.prerequisites.push((section, prerequisites));
            }
            NOTATION_SECTION => {
                let notation = read_notation(document, section, known)?;
                sections.notations.push((section, notation));
            }
            UNIT_SECTION => {
                let declaration = read_unit_section(document, section, true, &mut pool)?;
                if let Some(group) = declaration
                    .from
                    .as_ref()
                    .filter(|group| !group_order.contains(group))
                {
                    return Err(
                        document.error(section.line, LoadErrorKind::UnknownGroup(group.clone()))
                    );
                }
                sections.units.push((section, declaration));
            }
            other => {
                return Err(document.error(
                    section.line,
                    LoadErrorKind::UnknownSection(other.to_string()),
                ));
            }
        }
    }
    Ok(sections)
}

fn check_prerequisite_order(
    document: &Document,
    order: &[&str],
    prerequisites: &[(&Section, Vec<String>)],
) -> Result<BTreeMap<String, Vec<String>>, LoadError> {
    let position: BTreeMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(index, concept)| (*concept, index))
        .collect();
    let mut edges = BTreeMap::new();
    for (section, required) in prerequisites {
        let Some(concept_position) = position.get(section.name.as_str()) else {
            return Err(document.error(
                section.line,
                LoadErrorKind::ConceptNotInCurriculum(section.name.clone()),
            ));
        };
        for prerequisite in required {
            let Some(prerequisite_position) = position.get(prerequisite.as_str()) else {
                return Err(document.error(
                    section.line,
                    LoadErrorKind::ConceptNotInCurriculum(prerequisite.clone()),
                ));
            };
            if prerequisite_position >= concept_position {
                return Err(document.error(
                    section.line,
                    LoadErrorKind::PrerequisiteNotEarlier {
                        concept: section.name.clone(),
                        prerequisite: prerequisite.clone(),
                    },
                ));
            }
        }
        edges.insert(section.name.clone(), required.clone());
    }
    Ok(edges)
}

fn check_notations(
    document: &Document,
    group_order: &[String],
    notations: &[(&Section, NotationVersion)],
) -> Result<(), LoadError> {
    if notations.is_empty() {
        return Err(document.error(1, LoadErrorKind::NoNotation));
    }
    let mut by_identifier: BTreeMap<&str, Vec<&(&Section, NotationVersion)>> = BTreeMap::new();
    for entry in notations {
        let (section, notation) = entry;
        if !group_order.contains(&notation.from) {
            return Err(document.error(
                section.line,
                LoadErrorKind::UnknownGroup(notation.from.clone()),
            ));
        }
        by_identifier
            .entry(notation.identifier.as_str())
            .or_default()
            .push(entry);
    }
    let mut first_versions = Vec::new();
    for (identifier, versions) in &mut by_identifier {
        versions.sort_by_key(|(_, notation)| notation.version);
        for (expected, (section, notation)) in (1u32..).zip(versions.iter()) {
            if notation.version != expected {
                return Err(document.error(
                    section.line,
                    LoadErrorKind::NotationVersionGap((*identifier).to_string()),
                ));
            }
        }
        let (first_section, first) = versions[0];
        if let Some((section, _)) = versions
            .iter()
            .find(|(_, notation)| notation.from != first.from)
        {
            return Err(document.error(
                section.line,
                LoadErrorKind::NotationFromDiffers((*identifier).to_string()),
            ));
        }
        for pair in versions.windows(2) {
            let [(_, earlier), (section, later)] = pair else {
                continue;
            };
            if let Some(mode) = later
                .notation
                .modes_reading_less_than(&earlier.notation)
                .first()
            {
                return Err(document.error(
                    section.line,
                    LoadErrorKind::NotationReadsLess {
                        notation: notation_label(&later.identifier, later.version),
                        mode: *mode,
                    },
                ));
            }
        }
        first_versions.push((*first_section, first, versions[versions.len() - 1]));
    }
    let group_position = |group: &str| group_order.iter().position(|candidate| candidate == group);
    first_versions.sort_by_key(|(_, first, _)| group_position(&first.from));
    for pair in first_versions.windows(2) {
        let [
            (_, earlier_first, earlier_last),
            (section, later_first, later_last),
        ] = pair
        else {
            continue;
        };
        if earlier_first.from == later_first.from {
            return Err(document.error(
                section.line,
                LoadErrorKind::SharedFromGroup(later_first.from.clone()),
            ));
        }
        let (_, earlier) = earlier_last;
        let (last_section, later) = later_last;
        if let Some(mode) = later
            .notation
            .modes_reading_less_than(&earlier.notation)
            .first()
        {
            return Err(document.error(
                last_section.line,
                LoadErrorKind::NotationReadsLess {
                    notation: notation_label(&later.identifier, later.version),
                    mode: *mode,
                },
            ));
        }
    }
    let applies_from_first = first_versions
        .first()
        .is_some_and(|(_, first, _)| group_order.first() == Some(&first.from));
    if !applies_from_first {
        return Err(document.error(1, LoadErrorKind::NoNotationForFirstGroup));
    }
    Ok(())
}

fn read_shipped(
    identifier: &str,
    document: &Document,
    notations: &[NotationVersion],
) -> Result<Vec<ShippedNotation>, LoadError> {
    check_title(document, identifier)?;
    FieldReader::new(document, &document.fields, 1).finish()?;
    let mut shipped = Vec::new();
    for section in &document.sections {
        if section.kind != NOTATION_SECTION {
            return Err(document.error(
                section.line,
                LoadErrorKind::UnknownSection(section.kind.clone()),
            ));
        }
        let (notation_identifier, version) = parse_notation_heading(document, section)?;
        no_blocks(document, section, 0)?;
        let mut reader = FieldReader::new(document, &section.fields, section.line);
        let notation = read_modes(document, section, &mut reader)?;
        reader.finish()?;
        let label = notation_label(&notation_identifier, version);
        let current = notations
            .iter()
            .find(|current| current.identifier == notation_identifier && current.version == version)
            .ok_or_else(|| {
                document.error(
                    section.line,
                    LoadErrorKind::ShippedNotationMissing(label.clone()),
                )
            })?;
        if current.notation != notation {
            return Err(document.error(section.line, LoadErrorKind::ShippedNotationChanged(label)));
        }
        shipped.push(ShippedNotation {
            identifier: notation_identifier,
            version,
            notation,
        });
    }
    Ok(shipped)
}

fn read_text(
    document: &Document,
    group_order: &[String],
    order: &[&str],
) -> Result<CurriculumText, LoadError> {
    FieldReader::new(document, &document.fields, 1).finish()?;
    let mut text = CurriculumText {
        name: document.title.clone(),
        ..CurriculumText::default()
    };
    for section in &document.sections {
        no_blocks(document, section, 0)?;
        let mut reader = FieldReader::new(document, &section.fields, section.line);
        let name = reader.required_value("Name")?.value.clone();
        reader.finish()?;
        match section.kind.as_str() {
            GROUP_SECTION if group_order.contains(&section.name) => {
                text.groups.insert(section.name.clone(), name);
            }
            GROUP_SECTION => {
                return Err(document.error(
                    section.line,
                    LoadErrorKind::UnknownGroup(section.name.clone()),
                ));
            }
            CONCEPT_SECTION if order.contains(&section.name.as_str()) => {
                text.concepts.insert(section.name.clone(), name);
            }
            CONCEPT_SECTION => {
                return Err(document.error(
                    section.line,
                    LoadErrorKind::ConceptNotInCurriculum(section.name.clone()),
                ));
            }
            other => {
                return Err(document.error(
                    section.line,
                    LoadErrorKind::UnknownSection(other.to_string()),
                ));
            }
        }
    }
    Ok(text)
}

pub(crate) fn read_curriculum(
    identifier: &str,
    document: &Document,
    files: &CurriculumFiles,
    known: &Known,
) -> Result<Curriculum, LoadError> {
    check_title(document, identifier)?;
    if known.is_retired(identifier) {
        return Err(document.error(1, LoadErrorKind::RetiredIdentifier(identifier.to_string())));
    }
    let mut reader = FieldReader::new(document, &document.fields, 1);
    let locale_field = reader.required_value("Locale")?;
    if !is_locale(&locale_field.value) {
        return Err(document.error(
            locale_field.line,
            LoadErrorKind::InvalidLocale(locale_field.value.clone()),
        ));
    }
    let locale = locale_field.value.clone();
    let groups_field = reader.required_value("Groups")?;
    let group_order = list(&groups_field.value);
    let documents_field = reader.required("Documents")?;
    let documents = list(&documents_field.value);
    reader.finish()?;
    let mut seen_groups = BTreeSet::new();
    for group in &group_order {
        if !is_identifier(group) {
            return Err(document.error(
                groups_field.line,
                LoadErrorKind::InvalidIdentifier(group.clone()),
            ));
        }
        if !seen_groups.insert(group) {
            return Err(document.error(
                groups_field.line,
                LoadErrorKind::DuplicateIdentifier(group.clone()),
            ));
        }
    }
    check_sources(document, documents_field.line, known, &documents)?;
    let mut sections = read_sections(document, known, &group_order)?;
    let sections_units = std::mem::take(&mut sections.units);
    check_unique_stages(
        document,
        group_order.first().map(String::as_str),
        &sections_units
            .iter()
            .map(|(section, declaration)| (*section, declaration))
            .collect::<Vec<_>>(),
    )?;
    let sections_units: Vec<UnitDeclaration> = sections_units
        .into_iter()
        .map(|(_, declaration)| declaration)
        .collect();
    let mut groups = Vec::new();
    for group in &group_order {
        let (_, concepts) = sections.groups.get(group).ok_or_else(|| {
            document.error(
                groups_field.line,
                LoadErrorKind::GroupWithoutSection(group.clone()),
            )
        })?;
        groups.push(CurriculumGroup {
            identifier: group.clone(),
            concepts: concepts.clone(),
        });
    }
    let order: Vec<&str> = groups
        .iter()
        .flat_map(|group| group.concepts.iter().map(String::as_str))
        .collect();
    let prerequisites = check_prerequisite_order(document, &order, &sections.prerequisites)?;
    check_notations(document, &group_order, &sections.notations)?;
    if let Some((section, notation)) = sections
        .notations
        .iter()
        .find(|(_, notation)| known.is_retired(&notation.identifier))
    {
        return Err(document.error(
            section.line,
            LoadErrorKind::RetiredIdentifier(notation.identifier.clone()),
        ));
    }
    let notations: Vec<NotationVersion> = sections
        .notations
        .into_iter()
        .map(|(_, notation)| notation)
        .collect();
    let shipped = match &files.shipped {
        Some(shipped) => read_shipped(identifier, shipped, &notations)?,
        None => Vec::new(),
    };
    let mut texts = BTreeMap::new();
    for (text_locale, text_document) in &files.texts {
        texts.insert(
            text_locale.clone(),
            read_text(text_document, &group_order, &order)?,
        );
    }
    Ok(Curriculum {
        identifier: identifier.to_string(),
        locale,
        documents,
        groups,
        prerequisites,
        notations,
        shipped,
        units: sections_units,
        texts,
    })
}

#[cfg(test)]
mod tests {
    use crate::error::LoadErrorKind;
    use crate::load::load;
    use crate::model::{ConceptSet, DisplayUnit, QuantityKind};
    use calc_syntax::{NotationConflict, NotationMode};

    const BOOK: &str = "# book\n\nTitle: A Book\nAuthors: Ann Author\nYear: 2020\nIdentifier: https://example.org/book\nLicence: CC-BY-4.0\n";
    const PLAN: &str = "# plan\n\nTitle: A Plan\nAuthors: A Ministry\nYear: 2021\nIdentifier: https://example.org/plan\nLicence: unknown\n";
    const OBJECT: &str = "# circle\n\n## Role radius\nDimension: m\nQuantity: length\n\n## Role diameter\nDimension: m\nQuantity: length\n";
    const OBJECT_TEXT: &str = "# Circle\nRunning: circle\n\n## Role radius\nName: Radius\nRunning: radius\n\n## Role diameter\nName: Diameter\nRunning: diameter\n";
    const CIRCLE: &str = "# circle\n\nLevel: isced-1\nPrerequisites:\nStart: yes\nSources: book\n";
    const CIRCLE_TEXT: &str = "# Circle\n\nStatement: A circle.\n";
    const DIAMETER: &str =
        "# circle-diameter\n\nLevel: isced-2\nPrerequisites: circle\nSources: book\n";
    const DIAMETER_TEXT: &str = "# Diameter\n\nStatement: Twice the radius.\n";
    const HEAD: &str = "# school\n\nLocale: de-DE\nGroups: grade-5, grade-6\nDocuments: plan\n\n## Group grade-5\nConcepts: circle\n\n## Group grade-6\nConcepts: circle-diameter\n\n## Concept circle-diameter\nPrerequisites: circle\n";
    const MODE_SOURCES: &str = "Sources: book\nSource decimal_separator: book\nSource division_signs: book\nSource multiplication_signs: book\nSource juxtaposition: book\nSource coordinates: book\nSource mixed_numbers: book\nSource recurring_mark: book\n";
    const PRIMARY_MODES: &str = "decimal_separator: comma\ndivision_signs: colon\nmultiplication_signs: middle_dot\njuxtaposition: none\ncoordinates: parentheses_bar\nmixed_numbers: none\nrecurring_mark: none\n";

    fn notation(heading: &str, from: &str, modes: &str) -> String {
        format!("\n## Notation {heading}\nFrom: {from}\n{modes}{MODE_SOURCES}")
    }

    fn primary() -> String {
        notation("primary 1", "grade-5", PRIMARY_MODES)
    }

    fn algebra() -> String {
        notation(
            "algebra 1",
            "grade-6",
            &PRIMARY_MODES
                .replace("juxtaposition: none", "juxtaposition: number_letter")
                .replace("recurring_mark: none", "recurring_mark: bar"),
        )
    }

    fn curriculum() -> String {
        format!("{HEAD}{}{}", primary(), algebra())
    }

    fn load_curriculum(extra: &[(&str, &str)]) -> Result<ConceptSet, crate::LoadError> {
        let mut files: Vec<(&str, &str)> = vec![
            ("sources/book.md", BOOK),
            ("sources/plan.md", PLAN),
            ("objects/circle.md", OBJECT),
            ("objects/circle.en.md", OBJECT_TEXT),
            ("concepts/circle/node.md", CIRCLE),
            ("concepts/circle/en.md", CIRCLE_TEXT),
            ("concepts/circle-diameter/node.md", DIAMETER),
            ("concepts/circle-diameter/en.md", DIAMETER_TEXT),
        ];
        files.extend_from_slice(extra);
        let bytes: Vec<(&str, &[u8])> = files
            .iter()
            .map(|(path, text)| (*path, text.as_bytes()))
            .collect();
        load(&bytes)
    }

    fn kind_of(text: &str) -> LoadErrorKind {
        load_curriculum(&[("curricula/school/curriculum.md", text)])
            .unwrap_err()
            .kind
    }

    #[test]
    fn curriculum_loads_its_order_edges_and_notations() {
        let text = curriculum();
        let set = load_curriculum(&[("curricula/school/curriculum.md", &text)]).unwrap();
        let school = &set.curricula[0];
        assert_eq!(school.concept_order(), vec!["circle", "circle-diameter"]);
        assert_eq!(
            school.prerequisites.get("circle-diameter"),
            Some(&vec!["circle".to_string()])
        );
        assert_eq!(school.notations.len(), 2);
    }

    #[test]
    fn locale_file_names_groups_and_concepts() {
        let text = curriculum();
        let names =
            "# Schule\n\n## Group grade-5\nName: Klasse 5\n\n## Concept circle\nName: Kreis\n";
        let set = load_curriculum(&[
            ("curricula/school/curriculum.md", &text),
            ("curricula/school/de.md", names),
        ])
        .unwrap();
        let german = &set.curricula[0].texts["de"];
        assert_eq!(
            (german.name.as_str(), german.groups["grade-5"].as_str()),
            ("Schule", "Klasse 5")
        );
    }

    #[test]
    fn locale_file_naming_an_unknown_group_is_rejected() {
        let text = curriculum();
        let names = "# Schule\n\n## Group grade-9\nName: Klasse 9\n";
        let error = load_curriculum(&[
            ("curricula/school/curriculum.md", &text),
            ("curricula/school/de.md", names),
        ])
        .unwrap_err();
        assert_eq!(
            error.kind,
            LoadErrorKind::UnknownGroup("grade-9".to_string())
        );
    }

    #[test]
    fn invalid_locale_is_rejected() {
        assert_eq!(
            kind_of(&curriculum().replace("Locale: de-DE", "Locale: German")),
            LoadErrorKind::InvalidLocale("German".to_string())
        );
    }

    #[test]
    fn group_section_not_in_the_group_order_is_rejected() {
        assert_eq!(
            kind_of(&curriculum().replace("## Group grade-6", "## Group grade-7")),
            LoadErrorKind::UnknownGroup("grade-7".to_string())
        );
    }

    #[test]
    fn group_without_its_section_is_rejected() {
        assert_eq!(
            kind_of(&curriculum().replace(
                "Groups: grade-5, grade-6",
                "Groups: grade-5, grade-6, grade-7"
            )),
            LoadErrorKind::GroupWithoutSection("grade-7".to_string())
        );
    }

    #[test]
    fn concept_in_two_groups_is_rejected() {
        assert_eq!(
            kind_of(&curriculum().replace(
                "Concepts: circle-diameter",
                "Concepts: circle-diameter, circle"
            )),
            LoadErrorKind::DuplicateIdentifier("circle".to_string())
        );
    }

    #[test]
    fn unknown_concept_in_a_group_is_rejected() {
        assert_eq!(
            kind_of(&curriculum().replace("Concepts: circle\n", "Concepts: sphere\n")),
            LoadErrorKind::UnknownConcept("sphere".to_string())
        );
    }

    #[test]
    fn prerequisite_outside_the_curriculum_is_rejected() {
        assert_eq!(
            kind_of(&curriculum().replace("Prerequisites: circle", "Prerequisites: circle-area")),
            LoadErrorKind::ConceptNotInCurriculum("circle-area".to_string())
        );
    }

    #[test]
    fn prerequisite_that_comes_later_is_rejected() {
        let swapped = curriculum()
            .replace("Concepts: circle\n", "Concepts: circle-diameter\n")
            .replace(
                "Concepts: circle-diameter\n\n## Concept",
                "Concepts: circle\n\n## Concept",
            );
        assert_eq!(
            kind_of(&swapped),
            LoadErrorKind::PrerequisiteNotEarlier {
                concept: "circle-diameter".to_string(),
                prerequisite: "circle".to_string()
            }
        );
    }

    #[test]
    fn concept_that_is_its_own_prerequisite_is_rejected() {
        assert_eq!(
            kind_of(
                &curriculum().replace("Prerequisites: circle", "Prerequisites: circle-diameter")
            ),
            LoadErrorKind::PrerequisiteNotEarlier {
                concept: "circle-diameter".to_string(),
                prerequisite: "circle-diameter".to_string()
            }
        );
    }

    #[test]
    fn unknown_document_is_rejected() {
        assert_eq!(
            kind_of(&curriculum().replace("Documents: plan", "Documents: decree")),
            LoadErrorKind::UnknownSource("decree".to_string())
        );
    }

    #[test]
    fn unit_section_is_loaded_with_posed_and_displayed() {
        let text = format!(
            "{}\n## Unit speed\nPosed: m/s\nDisplayed: km/h\n",
            curriculum()
        );
        let set = load_curriculum(&[("curricula/school/curriculum.md", &text)]).unwrap();
        let declaration = &set.curricula[0].units[0];
        assert_eq!(
            (
                declaration.kind,
                declaration.posed.clone(),
                declaration.displayed.clone()
            ),
            (
                QuantityKind::Speed,
                Some(DisplayUnit::Units(vec!["m/s".to_string()])),
                vec![DisplayUnit::Units(vec!["km/h".to_string()])]
            )
        );
    }

    fn with_units(sections: &str) -> String {
        format!("{}\n{sections}", curriculum())
    }

    fn units(texts: &[&str]) -> Vec<DisplayUnit> {
        texts
            .iter()
            .map(|text| DisplayUnit::Units(vec![(*text).to_string()]))
            .collect()
    }

    #[test]
    fn displayed_list_is_loaded_in_its_order() {
        let text = with_units("## Unit length\nDisplayed: mm, cm, m, km\n");
        let set = load_curriculum(&[("curricula/school/curriculum.md", &text)]).unwrap();
        assert_eq!(
            set.curricula[0].units[0].displayed,
            units(&["mm", "cm", "m", "km"])
        );
    }

    #[test]
    fn displayed_list_with_a_unit_of_another_dimension_is_rejected() {
        assert_eq!(
            kind_of(&with_units("## Unit length\nDisplayed: mm, s\n")),
            LoadErrorKind::DisplayUnitOfOtherDimension {
                kind: "length".to_string(),
                unit: "s".to_string()
            }
        );
    }

    #[test]
    fn displayed_list_that_descends_is_rejected() {
        assert_eq!(
            kind_of(&with_units("## Unit length\nDisplayed: m, cm\n")),
            LoadErrorKind::DisplayedListNotAscending("m, cm".to_string())
        );
    }

    #[test]
    fn displayed_list_repeating_a_scale_factor_is_rejected() {
        assert_eq!(
            kind_of(&with_units("## Unit volume\nDisplayed: dm^3, L\n")),
            LoadErrorKind::DisplayedListNotAscending("dm^3, L".to_string())
        );
    }

    #[test]
    fn displayed_list_whose_factors_do_not_terminate_is_loaded() {
        let text = with_units("## Unit length\nDisplayed: in, ft, mi\n");
        let set = load_curriculum(&[("curricula/school/curriculum.md", &text)]).unwrap();
        assert_eq!(
            set.curricula[0].units[0].displayed,
            units(&["in", "ft", "mi"])
        );
    }

    #[test]
    fn displayed_list_with_a_factor_of_pi_is_rejected() {
        assert_eq!(
            kind_of(&with_units("## Unit angle\nDisplayed: deg, rad\n")),
            LoadErrorKind::DisplayedListMixesPi("deg, rad".to_string())
        );
    }

    #[test]
    fn compound_in_a_displayed_list_is_rejected() {
        assert_eq!(
            kind_of(&with_units("## Unit length\nDisplayed: m cm, km\n")),
            LoadErrorKind::CompoundNotAlone("m cm, km".to_string())
        );
    }

    #[test]
    fn compound_alone_is_a_list_of_one() {
        let text = with_units("## Unit time\nDisplayed: h min\n");
        let set = load_curriculum(&[("curricula/school/curriculum.md", &text)]).unwrap();
        assert_eq!(
            set.curricula[0].units[0].displayed,
            [DisplayUnit::Units(vec!["h".to_string(), "min".to_string()])]
        );
    }

    #[test]
    fn scale_in_a_displayed_list_is_rejected() {
        assert_eq!(
            kind_of(&with_units(
                "## Unit temperature\nDisplayed: celsius, fahrenheit\n"
            )),
            LoadErrorKind::ScaleNotAlone("celsius, fahrenheit".to_string())
        );
    }

    #[test]
    fn staged_unit_section_applies_from_its_group() {
        let text = with_units(
            "## Unit length\nDisplayed: cm, m\n\n## Unit length grade-6\nDisplayed: mm, cm, m, km\n",
        );
        let set = load_curriculum(&[("curricula/school/curriculum.md", &text)]).unwrap();
        let stages: Vec<Option<&str>> = set.curricula[0]
            .units
            .iter()
            .map(|declaration| declaration.from.as_deref())
            .collect();
        assert_eq!(stages, [None, Some("grade-6")]);
    }

    #[test]
    fn stage_of_an_unknown_group_is_rejected() {
        assert_eq!(
            kind_of(&with_units("## Unit length grade-9\nDisplayed: cm, m\n")),
            LoadErrorKind::UnknownGroup("grade-9".to_string())
        );
    }

    #[test]
    fn unstaged_section_and_one_staged_at_the_first_group_are_rejected() {
        assert_eq!(
            kind_of(&with_units(
                "## Unit length\nDisplayed: cm, m\n\n## Unit length grade-5\nDisplayed: m\n"
            )),
            LoadErrorKind::DuplicateUnitSection("length grade-5".to_string())
        );
    }

    #[test]
    fn unit_section_of_an_unknown_kind_is_rejected() {
        let text = format!("{}\n## Unit colour\nDisplayed: m\n", curriculum());
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::UnknownQuantityKind("colour".to_string())
        );
    }

    #[test]
    fn unit_section_with_a_wrong_dimension_names_its_line() {
        let text = format!("{}\n## Unit speed\nDisplayed: km\n", curriculum());
        let lines = text.lines().count();
        let error = load_curriculum(&[("curricula/school/curriculum.md", &text)]).unwrap_err();
        assert_eq!(
            (error.kind, error.line),
            (
                LoadErrorKind::DisplayUnitOfOtherDimension {
                    kind: "speed".to_string(),
                    unit: "km".to_string()
                },
                lines
            )
        );
    }

    #[test]
    fn posed_compound_is_rejected() {
        let text = format!(
            "{}\n## Unit time\nPosed: h min\nDisplayed: h min\n",
            curriculum()
        );
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::CompoundNotAllowed("time".to_string())
        );
    }

    #[test]
    fn curriculum_without_notation_is_rejected() {
        assert_eq!(kind_of(HEAD), LoadErrorKind::NoNotation);
    }

    #[test]
    fn notation_without_a_mode_key_is_rejected() {
        let text = format!("{HEAD}{}", primary().replace("recurring_mark: none\n", ""));
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::MissingKey("recurring_mark".to_string())
        );
    }

    #[test]
    fn notation_with_an_unknown_value_is_rejected() {
        let text = format!(
            "{HEAD}{}",
            primary().replace("mixed_numbers: none", "mixed_numbers: tab")
        );
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::InvalidModeValue {
                mode: NotationMode::MixedNumbers,
                value: "tab".to_string()
            }
        );
    }

    #[test]
    fn notation_heading_without_a_version_is_rejected() {
        let text = format!(
            "{HEAD}{}",
            primary().replace("## Notation primary 1", "## Notation primary")
        );
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::InvalidNotationHeading("primary".to_string())
        );
    }

    #[test]
    fn conflicting_notation_is_rejected() {
        let text = format!(
            "{HEAD}{}",
            primary().replace(
                "coordinates: parentheses_bar",
                "coordinates: parentheses_comma"
            )
        );
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::NotationConflict(NotationConflict::CommaDecimalWithCommaCoordinates)
        );
    }

    #[test]
    fn mode_source_that_does_not_exist_is_rejected() {
        let text = format!(
            "{HEAD}{}",
            primary().replace("Source coordinates: book", "Source coordinates: leaflet")
        );
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::UnknownSource("leaflet".to_string())
        );
    }

    #[test]
    fn mode_without_a_source_is_rejected() {
        let text = format!(
            "{HEAD}{}",
            primary().replace("Source coordinates: book\n", "")
        );
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::MissingKey("Source coordinates".to_string())
        );
    }

    #[test]
    fn sources_that_differ_from_the_mode_sources_are_rejected() {
        let text = format!(
            "{HEAD}{}",
            primary().replace("Sources: book", "Sources: book, plan")
        );
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::SourcesDiffer("primary 1".to_string())
        );
    }

    #[test]
    fn notation_versions_with_a_gap_are_rejected() {
        let text = format!(
            "{HEAD}{}{}",
            primary(),
            primary().replace("## Notation primary 1", "## Notation primary 3")
        );
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::NotationVersionGap("primary".to_string())
        );
    }

    #[test]
    fn notation_versions_with_different_from_groups_are_rejected() {
        let second = notation("primary 2", "grade-6", PRIMARY_MODES);
        let text = format!("{HEAD}{}{second}", primary());
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::NotationFromDiffers("primary".to_string())
        );
    }

    #[test]
    fn notation_from_an_unknown_group_is_rejected() {
        let text = format!(
            "{HEAD}{}",
            primary().replace("From: grade-5", "From: grade-9")
        );
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::UnknownGroup("grade-9".to_string())
        );
    }

    #[test]
    fn two_notations_from_one_group_are_rejected() {
        let text = format!(
            "{HEAD}{}{}",
            primary(),
            algebra().replace("From: grade-6", "From: grade-5")
        );
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::SharedFromGroup("grade-5".to_string())
        );
    }

    #[test]
    fn no_notation_for_the_first_group_is_rejected() {
        let text = format!("{HEAD}{}", algebra());
        assert_eq!(kind_of(&text), LoadErrorKind::NoNotationForFirstGroup);
    }

    #[test]
    fn later_group_notation_reading_less_is_rejected() {
        let narrower = algebra().replace("recurring_mark: bar", "recurring_mark: none");
        let wider_first = primary().replace("recurring_mark: none", "recurring_mark: dots");
        let text = format!("{HEAD}{wider_first}{narrower}");
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::NotationReadsLess {
                notation: "algebra 1".to_string(),
                mode: NotationMode::RecurringMark
            }
        );
    }

    #[test]
    fn later_version_reading_less_is_rejected() {
        let first = primary().replace("division_signs: colon", "division_signs: slash, colon");
        let second = primary().replace("## Notation primary 1", "## Notation primary 2");
        let text = format!("{HEAD}{first}{second}");
        assert_eq!(
            kind_of(&text),
            LoadErrorKind::NotationReadsLess {
                notation: "primary 2".to_string(),
                mode: NotationMode::DivisionSigns
            }
        );
    }

    #[test]
    fn shipped_version_that_matches_loads() {
        let text = curriculum();
        let shipped = format!("# school\n\n## Notation primary 1\n{PRIMARY_MODES}");
        let set = load_curriculum(&[
            ("curricula/school/curriculum.md", &text),
            ("curricula/school/shipped.md", &shipped),
        ])
        .unwrap();
        assert_eq!(set.curricula[0].shipped.len(), 1);
    }

    #[test]
    fn shipped_version_missing_from_the_curriculum_is_rejected() {
        let text = curriculum();
        let shipped = format!("# school\n\n## Notation primary 2\n{PRIMARY_MODES}");
        let error = load_curriculum(&[
            ("curricula/school/curriculum.md", &text),
            ("curricula/school/shipped.md", &shipped),
        ])
        .unwrap_err();
        assert_eq!(
            error.kind,
            LoadErrorKind::ShippedNotationMissing("primary 2".to_string())
        );
    }

    #[test]
    fn shipped_version_whose_values_changed_is_rejected() {
        let text = curriculum();
        let shipped = format!(
            "# school\n\n## Notation primary 1\n{}",
            PRIMARY_MODES.replace("division_signs: colon", "division_signs: slash")
        );
        let error = load_curriculum(&[
            ("curricula/school/curriculum.md", &text),
            ("curricula/school/shipped.md", &shipped),
        ])
        .unwrap_err();
        assert_eq!(
            error.kind,
            LoadErrorKind::ShippedNotationChanged("primary 1".to_string())
        );
    }

    #[test]
    fn shipped_file_without_a_curriculum_is_rejected() {
        let shipped = format!("# school\n\n## Notation primary 1\n{PRIMARY_MODES}");
        let error = load_curriculum(&[("curricula/school/shipped.md", &shipped)]).unwrap_err();
        assert_eq!(error.kind, LoadErrorKind::UnexpectedPath);
    }

    #[test]
    fn retired_curriculum_identifier_is_rejected() {
        let text = curriculum();
        let error = load_curriculum(&[
            ("curricula/school/curriculum.md", &text),
            ("retired.md", "# retired\n\nschool\n"),
        ])
        .unwrap_err();
        assert_eq!(
            error.kind,
            LoadErrorKind::RetiredIdentifier("school".to_string())
        );
    }
}
