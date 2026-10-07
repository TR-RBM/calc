use std::collections::BTreeMap;

use calc_concepts::QuantityKind;
use calc_expr::{ExprPool, NodeView};
use calc_i18n::Message;
use calc_syntax::parse_expression;
use calc_units::{Dimension, TemperatureScale, UnitId};

use crate::application_place::check_kind_unit;
use crate::readable::{
    ReadableValue, pick_unit, readable_pick, sorted_by_scale, written_candidates,
};

const OVERRIDE_SEPARATOR: char = '=';
const LIST_SEPARATOR: char = ',';

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisplayUnit {
    Unit(UnitId),
    Compound(Vec<UnitId>),
    Scale(TemperatureScale),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitsChoice {
    pub system: Option<String>,
    pub overrides: BTreeMap<QuantityKind, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnitsChoiceError {
    MissingSeparator { position: usize },
    UnknownKind { position: usize },
    RepeatedKind { position: usize },
    EmptyUnit { position: usize },
}

impl UnitsChoice {
    pub fn from_settings(
        system: Option<&str>,
        overrides: &[&str],
    ) -> Result<UnitsChoice, UnitsChoiceError> {
        let mut choice = UnitsChoice {
            system: system.map(str::to_string),
            overrides: BTreeMap::new(),
        };
        for (position, entry) in overrides.iter().enumerate() {
            let (kind, unit) = entry
                .split_once(OVERRIDE_SEPARATOR)
                .ok_or(UnitsChoiceError::MissingSeparator { position })?;
            let kind = QuantityKind::from_name(kind.trim())
                .ok_or(UnitsChoiceError::UnknownKind { position })?;
            let unit = unit.trim();
            if unit.is_empty() {
                return Err(UnitsChoiceError::EmptyUnit { position });
            }
            if choice.overrides.insert(kind, unit.to_string()).is_some() {
                return Err(UnitsChoiceError::RepeatedKind { position });
            }
        }
        Ok(choice)
    }

    pub fn unresolved_overrides(&self, pool: &mut ExprPool) -> Vec<QuantityKind> {
        self.overrides
            .iter()
            .filter(|(kind, text)| check_kind_unit(pool, **kind, text).is_err())
            .map(|(kind, _)| *kind)
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitSystem {
    pub identifier: String,
    pub displayed: BTreeMap<QuantityKind, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CurriculumUnits {
    pub identifier: String,
    pub posed: BTreeMap<QuantityKind, String>,
    pub displayed: BTreeMap<QuantityKind, String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Calculate,
    Explore,
    Learn,
    Train,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValuePlace {
    Given,
    Derived,
    TaskContent,
    AroundTask,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayQuestion {
    pub area: Area,
    pub place: ValuePlace,
    pub dimension: Dimension,
    pub stored: Option<UnitId>,
    pub kind: Option<QuantityKind>,
    pub written: Option<DisplayUnit>,
    pub stated: Option<DisplayUnit>,
    pub readable: Option<ReadableValue>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DisplaySources<'sources> {
    pub curriculum: Option<&'sources CurriculumUnits>,
    pub session_override: Option<&'sources UnitsChoice>,
    pub preference: Option<&'sources UnitsChoice>,
    pub systems: &'sources [UnitSystem],
    pub coherent_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecidedBy {
    Written,
    Stated,
    CurriculumPosed,
    CurriculumDisplayed,
    SessionOverride,
    Preference,
    TaskContent,
    CoherentOnly,
    Coherent,
    AsComputed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayDecision {
    pub unit: Option<DisplayUnit>,
    pub decided_by: DecidedBy,
    pub kind: Option<QuantityKind>,
    pub curriculum: Option<String>,
    pub picked: bool,
}

#[derive(Clone, Copy)]
enum Step<'sources> {
    Declared(&'sources BTreeMap<QuantityKind, String>, DecidedBy),
    Choice(&'sources UnitsChoice, DecidedBy),
}

fn allows_compound(kind: QuantityKind) -> bool {
    matches!(
        kind,
        QuantityKind::Time | QuantityKind::Length | QuantityKind::Mass
    )
}

fn single_unit(pool: &mut ExprPool, text: &str) -> Option<UnitId> {
    let expression = parse_expression(pool, &format!("1 {text}")).ok()?;
    match pool.node(expression).ok()? {
        NodeView::Quantity { unit, .. } => Some(unit),
        _ => None,
    }
}

fn is_larger(pool: &ExprPool, larger: UnitId, smaller: UnitId) -> bool {
    pool.units()
        .conversion_factor(larger, smaller)
        .is_ok_and(|factor| {
            let difference = factor.numerator() - factor.denominator();
            factor.pi_exponent() == 0 && !difference.is_negative() && !difference.is_zero()
        })
}

pub fn unit_source_message(decided_by: DecidedBy, curriculum: Option<&str>) -> Message {
    match (decided_by, curriculum) {
        (DecidedBy::Written | DecidedBy::Stated | DecidedBy::TaskContent, _) => {
            Message::CommonUnitSourceWritten
        }
        (DecidedBy::SessionOverride, _) => Message::CommonUnitSourceThisSession,
        (DecidedBy::Preference, _) => Message::CommonUnitSourcePreference,
        (DecidedBy::CurriculumPosed | DecidedBy::CurriculumDisplayed, Some(name)) => {
            Message::CommonUnitSourceCurriculum {
                name: name.to_owned(),
            }
        }
        (
            DecidedBy::CurriculumPosed
            | DecidedBy::CurriculumDisplayed
            | DecidedBy::CoherentOnly
            | DecidedBy::Coherent,
            _,
        ) => Message::CommonUnitSourceStoredUnit,
        (DecidedBy::AsComputed, _) => Message::CommonUnitSourceAsComputed,
    }
}

pub fn resolve_display_unit(
    pool: &mut ExprPool,
    text: &str,
    kind: Option<QuantityKind>,
) -> Option<DisplayUnit> {
    if let Some(scale) = TemperatureScale::from_name(text) {
        return Some(DisplayUnit::Scale(scale));
    }
    let parts: Vec<&str> = text.split_whitespace().collect();
    let [single] = parts.as_slice() else {
        if !kind.is_some_and(allows_compound) {
            return None;
        }
        let units = parts
            .iter()
            .map(|part| single_unit(pool, part))
            .collect::<Option<Vec<UnitId>>>()?;
        let descends = units
            .windows(2)
            .all(|pair| is_larger(pool, pair[0], pair[1]));
        return descends.then_some(DisplayUnit::Compound(units));
    };
    single_unit(pool, single).map(DisplayUnit::Unit)
}

pub fn units_of_the_dimension_of(text: &str) -> Vec<String> {
    let mut pool = ExprPool::new();
    let Some(unit) = resolve_display_unit(&mut pool, text, None) else {
        return Vec::new();
    };
    let Some(dimension) = display_dimension(&pool, &unit) else {
        return Vec::new();
    };
    let mut symbols: Vec<String> = Vec::new();
    for named in pool.units().named_units() {
        let Ok(symbol) = pool.units().symbol(named) else {
            continue;
        };
        let symbol = symbol.to_owned();
        let Ok(unit) = pool.units_mut().lookup(&symbol) else {
            continue;
        };
        if pool.units().dimension(unit).ok() == Some(dimension) {
            symbols.push(symbol);
        }
    }
    symbols.sort();
    symbols.dedup();
    let mut units = declared_units_of(&dimension);
    for symbol in symbols {
        if !units.contains(&symbol) {
            units.push(symbol);
        }
    }
    units
}

fn declared_units_of(dimension: &Dimension) -> Vec<String> {
    let Ok(concepts) = crate::solve::concept_set() else {
        return Vec::new();
    };
    let mut declared: Vec<String> = Vec::new();
    for system in &concepts.unit_systems {
        for unit in &system.units {
            if unit.kind.dimension() != *dimension {
                continue;
            }
            for shown in &unit.displayed {
                let calc_concepts::DisplayUnit::Units(parts) = shown else {
                    continue;
                };
                let text = parts.join(" ");
                if !text.is_empty() && !declared.contains(&text) {
                    declared.push(text);
                }
            }
        }
    }
    declared
}

pub fn display_dimension(pool: &ExprPool, unit: &DisplayUnit) -> Option<Dimension> {
    match unit {
        DisplayUnit::Unit(unit) => pool.units().dimension(*unit).ok(),
        DisplayUnit::Compound(units) => {
            let first = pool.units().dimension(*units.first()?).ok()?;
            units
                .iter()
                .all(|unit| pool.units().dimension(*unit).ok() == Some(first))
                .then_some(first)
        }
        DisplayUnit::Scale(_) => Some(Dimension::of_base(
            calc_units::BaseDimension::ThermodynamicTemperature,
        )),
    }
}

pub fn is_named_dimensionless_unit(pool: &ExprPool, unit: UnitId) -> bool {
    pool.units()
        .dimension(unit)
        .is_ok_and(|dimension| dimension.is_dimensionless())
        && pool
            .units()
            .factors(unit)
            .is_ok_and(|factors| matches!(factors, [single] if single.exponent() == 1))
}

fn takes_a_display_unit(question: &DisplayQuestion) -> bool {
    !question.dimension.is_dimensionless() || question.stored.is_some()
}

fn applicable(
    pool: &mut ExprPool,
    text: &str,
    kind: QuantityKind,
    dimension: Dimension,
) -> Option<DisplayUnit> {
    check_kind_unit(pool, kind, text).ok()?;
    let unit = resolve_display_unit(pool, text, Some(kind))?;
    (display_dimension(pool, &unit) == Some(dimension)).then_some(unit)
}

fn step_texts<'sources>(
    step: Step<'sources>,
    systems: &'sources [UnitSystem],
) -> Vec<(QuantityKind, &'sources str)> {
    match step {
        Step::Declared(declared, _) => declared
            .iter()
            .map(|(kind, text)| (*kind, text.as_str()))
            .collect(),
        Step::Choice(choice, _) => {
            let system = choice.system.as_deref().and_then(|identifier| {
                systems
                    .iter()
                    .find(|system| system.identifier == identifier)
            });
            let mut texts: BTreeMap<QuantityKind, &str> = system
                .map(|system| {
                    system
                        .displayed
                        .iter()
                        .map(|(kind, text)| (*kind, text.as_str()))
                        .collect()
                })
                .unwrap_or_default();
            for (kind, text) in &choice.overrides {
                texts.insert(*kind, text.as_str());
            }
            texts.into_iter().collect()
        }
    }
}

fn decided_by(step: Step) -> DecidedBy {
    match step {
        Step::Declared(_, decided_by) | Step::Choice(_, decided_by) => decided_by,
    }
}

fn is_single_named_unit(pool: &ExprPool, unit: UnitId) -> bool {
    pool.units()
        .factors(unit)
        .is_ok_and(|factors| matches!(factors, [factor] if factor.exponent() == 1))
}

fn unit_of_step(
    pool: &mut ExprPool,
    step: Step,
    systems: &[UnitSystem],
    question: &DisplayQuestion,
) -> Option<(DisplayUnit, QuantityKind)> {
    let texts = step_texts(step, systems);
    match question.kind {
        Some(kind) => {
            let (_, text) = texts.iter().find(|(declared, _)| *declared == kind)?;
            applicable(pool, text, kind, question.dimension).map(|unit| (unit, kind))
        }
        None => {
            let carries_a_named_unit = question
                .stored
                .is_some_and(|stored| is_single_named_unit(pool, stored));
            let mut matching = texts.iter().filter_map(|(kind, text)| {
                let unit = applicable(pool, text, *kind, question.dimension)?;
                (kind.dimension_determines_it() || carries_a_named_unit).then_some((unit, *kind))
            });
            let first = matching.next()?;
            matching.next().is_none().then_some(first)
        }
    }
}

fn choice_steps<'sources>(
    sources: &DisplaySources<'sources>,
) -> Vec<(&'sources UnitsChoice, DecidedBy)> {
    [
        sources
            .session_override
            .map(|choice| (choice, DecidedBy::SessionOverride)),
        sources
            .preference
            .map(|choice| (choice, DecidedBy::Preference)),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn system_list<'sources>(
    sources: &DisplaySources<'sources>,
    choice: &'sources UnitsChoice,
    kind: QuantityKind,
) -> Option<&'sources str> {
    let identifier = choice.system.as_deref()?;
    sources
        .systems
        .iter()
        .find(|system| system.identifier == identifier)?
        .displayed
        .get(&kind)
        .map(String::as_str)
}

fn declared_texts<'sources>(
    sources: &DisplaySources<'sources>,
    kind: QuantityKind,
) -> Vec<(&'sources str, DecidedBy)> {
    let mut texts = Vec::new();
    for (choice, decided_by) in choice_steps(sources) {
        if let Some(text) = choice.overrides.get(&kind) {
            texts.push((text.as_str(), decided_by));
        }
        if let Some(text) = system_list(sources, choice, kind) {
            texts.push((text, decided_by));
        }
    }
    if let Some(curriculum) = sources.curriculum
        && let Some(text) = curriculum.displayed.get(&kind)
    {
        texts.push((text.as_str(), DecidedBy::CurriculumDisplayed));
    }
    texts
}

fn list_units(
    pool: &mut ExprPool,
    text: &str,
    kind: QuantityKind,
    dimension: Dimension,
) -> Vec<UnitId> {
    let mut units = Vec::new();
    for entry in text.split(LIST_SEPARATOR) {
        match applicable(pool, entry.trim(), kind, dimension) {
            Some(DisplayUnit::Unit(unit)) => units.push(unit),
            _ => return Vec::new(),
        }
    }
    units
}

fn kind_of_question(
    pool: &mut ExprPool,
    question: &DisplayQuestion,
    sources: &DisplaySources,
) -> Option<QuantityKind> {
    if let Some(kind) = question.kind {
        return Some(kind);
    }
    let mut kinds: Vec<QuantityKind> = Vec::new();
    let declared: Vec<(QuantityKind, String)> = choice_steps(sources)
        .into_iter()
        .flat_map(|(choice, _)| {
            let system_texts: Vec<(QuantityKind, String)> = sources
                .systems
                .iter()
                .filter(|system| Some(system.identifier.as_str()) == choice.system.as_deref())
                .flat_map(|system| {
                    system
                        .displayed
                        .iter()
                        .map(|(kind, text)| (*kind, text.clone()))
                })
                .collect();
            choice
                .overrides
                .iter()
                .map(|(kind, text)| (*kind, text.clone()))
                .chain(system_texts)
                .collect::<Vec<(QuantityKind, String)>>()
        })
        .chain(
            sources
                .curriculum
                .into_iter()
                .flat_map(|curriculum| {
                    curriculum
                        .displayed
                        .iter()
                        .map(|(kind, text)| (*kind, text.clone()))
                })
                .collect::<Vec<(QuantityKind, String)>>(),
        )
        .collect();
    for (kind, text) in declared {
        if kinds.contains(&kind) {
            continue;
        }
        let units = list_units(pool, &text, kind, question.dimension);
        let single = applicable(pool, &text, kind, question.dimension);
        let carries_a_named_unit = question
            .stored
            .is_some_and(|stored| is_single_named_unit(pool, stored));
        if (!units.is_empty() || single.is_some())
            && (kind.dimension_determines_it() || carries_a_named_unit)
        {
            kinds.push(kind);
        }
    }
    match kinds.as_slice() {
        [single] => Some(*single),
        _ => None,
    }
}

fn decide_by_written_unit(
    pool: &mut ExprPool,
    question: &DisplayQuestion,
    coherent: UnitId,
    readable: &ReadableValue,
    decision: impl Fn(UnitId, DecidedBy, Option<String>, bool) -> DisplayDecision,
) -> Option<DisplayDecision> {
    let inside = written_candidates(
        pool,
        &readable.inside,
        question.dimension,
        None,
        coherent,
        &[],
    );
    if inside.is_empty() {
        return readable
            .composed
            .map(|unit| decision(unit, DecidedBy::Written, None, false));
    }
    let unit = readable_pick(pool, &inside, coherent, readable)
        .or_else(|| pick_unit(pool, &inside, coherent, readable))?;
    Some(decision(unit, DecidedBy::Written, None, true))
}

fn decide_by_pick(
    pool: &mut ExprPool,
    question: &DisplayQuestion,
    sources: &DisplaySources,
    readable: &ReadableValue,
) -> Option<DisplayDecision> {
    let found_kind = kind_of_question(pool, question, sources);
    let coherent = pool.units_mut().coherent_unit(&question.dimension).ok()?;
    let decision = |unit: UnitId, decided_by, curriculum: Option<String>, picked| DisplayDecision {
        unit: Some(DisplayUnit::Unit(unit)),
        decided_by,
        kind: found_kind,
        curriculum,
        picked,
    };
    let Some(kind) = found_kind else {
        return decide_by_written_unit(pool, question, coherent, readable, decision);
    };
    for (choice, decided_by) in choice_steps(sources) {
        if let Some(text) = choice.overrides.get(&kind)
            && let Some(DisplayUnit::Unit(unit)) = applicable(pool, text, kind, question.dimension)
        {
            return Some(decision(unit, decided_by, None, false));
        }
    }
    let list: Vec<(Vec<UnitId>, DecidedBy)> = declared_texts(sources, kind)
        .into_iter()
        .map(|(text, decided_by)| (text.to_string(), decided_by))
        .collect::<Vec<(String, DecidedBy)>>()
        .into_iter()
        .filter_map(|(text, decided_by)| {
            let units = list_units(pool, &text, kind, question.dimension);
            (!units.is_empty()).then_some((units, decided_by))
        })
        .collect();
    let first_list = list.first().cloned();
    let list_units = first_list
        .as_ref()
        .map(|(units, _)| units.clone())
        .unwrap_or_default();
    let inside = written_candidates(
        pool,
        &readable.inside,
        question.dimension,
        Some(kind),
        coherent,
        &list_units,
    );
    if !inside.is_empty()
        && let Some(unit) = readable_pick(pool, &inside, coherent, readable)
    {
        return Some(decision(unit, DecidedBy::Written, None, true));
    }
    let Some((units, decided_by)) = first_list else {
        if inside.is_empty() {
            return readable
                .composed
                .map(|unit| decision(unit, DecidedBy::Written, None, false));
        }
        let unit = pick_unit(pool, &inside, coherent, readable)?;
        return Some(decision(unit, DecidedBy::Written, None, true));
    };
    let units = sorted_by_scale(pool, units);
    let unit = pick_unit(pool, &units, coherent, readable)?;
    let curriculum = (decided_by == DecidedBy::CurriculumDisplayed)
        .then(|| {
            sources
                .curriculum
                .map(|curriculum| curriculum.identifier.clone())
        })
        .flatten();
    Some(decision(unit, decided_by, curriculum, true))
}

pub fn decide_display_unit(
    pool: &mut ExprPool,
    question: &DisplayQuestion,
    sources: &DisplaySources,
) -> DisplayDecision {
    let decision = |unit: Option<DisplayUnit>, decided_by| DisplayDecision {
        unit,
        decided_by,
        kind: question.kind,
        curriculum: None,
        picked: false,
    };
    if let Some(written) = &question.written {
        return decision(Some(written.clone()), DecidedBy::Written);
    }
    let states_units = matches!(question.area, Area::Learn | Area::Train);
    if let (true, Some(stated)) = (states_units, &question.stated) {
        return decision(Some(stated.clone()), DecidedBy::Stated);
    }
    let is_task_content = question.area == Area::Train && question.place == ValuePlace::TaskContent;
    if is_task_content {
        return decision(None, DecidedBy::TaskContent);
    }
    if sources.coherent_only {
        return decision(None, DecidedBy::CoherentOnly);
    }
    if !takes_a_display_unit(question) {
        return decision(None, DecidedBy::Coherent);
    }
    if let (Some(readable), Area::Calculate | Area::Explore) = (&question.readable, question.area)
        && let Some(picked) = decide_by_pick(pool, question, sources, readable)
    {
        return picked;
    }
    let curriculum_posed = sources
        .curriculum
        .filter(|_| question.place == ValuePlace::Given)
        .map(|curriculum| Step::Declared(&curriculum.posed, DecidedBy::CurriculumPosed));
    let curriculum_displayed = sources
        .curriculum
        .map(|curriculum| Step::Declared(&curriculum.displayed, DecidedBy::CurriculumDisplayed));
    let session_override = sources
        .session_override
        .map(|choice| Step::Choice(choice, DecidedBy::SessionOverride));
    let preference = sources
        .preference
        .map(|choice| Step::Choice(choice, DecidedBy::Preference));
    let order = match question.area {
        Area::Calculate | Area::Explore => vec![session_override, preference, curriculum_displayed],
        Area::Learn | Area::Train => vec![
            curriculum_posed,
            curriculum_displayed,
            session_override,
            preference,
        ],
    };
    for step in order.into_iter().flatten() {
        if let Some((unit, kind)) = unit_of_step(pool, step, sources.systems, question) {
            let decided_by = decided_by(step);
            let curriculum = matches!(
                decided_by,
                DecidedBy::CurriculumPosed | DecidedBy::CurriculumDisplayed
            )
            .then(|| {
                sources
                    .curriculum
                    .map(|curriculum| curriculum.identifier.clone())
            })
            .flatten();
            return DisplayDecision {
                unit: Some(unit),
                decided_by,
                kind: Some(kind),
                curriculum,
                picked: false,
            };
        }
    }
    if let (Area::Calculate | Area::Explore, None, Some(stored)) = (
        question.area,
        kind_of_question(pool, question, sources),
        question.stored,
    ) {
        return decision(Some(DisplayUnit::Unit(stored)), DecidedBy::AsComputed);
    }
    decision(None, DecidedBy::Coherent)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_units_of_a_length_hold_the_metre_and_the_inch() {
        let units = super::units_of_the_dimension_of("m");

        assert!(units.contains(&"m".to_owned()) && units.contains(&"in".to_owned()));
    }

    #[test]
    fn the_units_of_a_length_hold_no_unit_of_another_dimension() {
        let units = super::units_of_the_dimension_of("m");

        assert!(!units.contains(&"kg".to_owned()));
    }

    #[test]
    fn the_units_of_a_speed_hold_the_compounds_the_systems_declare() {
        let units = super::units_of_the_dimension_of("m/s");

        assert!(units.contains(&"m/s".to_owned()) && units.contains(&"mi/h".to_owned()));
    }

    #[test]
    fn a_declared_unit_comes_before_a_unit_only_the_table_names() {
        let units = super::units_of_the_dimension_of("m");

        let millimetre = units.iter().position(|unit| unit == "mm");
        let yard = units.iter().position(|unit| unit == "yd");
        assert!(millimetre < yard);
    }

    #[test]
    fn a_text_that_is_no_unit_has_no_units() {
        assert!(super::units_of_the_dimension_of("banana").is_empty());
    }

    use super::*;

    #[test]
    fn written_unit_names_its_source_as_written() {
        assert_eq!(
            unit_source_message(DecidedBy::Written, None),
            Message::CommonUnitSourceWritten
        );
    }

    #[test]
    fn curriculum_source_names_the_curriculum() {
        assert_eq!(
            unit_source_message(DecidedBy::CurriculumDisplayed, Some("de-by-gymnasium")),
            Message::CommonUnitSourceCurriculum {
                name: "de-by-gymnasium".to_owned()
            }
        );
    }

    #[test]
    fn a_computed_unit_names_its_source_as_computed() {
        assert_eq!(
            unit_source_message(DecidedBy::AsComputed, None),
            Message::CommonUnitSourceAsComputed
        );
    }

    #[test]
    fn coherent_unit_is_the_stored_unit() {
        assert_eq!(
            unit_source_message(DecidedBy::Coherent, None),
            Message::CommonUnitSourceStoredUnit
        );
    }

    fn unit(pool: &mut ExprPool, text: &str) -> DisplayUnit {
        resolve_display_unit(pool, text, None).unwrap()
    }

    fn dimension_of(pool: &mut ExprPool, text: &str) -> Dimension {
        let unit = unit(pool, text);
        display_dimension(pool, &unit).unwrap()
    }

    fn declared(entries: &[(QuantityKind, &str)]) -> BTreeMap<QuantityKind, String> {
        entries
            .iter()
            .map(|(kind, text)| (*kind, text.to_string()))
            .collect()
    }

    fn choice(entries: &[(QuantityKind, &str)]) -> UnitsChoice {
        UnitsChoice {
            system: None,
            overrides: declared(entries),
        }
    }

    fn speed_question(pool: &mut ExprPool, area: Area, place: ValuePlace) -> DisplayQuestion {
        DisplayQuestion {
            area,
            place,
            dimension: dimension_of(pool, "m/s"),
            stored: single_unit(pool, "m/s"),
            kind: Some(QuantityKind::Speed),
            written: None,
            stated: None,
            readable: None,
        }
    }

    fn decided(
        pool: &mut ExprPool,
        question: &DisplayQuestion,
        sources: &DisplaySources,
    ) -> (Option<DisplayUnit>, DecidedBy) {
        let decision = decide_display_unit(pool, question, sources);
        (decision.unit, decision.decided_by)
    }

    #[test]
    fn written_unit_comes_first_in_calculate() {
        let mut pool = ExprPool::new();
        let mut question = speed_question(&mut pool, Area::Calculate, ValuePlace::Derived);
        question.written = Some(unit(&mut pool, "mi/h"));
        let override_units = choice(&[(QuantityKind::Speed, "km/h")]);
        let sources = DisplaySources {
            session_override: Some(&override_units),
            ..DisplaySources::default()
        };

        let expected = (Some(unit(&mut pool, "mi/h")), DecidedBy::Written);
        assert_eq!(decided(&mut pool, &question, &sources), expected);
    }

    #[test]
    fn stated_task_unit_comes_first_in_train() {
        let mut pool = ExprPool::new();
        let mut question = speed_question(&mut pool, Area::Train, ValuePlace::TaskContent);
        question.stated = Some(unit(&mut pool, "km/h"));
        let preference = choice(&[(QuantityKind::Speed, "mi/h")]);
        let sources = DisplaySources {
            preference: Some(&preference),
            ..DisplaySources::default()
        };

        let expected = (Some(unit(&mut pool, "km/h")), DecidedBy::Stated);
        assert_eq!(decided(&mut pool, &question, &sources), expected);
    }

    #[test]
    fn stated_unit_does_not_apply_in_calculate() {
        let mut pool = ExprPool::new();
        let mut question = speed_question(&mut pool, Area::Calculate, ValuePlace::Derived);
        question.stated = Some(unit(&mut pool, "km/h"));

        let decision = decided(&mut pool, &question, &DisplaySources::default());

        assert_eq!(decision, (None, DecidedBy::Coherent));
    }

    #[test]
    fn train_task_content_without_a_stated_unit_is_coherent_despite_the_preference() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Train, ValuePlace::TaskContent);
        let preference = choice(&[(QuantityKind::Speed, "mi/h")]);
        let sources = DisplaySources {
            preference: Some(&preference),
            ..DisplaySources::default()
        };

        assert_eq!(
            decided(&mut pool, &question, &sources),
            (None, DecidedBy::TaskContent)
        );
    }

    #[test]
    fn value_around_a_train_task_follows_the_learn_order() {
        let mut pool = ExprPool::new();
        let mut question = speed_question(&mut pool, Area::Train, ValuePlace::AroundTask);
        question.dimension = dimension_of(&mut pool, "s");
        question.kind = Some(QuantityKind::Time);
        let preference = choice(&[(QuantityKind::Time, "min s")]);
        let sources = DisplaySources {
            preference: Some(&preference),
            ..DisplaySources::default()
        };

        let decision = decided(&mut pool, &question, &sources);

        assert_eq!(decision.1, DecidedBy::Preference);
    }

    #[test]
    fn learn_given_takes_the_curriculum_posed_unit() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Learn, ValuePlace::Given);
        let curriculum = CurriculumUnits {
            identifier: "de-by-gymnasium".to_string(),
            posed: declared(&[(QuantityKind::Speed, "km/h")]),
            displayed: declared(&[(QuantityKind::Speed, "m/s")]),
        };
        let sources = DisplaySources {
            curriculum: Some(&curriculum),
            ..DisplaySources::default()
        };

        let expected = (Some(unit(&mut pool, "km/h")), DecidedBy::CurriculumPosed);
        assert_eq!(decided(&mut pool, &question, &sources), expected);
    }

    #[test]
    fn learn_derived_value_takes_the_curriculum_displayed_unit() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Learn, ValuePlace::Derived);
        let curriculum = CurriculumUnits {
            identifier: "de-by-gymnasium".to_string(),
            posed: declared(&[(QuantityKind::Speed, "km/h")]),
            displayed: declared(&[(QuantityKind::Speed, "m/s")]),
        };
        let sources = DisplaySources {
            curriculum: Some(&curriculum),
            ..DisplaySources::default()
        };

        let expected = (Some(unit(&mut pool, "m/s")), DecidedBy::CurriculumDisplayed);
        assert_eq!(decided(&mut pool, &question, &sources), expected);
    }

    #[test]
    fn learn_given_without_posed_unit_takes_the_displayed_unit() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Learn, ValuePlace::Given);
        let curriculum = CurriculumUnits {
            identifier: "de-by-gymnasium".to_string(),
            posed: BTreeMap::new(),
            displayed: declared(&[(QuantityKind::Speed, "m/s")]),
        };
        let sources = DisplaySources {
            curriculum: Some(&curriculum),
            ..DisplaySources::default()
        };

        let decision = decided(&mut pool, &question, &sources);

        assert_eq!(decision.1, DecidedBy::CurriculumDisplayed);
    }

    #[test]
    fn learn_curriculum_comes_before_the_session_override() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Learn, ValuePlace::Derived);
        let curriculum = CurriculumUnits {
            identifier: "de-by-gymnasium".to_string(),
            posed: BTreeMap::new(),
            displayed: declared(&[(QuantityKind::Speed, "m/s")]),
        };
        let override_units = choice(&[(QuantityKind::Speed, "mi/h")]);
        let sources = DisplaySources {
            curriculum: Some(&curriculum),
            session_override: Some(&override_units),
            ..DisplaySources::default()
        };

        let decision = decided(&mut pool, &question, &sources);

        assert_eq!(decision.1, DecidedBy::CurriculumDisplayed);
    }

    #[test]
    fn learn_without_a_curriculum_unit_takes_the_session_override() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Learn, ValuePlace::Derived);
        let override_units = choice(&[(QuantityKind::Speed, "mi/h")]);
        let preference = choice(&[(QuantityKind::Speed, "km/h")]);
        let sources = DisplaySources {
            session_override: Some(&override_units),
            preference: Some(&preference),
            ..DisplaySources::default()
        };

        let expected = (Some(unit(&mut pool, "mi/h")), DecidedBy::SessionOverride);
        assert_eq!(decided(&mut pool, &question, &sources), expected);
    }

    #[test]
    fn calculate_session_override_comes_before_the_preference() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Calculate, ValuePlace::Derived);
        let override_units = choice(&[(QuantityKind::Speed, "mi/h")]);
        let preference = choice(&[(QuantityKind::Speed, "km/h")]);
        let sources = DisplaySources {
            session_override: Some(&override_units),
            preference: Some(&preference),
            ..DisplaySources::default()
        };

        let decision = decided(&mut pool, &question, &sources);

        assert_eq!(decision.1, DecidedBy::SessionOverride);
    }

    #[test]
    fn calculate_preference_comes_before_the_curriculum() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Calculate, ValuePlace::Derived);
        let preference = choice(&[(QuantityKind::Speed, "km/h")]);
        let curriculum = CurriculumUnits {
            identifier: "de-by-gymnasium".to_string(),
            posed: BTreeMap::new(),
            displayed: declared(&[(QuantityKind::Speed, "mi/h")]),
        };
        let sources = DisplaySources {
            preference: Some(&preference),
            curriculum: Some(&curriculum),
            ..DisplaySources::default()
        };

        let expected = (Some(unit(&mut pool, "km/h")), DecidedBy::Preference);
        assert_eq!(decided(&mut pool, &question, &sources), expected);
    }

    #[test]
    fn calculate_curriculum_displayed_unit_comes_before_the_coherent_unit() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Explore, ValuePlace::Derived);
        let curriculum = CurriculumUnits {
            identifier: "de-by-gymnasium".to_string(),
            posed: declared(&[(QuantityKind::Speed, "km/h")]),
            displayed: declared(&[(QuantityKind::Speed, "mi/h")]),
        };
        let sources = DisplaySources {
            curriculum: Some(&curriculum),
            ..DisplaySources::default()
        };

        let expected = (
            Some(unit(&mut pool, "mi/h")),
            DecidedBy::CurriculumDisplayed,
        );
        assert_eq!(decided(&mut pool, &question, &sources), expected);
    }

    #[test]
    fn decision_names_the_curriculum_that_decided() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Learn, ValuePlace::Derived);
        let curriculum = CurriculumUnits {
            identifier: "us-grade-8".to_string(),
            posed: BTreeMap::new(),
            displayed: declared(&[(QuantityKind::Speed, "mi/h")]),
        };
        let sources = DisplaySources {
            curriculum: Some(&curriculum),
            ..DisplaySources::default()
        };

        let decision = decide_display_unit(&mut pool, &question, &sources);

        assert_eq!(decision.curriculum.as_deref(), Some("us-grade-8"));
    }

    #[test]
    fn decision_by_the_preference_names_no_curriculum() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Calculate, ValuePlace::Derived);
        let preference = choice(&[(QuantityKind::Speed, "km/h")]);
        let curriculum = CurriculumUnits {
            identifier: "us-grade-8".to_string(),
            posed: BTreeMap::new(),
            displayed: declared(&[(QuantityKind::Speed, "mi/h")]),
        };
        let sources = DisplaySources {
            preference: Some(&preference),
            curriculum: Some(&curriculum),
            ..DisplaySources::default()
        };

        let decision = decide_display_unit(&mut pool, &question, &sources);

        assert_eq!(decision.curriculum, None);
    }

    #[test]
    fn decision_carries_the_kind_selected_by_dimension() {
        let mut pool = ExprPool::new();
        let mut question = speed_question(&mut pool, Area::Calculate, ValuePlace::Derived);
        question.kind = None;
        let preference = choice(&[(QuantityKind::Speed, "km/h"), (QuantityKind::Length, "mi")]);
        let sources = DisplaySources {
            preference: Some(&preference),
            ..DisplaySources::default()
        };

        let decision = decide_display_unit(&mut pool, &question, &sources);

        assert_eq!(decision.kind, Some(QuantityKind::Speed));
    }

    #[test]
    fn coherent_decision_keeps_the_known_kind() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Calculate, ValuePlace::Derived);

        let decision = decide_display_unit(&mut pool, &question, &DisplaySources::default());

        assert_eq!(
            (decision.decided_by, decision.kind),
            (DecidedBy::Coherent, Some(QuantityKind::Speed))
        );
    }

    #[test]
    fn compound_with_extra_spaces_resolves() {
        let mut pool = ExprPool::new();

        let compound = resolve_display_unit(&mut pool, " h  min ", Some(QuantityKind::Time));

        assert!(matches!(compound, Some(DisplayUnit::Compound(units)) if units.len() == 2));
    }

    #[test]
    fn unresolved_overrides_lists_the_kinds_whose_unit_does_not_resolve() {
        let mut pool = ExprPool::new();
        let units = choice(&[
            (QuantityKind::Speed, "km/h"),
            (QuantityKind::Length, "furlong"),
            (QuantityKind::Force, "N kN"),
        ]);

        let unresolved = units.unresolved_overrides(&mut pool);

        assert_eq!(unresolved, [QuantityKind::Length, QuantityKind::Force]);
    }

    #[test]
    fn nothing_declared_gives_the_coherent_unit() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Calculate, ValuePlace::Derived);

        let decision = decided(&mut pool, &question, &DisplaySources::default());

        assert_eq!(decision, (None, DecidedBy::Coherent));
    }

    #[test]
    fn coherent_only_skips_every_step_after_the_written_unit() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Calculate, ValuePlace::Derived);
        let preference = choice(&[(QuantityKind::Speed, "km/h")]);
        let sources = DisplaySources {
            preference: Some(&preference),
            coherent_only: true,
            ..DisplaySources::default()
        };

        assert_eq!(
            decided(&mut pool, &question, &sources),
            (None, DecidedBy::CoherentOnly)
        );
    }

    #[test]
    fn calculate_line_dimension_selects_the_one_declared_kind() {
        let mut pool = ExprPool::new();
        let mut question = speed_question(&mut pool, Area::Calculate, ValuePlace::Derived);
        question.kind = None;
        let preference = choice(&[(QuantityKind::Speed, "km/h"), (QuantityKind::Length, "mi")]);
        let sources = DisplaySources {
            preference: Some(&preference),
            ..DisplaySources::default()
        };

        let expected = (Some(unit(&mut pool, "km/h")), DecidedBy::Preference);
        assert_eq!(decided(&mut pool, &question, &sources), expected);
    }

    fn plain_number_question(pool: &mut ExprPool, stored: Option<UnitId>) -> DisplayQuestion {
        DisplayQuestion {
            area: Area::Calculate,
            place: ValuePlace::Derived,
            dimension: dimension_of(pool, "rad"),
            stored,
            kind: None,
            written: None,
            stated: None,
            readable: None,
        }
    }

    #[test]
    fn value_without_a_unit_takes_no_declared_angle() {
        let mut pool = ExprPool::new();
        let question = plain_number_question(&mut pool, None);
        let preference = choice(&[(QuantityKind::Angle, "deg")]);
        let sources = DisplaySources {
            preference: Some(&preference),
            ..DisplaySources::default()
        };

        assert_eq!(
            decided(&mut pool, &question, &sources),
            (None, DecidedBy::Coherent)
        );
    }

    #[test]
    fn value_in_radians_takes_the_declared_angle() {
        let mut pool = ExprPool::new();
        let radian = single_unit(&mut pool, "rad");
        let question = plain_number_question(&mut pool, radian);
        let preference = choice(&[(QuantityKind::Angle, "deg")]);
        let sources = DisplaySources {
            preference: Some(&preference),
            ..DisplaySources::default()
        };

        let expected = (Some(unit(&mut pool, "deg")), DecidedBy::Preference);
        assert_eq!(decided(&mut pool, &question, &sources), expected);
    }

    #[test]
    fn ratio_of_unlike_lengths_is_not_a_named_dimensionless_unit() {
        let mut pool = ExprPool::new();
        let metre = single_unit(&mut pool, "m").unwrap();
        let kilometre = single_unit(&mut pool, "km").unwrap();
        let ratio = pool.units_mut().divide(metre, kilometre).unwrap();

        assert!(!is_named_dimensionless_unit(&pool, ratio));
    }

    #[test]
    fn dimension_shared_by_two_declared_kinds_keeps_the_value_s_own_unit() {
        let mut pool = ExprPool::new();
        let question = DisplayQuestion {
            area: Area::Calculate,
            place: ValuePlace::Derived,
            dimension: dimension_of(&mut pool, "K"),
            stored: single_unit(&mut pool, "K"),
            kind: None,
            written: None,
            stated: None,
            readable: None,
        };
        let preference = choice(&[
            (QuantityKind::Temperature, "fahrenheit"),
            (QuantityKind::TemperatureDifference, "degF"),
        ]);
        let sources = DisplaySources {
            preference: Some(&preference),
            ..DisplaySources::default()
        };

        let stored = single_unit(&mut pool, "K").map(DisplayUnit::Unit);

        assert_eq!(
            decided(&mut pool, &question, &sources),
            (stored, DecidedBy::AsComputed)
        );
    }

    #[test]
    fn override_that_does_not_resolve_is_not_applied() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Calculate, ValuePlace::Derived);
        let override_units = choice(&[(QuantityKind::Speed, "furlong/fortnight")]);
        let preference = choice(&[(QuantityKind::Speed, "km/h")]);
        let sources = DisplaySources {
            session_override: Some(&override_units),
            preference: Some(&preference),
            ..DisplaySources::default()
        };

        let decision = decided(&mut pool, &question, &sources);

        assert_eq!(decision.1, DecidedBy::Preference);
    }

    #[test]
    fn override_of_the_wrong_dimension_is_not_applied() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Calculate, ValuePlace::Derived);
        let override_units = choice(&[(QuantityKind::Speed, "km")]);

        let sources = DisplaySources {
            session_override: Some(&override_units),
            ..DisplaySources::default()
        };

        assert_eq!(
            decided(&mut pool, &question, &sources),
            (None, DecidedBy::Coherent)
        );
    }

    #[test]
    fn scale_for_a_temperature_difference_is_not_applied() {
        let mut pool = ExprPool::new();
        let question = DisplayQuestion {
            area: Area::Calculate,
            place: ValuePlace::Derived,
            dimension: dimension_of(&mut pool, "K"),
            stored: single_unit(&mut pool, "K"),
            kind: Some(QuantityKind::TemperatureDifference),
            written: None,
            stated: None,
            readable: None,
        };
        let preference = choice(&[(QuantityKind::TemperatureDifference, "celsius")]);
        let sources = DisplaySources {
            preference: Some(&preference),
            ..DisplaySources::default()
        };

        assert_eq!(
            decided(&mut pool, &question, &sources),
            (None, DecidedBy::Coherent)
        );
    }

    #[test]
    fn unit_system_of_a_choice_gives_the_kind_unit() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Calculate, ValuePlace::Derived);
        let systems = [UnitSystem {
            identifier: "us-customary".to_string(),
            displayed: declared(&[(QuantityKind::Speed, "mi/h")]),
        }];
        let preference = UnitsChoice {
            system: Some("us-customary".to_string()),
            overrides: BTreeMap::new(),
        };
        let sources = DisplaySources {
            preference: Some(&preference),
            systems: &systems,
            ..DisplaySources::default()
        };

        let expected = (Some(unit(&mut pool, "mi/h")), DecidedBy::Preference);
        assert_eq!(decided(&mut pool, &question, &sources), expected);
    }

    #[test]
    fn kind_override_wins_over_the_system_of_the_same_choice() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Calculate, ValuePlace::Derived);
        let systems = [UnitSystem {
            identifier: "us-customary".to_string(),
            displayed: declared(&[(QuantityKind::Speed, "mi/h")]),
        }];
        let preference = UnitsChoice {
            system: Some("us-customary".to_string()),
            overrides: declared(&[(QuantityKind::Speed, "km/h")]),
        };
        let sources = DisplaySources {
            preference: Some(&preference),
            systems: &systems,
            ..DisplaySources::default()
        };

        let expected = (Some(unit(&mut pool, "km/h")), DecidedBy::Preference);
        assert_eq!(decided(&mut pool, &question, &sources), expected);
    }

    #[test]
    fn unknown_unit_system_is_not_applied() {
        let mut pool = ExprPool::new();
        let question = speed_question(&mut pool, Area::Calculate, ValuePlace::Derived);
        let preference = UnitsChoice {
            system: Some("imperial".to_string()),
            overrides: BTreeMap::new(),
        };
        let sources = DisplaySources {
            preference: Some(&preference),
            ..DisplaySources::default()
        };

        assert_eq!(
            decided(&mut pool, &question, &sources),
            (None, DecidedBy::Coherent)
        );
    }

    #[test]
    fn compound_of_descending_time_units_resolves() {
        let mut pool = ExprPool::new();

        let compound = resolve_display_unit(&mut pool, "h min", Some(QuantityKind::Time));

        let hour = single_unit(&mut pool, "h").unwrap();
        let minute = single_unit(&mut pool, "min").unwrap();
        assert_eq!(compound, Some(DisplayUnit::Compound(vec![hour, minute])));
    }

    #[test]
    fn compound_in_ascending_order_does_not_resolve() {
        let mut pool = ExprPool::new();

        let compound = resolve_display_unit(&mut pool, "min h", Some(QuantityKind::Time));

        assert_eq!(compound, None);
    }

    #[test]
    fn compound_for_speed_does_not_resolve() {
        let mut pool = ExprPool::new();

        let compound = resolve_display_unit(&mut pool, "km/h m/s", Some(QuantityKind::Speed));

        assert_eq!(compound, None);
    }

    #[test]
    fn scale_name_resolves_to_its_temperature_scale() {
        let mut pool = ExprPool::new();

        let scale = resolve_display_unit(&mut pool, "fahrenheit", None);

        assert_eq!(
            scale,
            Some(DisplayUnit::Scale(TemperatureScale::Fahrenheit))
        );
    }

    #[test]
    fn settings_read_system_and_kind_overrides() {
        let choice = UnitsChoice::from_settings(Some("si"), &["speed=km/h", "time = h min"]);

        assert_eq!(
            choice,
            Ok(UnitsChoice {
                system: Some("si".to_string()),
                overrides: declared(&[
                    (QuantityKind::Speed, "km/h"),
                    (QuantityKind::Time, "h min")
                ]),
            })
        );
    }

    #[test]
    fn setting_without_equals_sign_is_an_error() {
        let choice = UnitsChoice::from_settings(None, &["speed"]);

        assert_eq!(
            choice,
            Err(UnitsChoiceError::MissingSeparator { position: 0 })
        );
    }

    #[test]
    fn setting_with_unknown_kind_is_an_error() {
        let choice = UnitsChoice::from_settings(None, &["speed=km/h", "warmth=K"]);

        assert_eq!(choice, Err(UnitsChoiceError::UnknownKind { position: 1 }));
    }

    #[test]
    fn setting_repeating_a_kind_is_an_error() {
        let choice = UnitsChoice::from_settings(None, &["speed=km/h", "speed=mi/h"]);

        assert_eq!(choice, Err(UnitsChoiceError::RepeatedKind { position: 1 }));
    }

    #[test]
    fn setting_with_empty_unit_is_an_error() {
        let choice = UnitsChoice::from_settings(None, &["speed= "]);

        assert_eq!(choice, Err(UnitsChoiceError::EmptyUnit { position: 0 }));
    }
}
