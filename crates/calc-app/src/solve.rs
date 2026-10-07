use std::collections::BTreeSet;
use std::sync::OnceLock;

use calc_concepts::{BoundRelation, ConceptSet, ObjectKind, Role, load_embedded};
use calc_core::{
    BoundKind, BoundRule, Combine, EvaluatedScores, GivenValue, PhaseTwoError, RankError,
    RankedWay, RuleSet, Score, SearchError, SearchOutcome, SearchRequest, Way, evaluate_exact,
    evaluated_scores, not_reached_bounds, rank_ways, search_ways, to_coherent_units,
};
use std::time::Duration;

use calc_core::{
    ComputedResult, ForwardRequest, RoundingError, WayEvaluationError, decide_conditions,
    decide_derivation_conditions, evaluate_way, further_input_entries, list_forward,
};
use calc_exec::{Backend, Domain, Preference};
use calc_expr::{BinderKind, ExprId, ExprPool, NodeView, SymbolId, SymbolKind};
use calc_numbers::Number;
use calc_syntax::{PrintMode, parse_expression, print_expression};
use calc_units::UnitId;

use crate::json::{self, Json};
use crate::platform::Clock;
use crate::result_record::{BackendUse, CalculatorVersion, LineId, ResultRecord};
use crate::session_file::{JsonPath, SaveError, diagnostic_json, unit_text};
use std::collections::HashMap;

use calc_units::Dimension;

use crate::display_units::{DisplayUnit, resolve_display_unit};
use crate::solve_answer::{
    AnswerBody, AnswerBounds, AnswerCondition, AnswerQuantity, AnswerWay, BoundEnd, BoundFrom,
    ConditionParameter, FurtherInput, QuantityMark, ReachableAnswer, ReachableCondition,
    ReachableEntry, SolveAnswer, SolveCriterion, StoredScore, WayListing, answer_json,
    concept_set_version_text, criterion_json, reachable_answer_json,
};
use crate::solve_request::{
    GivenInput, Phase, RequestError, RequestErrorCode, SolveRequest, parse_request,
};
use crate::working_rules::{WorkingRule, derivation_root, evaluated_way, rule_chain};

const ROLE_SEPARATOR: char = '.';
const RULE_SEPARATOR: char = '/';
const PERMUTATION_SEPARATOR: char = '~';
const INVALID_REQUEST_STATUS: &str = "invalid_request";
const ANSWER_LINE_NUMBER: u64 = 1;

#[derive(Clone, Debug, PartialEq)]
pub enum SolveError {
    ConceptSetNotLoaded,
    RulesNotBuilt,
    Search(SearchError),
    PhaseTwo(PhaseTwoError),
    Rank(RankError),
    WayEvaluation(WayEvaluationError),
    Save(SaveError),
}

impl SolveError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ConceptSetNotLoaded => "concept_set_not_loaded",
            Self::RulesNotBuilt => "rules_not_built",
            Self::Search(_) => "search_failed",
            Self::PhaseTwo(_) => "phase_two_failed",
            Self::Rank(_) => "ranking_failed",
            Self::WayEvaluation(_) => "way_evaluation_failed",
            Self::Save(_) => "answer_not_written",
        }
    }
}

pub(crate) fn typed_error_value(code: &str, data: &[(&str, &str)]) -> Json {
    Json::object(vec![
        ("code", Json::string(code)),
        (
            "data",
            Json::object(
                data.iter()
                    .map(|(name, value)| (*name, Json::string(value)))
                    .collect(),
            ),
        ),
    ])
}

pub fn typed_error_json(code: &str, data: &[(&str, &str)]) -> Vec<u8> {
    json::write_canonical(&typed_error_value(code, data)).into_bytes()
}

#[derive(Clone, Debug, PartialEq)]
pub enum SolveFailure {
    InvalidRequest(RequestError),
    Internal(SolveError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyStatus {
    Reachable,
    Ways,
    Front,
    Solved,
    NotReached,
    InvalidRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolveReply {
    pub status: ReplyStatus,
    pub json: Vec<u8>,
}

pub struct EvaluationContext<'a> {
    pub backends: &'a [&'a dyn Backend],
    pub clock: &'a dyn Clock,
}

struct EvaluatedWay {
    holds: Vec<Option<bool>>,
    record: Option<Box<ResultRecord>>,
}

struct WayContext<'a> {
    concepts: &'a ConceptSet,
    rule_set: &'a RuleSet,
    wanted: &'a str,
    given: &'a [GivenValue],
    unit: Option<UnitId>,
    preference: Preference,
    backends: &'a [&'a dyn Backend],
    clock: &'a dyn Clock,
}

struct Resolved<'set> {
    object: &'set ObjectKind,
    wanted: Option<String>,
    given: Vec<(String, GivenValue)>,
    obtainable: Vec<String>,
    not_obtainable: Vec<String>,
}

pub(crate) fn concept_set() -> Result<&'static ConceptSet, SolveError> {
    static CONCEPTS: OnceLock<Option<ConceptSet>> = OnceLock::new();
    CONCEPTS
        .get_or_init(|| load_embedded().ok())
        .as_ref()
        .ok_or(SolveError::ConceptSetNotLoaded)
}

fn invalid(code: RequestErrorCode, path: JsonPath) -> SolveFailure {
    SolveFailure::InvalidRequest(RequestError::new(code, path))
}

fn role_identifier(object: &ObjectKind, name: &str) -> String {
    format!("{}{ROLE_SEPARATOR}{name}", object.identifier)
}

fn role_name(identifier: &str) -> String {
    identifier
        .split_once(ROLE_SEPARATOR)
        .map_or(identifier, |(_, name)| name)
        .to_owned()
}

fn find_role<'set>(object: &'set ObjectKind, name: &str) -> Option<&'set Role> {
    let identifier = role_identifier(object, name);
    object
        .roles
        .iter()
        .find(|role| role.identifier == identifier)
}

fn resolve_object<'set>(
    concepts: &'set ConceptSet,
    request: &SolveRequest,
) -> Result<&'set ObjectKind, SolveFailure> {
    let root = JsonPath::default();
    if let Some(name) = &request.object {
        return concepts
            .objects
            .iter()
            .find(|object| &object.identifier == name)
            .ok_or_else(|| invalid(RequestErrorCode::UnknownObject, root.member("object")));
    }
    let Some(wanted) = request.wanted.as_deref() else {
        return object_of_marked_roles(concepts, request);
    };
    let candidates: Vec<&ObjectKind> = concepts
        .objects
        .iter()
        .filter(|object| find_role(object, wanted).is_some())
        .collect();
    match candidates.as_slice() {
        [object] => Ok(object),
        [] => Err(invalid(
            RequestErrorCode::UnknownWanted,
            root.member("wanted"),
        )),
        _ => Err(invalid(
            RequestErrorCode::AmbiguousWanted,
            root.member("wanted"),
        )),
    }
}

fn object_of_marked_roles<'set>(
    concepts: &'set ConceptSet,
    request: &SolveRequest,
) -> Result<&'set ObjectKind, SolveFailure> {
    let root = JsonPath::default();
    let named: Vec<(String, JsonPath)> = request
        .given
        .iter()
        .enumerate()
        .map(|(position, given)| {
            (
                given.name.clone(),
                root.member("given").index(position).member("name"),
            )
        })
        .chain(
            request
                .obtainable
                .iter()
                .enumerate()
                .map(|(position, name)| (name.clone(), root.member("obtainable").index(position))),
        )
        .collect();
    if let Some((_, path)) = named.iter().find(|(name, _)| {
        !concepts
            .objects
            .iter()
            .any(|object| find_role(object, name).is_some())
    }) {
        return Err(invalid(RequestErrorCode::UnknownRole, path.clone()));
    }
    let candidates: Vec<&ObjectKind> = concepts
        .objects
        .iter()
        .filter(|object| {
            named
                .iter()
                .all(|(name, _)| find_role(object, name).is_some())
        })
        .collect();
    match candidates.as_slice() {
        [object] => Ok(object),
        _ => Err(invalid(
            RequestErrorCode::AmbiguousObject,
            root.member("object"),
        )),
    }
}

fn missing_wanted() -> SolveFailure {
    invalid(
        RequestErrorCode::MissingWanted,
        JsonPath::default().member("wanted"),
    )
}

fn is_negative(number: &Number) -> bool {
    match number {
        Number::Integer(integer) => integer.is_negative(),
        Number::Rational(rational) => rational.numerator().is_negative(),
        Number::F32(value) => value.is_sign_negative(),
        Number::F64(value) => value.is_sign_negative(),
    }
}

