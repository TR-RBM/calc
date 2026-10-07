use std::collections::{BTreeMap, BTreeSet};

use calc_core::graph::{Arrow, DirectedGraph, LayeringError, NodeId, longest_path_layering};
use calc_expr::{BinderKind, ExprId, ExprPool, NodeView};
use calc_units::{Dimension, UnitTable};

use crate::curriculum::{CurriculumFiles, Known, read_curriculum};
use crate::document::{Document, Field, FieldReader, Section, list, parse_document};
use crate::error::{LoadError, LoadErrorKind};
use crate::identifier::{
    is_corpus_id, is_identifier, is_locale, is_role_identifier, role_identifier, role_object,
    way_identifier,
};
use crate::learn_text::{DecimalMark, render_learn_text};
use crate::model::{
    ActivityDefinition, ActivityKind, ActivityShape, ActivityVariation, BoundDefinition,
    BoundRelation, ConceptNode, ConceptSet, ConceptText, ObjectKind, PatternDefinition,
    QuantityKind, Role, Source, WayDefinition,
};
use crate::patterns::validate_patterns;
use crate::units::read_unit_system;
use crate::version::concept_set_version;

const NODE_FILE: &str = "node.md";
const MARKDOWN_EXTENSION: &str = ".md";
const CONCEPTS_DIRECTORY: &str = "concepts/";
const OBJECTS_DIRECTORY: &str = "objects/";
const SOURCES_DIRECTORY: &str = "sources/";
const CURRICULA_DIRECTORY: &str = "curricula/";
const UNIT_SYSTEMS_DIRECTORY: &str = "unit-systems/";
const NOTATION_DIRECTORY: &str = "notation/";
const DECIMAL_MARK_KEY: &str = "Decimal separator";
const RETIRED_FILE: &str = "retired.md";
const CURRICULUM_FILE: &str = "curriculum.md";
const SHIPPED_FILE: &str = "shipped.md";
const ENGLISH: &str = "en";
const DIMENSIONLESS: &str = "1";
const WAY_SECTION: &str = "Way";
const BOUND_SECTION: &str = "Bound";
const ROLE_SECTION: &str = "Role";
const PATTERN_SECTION: &str = "Pattern";
const RECOGNIZED_YES: &str = "yes";
const START_KEY: &str = "Start";
const START_YES: &str = "yes";
const RECOGNIZED_NO: &str = "no";
const SCENE_SECTION: &str = "Scene";
const ACTIVITY_SECTION: &str = "Activity";
const FEWEST_ACTIVITY_SHAPES: usize = 2;
const AT_MOST: &str = "at-most";
const AT_LEAST: &str = "at-least";
const STRICTLY_BETWEEN: &str = "strictly-between";

