use std::collections::BTreeMap;

use calc_core::{Combine, Criterion, ErrorBound, Exactness, ResultValue};
use calc_expr::{ExprId, ExprPool, Head, NodeView, Operator};
use calc_i18n::{Locale, Message};
use calc_syntax::{PrintMode, parse_expression, print_expression};

use crate::session_file::unit_text;
use crate::solve::{concept_set, source_body};
use crate::solve_answer::{ConditionParameter, QuantityMark, SolveCriterion, StoredScore};
use crate::solve_request::{GivenInput, Phase, SolveRequest};
use crate::summary::value_text;

const NAMING_SEPARATOR: char = '=';
const PERMUTATION_SEPARATOR: char = '~';
const PLACEHOLDER_BASE: u32 = 0xE000;
const PERSON_EQUALS: &str = " = ";
const ROLE_SEPARATOR: char = '.';
const SOURCE_LOCALE: &str = "en";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormRow {
    Wanted,
    Given(usize),
    Criterion,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormErrorCode {
    MissingWanted,
    UnknownQuantity { text: String },
    UnknownObject { text: String },
    MalformedGiven { text: String },
    EmptyCriterion,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormError {
    pub row: FormRow,
    pub code: FormErrorCode,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SolveForm {
    pub wanted: String,
    pub given: Vec<String>,
    pub criterion: Vec<Criterion>,
    pub combine_as_front: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameForm {
    Heading,
    Running,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantityName {
    pub object: String,
    pub role: String,
    pub heading: String,
    pub running: String,
}

impl QuantityName {
    fn in_form(&self, form: NameForm) -> &str {
        match form {
            NameForm::Heading => &self.heading,
            NameForm::Running => &self.running,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ObjectName {
    identifier: String,
    heading: String,
    running: String,
}

impl ObjectName {
    fn in_form(&self, form: NameForm) -> &str {
        match form {
            NameForm::Heading => &self.heading,
            NameForm::Running => &self.running,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuleRoles {
    identifier: String,
    output: String,
    inputs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BoundText {
    identifier: String,
    inputs: Vec<String>,
    expressions: Vec<(String, Vec<String>)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolveVocabulary {
    objects: Vec<ObjectName>,
    quantities: Vec<QuantityName>,
    formulas: Vec<(String, String)>,
    rules: Vec<RuleRoles>,
    conditions: Vec<(String, Vec<String>)>,
    bounds: Vec<BoundText>,
}

fn localized_forms(
    names: &std::collections::BTreeMap<String, String>,
    running_names: &std::collections::BTreeMap<String, String>,
    identifier: &str,
    locale: &str,
) -> (String, String) {
    [locale, SOURCE_LOCALE]
        .into_iter()
        .find_map(|key| Some((names.get(key)?.clone(), running_names.get(key)?.clone())))
        .unwrap_or_else(|| (identifier.to_owned(), identifier.to_owned()))
}

impl SolveVocabulary {
    pub fn load(locale: &Locale) -> Option<Self> {
        Some(Self::from_concepts(concept_set().ok()?, locale.tag()))
    }

    fn from_concepts(concepts: &calc_concepts::ConceptSet, locale: &str) -> Self {
        let objects = concepts
            .objects
            .iter()
            .map(|object| {
                let (heading, running) = localized_forms(
                    &object.names,
                    &object.running_names,
                    &object.identifier,
                    locale,
                );
                ObjectName {
                    identifier: object.identifier.clone(),
                    heading,
                    running,
                }
            })
            .collect();
        let quantities = concepts
            .objects
            .iter()
            .flat_map(|object| {
                object.roles.iter().map(move |role| {
                    let short = role
                        .identifier
                        .split_once(ROLE_SEPARATOR)
                        .map_or(role.identifier.as_str(), |(_, name)| name);
                    let (heading, running) =
                        localized_forms(&role.names, &role.running_names, short, locale);
                    QuantityName {
                        object: object.identifier.clone(),
                        role: short.to_owned(),
                        heading,
                        running,
                    }
                })
            })
            .collect();
        let formulas = concepts
            .concepts
            .iter()
            .flat_map(|concept| concept.ways.iter())
            .map(|way| (way.identifier.clone(), way.formula.clone()))
            .collect();
        let conditions = concepts
            .concepts
            .iter()
            .flat_map(|concept| concept.ways.iter())
            .map(|way| (way.identifier.clone(), way.conditions.clone()))
            .collect();
        let rules = concepts
            .rule_set(&mut ExprPool::new())
            .map(|rule_set| {
                rule_set
                    .rules
                    .into_iter()
                    .map(|rule| RuleRoles {
                        identifier: rule.identifier,
                        output: rule.output,
                        inputs: rule.inputs,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let bounds = concepts
            .concepts
            .iter()
            .flat_map(|concept| concept.bounds.iter())
            .map(|bound| BoundText {
                identifier: bound.identifier.clone(),
                inputs: bound.inputs.clone(),
                expressions: bound
                    .expressions
                    .iter()
                    .map(|source| source_body(source, bound.inputs.len()))
                    .collect::<Option<Vec<_>>>()
                    .unwrap_or_default(),
            })
            .collect();
        Self {
            objects,
            quantities,
            formulas,
            rules,
            conditions,
            bounds,
        }
    }

    pub fn quantity_name(&self, object: Option<&str>, role: &str, form: NameForm) -> String {
        self.quantities
            .iter()
            .find(|quantity| {
                quantity.role == role && object.is_none_or(|object| quantity.object == object)
            })
            .map_or_else(
                || role.to_owned(),
                |quantity| quantity.in_form(form).to_owned(),
            )
    }

    pub fn names(&self) -> Vec<&str> {
        self.objects
            .iter()
            .flat_map(|object| [object.heading.as_str(), object.running.as_str()])
            .chain(
                self.quantities
                    .iter()
                    .flat_map(|quantity| [quantity.heading.as_str(), quantity.running.as_str()]),
            )
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn identifiers(&self) -> Vec<&str> {
        self.objects
            .iter()
            .map(|object| object.identifier.as_str())
            .chain(
                self.quantities
                    .iter()
                    .map(|quantity| quantity.role.as_str()),
            )
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn object_name(&self, identifier: &str, form: NameForm) -> String {
        self.objects
            .iter()
            .find(|object| object.identifier == identifier)
            .map_or_else(
                || identifier.to_owned(),
                |object| object.in_form(form).to_owned(),
            )
    }

    pub fn person_role_name(&self, role: &str, symbols: &BTreeMap<String, String>) -> String {
        if let Some(symbol) = symbols.get(role) {
            return symbol.clone();
        }
        let (object, short) = role
            .split_once(ROLE_SEPARATOR)
            .map_or((None, role), |(object, short)| (Some(object), short));
        self.quantity_name(object, short, NameForm::Running)
    }

    pub fn person_condition_text(
        &self,
        text: &str,
        parameters: &[ConditionParameter],
        symbols: &BTreeMap<String, String>,
    ) -> Option<String> {
        person_text(text, parameters, |role| {
            self.person_role_name(role, symbols)
        })
    }

    pub fn person_formula_text(
        &self,
        rule: &str,
        symbols: &BTreeMap<String, String>,
    ) -> Option<String> {
        let roles = self.rules.iter().find(|roles| roles.identifier == rule)?;
        let way = rule
            .split_once(PERMUTATION_SEPARATOR)
            .map_or(rule, |(way, _)| way);
        let source = self.formula_of(way)?;
        let (text, names) = source_body(source, roles.inputs.len())?;
        let parameters: Vec<ConditionParameter> = names
            .into_iter()
            .zip(&roles.inputs)
            .map(|(name, role)| ConditionParameter {
                name,
                role: role.clone(),
            })
            .collect();
        let body = self.person_condition_text(&text, &parameters, symbols)?;
        Some(format!(
            "{}{PERSON_EQUALS}{body}",
            self.person_role_name(&roles.output, symbols)
        ))
    }

    pub fn formula_of(&self, rule: &str) -> Option<&str> {
        self.formulas
            .iter()
            .find(|(identifier, _)| identifier == rule)
            .map(|(_, formula)| formula.as_str())
    }

    pub fn conditions_of(&self, rule: &str) -> &[String] {
        self.conditions
            .iter()
            .find(|(identifier, _)| identifier == rule)
            .map_or(&[], |(_, conditions)| conditions.as_slice())
    }

    pub fn bound_expressions_of(&self, bound: &str) -> Vec<String> {
        self.bounds
            .iter()
            .find(|known| known.identifier == bound)
            .map(|known| {
                known
                    .expressions
                    .iter()
                    .map(|(text, _)| text.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn person_bound_expressions_of(
        &self,
        bound: &str,
        symbols: &BTreeMap<String, String>,
    ) -> Vec<String> {
        let Some(known) = self.bounds.iter().find(|known| known.identifier == bound) else {
            return Vec::new();
        };
        known
            .expressions
            .iter()
            .map(|(text, names)| {
                let parameters: Vec<ConditionParameter> = names
                    .iter()
                    .zip(&known.inputs)
                    .map(|(name, role)| ConditionParameter {
                        name: name.clone(),
                        role: role.clone(),
                    })
                    .collect();
                self.person_condition_text(text, &parameters, symbols)
            })
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default()
    }

    fn object_named(&self, text: &str) -> Option<&str> {
        self.objects
            .iter()
            .find(|object| {
                object.identifier == text
                    || object.heading.eq_ignore_ascii_case(text)
                    || object.running.eq_ignore_ascii_case(text)
            })
            .map(|object| object.identifier.as_str())
    }

    fn role_named(&self, object: Option<&str>, text: &str) -> Option<&QuantityName> {
        self.quantities.iter().find(|quantity| {
            (quantity.role == text
                || quantity.heading.eq_ignore_ascii_case(text)
                || quantity.running.eq_ignore_ascii_case(text))
                && object.is_none_or(|object| quantity.object == object)
        })
    }
}

fn placeholder(position: usize) -> Option<String> {
    let offset = u32::try_from(position).ok()?;
    char::from_u32(PLACEHOLDER_BASE.checked_add(offset)?).map(String::from)
}

struct Placeholders {
    plain: std::collections::HashMap<String, ExprId>,
    grouped: std::collections::HashMap<String, ExprId>,
}

fn groups_its_operands(operator: Operator) -> bool {
    matches!(
        operator,
        Operator::Pow | Operator::Neg | Operator::Factorial | Operator::Sqrt
    )
}

fn with_placeholders(
    pool: &mut ExprPool,
    expression: ExprId,
    placeholders: &Placeholders,
    is_grouped: bool,
) -> Option<ExprId> {
    match pool.node(expression).ok()? {
        NodeView::Number(_) | NodeView::Bound(_) => Some(expression),
        NodeView::Symbol(symbol) => {
            let name = pool.symbol_name(symbol).ok()?;
            let table = if is_grouped {
                &placeholders.grouped
            } else {
                &placeholders.plain
            };
            Some(table.get(name).copied().unwrap_or(expression))
        }
        NodeView::Apply { head, arguments } => {
            let groups = matches!(head, Head::Operator(operator) if groups_its_operands(operator));
            let arguments = arguments.to_vec();
            let replaced = arguments
                .into_iter()
                .map(|argument| with_placeholders(pool, argument, placeholders, groups))
                .collect::<Option<Vec<_>>>()?;
            pool.apply(head, &replaced).ok()
        }
        NodeView::Quantity { value, unit } => {
            let value = with_placeholders(pool, value, placeholders, true)?;
            pool.quantity(value, unit).ok()
        }
        NodeView::Array { shape, elements } => {
            let shape = shape.to_vec();
            let elements = elements.to_vec();
            let replaced = elements
                .into_iter()
                .map(|element| with_placeholders(pool, element, placeholders, false))
                .collect::<Option<Vec<_>>>()?;
            pool.array(&shape, &replaced).ok()
        }
        NodeView::Bind {
            binder,
            arguments,
            body,
        } => {
            let arguments = arguments.to_vec();
            let replaced = arguments
                .into_iter()
                .map(|argument| with_placeholders(pool, argument, placeholders, false))
                .collect::<Option<Vec<_>>>()?;
            let body = with_placeholders(pool, body, placeholders, false)?;
            let name = pool.bound_name(expression).map(str::to_owned);
            let rebuilt = pool.bind(binder, &replaced, body).ok()?;
            if let Some(name) = name {
                pool.record_bound_name(rebuilt, &name).ok()?;
            }
            Some(rebuilt)
        }
    }
}

pub fn person_text(
    text: &str,
    parameters: &[ConditionParameter],
    name_of: impl Fn(&str) -> String,
) -> Option<String> {
    let mut pool = ExprPool::new();
    let parsed = parse_expression(&mut pool, text).ok()?;
    let mut placeholders = Placeholders {
        plain: std::collections::HashMap::new(),
        grouped: std::collections::HashMap::new(),
    };
    let mut replacements = Vec::new();
    for (position, parameter) in parameters.iter().enumerate() {
        let name = name_of(&parameter.role);
        let plain_mark = placeholder(position.checked_mul(2)?)?;
        let grouped_mark = placeholder(position.checked_mul(2)?.checked_add(1)?)?;
        for (mark, table) in [
            (&plain_mark, &mut placeholders.plain),
            (&grouped_mark, &mut placeholders.grouped),
        ] {
            let symbol = pool
                .intern_symbol(mark, calc_expr::SymbolKind::Variable)
                .ok()?;
            table.insert(parameter.name.clone(), pool.symbol(symbol).ok()?);
        }
        let grouped_name = if name.contains(char::is_whitespace) {
            format!("({name})")
        } else {
            name.clone()
        };
        replacements.push((plain_mark, name));
        replacements.push((grouped_mark, grouped_name));
    }
    let replaced = with_placeholders(&mut pool, parsed, &placeholders, false)?;
    let printed = print_expression(&pool, replaced, PrintMode::Unicode).ok()?;
    Some(replacements.iter().fold(printed, |text, (mark, name)| {
        text.replace(mark.as_str(), name)
    }))
}

pub fn cockpit_criterion() -> Vec<Criterion> {
    vec![
        Criterion::FewestMeasurements,
        Criterion::SmallestUncertainty,
    ]
}

enum GivenRow {
    Object(String),
    Quantity(GivenInput),
}

fn printed(pool: &ExprPool, expression: ExprId) -> Option<String> {
    print_expression(pool, expression, PrintMode::Unicode).ok()
}

fn given_value(pool: &ExprPool, expression: ExprId, name: String) -> Option<GivenInput> {
    let (value_expression, unit) = match pool.node(expression).ok()? {
        NodeView::Quantity { value, unit } => (value, unit_text(pool, unit).ok().flatten()),
        _ => (expression, None),
    };
    let (value, uncertainty, coverage_factor) = match pool.node(value_expression).ok()? {
        NodeView::Apply {
            head: Head::Operator(Operator::Uncertain),
            arguments: [value, uncertainty, _],
        } => (*value, Some(*uncertainty), None),
        NodeView::Apply {
            head: Head::Operator(Operator::UncertainExpanded),
            arguments: [value, uncertainty, factor, _],
        } => (*value, Some(*uncertainty), Some(*factor)),
        _ => (value_expression, None, None),
    };
    Some(GivenInput {
        name,
        value: printed(pool, value)?,
        uncertainty: match uncertainty {
            Some(uncertainty) => Some(printed(pool, uncertainty)?),
            None => None,
        },
        coverage_factor: match coverage_factor {
            Some(factor) => Some(printed(pool, factor)?),
            None => None,
        },
        unit,
    })
}

fn given_row(
    text: &str,
    index: usize,
    object: Option<&str>,
    vocabulary: &SolveVocabulary,
) -> Result<GivenRow, FormError> {
    let row = FormRow::Given(index);
    let error = |code| FormError { row, code };
    let Some((name, quantity)) = text.split_once(NAMING_SEPARATOR) else {
        return vocabulary
            .object_named(text.trim())
            .map(|object| GivenRow::Object(object.to_owned()))
            .ok_or_else(|| {
                error(FormErrorCode::UnknownObject {
                    text: text.trim().to_owned(),
                })
            });
    };
    let name = name.trim();
    let role = vocabulary
        .role_named(object, name)
        .map(|quantity| quantity.role.clone())
        .ok_or_else(|| {
            error(FormErrorCode::UnknownQuantity {
                text: name.to_owned(),
            })
        })?;
    let malformed = || {
        error(FormErrorCode::MalformedGiven {
            text: quantity.trim().to_owned(),
        })
    };
    let mut pool = ExprPool::new();
    let expression = parse_expression(&mut pool, quantity.trim()).map_err(|_| malformed())?;
    given_value(&pool, expression, role)
        .map(GivenRow::Quantity)
        .ok_or_else(malformed)
}

pub fn request_from_form(
    form: &SolveForm,
    vocabulary: &SolveVocabulary,
) -> Result<SolveRequest, FormError> {
    let wanted_text = form.wanted.trim();
    if wanted_text.is_empty() {
        return Err(FormError {
            row: FormRow::Wanted,
            code: FormErrorCode::MissingWanted,
        });
    }
    if form.criterion.is_empty() {
        return Err(FormError {
            row: FormRow::Criterion,
            code: FormErrorCode::EmptyCriterion,
        });
    }
    let rows: Vec<(usize, &str)> = form
        .given
        .iter()
        .enumerate()
        .map(|(index, text)| (index, text.trim()))
        .filter(|(_, text)| !text.is_empty())
        .collect();
    let mut object = None;
    for &(index, text) in rows
        .iter()
        .filter(|(_, text)| !text.contains(NAMING_SEPARATOR))
    {
        if let GivenRow::Object(identifier) = given_row(text, index, None, vocabulary)? {
            object = Some(identifier);
        }
    }
    let mut given = Vec::new();
    for &(index, text) in rows
        .iter()
        .filter(|(_, text)| text.contains(NAMING_SEPARATOR))
    {
        if let GivenRow::Quantity(input) = given_row(text, index, object.as_deref(), vocabulary)? {
            given.push(input);
        }
    }
    let wanted = vocabulary
        .role_named(object.as_deref(), wanted_text)
        .map(|quantity| quantity.role.clone())
        .ok_or_else(|| FormError {
            row: FormRow::Wanted,
            code: FormErrorCode::UnknownQuantity {
                text: wanted_text.to_owned(),
            },
        })?;
    let mut request = SolveRequest::with_defaults(wanted);
    request.phase = Phase::Evaluate;
    request.object = object;
    request.given = given;
    request.criterion = SolveCriterion {
        by: form.criterion.clone(),
        combine: if form.combine_as_front {
            Combine::Front
        } else {
            Combine::Order
        },
    };
    Ok(request)
}

pub fn form_error_message(error: &FormError) -> Message {
    let given_row = match error.row {
        FormRow::Given(index) => (index + 1).to_string(),
        FormRow::Wanted | FormRow::Criterion => String::new(),
    };
    match (&error.code, error.row) {
        (FormErrorCode::MissingWanted, _) => Message::ErrorSolveMissingWanted,
        (FormErrorCode::EmptyCriterion, _) => Message::ErrorSolveEmptyCriterion,
        (FormErrorCode::UnknownQuantity { text }, FormRow::Given(_)) => {
            Message::ErrorSolveUnknownGivenQuantity {
                row: given_row,
                name: text.clone(),
            }
        }
        (FormErrorCode::UnknownQuantity { text }, _) => {
            Message::ErrorSolveUnknownWanted { name: text.clone() }
        }
        (FormErrorCode::UnknownObject { text }, _) => Message::ErrorSolveUnknownObject {
            row: given_row,
            name: text.clone(),
        },
        (FormErrorCode::MalformedGiven { text }, _) => Message::ErrorSolveMalformedGiven {
            row: given_row,
            text: text.clone(),
        },
    }
}

pub const fn criterion_message(criterion: Criterion) -> Message {
    match criterion {
        Criterion::FewestMeasurements => Message::CommonCriterionFewestMeasurements,
        Criterion::FewestSteps => Message::CommonCriterionFewestSteps,
        Criterion::Exactness => Message::CommonCriterionExactness,
        Criterion::ErrorBound => Message::CommonCriterionErrorBound,
        Criterion::GateCount => Message::CommonCriterionGateCount,
        Criterion::SmallestUncertainty => Message::CommonCriterionSmallestUncertainty,
        Criterion::Conditioning => Message::CommonCriterionConditioning,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScoreText {
    Data(String),
    Words(Message),
}

pub fn score_text(score: &StoredScore) -> ScoreText {
    match score {
        StoredScore::Count(count) => ScoreText::Data(count.to_string()),
        StoredScore::Exactness(Exactness::Exact) => {
            ScoreText::Words(Message::CommonSolveScoreExact)
        }
        StoredScore::Exactness(Exactness::Machine) => {
            ScoreText::Words(Message::CommonSolveScoreMachine)
        }
        StoredScore::ErrorBound(ErrorBound::Zero) => {
            ScoreText::Words(Message::CommonSolveScoreZero)
        }
        StoredScore::ErrorBound(ErrorBound::Documented) => {
            ScoreText::Words(Message::CommonSolveScoreDocumented)
        }
        StoredScore::ErrorBound(ErrorBound::Unknown) => {
            ScoreText::Words(Message::CommonRecordRoundingUnknown)
        }
        StoredScore::Exact(number) => {
            ScoreText::Data(value_text(&ResultValue::Number(number.clone())))
        }
    }
}

pub const fn quantity_mark_message(mark: QuantityMark) -> Message {
    match mark {
        QuantityMark::Given => Message::CommonSolveMarkGiven,
        QuantityMark::Obtainable => Message::CommonSolveMarkYes,
        QuantityMark::NotObtainable => Message::CommonSolveMarkNo,
        QuantityMark::Unmarked => Message::CommonSolveMarkUnmarked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocabulary() -> SolveVocabulary {
        SolveVocabulary::load(&Locale::source()).expect("embedded concept set loads")
    }

    fn form(wanted: &str, given: &[&str]) -> SolveForm {
        SolveForm {
            wanted: wanted.to_owned(),
            given: given.iter().map(|text| (*text).to_owned()).collect(),
            criterion: cockpit_criterion(),
            combine_as_front: false,
        }
    }

    #[test]
    fn wanted_quantity_with_its_object_becomes_the_request() {
        let request =
            request_from_form(&form("area", &["circle"]), &vocabulary()).expect("form is complete");

        assert_eq!(
            (
                request.wanted.as_deref(),
                request.object.as_deref(),
                request.phase
            ),
            (Some("area"), Some("circle"), Phase::Evaluate)
        );
    }

    #[test]
    fn wanted_quantity_can_be_named_in_words() {
        let request = request_from_form(&form("central angle", &["circle"]), &vocabulary())
            .expect("form is complete");

        assert_eq!(request.wanted.as_deref(), Some("central-angle"));
    }

    #[test]
    fn given_quantity_is_read_as_an_input_language_expression() {
        let request = request_from_form(
            &form("area", &["circle", "diameter = 12.0 \u{00B1} 0.1 cm"]),
            &vocabulary(),
        )
        .expect("form is complete");

        assert_eq!(
            request.given,
            vec![GivenInput {
                name: "diameter".to_owned(),
                value: "12".to_owned(),
                uncertainty: Some("0.1".to_owned()),
                coverage_factor: None,
                unit: Some("cm".to_owned()),
            }]
        );
    }

    #[test]
    fn given_value_without_uncertainty_or_unit_keeps_only_the_value() {
        let request = request_from_form(&form("area", &["circle", "radius = 6"]), &vocabulary())
            .expect("form is complete");

        assert_eq!(
            (
                request.given[0].uncertainty.clone(),
                request.given[0].unit.clone()
            ),
            (None, None)
        );
    }

    #[test]
    fn empty_wanted_row_is_an_error_in_that_row() {
        assert_eq!(
            request_from_form(&form("  ", &[]), &vocabulary()),
            Err(FormError {
                row: FormRow::Wanted,
                code: FormErrorCode::MissingWanted
            })
        );
    }

    #[test]
    fn unknown_given_object_names_its_row() {
        assert_eq!(
            request_from_form(&form("area", &["", "hexagon"]), &vocabulary()),
            Err(FormError {
                row: FormRow::Given(1),
                code: FormErrorCode::UnknownObject {
                    text: "hexagon".to_owned()
                }
            })
        );
    }

    #[test]
    fn unknown_wanted_quantity_is_reported() {
        assert_eq!(
            request_from_form(&form("volume", &["circle"]), &vocabulary()),
            Err(FormError {
                row: FormRow::Wanted,
                code: FormErrorCode::UnknownQuantity {
                    text: "volume".to_owned()
                }
            })
        );
    }

    #[test]
    fn given_value_that_does_not_parse_is_malformed() {
        assert_eq!(
            request_from_form(&form("area", &["circle", "radius = 6 +"]), &vocabulary()),
            Err(FormError {
                row: FormRow::Given(1),
                code: FormErrorCode::MalformedGiven {
                    text: "6 +".to_owned()
                }
            })
        );
    }

    #[test]
    fn unknown_given_quantity_message_names_its_row_from_one() {
        let error = FormError {
            row: FormRow::Given(1),
            code: FormErrorCode::UnknownQuantity {
                text: "chord".to_owned(),
            },
        };

        assert_eq!(
            form_error_message(&error),
            Message::ErrorSolveUnknownGivenQuantity {
                row: "2".to_owned(),
                name: "chord".to_owned()
            }
        );
    }

    #[test]
    fn unknown_wanted_quantity_message_names_the_wanted_row() {
        let error = FormError {
            row: FormRow::Wanted,
            code: FormErrorCode::UnknownQuantity {
                text: "volume".to_owned(),
            },
        };

        assert_eq!(
            form_error_message(&error),
            Message::ErrorSolveUnknownWanted {
                name: "volume".to_owned()
            }
        );
    }

    #[test]
    fn empty_criterion_is_an_error_in_the_criterion_row() {
        let mut without_criterion = form("area", &["circle"]);
        without_criterion.criterion.clear();

        assert_eq!(
            request_from_form(&without_criterion, &vocabulary()),
            Err(FormError {
                row: FormRow::Criterion,
                code: FormErrorCode::EmptyCriterion
            })
        );
    }

    #[test]
    fn given_quantity_is_looked_up_in_the_named_object_even_when_named_after_it() {
        assert_eq!(
            request_from_form(&form("area", &["chord = 3", "triangle"]), &vocabulary()),
            Err(FormError {
                row: FormRow::Given(0),
                code: FormErrorCode::UnknownQuantity {
                    text: "chord".to_owned()
                }
            })
        );
    }

    #[test]
    fn cockpit_criterion_starts_with_fewest_measurements_then_smallest_uncertainty() {
        assert_eq!(
            cockpit_criterion(),
            vec![
                Criterion::FewestMeasurements,
                Criterion::SmallestUncertainty
            ]
        );
    }

    #[test]
    fn front_form_combines_the_criteria_as_a_front() {
        let mut front = form("area", &["circle"]);
        front.combine_as_front = true;

        let request = request_from_form(&front, &vocabulary()).expect("form is complete");

        assert_eq!(request.criterion.combine, Combine::Front);
    }

    #[test]
    fn count_score_is_its_number() {
        assert_eq!(
            score_text(&StoredScore::Count(3)),
            ScoreText::Data("3".to_owned())
        );
    }

    #[test]
    fn exactness_score_is_a_word() {
        assert_eq!(
            score_text(&StoredScore::Exactness(Exactness::Exact)),
            ScoreText::Words(Message::CommonSolveScoreExact)
        );
    }

    fn german_region_vocabulary() -> SolveVocabulary {
        let region = calc_i18n::LanguageTag::parse("de-DE").expect("well-formed tag");
        SolveVocabulary::load(&Locale::matching(&region)).expect("embedded concept set loads")
    }

    #[test]
    fn german_region_finds_the_german_role_names() {
        assert_eq!(
            german_region_vocabulary().quantity_name(Some("circle"), "diameter", NameForm::Heading),
            "Durchmesser"
        );
    }

    #[test]
    fn bound_expression_is_the_body_of_its_content_lambda() {
        let vocabulary =
            SolveVocabulary::load(&Locale::source()).expect("embedded concept set loads");

        assert_eq!(
            vocabulary.bound_expressions_of("triangle-area/two-sides"),
            vec!["a * b / 2".to_owned()]
        );
    }

    #[test]
    fn bound_expression_in_person_form_names_its_roles() {
        let vocabulary =
            SolveVocabulary::load(&Locale::source()).expect("embedded concept set loads");

        assert_eq!(
            vocabulary.person_bound_expressions_of("triangle-area/two-sides", &BTreeMap::new()),
            vec!["side a \u{00B7} side b / 2".to_owned()]
        );
    }

    #[test]
    fn unknown_bound_has_no_expressions() {
        let vocabulary =
            SolveVocabulary::load(&Locale::source()).expect("embedded concept set loads");

        assert!(vocabulary.bound_expressions_of("no-such-bound").is_empty());
    }

    #[test]
    fn german_region_finds_the_german_object_name() {
        assert_eq!(
            german_region_vocabulary().object_name("circle", NameForm::Running),
            "Kreis"
        );
    }

    #[test]
    fn identifiers_hold_the_role_identifiers() {
        let vocabulary = german_region_vocabulary();

        assert!(vocabulary.identifiers().contains(&"diameter"));
    }

    #[test]
    fn identifier_shared_by_two_objects_is_listed_once() {
        let vocabulary = german_region_vocabulary();

        let areas = vocabulary
            .identifiers()
            .into_iter()
            .filter(|identifier| *identifier == "area")
            .count();

        assert_eq!(areas, 1);
    }

    #[test]
    fn german_names_hold_the_object_name() {
        assert!(german_region_vocabulary().names().contains(&"Kreis"));
    }

    #[test]
    fn english_object_running_name_is_its_running_form() {
        let vocabulary =
            SolveVocabulary::load(&Locale::source()).expect("embedded concept set loads");

        assert_eq!(
            vocabulary.object_name("circle", NameForm::Running),
            "circle"
        );
    }

    #[test]
    fn english_object_heading_name_is_its_title() {
        let vocabulary =
            SolveVocabulary::load(&Locale::source()).expect("embedded concept set loads");

        assert_eq!(
            vocabulary.object_name("circle", NameForm::Heading),
            "Circle"
        );
    }

    const RIGHT_TRIANGLE: &str =
        "# right-triangle\n\n## Role hypotenuse-length\nDimension: m\nQuantity: length\n";
    const RIGHT_TRIANGLE_EN: &str = "# Right triangle\nRunning: right triangle\n\n## Role hypotenuse-length\nName: Hypotenuse length\nRunning: hypotenuse length\n";

    fn right_triangle_vocabulary(locale: &str) -> SolveVocabulary {
        let concepts = calc_concepts::load(&[
            ("objects/right-triangle.md", RIGHT_TRIANGLE.as_bytes()),
            ("objects/right-triangle.en.md", RIGHT_TRIANGLE_EN.as_bytes()),
        ])
        .expect("right triangle loads");
        SolveVocabulary::from_concepts(&concepts, locale)
    }

    #[test]
    fn hyphenated_object_identifier_is_never_its_name() {
        let vocabulary = right_triangle_vocabulary("en");

        assert_eq!(
            [
                vocabulary.object_name("right-triangle", NameForm::Heading),
                vocabulary.object_name("right-triangle", NameForm::Running)
            ],
            ["Right triangle", "right triangle"]
        );
    }

    #[test]
    fn hyphenated_role_identifier_is_never_its_name() {
        let vocabulary = right_triangle_vocabulary("en");

        assert_eq!(
            [
                vocabulary.quantity_name(
                    Some("right-triangle"),
                    "hypotenuse-length",
                    NameForm::Heading
                ),
                vocabulary.quantity_name(
                    Some("right-triangle"),
                    "hypotenuse-length",
                    NameForm::Running
                )
            ],
            ["Hypotenuse length", "hypotenuse length"]
        );
    }

    #[test]
    fn locale_without_its_file_takes_both_forms_from_english() {
        let vocabulary = right_triangle_vocabulary("de");

        assert_eq!(
            vocabulary.object_name("right-triangle", NameForm::Running),
            "right triangle"
        );
    }

    #[test]
    fn running_form_is_accepted_as_a_given_object() {
        let vocabulary = right_triangle_vocabulary("en");

        assert_eq!(
            vocabulary.object_named("right triangle"),
            Some("right-triangle")
        );
    }

    #[test]
    fn unknown_object_keeps_its_identifier() {
        assert_eq!(
            german_region_vocabulary().object_name("sphere", NameForm::Heading),
            "sphere"
        );
    }

    #[test]
    fn quantity_name_comes_from_the_concept_set() {
        assert_eq!(
            vocabulary().quantity_name(Some("circle"), "central-angle", NameForm::Running),
            "central angle"
        );
    }

    #[test]
    fn formula_is_found_by_rule_identifier() {
        assert!(vocabulary().formula_of("circle-area/from-radius").is_some());
    }

    fn shipped_way_conditions() -> Vec<(String, usize, String)> {
        concept_set()
            .unwrap()
            .concepts
            .iter()
            .flat_map(|concept| concept.ways.iter())
            .flat_map(|way| {
                way.conditions
                    .iter()
                    .map(|condition| (way.identifier.clone(), way.inputs.len(), condition.clone()))
            })
            .collect()
    }

    #[test]
    fn every_shipped_condition_text_parses_back_to_its_opened_body() {
        let mismatched: Vec<String> = shipped_way_conditions()
            .into_iter()
            .filter(|(_, parameters, source)| {
                let mut pool = ExprPool::new();
                let function = parse_expression(&mut pool, source).unwrap();
                let (opened, _) =
                    crate::solve::opened_lambda_body(&mut pool, function, *parameters).unwrap();
                let text = print_expression(&pool, opened, PrintMode::Ascii).unwrap();
                parse_expression(&mut pool, &text).ok() != Some(opened)
            })
            .map(|(way, _, source)| format!("{way}: {source}"))
            .collect();

        assert!(!shipped_way_conditions().is_empty());
        assert!(mismatched.is_empty(), "{mismatched:?}");
    }

    fn role_identifiers() -> Vec<String> {
        concept_set()
            .unwrap()
            .objects
            .iter()
            .flat_map(|object| object.roles.iter().map(|role| role.identifier.clone()))
            .collect()
    }

    fn shows_an_identifier(text: &str, identifiers: &[String]) -> bool {
        identifiers.iter().any(|identifier| {
            let short = identifier
                .split_once(ROLE_SEPARATOR)
                .map_or(identifier.as_str(), |(_, short)| short);
            text.contains(identifier.as_str()) || (short.contains('-') && text.contains(short))
        })
    }

    #[test]
    fn no_person_condition_text_of_a_shipped_way_contains_a_role_identifier() {
        let identifiers = role_identifiers();
        let german = Locale::matching(&calc_i18n::LanguageTag::parse("de").unwrap());
        let mut pool = ExprPool::new();
        let rule_set = concept_set().unwrap().rule_set(&mut pool).unwrap();
        let mut offending = Vec::new();
        for locale in [Locale::source(), german] {
            let vocabulary = SolveVocabulary::load(&locale).unwrap();
            for rule in &rule_set.rules {
                let way = rule
                    .identifier
                    .split_once(PERMUTATION_SEPARATOR)
                    .map_or(rule.identifier.as_str(), |(way, _)| way);
                for (_, _, source) in shipped_way_conditions()
                    .into_iter()
                    .filter(|(identifier, _, _)| identifier == way)
                {
                    let (text, names) = source_body(&source, rule.inputs.len()).unwrap();
                    let parameters: Vec<ConditionParameter> = names
                        .into_iter()
                        .zip(&rule.inputs)
                        .map(|(name, role)| ConditionParameter {
                            name,
                            role: role.clone(),
                        })
                        .collect();
                    let person = vocabulary
                        .person_condition_text(&text, &parameters, &BTreeMap::new())
                        .unwrap();
                    if shows_an_identifier(&person, &identifiers) {
                        offending.push(person);
                    }
                }
            }
        }

        assert!(offending.is_empty(), "{offending:?}");
    }

    #[test]
    fn person_formula_takes_the_curriculum_symbols() {
        let symbols = BTreeMap::from([
            ("circle.circumference".to_owned(), "U".to_owned()),
            ("circle.radius".to_owned(), "r".to_owned()),
        ]);

        let formula =
            vocabulary().person_formula_text("circle-circumference/from-radius", &symbols);

        assert_eq!(formula.as_deref(), Some("U = 2 · π · r"));
    }

    #[test]
    fn person_formula_without_symbols_takes_running_names() {
        let formula = vocabulary()
            .person_formula_text(
                "circle-chord/from-radius-and-central-angle",
                &BTreeMap::new(),
            )
            .unwrap();

        assert_eq!(formula, "chord = radius · √(2 - 2 · cos(central angle))");
    }

    #[test]
    fn person_condition_text_names_each_parameter_by_its_role() {
        let parameters = [
            ConditionParameter {
                name: "r".to_owned(),
                role: "circle.radius".to_owned(),
            },
            ConditionParameter {
                name: "t".to_owned(),
                role: "circle.central-angle".to_owned(),
            },
        ];

        let text = vocabulary().person_condition_text(
            "r > 0 and t > 0 and t < 2 * pi",
            &parameters,
            &BTreeMap::new(),
        );

        assert_eq!(
            text.as_deref(),
            Some("radius > 0 and central angle > 0 and central angle < 2 · π")
        );
    }

    fn side_a() -> [ConditionParameter; 1] {
        [ConditionParameter {
            name: "a".to_owned(),
            role: "triangle.side-a".to_owned(),
        }]
    }

    #[test]
    fn two_word_name_as_the_base_of_a_power_keeps_its_grouping() {
        let text = person_text("a^2 + a", &side_a(), |_| "side a".to_owned());

        assert_eq!(text.as_deref(), Some("(side a)² + side a"));
    }

    #[test]
    fn two_word_name_under_a_unary_minus_keeps_its_grouping() {
        let text = person_text("-a", &side_a(), |_| "side a".to_owned());

        assert_eq!(text.as_deref(), Some("-(side a)"));
    }

    #[test]
    fn one_word_name_as_the_base_of_a_power_needs_no_grouping() {
        let text = person_text("a^2", &side_a(), |_| "a".to_owned());

        assert_eq!(text.as_deref(), Some("a²"));
    }

    #[test]
    fn condition_with_a_unit_keeps_a_person_form() {
        let text = person_text("a > 2 m", &side_a(), |_| "side a".to_owned());

        assert!(text.is_some_and(|text| text.starts_with("side a > ")));
    }

    #[test]
    fn person_text_never_shows_a_parameter_name_that_has_a_symbol() {
        let parameters = [ConditionParameter {
            name: "r".to_owned(),
            role: "circle.radius".to_owned(),
        }];

        let text = person_text("r > 0", &parameters, |_| "Radius".to_owned());

        assert_eq!(text.as_deref(), Some("Radius > 0"));
    }
}