fn coherent_number(
    pool: &mut ExprPool,
    text: &str,
    unit: Option<&str>,
    role: &Role,
    value_path: &JsonPath,
    unit_path: &JsonPath,
) -> Result<Number, SolveFailure> {
    parse_expression(pool, text)
        .map_err(|_| invalid(RequestErrorCode::InvalidValue, value_path.clone()))?;
    let source = match unit {
        Some(unit) => format!("({text}) {unit}"),
        None => text.to_owned(),
    };
    let expression = parse_expression(pool, &source)
        .map_err(|_| invalid(RequestErrorCode::InvalidValue, unit_path.clone()))?;
    let coherent = to_coherent_units(pool, expression)
        .map_err(|_| invalid(RequestErrorCode::InvalidValue, unit_path.clone()))?;
    if coherent.dimension != role.dimension {
        return Err(invalid(
            RequestErrorCode::DimensionMismatch,
            unit_path.clone(),
        ));
    }
    evaluate_exact(pool, coherent.expression)
        .ok()
        .and_then(|evaluation| evaluation.rational_value().cloned())
        .ok_or_else(|| invalid(RequestErrorCode::InvalidValue, value_path.clone()))
}

fn given_value(
    pool: &mut ExprPool,
    object: &ObjectKind,
    given: &GivenInput,
    path: &JsonPath,
) -> Result<(String, GivenValue), SolveFailure> {
    let role = find_role(object, &given.name)
        .ok_or_else(|| invalid(RequestErrorCode::UnknownRole, path.member("name")))?;
    let unit_path = path.member("unit");
    let value = coherent_number(
        pool,
        &given.value,
        given.unit.as_deref(),
        role,
        &path.member("value"),
        &unit_path,
    )?;
    let standard_uncertainty = match &given.uncertainty {
        None => None,
        Some(text) => {
            let uncertainty_path = path.member("uncertainty");
            let uncertainty = coherent_number(
                pool,
                text,
                given.unit.as_deref(),
                role,
                &uncertainty_path,
                &unit_path,
            )?;
            if is_negative(&uncertainty) {
                return Err(invalid(RequestErrorCode::InvalidValue, uncertainty_path));
            }
            let factor_path = path.member("coverage_factor");
            let factor = match &given.coverage_factor {
                None => Number::from(1_i64),
                Some(text) => parse_expression(pool, text)
                    .ok()
                    .and_then(|expression| evaluate_exact(pool, expression).ok())
                    .and_then(|evaluation| evaluation.rational_value().cloned())
                    .filter(|factor| {
                        factor
                            .sub_exact(&Number::from(1_i64))
                            .is_ok_and(|excess| !is_negative(&excess))
                    })
                    .ok_or_else(|| invalid(RequestErrorCode::InvalidValue, factor_path.clone()))?,
            };
            Some(
                uncertainty
                    .div_exact(&factor)
                    .map_err(|_| invalid(RequestErrorCode::InvalidValue, factor_path))?,
            )
        }
    };
    Ok((
        role.identifier.clone(),
        GivenValue {
            quantity: role.identifier.clone(),
            value,
            standard_uncertainty,
        },
    ))
}

fn marked_roles(
    object: &ObjectKind,
    names: &[String],
    member: &str,
) -> Result<Vec<String>, SolveFailure> {
    names
        .iter()
        .enumerate()
        .map(|(position, name)| {
            find_role(object, name)
                .map(|role| role.identifier.clone())
                .ok_or_else(|| {
                    invalid(
                        RequestErrorCode::UnknownRole,
                        JsonPath::default().member(member).index(position),
                    )
                })
        })
        .collect()
}

fn resolve<'set>(
    pool: &mut ExprPool,
    concepts: &'set ConceptSet,
    request: &SolveRequest,
) -> Result<Resolved<'set>, SolveFailure> {
    let object = resolve_object(concepts, request)?;
    let wanted = request
        .wanted
        .as_deref()
        .map(|name| {
            find_role(object, name)
                .map(|role| role.identifier.clone())
                .ok_or_else(|| {
                    invalid(
                        RequestErrorCode::UnknownWanted,
                        JsonPath::default().member("wanted"),
                    )
                })
        })
        .transpose()?;
    let given_path = JsonPath::default().member("given");
    let given = request
        .given
        .iter()
        .enumerate()
        .map(|(position, given)| given_value(pool, object, given, &given_path.index(position)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Resolved {
        object,
        wanted,
        given,
        obtainable: marked_roles(object, &request.obtainable, "obtainable")?,
        not_obtainable: marked_roles(object, &request.not_obtainable, "not_obtainable")?,
    })
}

fn stored_score(score: &Score) -> Option<StoredScore> {
    match score {
        Score::Count(count) => Some(StoredScore::Count(*count)),
        Score::Exactness(exactness) => Some(StoredScore::Exactness(*exactness)),
        Score::ErrorBound(bound) => Some(StoredScore::ErrorBound(*bound)),
        Score::Rational(value) => Some(StoredScore::Exact(value.clone())),
        Score::Unknown => None,
    }
}

fn condition_text(
    pool: &ExprPool,
    concepts: &ConceptSet,
    rule_set: &RuleSet,
    rule_identifier: &str,
    condition: ExprId,
) -> (String, Option<Vec<ConditionParameter>>) {
    let rule = rule_set
        .rules
        .iter()
        .find(|rule| rule.identifier == rule_identifier);
    let written = rule.and_then(|rule| {
        let position = rule
            .conditions
            .iter()
            .position(|stated| *stated == condition)?;
        let way_identifier = rule_identifier
            .split_once(PERMUTATION_SEPARATOR)
            .map_or(rule_identifier, |(way, _)| way);
        concepts
            .concepts
            .iter()
            .flat_map(|concept| concept.ways.iter())
            .find(|way| way.identifier == way_identifier)
            .and_then(|way| way.conditions.get(position))
            .cloned()
    });
    let Some((source, rule)) = written.zip(rule) else {
        return (
            print_expression(pool, condition, PrintMode::Ascii).unwrap_or_default(),
            None,
        );
    };
    match source_body(&source, rule.inputs.len()) {
        Some((text, names)) => {
            let parameters = names
                .into_iter()
                .zip(&rule.inputs)
                .map(|(name, role)| ConditionParameter {
                    name,
                    role: role.clone(),
                })
                .collect();
            (text, Some(parameters))
        }
        None => (String::new(), None),
    }
}

pub(crate) fn source_body(source: &str, parameters: usize) -> Option<(String, Vec<String>)> {
    let mut own_pool = ExprPool::new();
    let parsed = parse_expression(&mut own_pool, source).ok()?;
    let (opened, names) = opened_lambda_body(&mut own_pool, parsed, parameters)?;
    let text = print_expression(&own_pool, opened, PrintMode::Ascii).ok()?;
    Some((text, names))
}

pub(crate) fn opened_lambda_body(
    pool: &mut ExprPool,
    function: ExprId,
    parameters: usize,
) -> Option<(ExprId, Vec<String>)> {
    let mut names = Vec::new();
    let mut symbols = Vec::new();
    let mut body = function;
    while names.len() < parameters {
        let NodeView::Bind {
            binder: BinderKind::Lambda,
            body: inner,
            ..
        } = pool.node(body).ok()?
        else {
            return None;
        };
        let name = pool.bound_name(body)?.to_owned();
        symbols.push(pool.intern_symbol(&name, SymbolKind::Variable).ok()?);
        names.push(name);
        body = inner;
    }
    let opened = open_bound(pool, body, &symbols, 0)?;
    Some((opened, names))
}

#[cfg(test)]
fn lambda_body_text(pool: &mut ExprPool, function: ExprId, parameters: usize) -> Option<String> {
    let (opened, _) = opened_lambda_body(pool, function, parameters)?;
    print_expression(pool, opened, PrintMode::Ascii).ok()
}

fn open_bound(
    pool: &mut ExprPool,
    expression: ExprId,
    names: &[SymbolId],
    depth: u32,
) -> Option<ExprId> {
    let opened = match pool.node(expression).ok()? {
        NodeView::Number(_) | NodeView::Symbol(_) => expression,
        NodeView::Bound(index) if index < depth => expression,
        NodeView::Bound(index) => {
            let outer = usize::try_from(index - depth).ok()?;
            match names.len().checked_sub(outer + 1) {
                Some(position) => pool.symbol(names[position]).ok()?,
                None => {
                    let count = u32::try_from(names.len()).ok()?;
                    pool.bound(index - count).ok()?
                }
            }
        }
        NodeView::Apply { head, arguments } => {
            let arguments = arguments.to_vec();
            let opened = arguments
                .into_iter()
                .map(|argument| open_bound(pool, argument, names, depth))
                .collect::<Option<Vec<_>>>()?;
            pool.apply(head, &opened).ok()?
        }
        NodeView::Bind {
            binder,
            arguments,
            body,
        } => {
            let arguments = arguments.to_vec();
            let opened_arguments = arguments
                .into_iter()
                .map(|argument| open_bound(pool, argument, names, depth))
                .collect::<Option<Vec<_>>>()?;
            let opened_body = open_bound(pool, body, names, depth + 1)?;
            let name = pool.bound_name(expression).map(str::to_owned);
            let bound = pool.bind(binder, &opened_arguments, opened_body).ok()?;
            if let Some(name) = name {
                pool.record_bound_name(bound, &name).ok()?;
            }
            bound
        }
        NodeView::Quantity { value, unit } => {
            let value = open_bound(pool, value, names, depth)?;
            pool.quantity(value, unit).ok()?
        }
        NodeView::Array { shape, elements } => {
            let shape = shape.to_vec();
            let elements = elements.to_vec();
            let opened = elements
                .into_iter()
                .map(|element| open_bound(pool, element, names, depth))
                .collect::<Option<Vec<_>>>()?;
            pool.array(&shape, &opened).ok()?
        }
    };
    Some(opened)
}