enum ContentPath<'path> {
    Node(&'path str),
    ConceptText {
        concept: &'path str,
        locale: &'path str,
    },
    Object(&'path str),
    ObjectText {
        object: &'path str,
        locale: &'path str,
    },
    Source(&'path str),
    Retired,
    Curriculum(&'path str),
    Shipped(&'path str),
    CurriculumText {
        curriculum: &'path str,
        locale: &'path str,
    },
    UnitSystem(&'path str),
    UnitSystemText {
        system: &'path str,
        locale: &'path str,
    },
    Notation(&'path str),
}

const RETIRED_TITLE: &str = "# retired";

fn classify(path: &str) -> Option<ContentPath<'_>> {
    if path == RETIRED_FILE {
        return Some(ContentPath::Retired);
    }
    if let Some(rest) = path.strip_prefix(CURRICULA_DIRECTORY) {
        let (curriculum, file) = rest.split_once('/')?;
        return match file {
            CURRICULUM_FILE => Some(ContentPath::Curriculum(curriculum)),
            SHIPPED_FILE => Some(ContentPath::Shipped(curriculum)),
            _ => {
                let locale = file.strip_suffix(MARKDOWN_EXTENSION)?;
                is_locale(locale).then_some(ContentPath::CurriculumText { curriculum, locale })
            }
        };
    }
    if let Some(rest) = path.strip_prefix(NOTATION_DIRECTORY) {
        let locale = rest.strip_suffix(MARKDOWN_EXTENSION)?;
        return is_locale(locale).then_some(ContentPath::Notation(locale));
    }
    if let Some(rest) = path.strip_prefix(UNIT_SYSTEMS_DIRECTORY) {
        let stem = rest.strip_suffix(MARKDOWN_EXTENSION)?;
        return match stem.split_once('.') {
            Some((system, locale)) if is_locale(locale) => {
                Some(ContentPath::UnitSystemText { system, locale })
            }
            Some(_) => None,
            None => (!stem.contains('/')).then_some(ContentPath::UnitSystem(stem)),
        };
    }
    if let Some(rest) = path.strip_prefix(CONCEPTS_DIRECTORY) {
        let (concept, file) = rest.split_once('/')?;
        if file == NODE_FILE {
            return Some(ContentPath::Node(concept));
        }
        let locale = file.strip_suffix(MARKDOWN_EXTENSION)?;
        return is_locale(locale).then_some(ContentPath::ConceptText { concept, locale });
    }
    if let Some(rest) = path.strip_prefix(OBJECTS_DIRECTORY) {
        let stem = rest.strip_suffix(MARKDOWN_EXTENSION)?;
        return match stem.split_once('.') {
            Some((object, locale)) if is_locale(locale) => {
                Some(ContentPath::ObjectText { object, locale })
            }
            Some(_) => None,
            None => Some(ContentPath::Object(stem)),
        };
    }
    let stem = path
        .strip_prefix(SOURCES_DIRECTORY)?
        .strip_suffix(MARKDOWN_EXTENSION)?;
    (!stem.contains('/')).then_some(ContentPath::Source(stem))
}

pub fn load(files: &[(&str, &[u8])]) -> Result<ConceptSet, LoadError> {
    let mut nodes = Vec::new();
    let mut concept_texts = Vec::new();
    let mut objects = Vec::new();
    let mut object_texts = Vec::new();
    let mut sources = Vec::new();
    let mut retired = Vec::new();
    let mut curriculum_files: BTreeMap<String, CurriculumFiles> = BTreeMap::new();
    let mut unit_system_files: BTreeMap<String, Document> = BTreeMap::new();
    let mut unit_system_texts: BTreeMap<String, Vec<(String, Document)>> = BTreeMap::new();
    let mut notations = Vec::new();
    for (path, bytes) in files {
        let unexpected = |kind| LoadError {
            file: (*path).to_string(),
            line: 0,
            kind,
        };
        let kind = classify(path).ok_or_else(|| unexpected(LoadErrorKind::UnexpectedPath))?;
        if matches!(kind, ContentPath::Retired) {
            retired.extend(read_retired(path, bytes)?);
            continue;
        }
        let document = parse_document(path, bytes)?;
        match kind {
            ContentPath::Node(concept) => nodes.push((concept.to_string(), document)),
            ContentPath::ConceptText { concept, locale } => {
                concept_texts.push((concept.to_string(), locale.to_string(), document))
            }
            ContentPath::Object(object) => objects.push((object.to_string(), document)),
            ContentPath::ObjectText { object, locale } => {
                object_texts.push((object.to_string(), locale.to_string(), document))
            }
            ContentPath::Source(source) => sources.push((source.to_string(), document)),
            ContentPath::Curriculum(curriculum) => {
                curriculum_entry(&mut curriculum_files, curriculum).curriculum = Some(document)
            }
            ContentPath::Shipped(curriculum) => {
                curriculum_entry(&mut curriculum_files, curriculum).shipped = Some(document)
            }
            ContentPath::CurriculumText { curriculum, locale } => {
                curriculum_entry(&mut curriculum_files, curriculum)
                    .texts
                    .push((locale.to_string(), document))
            }
            ContentPath::UnitSystem(system) => {
                unit_system_files.insert(system.to_string(), document);
            }
            ContentPath::UnitSystemText { system, locale } => unit_system_texts
                .entry(system.to_string())
                .or_default()
                .push((locale.to_string(), document)),
            ContentPath::Notation(locale) => notations.push((locale.to_string(), document)),
            ContentPath::Retired => {}
        }
    }
    let sources = sources
        .iter()
        .map(|(identifier, document)| read_source(identifier, document))
        .collect::<Result<Vec<_>, _>>()?;
    let mut units = UnitTable::new();
    let mut objects = objects
        .iter()
        .map(|(identifier, document)| read_object(identifier, document, &mut units))
        .collect::<Result<Vec<_>, _>>()?;
    for (object, locale, document) in &object_texts {
        apply_object_text(&mut objects, object, locale, document)?;
    }
    let mut concepts = nodes
        .iter()
        .map(|(identifier, document)| read_node(identifier, document))
        .collect::<Result<Vec<_>, _>>()?;
    let marks = notations
        .iter()
        .map(|(locale, document)| read_notation(locale, document))
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut text_pool = ExprPool::new();
    for (concept, locale, document) in &concept_texts {
        let mark = marks.get(locale).copied();
        apply_concept_text(
            &mut concepts,
            concept,
            locale,
            document,
            mark,
            &mut text_pool,
        )?;
    }
    sort_by_identifier(&mut concepts, &mut objects);
    let known = Known {
        concepts: &concepts,
        sources: &sources,
        retired: &retired,
    };
    let mut curricula = Vec::new();
    for (identifier, files) in &curriculum_files {
        let document = files.curriculum.as_ref().ok_or_else(|| {
            let orphan = files
                .shipped
                .iter()
                .chain(files.texts.iter().map(|(_, document)| document))
                .next()
                .map(|document| document.path.clone())
                .unwrap_or_default();
            set_error(orphan, LoadErrorKind::UnexpectedPath)
        })?;
        curricula.push(read_curriculum(identifier, document, files, &known)?);
    }
    let mut unit_pool = ExprPool::new();
    let mut unit_systems = Vec::new();
    for (system, texts) in &unit_system_texts {
        if !unit_system_files.contains_key(system) {
            let orphan = texts
                .first()
                .map(|(_, document)| document.path.clone())
                .unwrap_or_default();
            return Err(set_error(
                orphan,
                LoadErrorKind::UnknownUnitSystem(system.clone()),
            ));
        }
    }
    for (system, document) in &unit_system_files {
        if retired.iter().any(|identifier| identifier == system) {
            return Err(document.error(1, LoadErrorKind::RetiredIdentifier(system.clone())));
        }
        let texts = unit_system_texts
            .get(system)
            .map(Vec::as_slice)
            .unwrap_or_default();
        unit_systems.push(read_unit_system(system, document, texts, &mut unit_pool)?);
    }
    let mut set = ConceptSet {
        version: concept_set_version(files),
        concepts,
        objects,
        sources,
        curricula,
        unit_systems,
        retired,
    };
    set.sources
        .sort_by(|left, right| left.identifier.cmp(&right.identifier));
    validate(&set)?;
    Ok(set)
}

fn curriculum_entry<'files>(
    files: &'files mut BTreeMap<String, CurriculumFiles>,
    curriculum: &str,
) -> &'files mut CurriculumFiles {
    files
        .entry(curriculum.to_string())
        .or_insert_with(|| CurriculumFiles {
            curriculum: None,
            shipped: None,
            texts: Vec::new(),
        })
}

fn sort_by_identifier(concepts: &mut [ConceptNode], objects: &mut [ObjectKind]) {
    concepts.sort_by(|left, right| left.identifier.cmp(&right.identifier));
    objects.sort_by(|left, right| left.identifier.cmp(&right.identifier));
}

pub(crate) fn check_title(document: &Document, identifier: &str) -> Result<(), LoadError> {
    if !is_identifier(identifier) {
        return Err(document.error(1, LoadErrorKind::InvalidIdentifier(identifier.to_string())));
    }
    if document.title != identifier {
        return Err(document.error(1, LoadErrorKind::TitleDoesNotMatchPath));
    }
    Ok(())
}

fn read_retired(path: &str, bytes: &[u8]) -> Result<Vec<String>, LoadError> {
    let error = |line: usize, kind| LoadError {
        file: path.to_string(),
        line,
        kind,
    };
    let text = std::str::from_utf8(bytes).map_err(|_| error(0, LoadErrorKind::NotUtf8))?;
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line));
    if lines.next().map(|(_, line)| line) != Some(RETIRED_TITLE) {
        return Err(error(1, LoadErrorKind::MissingTitle));
    }
    let mut retired = Vec::new();
    for (number, line) in lines {
        let identifier = line.trim();
        if identifier.is_empty() {
            continue;
        }
        let valid = is_identifier(identifier)
            || identifier
                .split_once('/')
                .is_some_and(|(concept, name)| is_identifier(concept) && is_identifier(name))
            || is_role_identifier(identifier);
        if !valid {
            return Err(error(
                number,
                LoadErrorKind::InvalidIdentifier(identifier.to_string()),
            ));
        }
        retired.push(identifier.to_string());
    }
    Ok(retired)
}

fn read_source(identifier: &str, document: &Document) -> Result<Source, LoadError> {
    check_title(document, identifier)?;
    if let Some(section) = document.sections.first() {
        return Err(document.error(
            section.line,
            LoadErrorKind::UnknownSection(section.kind.clone()),
        ));
    }
    let mut reader = FieldReader::new(document, &document.fields, 1);
    let title = reader.required_value("Title")?.value.clone();
    let authors = list(&reader.required_value("Authors")?.value);
    let year_field = reader.required_value("Year")?;
    let year = year_field.value.parse::<u32>().map_err(|_| {
        document.error(
            year_field.line,
            LoadErrorKind::InvalidYear(year_field.value.clone()),
        )
    })?;
    let reference = reader.required_value("Identifier")?.value.clone();
    let licence = reader.required_value("Licence")?.value.clone();
    let retrieved = reader
        .optional("Retrieved")
        .map(|field| field.value.clone());
    reader.finish()?;
    Ok(Source {
        identifier: identifier.to_string(),
        title,
        authors,
        year,
        reference,
        licence,
        retrieved,
    })
}

fn parse_dimension(text: &str, units: &mut UnitTable) -> Option<Dimension> {
    if text == DIMENSIONLESS {
        return Some(Dimension::DIMENSIONLESS);
    }
    let mut dimension = Dimension::DIMENSIONLESS;
    let mut is_divisor = false;
    let mut rest = text;
    loop {
        let end = rest.find(['*', '/']).unwrap_or(rest.len());
        let (factor, remainder) = rest.split_at(end);
        let (name, exponent) = match factor.split_once('^') {
            Some((name, exponent)) => (name, exponent.parse::<i8>().ok()?),
            None => (factor, 1),
        };
        let unit = units.lookup(name).ok()?;
        let powered = units.dimension(unit).ok()?.power(exponent).ok()?;
        let signed = if is_divisor {
            powered.power(-1).ok()?
        } else {
            powered
        };
        dimension = dimension.multiply(&signed).ok()?;
        let mut characters = remainder.chars();
        match characters.next() {
            None => return Some(dimension),
            Some(separator) => {
                is_divisor = separator == '/';
                rest = characters.as_str();
            }
        }
    }
}

fn read_object(
    identifier: &str,
    document: &Document,
    units: &mut UnitTable,
) -> Result<ObjectKind, LoadError> {
    check_title(document, identifier)?;
    FieldReader::new(document, &document.fields, 1).finish()?;
    let mut roles = Vec::new();
    for section in &document.sections {
        if section.kind != ROLE_SECTION {
            return Err(document.error(
                section.line,
                LoadErrorKind::UnknownSection(section.kind.clone()),
            ));
        }
        if !is_identifier(&section.name) {
            return Err(document.error(
                section.line,
                LoadErrorKind::InvalidIdentifier(section.name.clone()),
            ));
        }
        no_blocks(document, section, 0)?;
        let mut reader = FieldReader::new(document, &section.fields, section.line);
        let dimension_field = reader.required_value("Dimension")?;
        let dimension = parse_dimension(&dimension_field.value, units).ok_or_else(|| {
            document.error(
                dimension_field.line,
                LoadErrorKind::InvalidDimension(dimension_field.value.clone()),
            )
        })?;
        let quantity_field = reader.required_value("Quantity")?;
        let quantity = QuantityKind::from_name(&quantity_field.value).ok_or_else(|| {
            document.error(
                quantity_field.line,
                LoadErrorKind::UnknownQuantityKind(quantity_field.value.clone()),
            )
        })?;
        reader.finish()?;
        roles.push(Role {
            identifier: role_identifier(identifier, &section.name),
            dimension,
            quantity,
            names: BTreeMap::new(),
            running_names: BTreeMap::new(),
        });
    }
    Ok(ObjectKind {
        identifier: identifier.to_string(),
        names: BTreeMap::new(),
        running_names: BTreeMap::new(),
        roles,
    })
}

pub(crate) fn no_blocks(
    document: &Document,
    section: &Section,
    allowed: usize,
) -> Result<(), LoadError> {
    if section.blocks.len() > allowed {
        return Err(document.error(
            section.line,
            LoadErrorKind::UnexpectedBlockCount {
                expected: allowed,
                found: section.blocks.len(),
            },
        ));
    }
    Ok(())
}

fn apply_object_text(
    objects: &mut [ObjectKind],
    identifier: &str,
    locale: &str,
    document: &Document,
) -> Result<(), LoadError> {
    let object = objects
        .iter_mut()
        .find(|object| object.identifier == identifier)
        .ok_or_else(|| document.error(1, LoadErrorKind::UnknownObject(identifier.to_string())))?;
    let mut object_reader = FieldReader::new(document, &document.fields, 1);
    let running = object_reader.required_value("Running")?.value.clone();
    object_reader.finish()?;
    object
        .names
        .insert(locale.to_string(), document.title.clone());
    object.running_names.insert(locale.to_string(), running);
    for section in &document.sections {
        if section.kind != ROLE_SECTION {
            return Err(document.error(
                section.line,
                LoadErrorKind::UnknownSection(section.kind.clone()),
            ));
        }
        no_blocks(document, section, 0)?;
        let role_id = role_identifier(identifier, &section.name);
        let role = object
            .roles
            .iter_mut()
            .find(|role| role.identifier == role_id)
            .ok_or_else(|| {
                document.error(section.line, LoadErrorKind::UnknownRole(role_id.clone()))
            })?;
        let mut reader = FieldReader::new(document, &section.fields, section.line);
        let name = reader.required_value("Name")?.value.clone();
        let running = reader.required_value("Running")?.value.clone();
        reader.finish()?;
        role.names.insert(locale.to_string(), name);
        role.running_names.insert(locale.to_string(), running);
    }
    Ok(())
}

fn read_node(identifier: &str, document: &Document) -> Result<ConceptNode, LoadError> {
    check_title(document, identifier)?;
    let mut reader = FieldReader::new(document, &document.fields, 1);
    let level_field = reader.required_value("Level")?;
    if !is_identifier(&level_field.value) {
        return Err(document.error(
            level_field.line,
            LoadErrorKind::InvalidIdentifier(level_field.value.clone()),
        ));
    }
    let level = level_field.value.clone();
    let prerequisites = list(&reader.required("Prerequisites")?.value);
    let beginning = match reader.optional(START_KEY) {
        None => false,
        Some(field) if field.value == START_YES => true,
        Some(field) => {
            return Err(
                document.error(field.line, LoadErrorKind::InvalidStart(field.value.clone()))
            );
        }
    };
    let rests_on = reader
        .optional("Rests on")
        .map(|field| list(&field.value))
        .unwrap_or_default();
    let curriculum_references = reader
        .optional("Curriculum references")
        .map(|field| list(&field.value))
        .unwrap_or_default();
    let sources = list(&reader.required_value("Sources")?.value);
    let exercises = reader
        .optional("Exercises")
        .map(|field| list(&field.value))
        .unwrap_or_default();
    reader.finish()?;
    if let Some(invalid) = rests_on.iter().find(|id| !is_corpus_id(id)) {
        return Err(document.error(1, LoadErrorKind::InvalidCorpusId(invalid.clone())));
    }
    let mut node = ConceptNode {
        identifier: identifier.to_string(),
        level,
        prerequisites,
        beginning,
        rests_on,
        curriculum_references,
        sources,
        exercises,
        texts: BTreeMap::new(),
        ways: Vec::new(),
        bounds: Vec::new(),
        patterns: Vec::new(),
        activities: Vec::new(),
    };
    for section in &document.sections {
        if !is_identifier(&section.name) {
            return Err(document.error(
                section.line,
                LoadErrorKind::InvalidIdentifier(section.name.clone()),
            ));
        }
        match section.kind.as_str() {
            WAY_SECTION => node.ways.push(read_way(identifier, document, section)?),
            BOUND_SECTION => node.bounds.push(read_bound(identifier, document, section)?),
            PATTERN_SECTION => node
                .patterns
                .push(read_pattern(identifier, document, section)?),
            ACTIVITY_SECTION => node
                .activities
                .push(read_activity(identifier, document, section)?),
            SCENE_SECTION => {
                return Err(document.error(section.line, LoadErrorKind::UnsupportedContent));
            }
            other => {
                return Err(document.error(
                    section.line,
                    LoadErrorKind::UnknownSection(other.to_string()),
                ));
            }
        }
    }
    Ok(node)
}

fn read_way(
    concept: &str,
    document: &Document,
    section: &Section,
) -> Result<WayDefinition, LoadError> {
    let mut reader = FieldReader::new(document, &section.fields, section.line);
    let output = reader.required_value("Output")?.value.clone();
    let inputs = list(&reader.required("Inputs")?.value);
    let relation_field = reader.required_value("Relation")?;
    if !is_identifier(&relation_field.value) {
        return Err(document.error(
            relation_field.line,
            LoadErrorKind::InvalidIdentifier(relation_field.value.clone()),
        ));
    }
    let relation = relation_field.value.clone();
    let sources = list(&reader.required_value("Sources")?.value);
    let rests_on = reader
        .optional("Rests on")
        .map(|field| list(&field.value))
        .unwrap_or_default();
    let permutations = reader
        .optional("Permutations")
        .map(|field| {
            list(&field.value)
                .iter()
                .map(|group| group.split(' ').map(str::to_string).collect())
                .collect()
        })
        .unwrap_or_default();
    let assumes = reader
        .optional("Assumes")
        .map(|field| list(&field.value))
        .unwrap_or_default();
    let recognized_field = reader.required_value("Recognized")?;
    let recognized = match recognized_field.value.as_str() {
        RECOGNIZED_YES => true,
        RECOGNIZED_NO => false,
        other => {
            return Err(document.error(
                recognized_field.line,
                LoadErrorKind::InvalidRecognized(other.to_string()),
            ));
        }
    };
    reader.finish()?;
    let mut blocks = section.blocks.iter().map(|block| block.text.clone());
    let formula = blocks
        .next()
        .ok_or_else(|| document.error(section.line, LoadErrorKind::MissingFormula))?;
    Ok(WayDefinition {
        identifier: way_identifier(concept, &section.name),
        output,
        inputs,
        relation,
        sources,
        rests_on,
        permutations,
        assumes,
        formula,
        conditions: blocks.collect(),
        recognized,
    })
}

fn read_activity(
    concept: &str,
    document: &Document,
    section: &Section,
) -> Result<ActivityDefinition, LoadError> {
    no_blocks(document, section, 0)?;
    let mut reader = FieldReader::new(document, &section.fields, section.line);
    let kind_field = reader.required_value("Kind")?;
    let kind = match kind_field.value.as_str() {
        "shape-matching" => ActivityKind::ShapeMatching,
        other => {
            return Err(document.error(
                kind_field.line,
                LoadErrorKind::UnknownActivityKind(other.to_string()),
            ));
        }
    };
    let shapes_field = reader.required_value("Shapes")?;
    let mut shapes = Vec::new();
    for name in list(&shapes_field.value) {
        let shape = match name.as_str() {
            "circle" => ActivityShape::Circle,
            "square" => ActivityShape::Square,
            "triangle" => ActivityShape::Triangle,
            _ => {
                return Err(
                    document.error(shapes_field.line, LoadErrorKind::UnknownActivityShape(name))
                );
            }
        };
        if !shapes.contains(&shape) {
            shapes.push(shape);
        }
    }
    if shapes.len() < FEWEST_ACTIVITY_SHAPES {
        return Err(document.error(
            shapes_field.line,
            LoadErrorKind::TooFewActivityShapes(shapes.len()),
        ));
    }
    let variation_field = reader.required_value("Varies")?;
    let variation = match variation_field.value.as_str() {
        "nothing" => ActivityVariation::Identical,
        "orientation" => ActivityVariation::Orientation,
        "size" => ActivityVariation::Size,
        other => {
            return Err(document.error(
                variation_field.line,
                LoadErrorKind::UnknownActivityVariation(other.to_string()),
            ));
        }
    };
    let sources = list(&reader.required_value("Sources")?.value);
    reader.finish()?;
    Ok(ActivityDefinition {
        identifier: way_identifier(concept, &section.name),
        kind,
        shapes,
        variation,
        sources,
    })
}

fn read_pattern(
    concept: &str,
    document: &Document,
    section: &Section,
) -> Result<PatternDefinition, LoadError> {
    let mut reader = FieldReader::new(document, &section.fields, section.line);
    let relation = match reader.optional("Relation") {
        Some(field) if !is_identifier(&field.value) => {
            return Err(document.error(
                field.line,
                LoadErrorKind::InvalidIdentifier(field.value.clone()),
            ));
        }
        Some(field) => Some(field.value.clone()),
        None => None,
    };
    reader.finish()?;
    if section.blocks.is_empty() || section.blocks.len() > 2 {
        return Err(document.error(
            section.line,
            LoadErrorKind::UnexpectedBlockCount {
                expected: 2,
                found: section.blocks.len(),
            },
        ));
    }
    Ok(PatternDefinition {
        identifier: way_identifier(concept, &section.name),
        relation,
        body: section.blocks[0].text.clone(),
        guard: section.blocks.get(1).map(|block| block.text.clone()),
    })
}

fn read_bound(
    concept: &str,
    document: &Document,
    section: &Section,
) -> Result<BoundDefinition, LoadError> {
    let mut reader = FieldReader::new(document, &section.fields, section.line);
    let role = reader.required_value("Role")?.value.clone();
    let inputs = list(&reader.required("Inputs")?.value);
    let relation_field = reader.required_value("Relation kind")?;
    let (relation, expression_count) = match relation_field.value.as_str() {
        AT_MOST => (BoundRelation::AtMost, 1),
        AT_LEAST => (BoundRelation::AtLeast, 1),
        STRICTLY_BETWEEN => (BoundRelation::StrictlyBetween, 2),
        other => {
            return Err(document.error(
                relation_field.line,
                LoadErrorKind::UnknownRelationKind(other.to_string()),
            ));
        }
    };
    let sources = list(&reader.required_value("Sources")?.value);
    reader.finish()?;
    if section.blocks.len() < expression_count {
        return Err(document.error(
            section.line,
            LoadErrorKind::UnexpectedBlockCount {
                expected: expression_count,
                found: section.blocks.len(),
            },
        ));
    }
    let (expressions, conditions) = section.blocks.split_at(expression_count);
    Ok(BoundDefinition {
        identifier: way_identifier(concept, &section.name),
        role,
        inputs,
        relation,
        sources,
        expressions: expressions.iter().map(|block| block.text.clone()).collect(),
        conditions: conditions.iter().map(|block| block.text.clone()).collect(),
    })
}

fn read_notation(locale: &str, document: &Document) -> Result<(String, DecimalMark), LoadError> {
    if document.title != locale {
        return Err(document.error(1, LoadErrorKind::TitleDoesNotMatchPath));
    }
    no_sections(document)?;
    let mut reader = FieldReader::new(document, &document.fields, 1);
    let field = reader.required_value(DECIMAL_MARK_KEY)?;
    let mark = DecimalMark::from_name(&field.value).ok_or_else(|| {
        document.error(
            field.line,
            LoadErrorKind::UnknownDecimalMark(field.value.clone()),
        )
    })?;
    reader.finish()?;
    Ok((locale.to_string(), mark))
}

fn no_sections(document: &Document) -> Result<(), LoadError> {
    match document.sections.first() {
        Some(section) => Err(document.error(
            section.line,
            LoadErrorKind::UnknownSection(section.kind.clone()),
        )),
        None => Ok(()),
    }
}

fn apply_concept_text(
    concepts: &mut [ConceptNode],
    identifier: &str,
    locale: &str,
    document: &Document,
    mark: Option<DecimalMark>,
    pool: &mut ExprPool,
) -> Result<(), LoadError> {
    let concept = concepts
        .iter_mut()
        .find(|concept| concept.identifier == identifier)
        .ok_or_else(|| document.error(1, LoadErrorKind::UnknownConcept(identifier.to_string())))?;
    no_sections(document)?;
    let mut reader = FieldReader::new(document, &document.fields, 1);
    let mut render = |field: &Field| {
        render_learn_text(&field.value, mark, locale, pool)
            .map_err(|kind| document.error(field.line, kind))
    };
    let statement = render(reader.required_value("Statement")?)?;
    let intuition = reader.optional("Intuition").map(&mut render).transpose()?;
    let examples = reader.optional("Examples").map(&mut render).transpose()?;
    let misconception = reader
        .optional("Misconception")
        .map(&mut render)
        .transpose()?;
    reader.finish()?;
    concept.texts.insert(
        locale.to_string(),
        ConceptText {
            name: document.title.clone(),
            statement,
            intuition,
            examples,
            misconception,
        },
    );
    Ok(())
}

fn node_path(concept: &str) -> String {
    format!("{CONCEPTS_DIRECTORY}{concept}/{NODE_FILE}")
}

fn set_error(file: String, kind: LoadErrorKind) -> LoadError {
    LoadError {
        file,
        line: 0,
        kind,
    }
}

fn validate(set: &ConceptSet) -> Result<(), LoadError> {
    let retired: BTreeSet<&str> = set.retired.iter().map(String::as_str).collect();
    let concept_ids: BTreeSet<&str> = set.concepts.iter().map(|c| c.identifier.as_str()).collect();
    let source_ids: BTreeSet<&str> = set.sources.iter().map(|s| s.identifier.as_str()).collect();
    let roles: BTreeMap<&str, &Role> = set
        .objects
        .iter()
        .flat_map(|object| object.roles.iter())
        .map(|role| (role.identifier.as_str(), role))
        .collect();
    let mut identifiers: Vec<(&str, String, String)> = Vec::new();
    identifiers.extend(set.sources.iter().map(|s| {
        (
            "source",
            s.identifier.clone(),
            format!("{SOURCES_DIRECTORY}{}{MARKDOWN_EXTENSION}", s.identifier),
        )
    }));
    identifiers.extend(set.objects.iter().map(|o| {
        (
            "object",
            o.identifier.clone(),
            format!("{OBJECTS_DIRECTORY}{}{MARKDOWN_EXTENSION}", o.identifier),
        )
    }));
    for concept in &set.concepts {
        identifiers.push((
            "concept",
            concept.identifier.clone(),
            node_path(&concept.identifier),
        ));
        identifiers.extend(concept.ways.iter().map(|way| {
            (
                "rule",
                way.identifier.clone(),
                node_path(&concept.identifier),
            )
        }));
        identifiers.extend(concept.bounds.iter().map(|bound| {
            (
                "rule",
                bound.identifier.clone(),
                node_path(&concept.identifier),
            )
        }));
    }
    let mut seen = BTreeSet::new();
    for (kind, identifier, file) in &identifiers {
        if retired.contains(identifier.as_str()) {
            return Err(set_error(
                file.clone(),
                LoadErrorKind::RetiredIdentifier(identifier.clone()),
            ));
        }
        if !seen.insert((*kind, identifier.clone())) {
            return Err(set_error(
                file.clone(),
                LoadErrorKind::DuplicateIdentifier(identifier.clone()),
            ));
        }
    }
    for object in &set.objects {
        if !object.names.contains_key(ENGLISH) {
            return Err(set_error(
                format!(
                    "{OBJECTS_DIRECTORY}{}{MARKDOWN_EXTENSION}",
                    object.identifier
                ),
                LoadErrorKind::MissingEnglishText(object.identifier.clone()),
            ));
        }
    }
    let check_sources = |file: &str, sources: &[String]| -> Result<(), LoadError> {
        match sources
            .iter()
            .find(|source| !source_ids.contains(source.as_str()))
        {
            Some(unknown) => Err(set_error(
                file.to_string(),
                LoadErrorKind::UnknownSource(unknown.clone()),
            )),
            None => Ok(()),
        }
    };
    let mut pool = ExprPool::new();
    for concept in &set.concepts {
        let file = node_path(&concept.identifier);
        if !concept.texts.contains_key(ENGLISH) {
            return Err(set_error(
                file,
                LoadErrorKind::MissingEnglishText(concept.identifier.clone()),
            ));
        }
        if let Some(unknown) = concept
            .prerequisites
            .iter()
            .find(|p| !concept_ids.contains(p.as_str()))
        {
            return Err(set_error(
                file,
                LoadErrorKind::UnknownConcept(unknown.clone()),
            ));
        }
        check_sources(&file, &concept.sources)?;
        for activity in &concept.activities {
            check_sources(&file, &activity.sources)?;
        }
        for way in &concept.ways {
            check_sources(&file, &way.sources)?;
            check_roles(&file, &roles, &way.output, &way.inputs)?;
            if let Some(unknown) = way
                .assumes
                .iter()
                .find(|a| !concept_ids.contains(a.as_str()))
            {
                return Err(set_error(
                    file,
                    LoadErrorKind::UnknownConcept(unknown.clone()),
                ));
            }
            if let Some(invalid) = way.rests_on.iter().find(|id| !is_corpus_id(id)) {
                return Err(set_error(
                    file,
                    LoadErrorKind::InvalidCorpusId(invalid.clone()),
                ));
            }
            check_function(&file, &mut pool, &way.formula, way.inputs.len())?;
            for condition in &way.conditions {
                check_function(&file, &mut pool, condition, way.inputs.len())?;
            }
            let object = role_object(&way.output).unwrap_or_default();
            for group in &way.permutations {
                for name in group {
                    let role = role_identifier(object, name);
                    if !roles.contains_key(role.as_str()) {
                        return Err(set_error(file, LoadErrorKind::UnknownRole(role)));
                    }
                }
            }
        }
        for bound in &concept.bounds {
            check_sources(&file, &bound.sources)?;
            check_roles(&file, &roles, &bound.role, &bound.inputs)?;
            for text in bound.expressions.iter().chain(&bound.conditions) {
                check_function(&file, &mut pool, text, bound.inputs.len())?;
            }
        }
    }
    validate_patterns(set)?;
    check_beginnings(set)?;
    check_prerequisites_acyclic(set)
}

fn check_roles(
    file: &str,
    roles: &BTreeMap<&str, &Role>,
    output: &str,
    inputs: &[String],
) -> Result<(), LoadError> {
    let object = role_object(output);
    for role in std::iter::once(output).chain(inputs.iter().map(String::as_str)) {
        if !is_role_identifier(role) || !roles.contains_key(role) {
            return Err(set_error(
                file.to_string(),
                LoadErrorKind::UnknownRole(role.to_string()),
            ));
        }
        if role_object(role) != object {
            return Err(set_error(
                file.to_string(),
                LoadErrorKind::RoleOfAnotherObject(role.to_string()),
            ));
        }
    }
    Ok(())
}

pub(crate) fn lambda_parameter_count(pool: &ExprPool, expression: ExprId) -> usize {
    let mut count = 0;
    let mut current = expression;
    while let Ok(NodeView::Bind {
        binder: BinderKind::Lambda,
        body,
        ..
    }) = pool.node(current)
    {
        count += 1;
        current = body;
    }
    count
}

pub(crate) fn parse_function(
    file: &str,
    pool: &mut ExprPool,
    text: &str,
    expected: usize,
) -> Result<ExprId, LoadError> {
    let expression = calc_syntax::parse_expression(pool, text)
        .map_err(|error| set_error(file.to_string(), LoadErrorKind::Formula(error.kind)))?;
    let found = lambda_parameter_count(pool, expression);
    if found == 0 && expected > 0 {
        return Err(set_error(
            file.to_string(),
            LoadErrorKind::FormulaNotAFunction,
        ));
    }
    if found != expected {
        return Err(set_error(
            file.to_string(),
            LoadErrorKind::ParameterCountMismatch { expected, found },
        ));
    }
    Ok(expression)
}

fn check_function(
    file: &str,
    pool: &mut ExprPool,
    text: &str,
    expected: usize,
) -> Result<(), LoadError> {
    parse_function(file, pool, text, expected).map(|_| ())
}

fn check_beginnings(set: &ConceptSet) -> Result<(), LoadError> {
    for concept in &set.concepts {
        let kind = match (concept.beginning, concept.prerequisites.is_empty()) {
            (false, true) => LoadErrorKind::UndeclaredBeginning(concept.identifier.clone()),
            (true, false) => LoadErrorKind::BeginningWithPrerequisites(concept.identifier.clone()),
            _ => continue,
        };
        return Err(set_error(node_path(&concept.identifier), kind));
    }
    Ok(())
}

fn check_prerequisites_acyclic(set: &ConceptSet) -> Result<(), LoadError> {
    let index: BTreeMap<&str, usize> = set
        .concepts
        .iter()
        .enumerate()
        .map(|(position, concept)| (concept.identifier.as_str(), position))
        .collect();
    let arrows: Vec<Arrow> = set
        .concepts
        .iter()
        .enumerate()
        .flat_map(|(target, concept)| {
            concept
                .prerequisites
                .iter()
                .filter_map(|prerequisite| index.get(prerequisite.as_str()))
                .map(move |source| Arrow {
                    source: NodeId(*source),
                    target: NodeId(target),
                })
                .collect::<Vec<_>>()
        })
        .collect();
    if set.concepts.is_empty() {
        return Ok(());
    }
    let Ok(graph) = DirectedGraph::new(set.concepts.len(), &arrows) else {
        return Ok(());
    };
    match longest_path_layering(&graph) {
        Ok(_) => Ok(()),
        Err(LayeringError::Cycle(node)) => {
            let concept = set
                .concepts
                .get(node.0)
                .map(|concept| concept.identifier.clone())
                .unwrap_or_default();
            Err(set_error(
                node_path(&concept),
                LoadErrorKind::PrerequisiteCycle(concept),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "# book\n\nTitle: A Book\nAuthors: Ann Author\nYear: 2020\nIdentifier: https://example.org/book\nLicence: CC-BY-4.0\n";
    const OBJECT: &str = "# circle\n\n## Role radius\nDimension: m\nQuantity: length\n\n## Role diameter\nDimension: m\nQuantity: length\n\n## Role area\nDimension: m^2\nQuantity: area\n";
    const OBJECT_TEXT: &str = "# Circle\nRunning: circle\n\n## Role radius\nName: Radius\nRunning: radius\n\n## Role diameter\nName: Diameter\nRunning: diameter\n\n## Role area\nName: Area\nRunning: area\n";
    const BASE_NODE: &str = "# circle\n\nLevel: isced-1\nPrerequisites:\nStart: yes\nRests on: M-GEO-D-003\nSources: book\n";
    const BASE_TEXT: &str = "# Circle\n\nStatement: A circle is a set of points.\n";
    const DIAMETER_NODE: &str = "# circle-diameter\n\nLevel: isced-2\nPrerequisites: circle\nSources: book\n\n## Way from-radius\nOutput: circle.diameter\nInputs: circle.radius\nRelation: circle-diameter-radius\nSources: book\nRecognized: yes\n```calc\nr |-> 2 * r\n```\n```calc\nr |-> r > 0\n```\n";
    const DIAMETER_TEXT: &str = "# Diameter\n\nStatement: The diameter is twice the radius.\n";

    fn files<'a>(extra: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a [u8])> {
        let mut base: Vec<(&str, &str)> = vec![
            ("sources/book.md", SOURCE),
            ("objects/circle.md", OBJECT),
            ("objects/circle.en.md", OBJECT_TEXT),
            ("concepts/circle/node.md", BASE_NODE),
            ("concepts/circle/en.md", BASE_TEXT),
            ("concepts/circle-diameter/node.md", DIAMETER_NODE),
            ("concepts/circle-diameter/en.md", DIAMETER_TEXT),
        ];
        for (path, text) in extra {
            base.retain(|(existing, _)| existing != path);
            base.push((path, text));
        }
        base.into_iter()
            .map(|(path, text)| (path, text.as_bytes()))
            .collect()
    }

    fn load_with(extra: &[(&str, &str)]) -> Result<ConceptSet, LoadError> {
        load(&files(extra))
    }

    fn kind_with(extra: &[(&str, &str)]) -> LoadErrorKind {
        load_with(extra).unwrap_err().kind
    }

    #[test]
    fn valid_files_load_concepts_objects_and_sources() {
        let set = load_with(&[]).unwrap();
        assert_eq!(
            (set.concepts.len(), set.objects.len(), set.sources.len()),
            (2, 1, 1)
        );
    }

    #[test]
    fn way_keeps_its_fields_and_blocks() {
        let set = load_with(&[]).unwrap();
        let way = &set.concepts[1].ways[0];
        assert_eq!(way.identifier, "circle-diameter/from-radius");
        assert_eq!(way.inputs, vec!["circle.radius".to_string()]);
        assert_eq!(way.formula, "r |-> 2 * r");
        assert_eq!(way.conditions, vec!["r |-> r > 0".to_string()]);
    }

    #[test]
    fn role_keeps_dimension_quantity_and_localized_name() {
        let set = load_with(&[]).unwrap();
        let area = &set.objects[0].roles[2];
        assert_eq!(area.identifier, "circle.area");
        assert_eq!(area.quantity, QuantityKind::Area);
        assert_eq!(area.dimension.exponents(), [2, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(area.names.get("en").map(String::as_str), Some("Area"));
    }

    #[test]
    fn object_kind_keeps_its_running_name_per_locale() {
        let set = load_with(&[]).unwrap();
        assert_eq!(
            set.objects[0].running_names.get("en").map(String::as_str),
            Some("circle")
        );
    }

    #[test]
    fn role_keeps_its_running_name_beside_its_heading_name() {
        let set = load_with(&[]).unwrap();
        let radius = &set.objects[0].roles[0];
        assert_eq!(
            (
                radius.names.get("en").map(String::as_str),
                radius.running_names.get("en").map(String::as_str)
            ),
            (Some("Radius"), Some("radius"))
        );
    }

    #[test]
    fn object_kind_text_without_a_running_name_is_rejected() {
        let text = OBJECT_TEXT.replace("Running: circle\n", "");
        assert_eq!(
            kind_with(&[("objects/circle.en.md", &text)]),
            LoadErrorKind::MissingKey("Running".to_string())
        );
    }

    #[test]
    fn role_without_a_running_name_is_rejected() {
        let text = OBJECT_TEXT.replace("Running: area\n", "");
        assert_eq!(
            kind_with(&[("objects/circle.en.md", &text)]),
            LoadErrorKind::MissingKey("Running".to_string())
        );
    }

    #[test]
    fn concept_text_is_stored_per_locale() {
        let set = load_with(&[(
            "concepts/circle/de.md",
            "# Kreis\n\nStatement: Ein Kreis.\n",
        )])
        .unwrap();
        assert_eq!(
            set.concepts[0]
                .texts
                .get("de")
                .map(|text| text.name.as_str()),
            Some("Kreis")
        );
    }

    #[test]
    fn empty_rests_on_is_allowed() {
        let set = load_with(&[]).unwrap();
        assert!(set.concepts[1].rests_on.is_empty());
    }

    #[test]
    fn unknown_path_is_rejected() {
        assert_eq!(
            kind_with(&[("notes/todo.md", "# todo\n")]),
            LoadErrorKind::UnexpectedPath
        );
    }

    #[test]
    fn file_under_a_curriculum_that_is_not_a_curriculum_file_is_rejected() {
        assert_eq!(
            kind_with(&[("curricula/de-by/notes.txt", "# de-by\n")]),
            LoadErrorKind::UnexpectedPath
        );
    }

    #[test]
    fn title_must_match_the_path() {
        assert_eq!(
            kind_with(&[("sources/book.md", &SOURCE.replace("# book", "# volume"))]),
            LoadErrorKind::TitleDoesNotMatchPath
        );
    }

    #[test]
    fn unknown_key_is_rejected() {
        let node = format!("{BASE_NODE}Colour: red\n");
        assert_eq!(
            kind_with(&[("concepts/circle/node.md", &node)]),
            LoadErrorKind::UnknownKey("Colour".to_string())
        );
    }

    #[test]
    fn missing_required_key_is_rejected() {
        let node = BASE_NODE.replace("Level: isced-1\n", "");
        assert_eq!(
            kind_with(&[("concepts/circle/node.md", &node)]),
            LoadErrorKind::MissingKey("Level".to_string())
        );
    }

    #[test]
    fn unknown_prerequisite_is_rejected() {
        let node = DIAMETER_NODE.replace("Prerequisites: circle", "Prerequisites: sphere");
        assert_eq!(
            kind_with(&[("concepts/circle-diameter/node.md", &node)]),
            LoadErrorKind::UnknownConcept("sphere".to_string())
        );
    }

    #[test]
    fn unknown_source_is_rejected() {
        let node = BASE_NODE.replace("Sources: book", "Sources: pamphlet");
        assert_eq!(
            kind_with(&[("concepts/circle/node.md", &node)]),
            LoadErrorKind::UnknownSource("pamphlet".to_string())
        );
    }

    #[test]
    fn unknown_role_is_rejected() {
        let node = DIAMETER_NODE.replace("Output: circle.diameter", "Output: circle.colour");
        assert_eq!(
            kind_with(&[("concepts/circle-diameter/node.md", &node)]),
            LoadErrorKind::UnknownRole("circle.colour".to_string())
        );
    }

    #[test]
    fn inputs_from_another_object_are_rejected() {
        let square = "# square\n\n## Role side\nDimension: m\nQuantity: length\n";
        let square_text = "# Square\nRunning: square\n\n## Role side\nName: Side\nRunning: side\n";
        let node = DIAMETER_NODE.replace("Inputs: circle.radius", "Inputs: square.side");
        assert_eq!(
            kind_with(&[
                ("objects/square.md", square),
                ("objects/square.en.md", square_text),
                ("concepts/circle-diameter/node.md", &node)
            ]),
            LoadErrorKind::RoleOfAnotherObject("square.side".to_string())
        );
    }

    #[test]
    fn unknown_quantity_kind_is_rejected() {
        let object = OBJECT.replacen("Quantity: length", "Quantity: colour", 1);
        assert_eq!(
            kind_with(&[("objects/circle.md", &object)]),
            LoadErrorKind::UnknownQuantityKind("colour".to_string())
        );
    }

    #[test]
    fn unknown_unit_in_a_dimension_is_rejected() {
        let object = OBJECT.replacen("Dimension: m\n", "Dimension: furlong\n", 1);
        assert_eq!(
            kind_with(&[("objects/circle.md", &object)]),
            LoadErrorKind::InvalidDimension("furlong".to_string())
        );
    }

    #[test]
    fn formula_with_wrong_parameter_count_is_rejected() {
        let node = DIAMETER_NODE.replace("r |-> 2 * r", "r |-> s |-> 2 * r");
        assert_eq!(
            kind_with(&[("concepts/circle-diameter/node.md", &node)]),
            LoadErrorKind::ParameterCountMismatch {
                expected: 1,
                found: 2
            }
        );
    }

    #[test]
    fn formula_that_does_not_parse_is_rejected() {
        let node = DIAMETER_NODE.replace("r |-> 2 * r", "r |-> 2 *");
        assert!(matches!(
            kind_with(&[("concepts/circle-diameter/node.md", &node)]),
            LoadErrorKind::Formula(_)
        ));
    }

    #[test]
    fn way_without_a_formula_is_rejected() {
        let node = DIAMETER_NODE.split("```calc").next().unwrap().to_string();
        assert_eq!(
            kind_with(&[("concepts/circle-diameter/node.md", &node)]),
            LoadErrorKind::MissingFormula
        );
    }

    #[test]
    fn concept_without_english_text_is_rejected() {
        let mut set = files(&[]);
        set.retain(|(path, _)| *path != "concepts/circle-diameter/en.md");
        assert_eq!(
            load(&set).unwrap_err().kind,
            LoadErrorKind::MissingEnglishText("circle-diameter".to_string())
        );
    }

    #[test]
    fn retired_identifier_cannot_appear_again() {
        assert_eq!(
            kind_with(&[("retired.md", "# retired\n\ncircle-diameter/from-radius\n")]),
            LoadErrorKind::RetiredIdentifier("circle-diameter/from-radius".to_string())
        );
    }

    #[test]
    fn prerequisite_cycle_is_rejected() {
        let node = BASE_NODE.replace(
            "Prerequisites:\nStart: yes",
            "Prerequisites: circle-diameter",
        );
        assert!(matches!(
            kind_with(&[("concepts/circle/node.md", &node)]),
            LoadErrorKind::PrerequisiteCycle(_)
        ));
    }

    #[test]
    fn invalid_corpus_id_is_rejected() {
        let node = BASE_NODE.replace("M-GEO-D-003", "GEO-3");
        assert_eq!(
            kind_with(&[("concepts/circle/node.md", &node)]),
            LoadErrorKind::InvalidCorpusId("GEO-3".to_string())
        );
    }

    #[test]
    fn way_without_recognized_is_rejected() {
        let node = DIAMETER_NODE.replace("Recognized: yes\n", "");
        assert_eq!(
            kind_with(&[("concepts/circle-diameter/node.md", &node)]),
            LoadErrorKind::MissingKey("Recognized".to_string())
        );
    }

    #[test]
    fn recognized_other_than_yes_or_no_is_rejected() {
        let node = DIAMETER_NODE.replace("Recognized: yes", "Recognized: maybe");
        assert_eq!(
            kind_with(&[("concepts/circle-diameter/node.md", &node)]),
            LoadErrorKind::InvalidRecognized("maybe".to_string())
        );
    }

    #[test]
    fn pattern_section_keeps_its_relation_body_and_guard() {
        let node = format!(
            "{DIAMETER_NODE}\n## Pattern halved\nRelation: circle-diameter-radius\n```calc\nr |-> r + r\n```\n```calc\nr |-> r > 0\n```\n"
        );
        let set = load_with(&[("concepts/circle-diameter/node.md", &node)]).unwrap();
        let pattern = &set.concepts[1].patterns[0];
        assert_eq!(
            (
                pattern.identifier.as_str(),
                pattern.relation.as_deref(),
                pattern.body.as_str(),
                pattern.guard.as_deref()
            ),
            (
                "circle-diameter/halved",
                Some("circle-diameter-radius"),
                "r |-> r + r",
                Some("r |-> r > 0")
            )
        );
    }

    #[test]
    fn pattern_section_with_three_blocks_is_rejected() {
        let node = format!(
            "{BASE_NODE}\n## Pattern squares\n```calc\nx |-> x^2\n```\n```calc\nx |-> x > 0\n```\n```calc\nx |-> x < 1\n```\n"
        );
        assert_eq!(
            kind_with(&[("concepts/circle/node.md", &node)]),
            LoadErrorKind::UnexpectedBlockCount {
                expected: 2,
                found: 3
            }
        );
    }

    #[test]
    fn bound_with_too_few_expressions_is_rejected() {
        let node = format!(
            "{BASE_NODE}\n## Bound range\nRole: circle.area\nInputs: circle.radius\nRelation kind: strictly-between\nSources: book\n```calc\nr |-> 0\n```\n"
        );
        assert_eq!(
            kind_with(&[("concepts/circle/node.md", &node)]),
            LoadErrorKind::UnexpectedBlockCount {
                expected: 2,
                found: 1
            }
        );
    }

    const METRIC: &str =
        "# metric\n\n## Unit speed\nDisplayed: km/h\n\n## Unit time\nDisplayed: h min\n";

    #[test]
    fn unit_system_loads_its_declarations_and_name() {
        let set = load_with(&[
            ("unit-systems/metric.md", METRIC),
            ("unit-systems/metric.en.md", "# Metric\n"),
        ])
        .unwrap();
        let system = &set.unit_systems[0];
        assert_eq!(
            (
                system.units.len(),
                system.names.get("en").map(String::as_str)
            ),
            (2, Some("Metric"))
        );
    }

    #[test]
    fn unit_system_with_posed_is_rejected() {
        let system = METRIC.replace("Displayed: km/h", "Posed: km/h\nDisplayed: km/h");
        assert_eq!(
            kind_with(&[
                ("unit-systems/metric.md", &system),
                ("unit-systems/metric.en.md", "# Metric\n")
            ]),
            LoadErrorKind::UnknownKey("Posed".to_string())
        );
    }

    #[test]
    fn unit_system_without_english_name_is_rejected() {
        assert_eq!(
            kind_with(&[("unit-systems/metric.md", METRIC)]),
            LoadErrorKind::MissingEnglishText("metric".to_string())
        );
    }

    #[test]
    fn unit_system_name_without_its_system_is_rejected() {
        assert_eq!(
            kind_with(&[("unit-systems/imperial.en.md", "# Imperial\n")]),
            LoadErrorKind::UnknownUnitSystem("imperial".to_string())
        );
    }

    #[test]
    fn unit_system_with_an_invalid_unit_is_rejected() {
        let system = METRIC.replace("Displayed: km/h", "Displayed: furlong/h");
        assert_eq!(
            kind_with(&[
                ("unit-systems/metric.md", &system),
                ("unit-systems/metric.en.md", "# Metric\n")
            ]),
            LoadErrorKind::InvalidDisplayUnit("furlong/h".to_string())
        );
    }

    #[test]
    fn unit_system_with_a_staged_section_is_rejected() {
        let system = METRIC.replace("## Unit speed", "## Unit speed grade-5");
        assert_eq!(
            kind_with(&[
                ("unit-systems/metric.md", &system),
                ("unit-systems/metric.en.md", "# Metric\n")
            ]),
            LoadErrorKind::UnitStageOutsideCurriculum("grade-5".to_string())
        );
    }

    #[test]
    fn error_names_the_file_and_line() {
        let node = format!("{BASE_NODE}\nstray text\n");
        let error = load_with(&[("concepts/circle/node.md", &node)]).unwrap_err();
        assert_eq!(
            (error.file.as_str(), error.line),
            ("concepts/circle/node.md", 9)
        );
    }

    fn node_with_activity(activity: &str) -> String {
        format!("{BASE_NODE}\n## Activity first\n{activity}")
    }

    const ACTIVITY: &str =
        "Kind: shape-matching\nShapes: circle, square\nVaries: nothing\nSources: book\n";

    #[test]
    fn an_activity_keeps_its_kind_shapes_variation_and_sources() {
        let node = node_with_activity(ACTIVITY);
        let set = load_with(&[("concepts/circle/node.md", &node)]).unwrap();
        assert_eq!(
            set.concepts[0].activities,
            vec![ActivityDefinition {
                identifier: "circle/first".to_string(),
                kind: ActivityKind::ShapeMatching,
                shapes: vec![ActivityShape::Circle, ActivityShape::Square],
                variation: ActivityVariation::Identical,
                sources: vec!["book".to_string()],
            }]
        );
    }

    #[test]
    fn an_activity_of_an_unknown_kind_is_rejected() {
        let node = node_with_activity(&ACTIVITY.replace("shape-matching", "puzzle"));
        assert_eq!(
            kind_with(&[("concepts/circle/node.md", &node)]),
            LoadErrorKind::UnknownActivityKind("puzzle".to_string())
        );
    }

    #[test]
    fn an_activity_shape_outside_the_list_is_rejected() {
        let node = node_with_activity(&ACTIVITY.replace("square", "hexagon"));
        assert_eq!(
            kind_with(&[("concepts/circle/node.md", &node)]),
            LoadErrorKind::UnknownActivityShape("hexagon".to_string())
        );
    }

    #[test]
    fn a_matching_with_one_shape_is_rejected() {
        let node = node_with_activity(&ACTIVITY.replace("circle, square", "circle, circle"));
        assert_eq!(
            kind_with(&[("concepts/circle/node.md", &node)]),
            LoadErrorKind::TooFewActivityShapes(1)
        );
    }

    #[test]
    fn an_activity_variation_outside_the_list_is_rejected() {
        let node = node_with_activity(&ACTIVITY.replace("nothing", "colour"));
        assert_eq!(
            kind_with(&[("concepts/circle/node.md", &node)]),
            LoadErrorKind::UnknownActivityVariation("colour".to_string())
        );
    }

    #[test]
    fn an_activity_source_must_exist() {
        let node = node_with_activity(&ACTIVITY.replace("Sources: book", "Sources: nowhere"));
        assert_eq!(
            kind_with(&[("concepts/circle/node.md", &node)]),
            LoadErrorKind::UnknownSource("nowhere".to_string())
        );
    }

    #[test]
    fn a_concept_without_prerequisites_must_be_a_declared_beginning() {
        let node = BASE_NODE.replace("Start: yes\n", "");
        assert_eq!(
            kind_with(&[("concepts/circle/node.md", &node)]),
            LoadErrorKind::UndeclaredBeginning("circle".to_string())
        );
    }

    #[test]
    fn a_declared_beginning_has_no_prerequisites() {
        let node = BASE_NODE.replace("Prerequisites:", "Prerequisites: circle-diameter");
        assert_eq!(
            kind_with(&[("concepts/circle/node.md", &node)]),
            LoadErrorKind::BeginningWithPrerequisites("circle".to_string())
        );
    }

    #[test]
    fn start_takes_only_yes() {
        let node = BASE_NODE.replace("Start: yes", "Start: maybe");
        assert_eq!(
            kind_with(&[("concepts/circle/node.md", &node)]),
            LoadErrorKind::InvalidStart("maybe".to_string())
        );
    }
}