fn with_unit(computed: ComputedResult, unit: Option<UnitId>) -> ComputedResult {
    let Ok(mut rebuilt) = ComputedResult::new(
        computed.kind(),
        computed.value().clone(),
        unit,
        computed.method().clone(),
    ) else {
        return computed;
    };
    if let RoundingError::Bound(bound) = computed.rounding_error() {
        match rebuilt.clone().with_rounding_bound(bound.clone()) {
            Ok(bounded) => rebuilt = bounded,
            Err(_) => return computed,
        }
    }
    if let Some(uncertainty) = computed.uncertainty() {
        rebuilt = rebuilt.with_uncertainty(uncertainty.clone());
    }
    if let Some(seed) = computed.seed() {
        rebuilt = rebuilt.with_seed(seed.clone());
    }
    let rebuilt = computed
        .corpus_references()
        .fold(rebuilt, |result, reference| {
            result.with_corpus_reference(reference)
        });
    computed
        .sources()
        .fold(rebuilt, |result, source| result.with_source(source))
}

fn way_record(
    pool: &mut ExprPool,
    context: &WayContext<'_>,
    way: &Way,
    evaluable: bool,
) -> Result<EvaluatedWay, SolveError> {
    let undecided = vec![None; way.conditions.len()];
    if !evaluable {
        return Ok(EvaluatedWay {
            holds: undecided,
            record: None,
        });
    }
    let outcomes = decide_conditions(pool, context.rule_set, context.wanted, way, context.given)
        .map_err(SolveError::PhaseTwo)?;
    let holds: Vec<Option<bool>> = way
        .conditions
        .iter()
        .map(|condition| {
            outcomes
                .iter()
                .find(|outcome| {
                    outcome.rule == condition.rule && outcome.condition == condition.condition
                })
                .and_then(|outcome| outcome.holds)
        })
        .collect();
    if holds.iter().any(|holds| *holds != Some(true)) {
        return Ok(EvaluatedWay {
            holds,
            record: None,
        });
    }
    let started = context.clock.monotonic_nanoseconds();
    let evaluation = evaluate_way(
        pool,
        context.rule_set,
        context.wanted,
        way,
        context.given,
        context.backends,
        context.preference,
    )
    .map_err(SolveError::WayEvaluation)?;
    let finished = context.clock.monotonic_nanoseconds();
    let computed = way
        .derivation
        .iter()
        .filter_map(|rule| rule.split_once(RULE_SEPARATOR).map(|(concept, _)| concept))
        .filter_map(|concept| {
            context
                .concepts
                .concepts
                .iter()
                .find(|node| node.identifier == concept)
        })
        .flat_map(|node| node.rests_on.iter())
        .fold(
            with_unit(evaluation.result, context.unit),
            |result, reference| result.with_corpus_reference(reference),
        );
    let backend = BackendUse {
        preference: context.preference,
        width: Domain::F64,
        selected: evaluation.backend,
        skipped: evaluation.skipped.iter().map(Into::into).collect(),
        approximate_operations: evaluation.approximate_operations,
        modes: evaluation.backend.map_or_else(Vec::new, |selected| {
            context
                .backends
                .iter()
                .find(|backend| backend.kind() == selected)
                .map_or_else(Vec::new, |backend| BackendUse::modes_of(*backend))
        }),
    };
    let record = ResultRecord::new(
        computed,
        evaluation
            .backend
            .is_some()
            .then_some(backend)
            .into_iter()
            .collect(),
        [],
        context.clock.now_utc(),
        Duration::from_nanos(finished.saturating_sub(started)),
        CalculatorVersion::current(),
    );
    Ok(EvaluatedWay {
        holds,
        record: Some(Box::new(record)),
    })
}

fn answer_way(
    pool: &mut ExprPool,
    context: &WayContext<'_>,
    way: &Way,
    ranked: &RankedWay,
    rank: Option<u64>,
    evaluable: bool,
) -> Result<AnswerWay, SolveError> {
    let rule_set = context.rule_set;
    let EvaluatedWay { holds, record } = way_record(pool, context, way, evaluable)?;
    let conditions = way
        .conditions
        .iter()
        .zip(holds)
        .map(|(condition, holds)| {
            let (text, parameters) = condition_text(
                pool,
                context.concepts,
                rule_set,
                &condition.rule,
                condition.condition,
            );
            AnswerCondition {
                text,
                parameters,
                holds,
            }
        })
        .collect();
    Ok(AnswerWay {
        rank,
        layer: ranked.layer.map(u64::from),
        rule: way.last_rule.clone(),
        derivation: way.derivation.clone(),
        inputs: way.inputs.iter().map(|input| role_name(input)).collect(),
        obtainable: way.obtainable,
        scores: ranked.scores.iter().map(stored_score).collect(),
        conditions,
        sources: way.sources.clone(),
        record,
    })
}

fn bound_rules(pool: &mut ExprPool, concepts: &ConceptSet) -> Result<Vec<BoundRule>, SolveError> {
    Ok(concepts
        .bounds(pool)
        .map_err(|_| SolveError::RulesNotBuilt)?
        .into_iter()
        .map(|bound| BoundRule {
            identifier: bound.identifier,
            role: bound.role,
            inputs: bound.inputs,
            kind: match bound.relation {
                BoundRelation::AtMost => BoundKind::AtMost,
                BoundRelation::AtLeast => BoundKind::AtLeast,
                BoundRelation::StrictlyBetween => BoundKind::StrictlyBetween,
            },
            formulas: bound.expressions,
            conditions: bound.conditions,
            sources: bound.sources,
        })
        .collect())
}

fn quantities(resolved: &Resolved<'_>, listed: &BTreeSet<String>) -> Vec<AnswerQuantity> {
    let mut names: BTreeSet<String> = listed.clone();
    names.extend(resolved.wanted.iter().cloned());
    names.extend(resolved.given.iter().map(|(quantity, _)| quantity.clone()));
    names.extend(resolved.obtainable.iter().cloned());
    names.extend(resolved.not_obtainable.iter().cloned());
    let mut quantities: Vec<AnswerQuantity> = names
        .into_iter()
        .map(|quantity| {
            let given = resolved
                .given
                .iter()
                .find(|(given_quantity, _)| *given_quantity == quantity);
            let mark = if given.is_some() {
                QuantityMark::Given
            } else if resolved.obtainable.contains(&quantity) {
                QuantityMark::Obtainable
            } else if resolved.not_obtainable.contains(&quantity) {
                QuantityMark::NotObtainable
            } else {
                QuantityMark::Unmarked
            };
            AnswerQuantity {
                name: role_name(&quantity),
                mark,
                value: given.map(|(_, value)| calc_core::ResultValue::Number(value.value.clone())),
            }
        })
        .collect();
    quantities.sort_by(|left, right| left.name.cmp(&right.name));
    quantities
}

fn coherent_unit_text(pool: &mut ExprPool, role: &Role) -> Option<String> {
    let unit = pool.units_mut().coherent_unit(&role.dimension).ok()?;
    unit_text(pool, unit).ok().flatten()
}

pub(crate) struct SolveWorking {
    pub rules: Vec<WorkingRule>,
    pub expression: ExprId,
}

fn role_units(
    pool: &mut ExprPool,
    object: &ObjectKind,
    request: &SolveRequest,
) -> HashMap<String, (UnitId, UnitId)> {
    let mut written: HashMap<Dimension, UnitId> = HashMap::new();
    let mut units = HashMap::new();
    for given in &request.given {
        let Some(role) = find_role(object, &given.name) else {
            continue;
        };
        let Some(text) = given.unit.as_deref() else {
            continue;
        };
        let Some(DisplayUnit::Unit(unit)) = resolve_display_unit(pool, text, None) else {
            continue;
        };
        if pool.units().dimension(unit).ok() == Some(role.dimension) {
            written.insert(role.dimension, unit);
        }
    }
    for role in &object.roles {
        let Ok(coherent) = pool.units_mut().coherent_unit(&role.dimension) else {
            continue;
        };
        let target = written.get(&role.dimension).copied().unwrap_or(coherent);
        units.insert(role.identifier.clone(), (coherent, target));
    }
    units
}

pub(crate) fn solve_working(
    pool: &mut ExprPool,
    request: &SolveRequest,
    evaluation: &EvaluationContext<'_>,
) -> Option<SolveWorking> {
    let concepts = concept_set().ok()?;
    let answer = answer_request(pool, request, evaluation).ok()?;
    let way = evaluated_way(&answer)?;
    let resolved = resolve(pool, concepts, request).ok()?;
    let rule_set = concepts.rule_set(pool).ok()?;
    let wanted = resolved.wanted.clone()?;
    let given: Vec<GivenValue> = resolved
        .given
        .iter()
        .map(|(_, value)| value.clone())
        .collect();
    let derivation = way.derivation.clone();
    let text_of = |pool: &mut ExprPool, rule: &str, condition: ExprId| {
        let (text, parameters) = condition_text(pool, concepts, &rule_set, rule, condition);
        (text, parameters.unwrap_or_default())
    };
    let units = role_units(pool, resolved.object, request);
    let rules = rule_chain(
        pool,
        &rule_set,
        &wanted,
        &derivation,
        &given,
        &units,
        &text_of,
    )?;
    let expression = derivation_root(pool, &rule_set, &wanted, &derivation, &given)?;
    Some(SolveWorking { rules, expression })
}

pub(crate) fn answer_reachable(
    pool: &mut ExprPool,
    request: &SolveRequest,
) -> Result<ReachableAnswer, SolveFailure> {
    let concepts = concept_set().map_err(SolveFailure::Internal)?;
    let resolved = resolve(pool, concepts, request)?;
    let rule_set = concepts
        .rule_set(pool)
        .map_err(|_| SolveFailure::Internal(SolveError::RulesNotBuilt))?;
    let given_values: Vec<GivenValue> = resolved
        .given
        .iter()
        .map(|(_, value)| value.clone())
        .collect();
    let forward = ForwardRequest {
        given: resolved
            .given
            .iter()
            .map(|(quantity, _)| quantity.clone())
            .collect(),
        obtainable: resolved.obtainable.clone(),
        cap: request.cap,
    };
    let listing = list_forward(&rule_set, &forward)
        .map_err(|error| SolveFailure::Internal(SolveError::Search(error)))?;
    let mut entries = Vec::new();
    for entry in &listing.entries {
        let outcomes = decide_derivation_conditions(
            pool,
            &rule_set,
            &entry.name,
            &entry.derivation,
            &given_values,
        )
        .map_err(|error| SolveFailure::Internal(SolveError::PhaseTwo(error)))?;
        let conditions = entry
            .conditions
            .iter()
            .map(|condition| {
                let (text, parameters) = condition_text(
                    pool,
                    concepts,
                    &rule_set,
                    &condition.rule,
                    condition.condition,
                );
                (condition, text, parameters)
            })
            .map(|(condition, text, parameters)| ReachableCondition {
                rule: condition.rule.clone(),
                text,
                parameters,
                holds: outcomes
                    .iter()
                    .find(|outcome| {
                        outcome.rule == condition.rule && outcome.condition == condition.condition
                    })
                    .and_then(|outcome| outcome.holds),
            })
            .collect();
        entries.push(ReachableEntry {
            name: role_name(&entry.name),
            steps: u64::from(entry.steps),
            rule: entry.rule.clone(),
            inputs: entry.inputs.iter().map(|input| role_name(input)).collect(),
            derivation: entry.derivation.clone(),
            uses: entry.uses.iter().map(|role| role_name(role)).collect(),
            conditions,
            needs_obtainable: entry.needs_obtainable,
            sources: entry.sources.clone(),
        });
    }
    let listed: BTreeSet<String> = listing
        .entries
        .iter()
        .map(|entry| entry.name.clone())
        .collect();
    Ok(ReachableAnswer {
        concept_set_version: concepts.version,
        object: resolved.object.identifier.clone(),
        truncated: listing.truncated,
        reachable_total: u64::from(listing.reachable_total),
        entries,
        quantities: quantities(&resolved, &listed),
    })
}

pub(crate) fn answer_request(
    pool: &mut ExprPool,
    request: &SolveRequest,
    evaluation: &EvaluationContext<'_>,
) -> Result<SolveAnswer, SolveFailure> {
    let concepts = concept_set().map_err(SolveFailure::Internal)?;
    let resolved = resolve(pool, concepts, request)?;
    let wanted = resolved.wanted.clone().ok_or_else(missing_wanted)?;
    let wanted_name = request.wanted.as_deref().ok_or_else(missing_wanted)?;
    let rule_set = concepts
        .rule_set(pool)
        .map_err(|_| SolveFailure::Internal(SolveError::RulesNotBuilt))?;
    let given_values: Vec<GivenValue> = resolved
        .given
        .iter()
        .map(|(_, value)| value.clone())
        .collect();
    let given_quantities: Vec<String> = resolved
        .given
        .iter()
        .map(|(quantity, _)| quantity.clone())
        .collect();
    let search = SearchRequest {
        wanted: wanted.clone(),
        given: given_quantities.clone(),
        obtainable: resolved.obtainable.clone(),
        not_obtainable: resolved.not_obtainable.clone(),
        only_obtainable: request.only_obtainable,
        way_cap: request.cap,
        work_budget: request.work_budget,
    };
    let outcome = search_ways(pool, &rule_set, &search)
        .map_err(|error| SolveFailure::Internal(SolveError::Search(error)))?;
    let criterion = request.criterion.clone();
    let body = match outcome {
        SearchOutcome::NotReached { missing_any_of, .. } => {
            let rules = bound_rules(pool, concepts).map_err(SolveFailure::Internal)?;
            let bounds = not_reached_bounds(pool, &wanted, &rules, &given_values)
                .map_err(|error| SolveFailure::Internal(SolveError::PhaseTwo(error)))?;
            let wanted_role = find_role(resolved.object, wanted_name);
            let unit = wanted_role.and_then(|role| coherent_unit_text(pool, role));
            let listed: BTreeSet<String> = missing_any_of.iter().flatten().cloned().collect();
            let entries = further_input_entries(
                pool,
                &rule_set,
                &wanted,
                &given_values,
                &resolved.obtainable,
                &missing_any_of,
            )
            .map_err(|error| SolveFailure::Internal(SolveError::PhaseTwo(error)))?;
            let further_inputs = entries
                .iter()
                .map(|entry| FurtherInput {
                    add: entry
                        .add
                        .iter()
                        .map(|quantity| role_name(quantity))
                        .collect(),
                    rule: Some(entry.rule.clone()),
                    derivation: Some(entry.derivation.clone()),
                    between: None,
                    conditions: Some(
                        entry
                            .conditions
                            .iter()
                            .map(|outcome| {
                                let (text, parameters) = condition_text(
                                    pool,
                                    concepts,
                                    &rule_set,
                                    &outcome.rule,
                                    outcome.condition,
                                );
                                ReachableCondition {
                                    rule: outcome.rule.clone(),
                                    text,
                                    parameters,
                                    holds: outcome.holds,
                                }
                            })
                            .collect(),
                    ),
                })
                .collect();
            let body = AnswerBody::NotReached {
                missing_any_of: further_inputs,
                bounds: bounds.map(|bounds| {
                    let end = |end: Option<calc_core::BoundEnd>| {
                        end.map(|end| BoundEnd {
                            value: calc_core::ResultValue::Number(end.value),
                            inclusive: end.inclusive,
                        })
                    };
                    Box::new(AnswerBounds {
                        lower: end(bounds.lower),
                        upper: end(bounds.upper),
                        unit,
                        sources: bounds.sources,
                        from: Some(
                            bounds
                                .from
                                .into_iter()
                                .map(|origin| BoundFrom {
                                    bound: origin.bound,
                                    kind: origin.kind,
                                    inputs: origin
                                        .inputs
                                        .iter()
                                        .map(|input| role_name(input))
                                        .collect(),
                                    ends: origin.ends,
                                })
                                .collect(),
                        ),
                    })
                }),
            };
            return Ok(SolveAnswer {
                concept_set_version: concepts.version,
                criterion,
                body,
                quantities: quantities(&resolved, &listed),
            });
        }
        SearchOutcome::Reached(list) => list,
    };
    let evaluable: Vec<bool> = body
        .ways
        .iter()
        .map(|way| {
            request.phase == Phase::Evaluate
                && way
                    .inputs
                    .iter()
                    .all(|input| given_quantities.contains(input))
        })
        .collect();
    let evaluated = body
        .ways
        .iter()
        .zip(&evaluable)
        .map(|(way, is_evaluable)| {
            if *is_evaluable {
                evaluated_scores(pool, &rule_set, &wanted, way, &given_values)
            } else {
                Ok(EvaluatedScores::default())
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| SolveFailure::Internal(SolveError::PhaseTwo(error)))?;
    let ranked = rank_ways(&body.ways, &criterion.by, criterion.combine, &evaluated)
        .map_err(|error| SolveFailure::Internal(SolveError::Rank(error)))?;
    let listed: BTreeSet<String> = body
        .ways
        .iter()
        .flat_map(|way| way.inputs.iter().cloned())
        .collect();
    let wanted_unit = find_role(resolved.object, wanted_name)
        .and_then(|role| pool.units_mut().coherent_unit(&role.dimension).ok());
    let context = WayContext {
        concepts,
        rule_set: &rule_set,
        wanted: &wanted,
        given: &given_values,
        unit: wanted_unit,
        preference: request.backend,
        backends: evaluation.backends,
        clock: evaluation.clock,
    };
    let mut answer_ways = Vec::new();
    for (position, ranked_way) in ranked.iter().enumerate() {
        let Some(way) = body.ways.get(ranked_way.way) else {
            continue;
        };
        let rank = match criterion.combine {
            Combine::Order => u64::try_from(position + 1).ok(),
            Combine::Front => None,
        };
        let is_evaluable = evaluable.get(ranked_way.way).copied().unwrap_or(false);
        answer_ways.push(
            answer_way(pool, &context, way, ranked_way, rank, is_evaluable)
                .map_err(SolveFailure::Internal)?,
        );
    }
    let listing = WayListing {
        truncated_by: body.truncated_by,
        complete_through_steps: u64::from(body.complete_through_steps),
        ways_found: u64::from(body.ways_found),
    };
    let body = match criterion.combine {
        Combine::Front => {
            let (front, dominated) = answer_ways
                .into_iter()
                .partition(|way| way.layer == Some(1));
            AnswerBody::Front {
                listing,
                front,
                dominated,
            }
        }
        Combine::Order if answer_ways.iter().any(|way| way.record.is_some()) => {
            AnswerBody::Solved {
                listing,
                ways: answer_ways,
            }
        }
        Combine::Order => AnswerBody::Ways {
            listing,
            ways: answer_ways,
        },
    };
    Ok(SolveAnswer {
        concept_set_version: concepts.version,
        criterion,
        body,
        quantities: quantities(&resolved, &listed),
    })
}

fn invalid_request_json(
    error: &RequestError,
    criterion: &SolveCriterion,
    version: u64,
) -> Result<Vec<u8>, SolveError> {
    let line = LineId::from_number(ANSWER_LINE_NUMBER).ok_or(SolveError::RulesNotBuilt)?;
    let error_json = diagnostic_json(&error.diagnostic(), line).map_err(SolveError::Save)?;
    Ok(json::write_canonical(&Json::object(vec![
        ("status", Json::string(INVALID_REQUEST_STATUS)),
        (
            "concept_set_version",
            Json::String(concept_set_version_text(version)),
        ),
        ("criterion", criterion_json(criterion)),
        ("error", error_json),
    ]))
    .into_bytes())
}

pub fn solve(request: &[u8], evaluation: &EvaluationContext<'_>) -> Result<SolveReply, SolveError> {
    let concepts = concept_set()?;
    let parsed = match parse_request(request) {
        Ok(parsed) => parsed,
        Err(error) => {
            return Ok(SolveReply {
                status: ReplyStatus::InvalidRequest,
                json: invalid_request_json(
                    &error,
                    &SolveCriterion::program_default(),
                    concepts.version,
                )?,
            });
        }
    };
    let mut pool = ExprPool::new();
    let line = LineId::from_number(ANSWER_LINE_NUMBER).ok_or(SolveError::RulesNotBuilt)?;
    if parsed.phase == Phase::Reachable {
        return match answer_reachable(&mut pool, &parsed) {
            Ok(answer) => Ok(SolveReply {
                status: ReplyStatus::Reachable,
                json: json::write_canonical(
                    &reachable_answer_json(&answer, line).map_err(SolveError::Save)?,
                )
                .into_bytes(),
            }),
            Err(SolveFailure::InvalidRequest(error)) => Ok(SolveReply {
                status: ReplyStatus::InvalidRequest,
                json: invalid_request_json(&error, &parsed.criterion, concepts.version)?,
            }),
            Err(SolveFailure::Internal(error)) => Err(error),
        };
    }
    let answer = match answer_request(&mut pool, &parsed, evaluation) {
        Ok(answer) => answer,
        Err(SolveFailure::InvalidRequest(error)) => {
            return Ok(SolveReply {
                status: ReplyStatus::InvalidRequest,
                json: invalid_request_json(&error, &parsed.criterion, concepts.version)?,
            });
        }
        Err(SolveFailure::Internal(error)) => return Err(error),
    };
    let status = match answer.body {
        AnswerBody::Ways { .. } => ReplyStatus::Ways,
        AnswerBody::Front { .. } => ReplyStatus::Front,
        AnswerBody::Solved { .. } => ReplyStatus::Solved,
        AnswerBody::NotReached { .. } => ReplyStatus::NotReached,
    };
    let json = answer_json(&pool, &answer, line).map_err(SolveError::Save)?;
    Ok(SolveReply {
        status,
        json: json::write_canonical(&json).into_bytes(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::tests::{fixed_clock, session};
    use crate::session::{Outcome, RoleMark, Session, SessionError};
    use calc_core::Criterion;

    const CIRCUMFERENCE: &str =
        r#"{"phase": "ways", "object": "circle", "wanted": "circumference"}"#;

    fn reply(request: &str) -> SolveReply {
        let backends = crate::registered_backends();
        let backend_references: Vec<&dyn Backend> = backends.iter().map(AsRef::as_ref).collect();
        let clock = fixed_clock();
        let evaluation = EvaluationContext {
            backends: &backend_references,
            clock: clock.as_ref(),
        };
        solve(request.as_bytes(), &evaluation).unwrap()
    }

    fn text(reply: &SolveReply) -> String {
        String::from_utf8(reply.json.clone()).unwrap()
    }

    fn request(text: &str) -> SolveRequest {
        parse_request(text.as_bytes()).unwrap()
    }

    fn answer_of(session: &Session, id: LineId) -> SolveAnswer {
        match session.line(id).unwrap().outcome() {
            Outcome::Answer(answer) => (**answer).clone(),
            other => panic!("expected an answer, found {other:?}"),
        }
    }

    fn ways_of(answer: &SolveAnswer) -> &[AnswerWay] {
        match &answer.body {
            AnswerBody::Ways { ways, .. } | AnswerBody::Solved { ways, .. } => ways,
            AnswerBody::Front { front, .. } => front,
            AnswerBody::NotReached { .. } => &[],
        }
    }

    fn circumference_line(session: &mut Session) -> LineId {
        session.enter_solve(&request(CIRCUMFERENCE)).unwrap()
    }

    #[test]
    fn solve_lists_the_ways_to_the_circumference_best_first() {
        let answer = reply(CIRCUMFERENCE);
        assert_eq!(answer.status, ReplyStatus::Ways);
        assert!(text(&answer).contains("\"rule\": \"circle-circumference/from-diameter\","));
    }

    #[test]
    fn typed_error_is_canonical_json_with_code_and_data() {
        assert_eq!(
            String::from_utf8(typed_error_json(
                "unknown_option",
                &[("argument", "--fast")]
            ))
            .unwrap(),
            "{\n  \"code\": \"unknown_option\",\n  \"data\": {\n    \"argument\": \"--fast\"\n  }\n}\n"
        );
    }

    #[test]
    fn every_solve_error_has_its_stable_code() {
        let line = LineId::from_number(1).unwrap();
        let codes: Vec<&str> = [
            SolveError::ConceptSetNotLoaded,
            SolveError::RulesNotBuilt,
            SolveError::Search(SearchError::UnknownQuantity("circle.area".to_owned())),
            SolveError::PhaseTwo(PhaseTwoError::MissingValue("circle.area".to_owned())),
            SolveError::Rank(RankError::NoCriterion),
            SolveError::WayEvaluation(WayEvaluationError::MissingValue("circle.area".to_owned())),
            SolveError::Save(SaveError::CountTooLarge(line)),
        ]
        .iter()
        .map(SolveError::code)
        .collect();
        assert_eq!(
            codes,
            [
                "concept_set_not_loaded",
                "rules_not_built",
                "search_failed",
                "phase_two_failed",
                "ranking_failed",
                "way_evaluation_failed",
                "answer_not_written",
            ]
        );
    }

    #[test]
    fn solve_gives_the_same_bytes_for_the_same_request() {
        assert_eq!(reply(CIRCUMFERENCE), reply(CIRCUMFERENCE));
    }

    #[test]
    fn solve_with_every_input_not_obtainable_is_not_reached() {
        let answer = reply(
            r#"{"phase": "ways", "object": "circle", "wanted": "circumference", "not_obtainable": ["radius", "diameter", "area", "chord", "sagitta", "central-angle"]}"#,
        );
        assert_eq!(answer.status, ReplyStatus::NotReached);
    }

    #[test]
    fn not_reached_triangle_area_states_the_bound_from_two_sides() {
        let answer = reply(
            r#"{"phase": "evaluate", "object": "triangle", "wanted": "area", "given": [{"name": "side-a", "value": "3", "unit": "m"}, {"name": "side-b", "value": "4", "unit": "m"}], "not_obtainable": ["side-c", "angle-gamma", "angle-alpha", "angle-beta", "height-b"]}"#,
        );
        let json = text(&answer);
        assert_eq!(answer.status, ReplyStatus::NotReached, "{json}");
        let bounds = &json[json.find("\"bounds\"").unwrap()..json.find("\"quantities\"").unwrap()];
        assert!(bounds.contains("\"lower\": null,"), "{bounds}");
        assert!(bounds.contains("\"digits\": \"6\""), "{bounds}");
        assert!(bounds.contains("\"unit\": \"m^2\","), "{bounds}");
    }

    fn evaluated_way_answer(given: &str) -> SolveAnswer {
        let mut solving = session();
        let id = solving
            .enter_solve(&request(&format!(
                r#"{{"phase": "evaluate", "object": "circle", "wanted": "circumference", "given": [{given}], "only_obtainable": true}}"#
            )))
            .unwrap();
        answer_of(&solving, id)
    }

    #[test]
    fn evaluated_way_is_solved_with_a_record_in_the_wanted_unit() {
        let answer = evaluated_way_answer(r#"{"name": "diameter", "value": "2", "unit": "m"}"#);
        let AnswerBody::Solved { ways, .. } = &answer.body else {
            panic!("expected solved, found {:?}", answer.body);
        };
        let way = &ways[0];
        let record = way.record.as_deref().unwrap();
        let mut units = session();
        let metre = units.enter("1 m").unwrap();
        let expected_unit = match units.line(metre).unwrap().outcome() {
            Outcome::Result(result) => result.computed().unit(),
            other => panic!("expected a result, found {other:?}"),
        };
        assert_eq!(way.conditions[0].holds, Some(true));
        assert_eq!(record.computed().unit(), expected_unit);
        assert_eq!(record.computed().method().name, "way_evaluation");
        assert_eq!(
            record.sources().collect::<Vec<_>>(),
            ["openstax-prealgebra-2e"]
        );
    }

    #[test]
    fn evaluated_way_carries_the_propagated_standard_uncertainty() {
        let answer = evaluated_way_answer(
            r#"{"name": "diameter", "value": "2", "uncertainty": "0.02", "coverage_factor": "2", "unit": "m"}"#,
        );
        let record = ways_of(&answer)[0].record.as_deref().unwrap();
        assert!(record.computed().uncertainty().is_some());
    }

    #[test]
    fn way_whose_condition_fails_has_no_record() {
        let answer = evaluated_way_answer(r#"{"name": "diameter", "value": "-2", "unit": "m"}"#);
        let way = &ways_of(&answer)[0];
        assert_eq!(answer.outcome_status(), crate::OutcomeStatus::Ways);
        assert_eq!(
            (way.conditions[0].holds, way.record.is_none()),
            (Some(false), true)
        );
    }

    #[test]
    fn saved_solved_line_opens_to_an_equal_answer() {
        let mut solving = session();
        let id = solving
            .enter_solve(&request(
                r#"{"phase": "evaluate", "object": "circle", "wanted": "circumference", "given": [{"name": "diameter", "value": "2", "unit": "m"}]}"#,
            ))
            .unwrap();
        let bytes = solving.save_to_bytes().unwrap();
        let opened = Session::open_from_bytes(
            &bytes,
            crate::session::tests::fixed_clock(),
            crate::registered_backends(),
        )
        .unwrap();
        assert_eq!(answer_of(&opened, id), answer_of(&solving, id));
    }

    const REACHABLE_FROM_RADIUS: &str =
        r#"{"phase": "reachable", "given": [{"name": "radius", "value": "2", "unit": "m"}]}"#;

    fn reachable_of(session: &Session, id: LineId) -> ReachableAnswer {
        match session.line(id).unwrap().outcome() {
            Outcome::Reachable(answer) => (**answer).clone(),
            other => panic!("expected a reachable answer, found {other:?}"),
        }
    }

    fn reachable_line(session: &mut Session, text: &str) -> LineId {
        session.enter_solve(&request(text)).unwrap()
    }

    fn names(answer: &ReachableAnswer) -> Vec<&str> {
        answer
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect()
    }

    #[test]
    fn reachable_from_the_radius_lists_what_follows_by_steps_then_name() {
        let mut solving = session();
        let id = reachable_line(&mut solving, REACHABLE_FROM_RADIUS);
        let answer = reachable_of(&solving, id);
        assert_eq!(answer.object, "circle");
        assert_eq!(&names(&answer)[..3], ["area", "circumference", "diameter"]);
        assert!(
            answer
                .entries
                .iter()
                .all(|entry| entry.steps >= 1 && !entry.needs_obtainable)
        );
    }

    #[test]
    fn reachable_entry_decides_its_conditions_from_the_given_value() {
        let mut solving = session();
        let id = reachable_line(&mut solving, REACHABLE_FROM_RADIUS);
        let area = &reachable_of(&solving, id).entries[0];
        assert_eq!(
            (area.conditions[0].text.as_str(), area.conditions[0].holds),
            ("r > 0", Some(true))
        );
    }

    #[test]
    fn reachable_entry_through_an_obtainable_role_needs_it() {
        let mut solving = session();
        let id = reachable_line(
            &mut solving,
            r#"{"phase": "reachable", "given": [{"name": "side-a", "value": "3", "unit": "m"}], "obtainable": ["side-b", "angle-gamma"]}"#,
        );
        let answer = reachable_of(&solving, id);
        let area = answer
            .entries
            .iter()
            .find(|entry| entry.name == "area")
            .unwrap();
        assert!(area.needs_obtainable);
        assert!(area.uses.contains(&"side-b".to_owned()));
    }

    #[test]
    fn reachable_cap_truncates_but_keeps_the_exact_total() {
        let mut solving = session();
        let id = reachable_line(
            &mut solving,
            r#"{"phase": "reachable", "given": [{"name": "radius", "value": "2", "unit": "m"}], "cap": 1}"#,
        );
        let answer = reachable_of(&solving, id);
        assert_eq!(answer.entries.len(), 1);
        assert!(answer.truncated);
        assert!(answer.reachable_total > 1);
    }

    #[test]
    fn reachable_request_with_roles_of_two_objects_is_ambiguous_object() {
        let answer = reply(
            r#"{"phase": "reachable", "given": [{"name": "radius", "value": "2", "unit": "m"}, {"name": "side-a", "value": "3", "unit": "m"}]}"#,
        );
        assert!(text(&answer).contains("\"code\": \"ambiguous_object\","));
    }

    #[test]
    fn reachable_request_without_roles_or_object_is_ambiguous_object() {
        let answer = reply(r#"{"phase": "reachable"}"#);
        assert_eq!(answer.status, ReplyStatus::InvalidRequest);
        assert!(text(&answer).contains("\"code\": \"ambiguous_object\","));
    }

    #[test]
    fn reachable_request_with_an_unknown_role_names_it() {
        let answer = reply(r#"{"phase": "reachable", "obtainable": ["happiness"]}"#);
        assert!(text(&answer).contains("\"code\": \"unknown_role\","));
        assert!(text(&answer).contains("\"path\": \"obtainable[0]\""));
    }

    #[test]
    fn solve_answers_a_reachable_request_with_the_a034_members() {
        let answer = reply(REACHABLE_FROM_RADIUS);
        let json = text(&answer);
        assert_eq!(answer.status, ReplyStatus::Reachable);
        assert!(json.starts_with("{\n  \"status\": \"reachable\",\n  \"concept_set_version\": \""));
        let order: Vec<usize> = [
            "\"object\"",
            "\"truncated\"",
            "\"reachable_total\"",
            "\"reachable\":",
            "\"quantities\"",
        ]
        .iter()
        .map(|key| json.find(key).unwrap())
        .collect();
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn mark_obtainable_on_a_reachable_line_extends_the_listing() {
        let mut solving = session();
        let id = reachable_line(
            &mut solving,
            r#"{"phase": "reachable", "given": [{"name": "side-a", "value": "3", "unit": "m"}]}"#,
        );
        let before = reachable_of(&solving, id).reachable_total;
        solving.mark(id, "side-b", RoleMark::Obtainable).unwrap();
        solving
            .mark(id, "angle-gamma", RoleMark::Obtainable)
            .unwrap();
        assert!(reachable_of(&solving, id).reachable_total > before);
    }

    #[test]
    fn mark_not_obtainable_on_a_reachable_line_is_unexpected_member() {
        let mut solving = session();
        let id = reachable_line(&mut solving, REACHABLE_FROM_RADIUS);
        assert!(matches!(
            solving.mark(id, "diameter", RoleMark::NotObtainable),
            Err(SessionError::InvalidRequest(RequestError {
                code: RequestErrorCode::UnexpectedMember,
                ..
            }))
        ));
    }

    #[test]
    fn set_given_on_a_reachable_line_keeps_the_reachable_phase() {
        let mut solving = session();
        let id = reachable_line(&mut solving, REACHABLE_FROM_RADIUS);
        solving
            .set_given(
                id,
                "radius",
                Some(GivenInput {
                    name: "radius".to_owned(),
                    value: "-1".to_owned(),
                    uncertainty: None,
                    coverage_factor: None,
                    unit: Some("m".to_owned()),
                }),
            )
            .unwrap();
        assert_eq!(
            solving.solve_request_of(id).unwrap().phase,
            Phase::Reachable
        );
        assert_eq!(
            reachable_of(&solving, id).entries[0].conditions[0].holds,
            Some(false)
        );
    }

    #[test]
    fn set_criterion_on_a_reachable_line_is_unexpected_member() {
        let mut solving = session();
        let id = reachable_line(&mut solving, REACHABLE_FROM_RADIUS);
        assert!(matches!(
            solving.set_criterion(id, SolveCriterion::program_default()),
            Err(SessionError::InvalidRequest(RequestError {
                code: RequestErrorCode::UnexpectedMember,
                ..
            }))
        ));
    }

    #[test]
    fn find_ways_on_a_reachable_line_is_unexpected_member() {
        let mut solving = session();
        let id = reachable_line(&mut solving, REACHABLE_FROM_RADIUS);
        assert!(matches!(
            solving.find_ways(id),
            Err(SessionError::InvalidRequest(RequestError {
                code: RequestErrorCode::UnexpectedMember,
                ..
            }))
        ));
    }

    #[test]
    fn set_cap_on_a_reachable_line_caps_the_listing() {
        let mut solving = session();
        let id = reachable_line(&mut solving, REACHABLE_FROM_RADIUS);
        solving.set_cap(id, 2).unwrap();
        assert_eq!(reachable_of(&solving, id).entries.len(), 2);
    }

    #[test]
    fn choose_wanted_turns_the_listing_into_a_way_search() {
        let mut solving = session();
        let id = reachable_line(&mut solving, REACHABLE_FROM_RADIUS);
        solving
            .choose_wanted(id, "area", SolveCriterion::program_default(), 5)
            .unwrap();
        let chosen = solving.solve_request_of(id).unwrap().clone();
        assert_eq!(
            (
                chosen.phase,
                chosen.object.as_deref(),
                chosen.wanted.as_deref(),
                chosen.cap
            ),
            (Phase::Ways, Some("circle"), Some("area"), 5)
        );
        assert_eq!(chosen.given.len(), 1);
        assert!(matches!(
            solving.line(id).unwrap().outcome(),
            Outcome::Answer(_)
        ));
    }

    #[test]
    fn choose_wanted_on_a_reopened_reachable_line_finds_its_object_again() {
        let mut solving = session();
        let id = reachable_line(&mut solving, REACHABLE_FROM_RADIUS);
        let bytes = solving.save_to_bytes().unwrap();
        let mut opened = Session::open_from_bytes(
            &bytes,
            crate::session::tests::fixed_clock(),
            crate::registered_backends(),
        )
        .unwrap();
        opened
            .choose_wanted(id, "area", SolveCriterion::program_default(), 5)
            .unwrap();
        assert_eq!(
            opened.solve_request_of(id).unwrap().object.as_deref(),
            Some("circle")
        );
        assert!(matches!(
            opened.line(id).unwrap().outcome(),
            Outcome::Answer(_)
        ));
    }

    #[test]
    fn choose_wanted_on_a_ways_line_is_unexpected_member() {
        let mut solving = session();
        let id = circumference_line(&mut solving);
        assert!(matches!(
            solving.choose_wanted(id, "area", SolveCriterion::program_default(), 5),
            Err(SessionError::InvalidRequest(RequestError {
                code: RequestErrorCode::UnexpectedMember,
                ..
            }))
        ));
    }

    #[test]
    fn saved_reachable_line_is_stored_not_evaluated_and_keeps_its_request() {
        let mut solving = session();
        let id = reachable_line(&mut solving, REACHABLE_FROM_RADIUS);
        let bytes = solving.save_to_bytes().unwrap();
        let opened = Session::open_from_bytes(
            &bytes,
            crate::session::tests::fixed_clock(),
            crate::registered_backends(),
        )
        .unwrap();
        assert_eq!(opened.line(id).unwrap().outcome(), &Outcome::NotEvaluated);
        assert_eq!(opened.solve_request_of(id), solving.solve_request_of(id));
    }

    fn condition_text_of(rule_identifier: &str) -> String {
        let concepts = concept_set().unwrap();
        let mut pool = ExprPool::new();
        let rule_set = concepts.rule_set(&mut pool).unwrap();
        let rule = rule_set
            .rules
            .iter()
            .find(|rule| rule.identifier == rule_identifier)
            .unwrap();
        condition_text(
            &pool,
            concepts,
            &rule_set,
            rule_identifier,
            rule.conditions[0],
        )
        .0
    }

    fn body_of(source: &str, parameters: usize) -> Option<String> {
        let mut pool = ExprPool::new();
        let parsed = parse_expression(&mut pool, source).unwrap();
        lambda_body_text(&mut pool, parsed, parameters)
    }

    #[test]
    fn lambda_body_names_its_parameters() {
        assert_eq!(body_of("a |-> b |-> a + b", 2).as_deref(), Some("a + b"));
    }

    #[test]
    fn lambda_body_keeps_a_nested_lambda_with_its_own_parameter() {
        assert_eq!(
            body_of("a |-> (x |-> x + a)", 1).as_deref(),
            Some("x |-> x + a")
        );
    }

    #[test]
    fn body_below_fewer_parameters_keeps_the_remaining_lambda() {
        assert_eq!(
            body_of("a |-> b |-> a > 0", 1).as_deref(),
            Some("b |-> a > 0")
        );
    }

    #[test]
    fn more_parameters_than_lambdas_has_no_body() {
        assert_eq!(body_of("a |-> a > 0", 2), None);
    }

    #[test]
    fn condition_text_is_the_body_with_the_parameter_names_of_its_own_content() {
        assert_eq!(
            condition_text_of("triangle-area/base-height"),
            "b > 0 and h > 0"
        );
    }

    #[test]
    fn condition_text_keeps_its_names_when_the_pool_shares_the_condition_node() {
        assert_eq!(
            condition_text_of("circle-sagitta/radius-from-chord-and-sagitta"),
            "s > 0 and h > 0"
        );
        assert_eq!(
            condition_text_of("triangle-area/base-height"),
            "b > 0 and h > 0"
        );
    }

    fn triangle_not_reached(side_a: &str) -> SolveAnswer {
        let mut solving = session();
        let id = solving
            .enter_solve(&request(&format!(
                r#"{{"phase": "evaluate", "object": "triangle", "wanted": "area", "given": [{{"name": "side-a", "value": "{side_a}", "unit": "m"}}, {{"name": "side-b", "value": "4", "unit": "m"}}], "only_obtainable": true}}"#
            )))
            .unwrap();
        answer_of(&solving, id)
    }

    fn further_input(answer: &SolveAnswer, role: &str) -> FurtherInput {
        let AnswerBody::NotReached { missing_any_of, .. } = &answer.body else {
            panic!("expected not reached, found {:?}", answer.body);
        };
        missing_any_of
            .iter()
            .find(|entry| entry.add == [role])
            .cloned()
            .unwrap()
    }

    #[test]
    fn further_input_carries_the_rule_and_derivation_it_completes() {
        let angle = further_input(&triangle_not_reached("3"), "angle-gamma");
        assert_eq!(
            (angle.rule.as_deref(), angle.derivation.clone()),
            (
                Some("triangle-area/two-sides-angle"),
                Some(vec!["triangle-area/two-sides-angle".to_owned()])
            )
        );
    }

    #[test]
    fn further_input_condition_with_an_unknown_angle_is_undecided() {
        let angle = further_input(&triangle_not_reached("3"), "angle-gamma");
        let conditions = angle.conditions.unwrap();
        assert_eq!(
            (
                conditions[0].rule.as_str(),
                conditions[0].text.as_str(),
                conditions[0].holds
            ),
            (
                "triangle-area/two-sides-angle",
                "a > 0 and b > 0 and g > 0 and g < pi",
                None
            )
        );
    }

    #[test]
    fn further_input_condition_fails_when_a_given_side_is_negative() {
        let angle = further_input(&triangle_not_reached("-3"), "angle-gamma");
        assert_eq!(angle.conditions.unwrap()[0].holds, Some(false));
    }

    #[test]
    fn bounds_name_the_bound_they_follow_from() {
        let answer = triangle_not_reached("3");
        let AnswerBody::NotReached { bounds, .. } = &answer.body else {
            panic!("expected not reached");
        };
        assert_eq!(
            bounds.as_ref().unwrap().from,
            Some(vec![BoundFrom {
                bound: "triangle-area/two-sides".to_owned(),
                kind: calc_core::BoundKind::AtMost,
                inputs: vec!["side-a".to_owned(), "side-b".to_owned()],
                ends: vec![calc_core::BoundSide::Upper],
            }])
        );
    }

    #[test]
    fn unknown_wanted_is_an_invalid_request_naming_its_path() {
        let answer = reply(r#"{"phase": "ways", "wanted": "happiness"}"#);
        let json = text(&answer);
        assert_eq!(answer.status, ReplyStatus::InvalidRequest);
        assert!(json.contains("\"code\": \"unknown_wanted\","));
        assert!(json.contains("\"path\": \"wanted\""));
    }

    #[test]
    fn malformed_request_is_an_invalid_request() {
        assert_eq!(reply("{").status, ReplyStatus::InvalidRequest);
    }

    #[test]
    fn given_value_with_a_unit_of_another_dimension_is_a_dimension_mismatch() {
        let answer = reply(
            r#"{"phase": "evaluate", "object": "circle", "wanted": "circumference", "given": [{"name": "diameter", "value": "2", "unit": "s"}]}"#,
        );
        assert!(text(&answer).contains("\"code\": \"dimension_mismatch\","));
    }

    #[test]
    fn given_value_in_centimetres_is_stated_in_coherent_units() {
        let answer = reply(
            r#"{"phase": "evaluate", "object": "circle", "wanted": "circumference", "given": [{"name": "diameter", "value": "2", "unit": "cm"}]}"#,
        );
        assert!(text(&answer).contains("\"numerator\": \"1\",\n"));
        assert!(text(&answer).contains("\"denominator\": \"50\"\n"));
    }

    #[test]
    fn front_criterion_gives_a_front_answer() {
        let answer = reply(
            r#"{"phase": "ways", "object": "circle", "wanted": "circumference", "criterion": {"by": ["fewest_measurements", "gate_count"], "combine": "front"}}"#,
        );
        assert_eq!(answer.status, ReplyStatus::Front);
    }

    #[test]
    fn enter_solve_adds_a_line_holding_the_request_and_its_answer() {
        let mut solving = session();
        let id = circumference_line(&mut solving);
        let line = solving.line(id).unwrap();
        assert!(line.input().starts_with("solve {\"phase\": \"ways\""));
        assert_eq!(
            answer_of(&solving, id).outcome_status(),
            crate::OutcomeStatus::Ways
        );
    }

    #[test]
    fn enter_solve_with_an_invalid_request_adds_no_line() {
        let mut solving = session();
        let result = solving.enter_solve(&request(r#"{"phase": "ways", "wanted": "happiness"}"#));
        assert!(matches!(result, Err(SessionError::InvalidRequest(_))));
        assert!(solving.lines().is_empty());
    }

    #[test]
    fn mark_not_obtainable_removes_the_ways_through_that_role() {
        let mut solving = session();
        let id = circumference_line(&mut solving);
        solving
            .mark(id, "diameter", RoleMark::NotObtainable)
            .unwrap();
        let answer = answer_of(&solving, id);
        assert!(
            ways_of(&answer)
                .iter()
                .all(|way| !way.inputs.contains(&"diameter".to_owned()))
        );
        assert!(
            solving
                .line(id)
                .unwrap()
                .input()
                .contains("\"not_obtainable\": [\"diameter\"]")
        );
    }

    #[test]
    fn mark_a_given_role_not_obtainable_is_contradictory_and_changes_nothing() {
        let mut solving = session();
        let id = circumference_line(&mut solving);
        solving
            .set_given(
                id,
                "diameter",
                Some(GivenInput {
                    name: "diameter".to_owned(),
                    value: "2".to_owned(),
                    uncertainty: None,
                    coverage_factor: None,
                    unit: Some("m".to_owned()),
                }),
            )
            .unwrap();
        let before = solving.line(id).unwrap().clone();
        let result = solving.mark(id, "diameter", RoleMark::NotObtainable);
        assert!(matches!(result, Err(SessionError::InvalidRequest(_))));
        assert_eq!(solving.line(id).unwrap(), &before);
    }

    #[test]
    fn set_given_switches_the_phase_to_evaluate_and_marks_the_role_given() {
        let mut solving = session();
        let id = circumference_line(&mut solving);
        solving
            .set_given(
                id,
                "radius",
                Some(GivenInput {
                    name: "radius".to_owned(),
                    value: "1.5".to_owned(),
                    uncertainty: Some("0.01".to_owned()),
                    coverage_factor: None,
                    unit: Some("m".to_owned()),
                }),
            )
            .unwrap();
        let answer = answer_of(&solving, id);
        let radius = answer
            .quantities
            .iter()
            .find(|quantity| quantity.name == "radius")
            .unwrap();
        assert_eq!(solving.solve_request_of(id).unwrap().phase, Phase::Evaluate);
        assert_eq!(radius.mark, QuantityMark::Given);
    }

    #[test]
    fn set_criterion_rewrites_the_criterion_of_the_request() {
        let mut solving = session();
        let id = circumference_line(&mut solving);
        let criterion = SolveCriterion {
            by: vec![Criterion::FewestMeasurements],
            combine: Combine::Order,
        };
        solving.set_criterion(id, criterion.clone()).unwrap();
        assert_eq!(answer_of(&solving, id).criterion, criterion);
    }

    #[test]
    fn set_cap_grows_the_work_budget_in_proportion() {
        let mut solving = session();
        let id = circumference_line(&mut solving);
        solving.set_cap(id, 10).unwrap();
        let request = solving.solve_request_of(id).unwrap();
        assert_eq!((request.cap, request.work_budget), (10, 200_000));
    }

    #[test]
    fn set_cap_above_one_thousand_is_invalid() {
        let mut solving = session();
        let id = circumference_line(&mut solving);
        assert!(matches!(
            solving.set_cap(id, 1001),
            Err(SessionError::InvalidRequest(_))
        ));
    }

    #[test]
    fn find_ways_sets_the_phase_back_to_ways() {
        let mut solving = session();
        let id = circumference_line(&mut solving);
        solving
            .set_given(
                id,
                "radius",
                Some(GivenInput {
                    name: "radius".to_owned(),
                    value: "1".to_owned(),
                    uncertainty: None,
                    coverage_factor: None,
                    unit: Some("m".to_owned()),
                }),
            )
            .unwrap();
        solving.find_ways(id).unwrap();
        assert_eq!(solving.solve_request_of(id).unwrap().phase, Phase::Ways);
    }

    #[test]
    fn solve_command_on_an_expression_line_is_not_a_solve_line() {
        let mut solving = session();
        let id = solving.enter("1 + 1").unwrap();
        assert_eq!(solving.find_ways(id), Err(SessionError::NotASolveLine(id)));
    }

    #[test]
    fn saved_solve_line_opens_with_its_request_and_answer() {
        let mut solving = session();
        let id = circumference_line(&mut solving);
        let bytes = solving.save_to_bytes().unwrap();
        let opened = Session::open_from_bytes(
            &bytes,
            crate::session::tests::fixed_clock(),
            crate::registered_backends(),
        )
        .unwrap();
        assert_eq!(opened.lines(), solving.lines());
        assert_eq!(opened.solve_request_of(id), solving.solve_request_of(id));
    }

    #[test]
    fn condition_names_its_parameters_with_the_roles_of_its_rule() {
        let concepts = concept_set().unwrap();
        let mut pool = ExprPool::new();
        let rule_set = concepts.rule_set(&mut pool).unwrap();
        let identifier = "circle-chord/from-radius-and-central-angle";
        let rule = rule_set
            .rules
            .iter()
            .find(|rule| rule.identifier == identifier)
            .unwrap();

        let (text, parameters) =
            condition_text(&pool, concepts, &rule_set, identifier, rule.conditions[0]);

        assert_eq!(
            (text.as_str(), parameters),
            (
                "r > 0 and t > 0 and t < 2 * pi",
                Some(vec![
                    ConditionParameter {
                        name: "r".to_owned(),
                        role: "circle.radius".to_owned()
                    },
                    ConditionParameter {
                        name: "t".to_owned(),
                        role: "circle.central-angle".to_owned()
                    }
                ])
            )
        );
    }
}
