use calc_core::{
    BoundKind, BoundSide, Combine, Criterion, ErrorBound, Exactness, ResultValue, Truncation,
};
use calc_expr::ExprPool;
use calc_numbers::Number;

use crate::json::Json;
use crate::result_record::{LineId, ResultRecord};
use crate::session_file::{
    JsonPath, LoadError, MAXIMUM_SAFE_COUNT, PathSegment, SaveError, array, boolean, count,
    identifier, members, number, number_json, one_of, optional, record, record_json, string, unit,
    value, value_json,
};

const VERSION_HEX_DIGITS: usize = 16;
pub(crate) const TIE_BREAK: [&str; 5] = [
    "exactness",
    "error_bound",
    "gate_count",
    "rule_id",
    "inputs",
];
pub(crate) const CRITERIA: [(&str, Criterion); 7] = [
    ("fewest_measurements", Criterion::FewestMeasurements),
    ("fewest_steps", Criterion::FewestSteps),
    ("exactness", Criterion::Exactness),
    ("error_bound", Criterion::ErrorBound),
    ("gate_count", Criterion::GateCount),
    ("smallest_uncertainty", Criterion::SmallestUncertainty),
    ("conditioning", Criterion::Conditioning),
];
pub(crate) const COMBINES: [(&str, Combine); 2] =
    [("order", Combine::Order), ("front", Combine::Front)];
const TRUNCATIONS: [(&str, Truncation); 2] = [
    ("cap", Truncation::WayCap),
    ("work_budget", Truncation::WorkBudget),
];
const EXACTNESSES: [(&str, Exactness); 2] =
    [("exact", Exactness::Exact), ("machine", Exactness::Machine)];
const ERROR_BOUNDS: [(&str, ErrorBound); 3] = [
    ("zero", ErrorBound::Zero),
    ("documented", ErrorBound::Documented),
    ("unknown", ErrorBound::Unknown),
];
const MARKS: [(&str, QuantityMark); 4] = [
    ("given", QuantityMark::Given),
    ("obtainable", QuantityMark::Obtainable),
    ("not_obtainable", QuantityMark::NotObtainable),
    ("unmarked", QuantityMark::Unmarked),
];
const BOUND_KINDS: [(&str, BoundKind); 3] = [
    ("at_most", BoundKind::AtMost),
    ("at_least", BoundKind::AtLeast),
    ("strictly_between", BoundKind::StrictlyBetween),
];
const BOUND_SIDES: [(&str, BoundSide); 2] =
    [("lower", BoundSide::Lower), ("upper", BoundSide::Upper)];
const WAYS_STATUS: &str = "ways";
const FRONT_STATUS: &str = "front";
const SOLVED_STATUS: &str = "solved";
const NOT_REACHED_STATUS: &str = "not_reached";
const REACHABLE_STATUS: &str = "reachable";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolveAnswer {
    pub concept_set_version: u64,
    pub criterion: SolveCriterion,
    pub body: AnswerBody,
    pub quantities: Vec<AnswerQuantity>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolveCriterion {
    pub by: Vec<Criterion>,
    pub combine: Combine,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnswerBody {
    Ways {
        listing: WayListing,
        ways: Vec<AnswerWay>,
    },
    Front {
        listing: WayListing,
        front: Vec<AnswerWay>,
        dominated: Vec<AnswerWay>,
    },
    Solved {
        listing: WayListing,
        ways: Vec<AnswerWay>,
    },
    NotReached {
        missing_any_of: Vec<FurtherInput>,
        bounds: Option<Box<AnswerBounds>>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WayListing {
    pub truncated_by: Option<Truncation>,
    pub complete_through_steps: u64,
    pub ways_found: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerWay {
    pub rank: Option<u64>,
    pub layer: Option<u64>,
    pub rule: Option<String>,
    pub derivation: Vec<String>,
    pub inputs: Vec<String>,
    pub obtainable: bool,
    pub scores: Vec<Option<StoredScore>>,
    pub conditions: Vec<AnswerCondition>,
    pub sources: Vec<String>,
    pub record: Option<Box<ResultRecord>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoredScore {
    Count(u64),
    Exactness(Exactness),
    ErrorBound(ErrorBound),
    Exact(Number),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConditionParameter {
    pub name: String,
    pub role: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerCondition {
    pub text: String,
    pub parameters: Option<Vec<ConditionParameter>>,
    pub holds: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FurtherInput {
    pub add: Vec<String>,
    pub rule: Option<String>,
    pub derivation: Option<Vec<String>>,
    pub between: Option<Vec<String>>,
    pub conditions: Option<Vec<ReachableCondition>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerBounds {
    pub lower: Option<BoundEnd>,
    pub upper: Option<BoundEnd>,
    pub unit: Option<String>,
    pub sources: Vec<String>,
    pub from: Option<Vec<BoundFrom>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundFrom {
    pub bound: String,
    pub kind: BoundKind,
    pub inputs: Vec<String>,
    pub ends: Vec<BoundSide>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundEnd {
    pub value: ResultValue,
    pub inclusive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerQuantity {
    pub name: String,
    pub mark: QuantityMark,
    pub value: Option<ResultValue>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuantityMark {
    Given,
    Obtainable,
    NotObtainable,
    Unmarked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachableAnswer {
    pub concept_set_version: u64,
    pub object: String,
    pub truncated: bool,
    pub reachable_total: u64,
    pub entries: Vec<ReachableEntry>,
    pub quantities: Vec<AnswerQuantity>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachableEntry {
    pub name: String,
    pub steps: u64,
    pub rule: String,
    pub inputs: Vec<String>,
    pub derivation: Vec<String>,
    pub uses: Vec<String>,
    pub conditions: Vec<ReachableCondition>,
    pub needs_obtainable: bool,
    pub sources: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachableCondition {
    pub rule: String,
    pub text: String,
    pub parameters: Option<Vec<ConditionParameter>>,
    pub holds: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutcomeStatus {
    Ways,
    NotReached,
    Solved,
}

impl SolveAnswer {
    pub fn outcome_status(&self) -> OutcomeStatus {
        match self.body {
            AnswerBody::Ways { .. } | AnswerBody::Front { .. } => OutcomeStatus::Ways,
            AnswerBody::Solved { .. } => OutcomeStatus::Solved,
            AnswerBody::NotReached { .. } => OutcomeStatus::NotReached,
        }
    }

    pub(crate) fn records(&self) -> Vec<(JsonPath, &ResultRecord)> {
        let groups: Vec<(&str, &[AnswerWay])> = match &self.body {
            AnswerBody::Ways { ways, .. } | AnswerBody::Solved { ways, .. } => {
                vec![("ways", ways)]
            }
            AnswerBody::Front {
                front, dominated, ..
            } => vec![("front", front), ("dominated", dominated)],
            AnswerBody::NotReached { .. } => Vec::new(),
        };
        groups
            .into_iter()
            .flat_map(|(group, ways)| {
                ways.iter().enumerate().filter_map(move |(position, way)| {
                    way.record.as_deref().map(|record| {
                        (
                            JsonPath(vec![
                                PathSegment::Member(group.to_owned()),
                                PathSegment::Index(position),
                                PathSegment::Member("record".to_owned()),
                            ]),
                            record,
                        )
                    })
                })
            })
            .collect()
    }
}

pub(crate) fn outcome_status_name(status: OutcomeStatus) -> &'static str {
    match status {
        OutcomeStatus::Ways => WAYS_STATUS,
        OutcomeStatus::NotReached => NOT_REACHED_STATUS,
        OutcomeStatus::Solved => SOLVED_STATUS,
    }
}

pub(crate) fn outcome_status_from_name(name: &str) -> Option<OutcomeStatus> {
    match name {
        WAYS_STATUS => Some(OutcomeStatus::Ways),
        NOT_REACHED_STATUS => Some(OutcomeStatus::NotReached),
        SOLVED_STATUS => Some(OutcomeStatus::Solved),
        _ => None,
    }
}

pub(crate) fn name_of<T: PartialEq + Copy>(
    choices: &[(&'static str, T)],
    wanted: T,
) -> &'static str {
    choices
        .iter()
        .find(|(_, choice)| *choice == wanted)
        .map_or("", |(name, _)| name)
}

fn count_json(value: u64, line: LineId) -> Result<Json, SaveError> {
    if value <= MAXIMUM_SAFE_COUNT {
        Ok(Json::Count(value))
    } else {
        Err(SaveError::CountTooLarge(line))
    }
}

fn strings_json(texts: &[String]) -> Json {
    Json::Array(texts.iter().map(|text| Json::string(text)).collect())
}

fn score_json(score: &StoredScore, line: LineId) -> Result<Json, SaveError> {
    Ok(match score {
        StoredScore::Count(value) => count_json(*value, line)?,
        StoredScore::Exactness(exactness) => Json::string(name_of(&EXACTNESSES, *exactness)),
        StoredScore::ErrorBound(bound) => Json::string(name_of(&ERROR_BOUNDS, *bound)),
        StoredScore::Exact(value) => number_json(value),
    })
}

fn way_json(
    pool: &ExprPool,
    way: &AnswerWay,
    criterion: &SolveCriterion,
    line: LineId,
) -> Result<Json, SaveError> {
    let optional_count =
        |value: Option<u64>| value.map(|value| count_json(value, line)).transpose();
    let scores = criterion
        .by
        .iter()
        .zip(&way.scores)
        .map(|(by, score)| {
            let json = score
                .as_ref()
                .map(|score| score_json(score, line))
                .transpose()?;
            Ok((name_of(&CRITERIA, *by).to_owned(), Json::optional(json)))
        })
        .collect::<Result<Vec<_>, SaveError>>()?;
    let conditions = way
        .conditions
        .iter()
        .map(|condition| {
            Json::object(vec![
                ("text", Json::string(&condition.text)),
                (
                    "parameters",
                    parameters_json(condition.parameters.as_deref()),
                ),
                ("holds", Json::optional(condition.holds.map(Json::Boolean))),
            ])
        })
        .collect();
    let record = way
        .record
        .as_deref()
        .map(|record| record_json(pool, record, line))
        .transpose()?;
    Ok(Json::object(vec![
        ("rank", Json::optional(optional_count(way.rank)?)),
        ("layer", Json::optional(optional_count(way.layer)?)),
        (
            "rule",
            Json::optional(way.rule.as_deref().map(Json::string)),
        ),
        ("derivation", strings_json(&way.derivation)),
        ("inputs", strings_json(&way.inputs)),
        ("obtainable", Json::Boolean(way.obtainable)),
        ("criterion", Json::Object(scores)),
        ("conditions", Json::Array(conditions)),
        ("sources", strings_json(&way.sources)),
        ("record", Json::optional(record)),
    ]))
}

fn ways_json(
    pool: &ExprPool,
    ways: &[AnswerWay],
    criterion: &SolveCriterion,
    line: LineId,
) -> Result<Json, SaveError> {
    Ok(Json::Array(
        ways.iter()
            .map(|way| way_json(pool, way, criterion, line))
            .collect::<Result<_, _>>()?,
    ))
}

fn listing_members(
    listing: &WayListing,
    line: LineId,
) -> Result<Vec<(&'static str, Json)>, SaveError> {
    Ok(vec![
        ("truncated", Json::Boolean(listing.truncated_by.is_some())),
        (
            "truncated_by",
            Json::optional(
                listing
                    .truncated_by
                    .map(|truncation| Json::string(name_of(&TRUNCATIONS, truncation))),
            ),
        ),
        (
            "complete_through_steps",
            count_json(listing.complete_through_steps, line)?,
        ),
        ("ways_found", count_json(listing.ways_found, line)?),
    ])
}

fn bound_end_json(end: Option<&BoundEnd>, line: LineId) -> Result<Json, SaveError> {
    Ok(match end {
        None => Json::Null,
        Some(end) => Json::object(vec![
            ("value", value_json(&end.value, line)?),
            ("inclusive", Json::Boolean(end.inclusive)),
        ]),
    })
}

pub(crate) fn criterion_json(criterion: &SolveCriterion) -> Json {
    Json::object(vec![
        (
            "by",
            Json::Array(
                criterion
                    .by
                    .iter()
                    .map(|by| Json::string(name_of(&CRITERIA, *by)))
                    .collect(),
            ),
        ),
        (
            "combine",
            Json::string(name_of(&COMBINES, criterion.combine)),
        ),
        (
            "tie_break",
            Json::Array(TIE_BREAK.iter().map(|name| Json::string(name)).collect()),
        ),
    ])
}

pub(crate) fn concept_set_version_text(version: u64) -> String {
    format!("{version:0width$x}", width = VERSION_HEX_DIGITS)
}

fn quantities_json(quantities: &[AnswerQuantity], line: LineId) -> Result<Json, SaveError> {
    Ok(Json::Array(
        quantities
            .iter()
            .map(|quantity| {
                Ok(Json::object(vec![
                    ("name", Json::string(&quantity.name)),
                    ("mark", Json::string(name_of(&MARKS, quantity.mark))),
                    (
                        "value",
                        Json::optional(
                            quantity
                                .value
                                .as_ref()
                                .map(|value| value_json(value, line))
                                .transpose()?,
                        ),
                    ),
                ]))
            })
            .collect::<Result<Vec<_>, SaveError>>()?,
    ))
}

fn rule_conditions_json(conditions: &[ReachableCondition]) -> Json {
    Json::Array(
        conditions
            .iter()
            .map(|condition| {
                Json::object(vec![
                    ("rule", Json::string(&condition.rule)),
                    ("text", Json::string(&condition.text)),
                    (
                        "parameters",
                        parameters_json(condition.parameters.as_deref()),
                    ),
                    ("holds", Json::optional(condition.holds.map(Json::Boolean))),
                ])
            })
            .collect(),
    )
}

fn bound_origins_json(origins: &[BoundFrom]) -> Json {
    Json::Array(
        origins
            .iter()
            .map(|origin| {
                Json::object(vec![
                    ("bound", Json::string(&origin.bound)),
                    ("kind", Json::string(name_of(&BOUND_KINDS, origin.kind))),
                    ("inputs", strings_json(&origin.inputs)),
                    (
                        "ends",
                        Json::Array(
                            origin
                                .ends
                                .iter()
                                .map(|side| Json::string(name_of(&BOUND_SIDES, *side)))
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect(),
    )
}

fn parameters_json(parameters: Option<&[ConditionParameter]>) -> Json {
    Json::optional(parameters.map(|parameters| {
        Json::Array(
            parameters
                .iter()
                .map(|parameter| {
                    Json::object(vec![
                        ("name", Json::string(&parameter.name)),
                        ("role", Json::string(&parameter.role)),
                    ])
                })
                .collect(),
        )
    }))
}

fn parameters(json: &Json, path: &JsonPath) -> Result<Option<Vec<ConditionParameter>>, LoadError> {
    let Some(json) = optional(json) else {
        return Ok(None);
    };
    array(json, path)?
        .iter()
        .enumerate()
        .map(|(position, element)| {
            let parameter_path = path.index(position);
            let found = members(element, &parameter_path, &["name", "role"])?;
            Ok(ConditionParameter {
                name: identifier(found[0], &parameter_path.member("name"))?.to_owned(),
                role: identifier(found[1], &parameter_path.member("role"))?.to_owned(),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn rule_conditions(json: &Json, path: &JsonPath) -> Result<Vec<ReachableCondition>, LoadError> {
    array(json, path)?
        .iter()
        .enumerate()
        .map(|(position, element)| {
            let condition_path = path.index(position);
            let found = members(
                element,
                &condition_path,
                &["rule", "text", "parameters", "holds"],
            )?;
            let holds_path = condition_path.member("holds");
            Ok(ReachableCondition {
                rule: identifier(found[0], &condition_path.member("rule"))?.to_owned(),
                text: identifier(found[1], &condition_path.member("text"))?.to_owned(),
                parameters: parameters(found[2], &condition_path.member("parameters"))?,
                holds: optional(found[3])
                    .map(|holds| boolean(holds, &holds_path))
                    .transpose()?,
            })
        })
        .collect()
}

fn bound_origins(json: &Json, path: &JsonPath) -> Result<Vec<BoundFrom>, LoadError> {
    let origins = array(json, path)?
        .iter()
        .enumerate()
        .map(|(position, element)| {
            let origin_path = path.index(position);
            let found = members(element, &origin_path, &["bound", "kind", "inputs", "ends"])?;
            let ends_path = origin_path.member("ends");
            let ends = array(found[3], &ends_path)?
                .iter()
                .enumerate()
                .map(|(end, side)| one_of(side, &ends_path.index(end), &BOUND_SIDES))
                .collect::<Result<Vec<_>, _>>()?;
            if ends.is_empty() || ends.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(LoadError::InvalidValue(ends_path));
            }
            Ok(BoundFrom {
                bound: identifier(found[0], &origin_path.member("bound"))?.to_owned(),
                kind: one_of(found[1], &origin_path.member("kind"), &BOUND_KINDS)?,
                inputs: sorted_identifiers(found[2], &origin_path.member("inputs"))?,
                ends,
            })
        })
        .collect::<Result<Vec<_>, LoadError>>()?;
    if let Some(position) = origins
        .windows(2)
        .position(|pair| pair[0].bound >= pair[1].bound)
    {
        return Err(LoadError::InvalidValue(
            path.index(position + 1).member("bound"),
        ));
    }
    Ok(origins)
}

pub(crate) fn reachable_answer_json(
    answer: &ReachableAnswer,
    line: LineId,
) -> Result<Json, SaveError> {
    let entries = answer
        .entries
        .iter()
        .map(|entry| {
            let conditions = entry
                .conditions
                .iter()
                .map(|condition| {
                    Json::object(vec![
                        ("rule", Json::string(&condition.rule)),
                        ("text", Json::string(&condition.text)),
                        (
                            "parameters",
                            parameters_json(condition.parameters.as_deref()),
                        ),
                        ("holds", Json::optional(condition.holds.map(Json::Boolean))),
                    ])
                })
                .collect();
            Ok(Json::object(vec![
                ("name", Json::string(&entry.name)),
                ("steps", count_json(entry.steps, line)?),
                ("rule", Json::string(&entry.rule)),
                ("inputs", strings_json(&entry.inputs)),
                ("derivation", strings_json(&entry.derivation)),
                ("uses", strings_json(&entry.uses)),
                ("conditions", Json::Array(conditions)),
                ("needs_obtainable", Json::Boolean(entry.needs_obtainable)),
                ("sources", strings_json(&entry.sources)),
            ]))
        })
        .collect::<Result<Vec<_>, SaveError>>()?;
    Ok(Json::object(vec![
        ("status", Json::string(REACHABLE_STATUS)),
        (
            "concept_set_version",
            Json::String(concept_set_version_text(answer.concept_set_version)),
        ),
        ("object", Json::string(&answer.object)),
        ("truncated", Json::Boolean(answer.truncated)),
        ("reachable_total", count_json(answer.reachable_total, line)?),
        ("reachable", Json::Array(entries)),
        ("quantities", quantities_json(&answer.quantities, line)?),
    ]))
}

pub(crate) fn answer_json(
    pool: &ExprPool,
    answer: &SolveAnswer,
    line: LineId,
) -> Result<Json, SaveError> {
    let status = match answer.body {
        AnswerBody::Ways { .. } => WAYS_STATUS,
        AnswerBody::Front { .. } => FRONT_STATUS,
        AnswerBody::Solved { .. } => SOLVED_STATUS,
        AnswerBody::NotReached { .. } => NOT_REACHED_STATUS,
    };
    let criterion = &answer.criterion;
    let mut members = vec![
        ("status", Json::string(status)),
        (
            "concept_set_version",
            Json::String(concept_set_version_text(answer.concept_set_version)),
        ),
        ("criterion", criterion_json(criterion)),
    ];
    match &answer.body {
        AnswerBody::Ways { listing, ways } | AnswerBody::Solved { listing, ways } => {
            members.extend(listing_members(listing, line)?);
            members.push(("ways", ways_json(pool, ways, criterion, line)?));
        }
        AnswerBody::Front {
            listing,
            front,
            dominated,
        } => {
            members.extend(listing_members(listing, line)?);
            members.push(("front", ways_json(pool, front, criterion, line)?));
            members.push(("dominated", ways_json(pool, dominated, criterion, line)?));
        }
        AnswerBody::NotReached {
            missing_any_of,
            bounds,
        } => {
            let missing = missing_any_of
                .iter()
                .map(|further| {
                    Json::object(vec![
                        ("add", strings_json(&further.add)),
                        (
                            "rule",
                            Json::optional(further.rule.as_deref().map(Json::string)),
                        ),
                        (
                            "derivation",
                            Json::optional(further.derivation.as_deref().map(strings_json)),
                        ),
                        (
                            "between",
                            Json::optional(further.between.as_deref().map(strings_json)),
                        ),
                        (
                            "conditions",
                            Json::optional(further.conditions.as_deref().map(rule_conditions_json)),
                        ),
                    ])
                })
                .collect();
            members.push(("missing_any_of", Json::Array(missing)));
            let bounds = match bounds {
                None => Json::Null,
                Some(bounds) => Json::object(vec![
                    ("lower", bound_end_json(bounds.lower.as_ref(), line)?),
                    ("upper", bound_end_json(bounds.upper.as_ref(), line)?),
                    (
                        "unit",
                        Json::optional(bounds.unit.as_deref().map(Json::string)),
                    ),
                    ("sources", strings_json(&bounds.sources)),
                    (
                        "from",
                        Json::optional(bounds.from.as_deref().map(bound_origins_json)),
                    ),
                ]),
            };
            members.push(("bounds", bounds));
        }
    }
    let quantities = answer
        .quantities
        .iter()
        .map(|quantity| {
            Ok(Json::object(vec![
                ("name", Json::string(&quantity.name)),
                ("mark", Json::string(name_of(&MARKS, quantity.mark))),
                (
                    "value",
                    Json::optional(
                        quantity
                            .value
                            .as_ref()
                            .map(|value| value_json(value, line))
                            .transpose()?,
                    ),
                ),
            ]))
        })
        .collect::<Result<Vec<_>, SaveError>>()?;
    members.push(("quantities", Json::Array(quantities)));
    Ok(Json::object(members))
}

fn identifiers(json: &Json, path: &JsonPath) -> Result<Vec<String>, LoadError> {
    array(json, path)?
        .iter()
        .enumerate()
        .map(|(position, element)| identifier(element, &path.index(position)).map(str::to_owned))
        .collect()
}

pub(crate) fn sorted_identifiers(json: &Json, path: &JsonPath) -> Result<Vec<String>, LoadError> {
    let texts = identifiers(json, path)?;
    if let Some(position) = texts.windows(2).position(|pair| pair[0] >= pair[1]) {
        return Err(LoadError::InvalidValue(path.index(position + 1)));
    }
    Ok(texts)
}

fn optional_count(json: &Json, path: &JsonPath) -> Result<Option<u64>, LoadError> {
    optional(json).map(|json| count(json, path)).transpose()
}

fn status_of<'json>(json: &'json Json, path: &JsonPath) -> Result<&'json str, LoadError> {
    let status_path = path.member("status");
    match json {
        Json::Object(object) => object
            .iter()
            .find(|(name, _)| name == "status")
            .map(|(_, member)| string(member, &status_path))
            .transpose()?
            .ok_or(LoadError::MissingMember(status_path)),
        _ => Err(LoadError::InvalidValue(path.clone())),
    }
}

fn criterion(json: &Json, path: &JsonPath) -> Result<SolveCriterion, LoadError> {
    let parts = members(json, path, &["by", "combine", "tie_break"])?;
    let by_path = path.member("by");
    let mut by = Vec::new();
    for (position, element) in array(parts[0], &by_path)?.iter().enumerate() {
        let element_path = by_path.index(position);
        let criterion = one_of(element, &element_path, &CRITERIA)?;
        if by.contains(&criterion) {
            return Err(LoadError::InvalidValue(element_path));
        }
        by.push(criterion);
    }
    if by.is_empty() {
        return Err(LoadError::InvalidValue(by_path));
    }
    let combine = one_of(parts[1], &path.member("combine"), &COMBINES)?;
    let tie_break_path = path.member("tie_break");
    let tie_break = identifiers(parts[2], &tie_break_path)?;
    if tie_break != TIE_BREAK {
        return Err(LoadError::InvalidValue(tie_break_path));
    }
    Ok(SolveCriterion { by, combine })
}

fn score(json: &Json, path: &JsonPath, by: Criterion) -> Result<Option<StoredScore>, LoadError> {
    let Some(json) = optional(json) else {
        return Ok(None);
    };
    let stored = match by {
        Criterion::FewestMeasurements | Criterion::FewestSteps | Criterion::GateCount => {
            StoredScore::Count(count(json, path)?)
        }
        Criterion::Exactness => StoredScore::Exactness(one_of(json, path, &EXACTNESSES)?),
        Criterion::ErrorBound => StoredScore::ErrorBound(one_of(json, path, &ERROR_BOUNDS)?),
        Criterion::SmallestUncertainty | Criterion::Conditioning => match number(json, path)? {
            exact @ (Number::Integer(_) | Number::Rational(_)) => StoredScore::Exact(exact),
            Number::F32(_) | Number::F64(_) => return Err(LoadError::InvalidValue(path.clone())),
        },
    };
    Ok(Some(stored))
}

fn way(
    pool: &mut ExprPool,
    json: &Json,
    path: &JsonPath,
    criterion: &SolveCriterion,
) -> Result<AnswerWay, LoadError> {
    let parts = members(
        json,
        path,
        &[
            "rank",
            "layer",
            "rule",
            "derivation",
            "inputs",
            "obtainable",
            "criterion",
            "conditions",
            "sources",
            "record",
        ],
    )?;
    let rule_path = path.member("rule");
    let rule = optional(parts[2])
        .map(|rule| identifier(rule, &rule_path).map(str::to_owned))
        .transpose()?;
    let scores_path = path.member("criterion");
    let names: Vec<&str> = criterion
        .by
        .iter()
        .map(|by| name_of(&CRITERIA, *by))
        .collect();
    let score_json = members(parts[6], &scores_path, &names)?;
    let scores = criterion
        .by
        .iter()
        .zip(score_json)
        .zip(&names)
        .map(|((by, json), name)| score(json, &scores_path.member(name), *by))
        .collect::<Result<Vec<_>, _>>()?;
    let conditions_path = path.member("conditions");
    let conditions = array(parts[7], &conditions_path)?
        .iter()
        .enumerate()
        .map(|(position, condition)| {
            let condition_path = conditions_path.index(position);
            let found = members(condition, &condition_path, &["text", "parameters", "holds"])?;
            let holds_path = condition_path.member("holds");
            Ok(AnswerCondition {
                text: identifier(found[0], &condition_path.member("text"))?.to_owned(),
                parameters: parameters(found[1], &condition_path.member("parameters"))?,
                holds: optional(found[2])
                    .map(|holds| boolean(holds, &holds_path))
                    .transpose()?,
            })
        })
        .collect::<Result<Vec<_>, LoadError>>()?;
    let record_path = path.member("record");
    let way_record = optional(parts[9])
        .map(|json| record(pool, json, &record_path).map(Box::new))
        .transpose()?;
    Ok(AnswerWay {
        rank: optional_count(parts[0], &path.member("rank"))?,
        layer: optional_count(parts[1], &path.member("layer"))?,
        rule,
        derivation: identifiers(parts[3], &path.member("derivation"))?,
        inputs: sorted_identifiers(parts[4], &path.member("inputs"))?,
        obtainable: boolean(parts[5], &path.member("obtainable"))?,
        scores,
        conditions,
        sources: sorted_identifiers(parts[8], &path.member("sources"))?,
        record: way_record,
    })
}

fn ways(
    pool: &mut ExprPool,
    json: &Json,
    path: &JsonPath,
    criterion: &SolveCriterion,
) -> Result<Vec<AnswerWay>, LoadError> {
    let listed = array(json, path)?
        .iter()
        .enumerate()
        .map(|(position, element)| way(pool, element, &path.index(position), criterion))
        .collect::<Result<Vec<_>, _>>()?;
    for (position, listed_way) in listed.iter().enumerate() {
        let way_path = path.index(position);
        let expected_rank = u64::try_from(position + 1).ok();
        match criterion.combine {
            Combine::Order if listed_way.layer.is_some() => {
                return Err(LoadError::InvalidValue(way_path.member("layer")));
            }
            Combine::Order if listed_way.rank != expected_rank => {
                return Err(LoadError::InvalidValue(way_path.member("rank")));
            }
            Combine::Front if listed_way.rank.is_some() => {
                return Err(LoadError::InvalidValue(way_path.member("rank")));
            }
            Combine::Front if !listed_way.layer.is_some_and(|layer| layer >= 1) => {
                return Err(LoadError::InvalidValue(way_path.member("layer")));
            }
            Combine::Order | Combine::Front => {}
        }
    }
    Ok(listed)
}

fn listing(parts: &[&Json], path: &JsonPath) -> Result<WayListing, LoadError> {
    let truncated = boolean(parts[0], &path.member("truncated"))?;
    let truncated_by_path = path.member("truncated_by");
    let truncated_by = optional(parts[1])
        .map(|json| one_of(json, &truncated_by_path, &TRUNCATIONS))
        .transpose()?;
    if truncated != truncated_by.is_some() {
        return Err(LoadError::InvalidValue(truncated_by_path));
    }
    Ok(WayListing {
        truncated_by,
        complete_through_steps: count(parts[2], &path.member("complete_through_steps"))?,
        ways_found: count(parts[3], &path.member("ways_found"))?,
    })
}

fn bound_end(json: &Json, path: &JsonPath) -> Result<Option<BoundEnd>, LoadError> {
    optional(json)
        .map(|json| {
            let parts = members(json, path, &["value", "inclusive"])?;
            Ok(BoundEnd {
                value: value(parts[0], &path.member("value"))?,
                inclusive: boolean(parts[1], &path.member("inclusive"))?,
            })
        })
        .transpose()
}

fn quantities(json: &Json, path: &JsonPath) -> Result<Vec<AnswerQuantity>, LoadError> {
    array(json, path)?
        .iter()
        .enumerate()
        .map(|(position, element)| {
            let quantity_path = path.index(position);
            let parts = members(element, &quantity_path, &["name", "mark", "value"])?;
            let value_path = quantity_path.member("value");
            Ok(AnswerQuantity {
                name: identifier(parts[0], &quantity_path.member("name"))?.to_owned(),
                mark: one_of(parts[1], &quantity_path.member("mark"), &MARKS)?,
                value: optional(parts[2])
                    .map(|json| value(json, &value_path))
                    .transpose()?,
            })
        })
        .collect()
}

fn has_record(ways: &[AnswerWay]) -> bool {
    ways.iter().any(|way| way.record.is_some())
}

pub(crate) fn answer(
    pool: &mut ExprPool,
    json: &Json,
    path: &JsonPath,
) -> Result<SolveAnswer, LoadError> {
    let status = status_of(json, path)?;
    let listing_names = [
        "truncated",
        "truncated_by",
        "complete_through_steps",
        "ways_found",
    ];
    let head = ["status", "concept_set_version", "criterion"];
    let names: Vec<&str> = match status {
        WAYS_STATUS | SOLVED_STATUS => head
            .iter()
            .chain(&listing_names)
            .chain(&["ways", "quantities"])
            .copied()
            .collect(),
        FRONT_STATUS => head
            .iter()
            .chain(&listing_names)
            .chain(&["front", "dominated", "quantities"])
            .copied()
            .collect(),
        NOT_REACHED_STATUS => head
            .iter()
            .chain(&["missing_any_of", "bounds", "quantities"])
            .copied()
            .collect(),
        _ => return Err(LoadError::InvalidValue(path.member("status"))),
    };
    let parts = members(json, path, &names)?;
    let version_path = path.member("concept_set_version");
    let version_text = string(parts[1], &version_path)?;
    let is_version = version_text.len() == VERSION_HEX_DIGITS
        && version_text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    let concept_set_version = is_version
        .then(|| u64::from_str_radix(version_text, 16).ok())
        .flatten()
        .ok_or(LoadError::InvalidValue(version_path))?;
    let answer_criterion = criterion(parts[2], &path.member("criterion"))?;
    let quantities_json = parts[parts.len() - 1];
    let body = match status {
        WAYS_STATUS | SOLVED_STATUS => {
            if answer_criterion.combine != Combine::Order {
                return Err(LoadError::InvalidValue(
                    path.member("criterion").member("combine"),
                ));
            }
            let listed = listing(&parts[3..7], path)?;
            let listed_ways = ways(pool, parts[7], &path.member("ways"), &answer_criterion)?;
            if (status == SOLVED_STATUS) != has_record(&listed_ways) {
                return Err(LoadError::InvalidValue(path.member("status")));
            }
            if status == SOLVED_STATUS {
                AnswerBody::Solved {
                    listing: listed,
                    ways: listed_ways,
                }
            } else {
                AnswerBody::Ways {
                    listing: listed,
                    ways: listed_ways,
                }
            }
        }
        FRONT_STATUS => {
            if answer_criterion.combine != Combine::Front {
                return Err(LoadError::InvalidValue(
                    path.member("criterion").member("combine"),
                ));
            }
            AnswerBody::Front {
                listing: listing(&parts[3..7], path)?,
                front: ways(pool, parts[7], &path.member("front"), &answer_criterion)?,
                dominated: ways(pool, parts[8], &path.member("dominated"), &answer_criterion)?,
            }
        }
        _ => {
            let missing_path = path.member("missing_any_of");
            let missing_any_of = array(parts[3], &missing_path)?
                .iter()
                .enumerate()
                .map(|(position, element)| {
                    let entry_path = missing_path.index(position);
                    let found = members(
                        element,
                        &entry_path,
                        &["add", "rule", "derivation", "between", "conditions"],
                    )?;
                    let add_path = entry_path.member("add");
                    let add = sorted_identifiers(found[0], &add_path)?;
                    if add.is_empty() {
                        return Err(LoadError::InvalidValue(add_path));
                    }
                    let between_path = entry_path.member("between");
                    let rule_path = entry_path.member("rule");
                    Ok(FurtherInput {
                        add,
                        rule: optional(found[1])
                            .map(|json| identifier(json, &rule_path).map(str::to_owned))
                            .transpose()?,
                        derivation: optional(found[2])
                            .map(|json| identifiers(json, &entry_path.member("derivation")))
                            .transpose()?,
                        between: optional(found[3])
                            .map(|json| identifiers(json, &between_path))
                            .transpose()?,
                        conditions: optional(found[4])
                            .map(|json| rule_conditions(json, &entry_path.member("conditions")))
                            .transpose()?,
                    })
                })
                .collect::<Result<Vec<_>, LoadError>>()?;
            let bounds_path = path.member("bounds");
            let bounds = optional(parts[4])
                .map(|json| {
                    let found = members(
                        json,
                        &bounds_path,
                        &["lower", "upper", "unit", "sources", "from"],
                    )?;
                    let unit_path = bounds_path.member("unit");
                    let unit_text = optional(found[2])
                        .map(|json| {
                            unit(pool, json, &unit_path)?;
                            string(json, &unit_path).map(str::to_owned)
                        })
                        .transpose()?;
                    Ok(Box::new(AnswerBounds {
                        lower: bound_end(found[0], &bounds_path.member("lower"))?,
                        upper: bound_end(found[1], &bounds_path.member("upper"))?,
                        unit: unit_text,
                        sources: sorted_identifiers(found[3], &bounds_path.member("sources"))?,
                        from: optional(found[4])
                            .map(|json| bound_origins(json, &bounds_path.member("from")))
                            .transpose()?,
                    }))
                })
                .transpose()?;
            AnswerBody::NotReached {
                missing_any_of,
                bounds,
            }
        }
    };
    Ok(SolveAnswer {
        concept_set_version,
        criterion: answer_criterion,
        body,
        quantities: quantities(quantities_json, &path.member("quantities"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;
    use crate::session::tests::session;
    use crate::session::{Line, Outcome, Settings};
    use crate::session_file::{decode, encode};

    const DIAMETER: &str = "circle.diameter";
    const RADIUS: &str = "circle.radius";
    const SOURCE: &str = "openstax-prealgebra-2e";

    fn line(number: u64) -> LineId {
        LineId::from_number(number).unwrap()
    }

    fn evaluated_record() -> Box<ResultRecord> {
        let mut evaluated = session();
        let id = evaluated.enter("2 * 3").unwrap();
        let Outcome::Result(record) = evaluated.line(id).unwrap().outcome().clone() else {
            panic!("expected a result");
        };
        Box::new(record.with_sources([SOURCE.to_owned()]))
    }

    fn way(rank: Option<u64>, layer: Option<u64>, input: &str) -> AnswerWay {
        AnswerWay {
            rank,
            layer,
            rule: Some(format!("circle-circumference/from-{input}")),
            derivation: vec![format!("circle-circumference/from-{input}")],
            inputs: vec![format!("circle.{input}")],
            obtainable: true,
            scores: vec![
                Some(StoredScore::Exactness(Exactness::Machine)),
                Some(StoredScore::ErrorBound(ErrorBound::Documented)),
                Some(StoredScore::Count(3)),
            ],
            conditions: vec![AnswerCondition {
                text: format!("{input} > 0"),
                parameters: Some(vec![ConditionParameter {
                    name: input.to_owned(),
                    role: format!("circle.{input}"),
                }]),
                holds: None,
            }],
            sources: vec![SOURCE.to_owned()],
            record: None,
        }
    }

    fn listing() -> WayListing {
        WayListing {
            truncated_by: None,
            complete_through_steps: 4,
            ways_found: 2,
        }
    }

    fn answer(combine: Combine, body: AnswerBody) -> SolveAnswer {
        SolveAnswer {
            concept_set_version: 0x0123_4567_89ab_cdef,
            criterion: SolveCriterion {
                by: vec![
                    Criterion::Exactness,
                    Criterion::ErrorBound,
                    Criterion::GateCount,
                ],
                combine,
            },
            body,
            quantities: vec![AnswerQuantity {
                name: DIAMETER.to_owned(),
                mark: QuantityMark::Given,
                value: Some(ResultValue::Number(Number::from(2_i64))),
            }],
        }
    }

    fn ways_answer() -> SolveAnswer {
        answer(
            Combine::Order,
            AnswerBody::Ways {
                listing: listing(),
                ways: vec![way(Some(1), None, "diameter"), way(Some(2), None, "radius")],
            },
        )
    }

    fn solve_lines(answer: SolveAnswer) -> Vec<Line> {
        vec![Line::restored(
            line(1),
            None,
            "solve {}".to_owned(),
            Outcome::Answer(Box::new(answer)),
        )]
    }

    fn save(lines: &[Line]) -> String {
        let bytes = encode(&ExprPool::new(), &Settings::default(), None, 2, lines).unwrap();
        String::from_utf8(bytes).unwrap()
    }

    fn load(text: &str) -> Result<Vec<Line>, LoadError> {
        decode(&mut ExprPool::new(), text.as_bytes()).map(|data| data.lines)
    }

    fn round_trips(answer: SolveAnswer) -> String {
        let lines = solve_lines(answer);
        let text = save(&lines);
        let loaded = load(&text).unwrap();
        assert_eq!(loaded, lines);
        assert_eq!(save(&loaded), text);
        text
    }

    fn replaced(text: &str, original: &str, replacement: &str) -> String {
        assert!(text.contains(original), "{original}");
        text.replacen(original, replacement, 1)
    }

    fn statuses(text: &str) -> Vec<&str> {
        text.split("\"status\": \"")
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .collect()
    }

    fn path(segments: &[&str]) -> JsonPath {
        JsonPath(
            segments
                .iter()
                .map(|segment| match segment.parse::<usize>() {
                    Ok(position) => PathSegment::Index(position),
                    Err(_) => PathSegment::Member((*segment).to_owned()),
                })
                .collect(),
        )
    }

    #[test]
    fn ways_outcome_round_trips() {
        let text = round_trips(ways_answer());

        assert_eq!(statuses(&text), vec!["ways", "ways"]);
        assert!(text.contains("\"concept_set_version\": \"0123456789abcdef\","));
    }

    #[test]
    fn front_answer_round_trips_as_a_ways_outcome() {
        let front = answer(
            Combine::Front,
            AnswerBody::Front {
                listing: listing(),
                front: vec![way(None, Some(1), "diameter")],
                dominated: vec![way(None, Some(2), "radius")],
            },
        );

        let text = round_trips(front);

        assert_eq!(statuses(&text), vec!["ways", "front"]);
    }

    #[test]
    fn solved_outcome_round_trips_with_record_sources() {
        let mut evaluated = way(Some(1), None, "diameter");
        evaluated.record = Some(evaluated_record());
        let solved = answer(
            Combine::Order,
            AnswerBody::Solved {
                listing: listing(),
                ways: vec![evaluated],
            },
        );

        let text = round_trips(solved);

        assert_eq!(statuses(&text)[..2], ["solved", "solved"]);
        let record_start = text.find("\"corpus_references\"").unwrap();
        let record_sources = &text[record_start..text.find("\"seed\"").unwrap()];
        assert!(record_sources.contains(SOURCE));
    }

    fn without_recognized(text: &str) -> String {
        let mut document = crate::json::parse(text.as_bytes()).unwrap();
        *member_mut(&mut document, "version") = Json::Count(10);
        let Json::Array(lines) = member_mut(&mut document, "lines") else {
            panic!("expected lines");
        };
        for line in lines {
            let outcome = member_mut(line, "outcome");
            remove_member(outcome, "recognized");
            if let Json::Object(members) = outcome
                && members.iter().any(|(name, _)| name == "record")
            {
                remove_member(member_mut(outcome, "record"), "recognized");
            }
            let Json::Object(members) = outcome else {
                continue;
            };
            if !members.iter().any(|(name, _)| name == "answer") {
                continue;
            }
            let answer = member_mut(outcome, "answer");
            for list in ["ways", "front", "dominated", "entries", "missing_any_of"] {
                let Json::Object(members) = answer else {
                    continue;
                };
                if !members.iter().any(|(name, _)| name == list) {
                    continue;
                }
                let Json::Array(items) = member_mut(answer, list) else {
                    continue;
                };
                for item in items {
                    remove_member(item, "recognized");
                    let Json::Object(members) = item else {
                        continue;
                    };
                    if members.iter().any(|(name, _)| name == "record") {
                        remove_member(member_mut(item, "record"), "recognized");
                    }
                }
            }
        }
        let stripped = crate::json::write_canonical(&document);
        assert!(!stripped.contains("recognized"));
        stripped
    }

    #[test]
    fn a_version_10_solved_line_converts_with_its_way_record() {
        let mut evaluated = way(Some(1), None, "diameter");
        evaluated.record = Some(evaluated_record());
        let solved = answer(
            Combine::Order,
            AnswerBody::Solved {
                listing: listing(),
                ways: vec![evaluated],
            },
        );
        let lines = solve_lines(solved);
        let text = save(&lines);
        let older = without_recognized(&text);

        assert_eq!(load(&older).unwrap(), lines);
    }

    #[test]
    fn a_version_10_front_answer_converts_with_its_way_record() {
        let mut evaluated = way(None, Some(1), "diameter");
        evaluated.record = Some(evaluated_record());
        let front = answer(
            Combine::Front,
            AnswerBody::Front {
                listing: listing(),
                front: vec![evaluated],
                dominated: vec![way(None, Some(2), "radius")],
            },
        );
        let lines = solve_lines(front);
        let text = save(&lines);
        let older = without_recognized(&text);

        assert_eq!(load(&older).unwrap(), lines);
    }

    #[test]
    fn not_reached_outcome_round_trips_with_bounds() {
        let not_reached = answer(
            Combine::Order,
            AnswerBody::NotReached {
                missing_any_of: vec![FurtherInput {
                    add: vec![RADIUS.to_owned()],
                    rule: Some("circle-circumference/from-radius".to_owned()),
                    derivation: Some(vec!["circle-circumference/from-radius".to_owned()]),
                    between: None,
                    conditions: Some(vec![ReachableCondition {
                        rule: "circle-circumference/from-radius".to_owned(),
                        text: "r > 0".to_owned(),
                        parameters: Some(vec![ConditionParameter {
                            name: "r".to_owned(),
                            role: RADIUS.to_owned(),
                        }]),
                        holds: None,
                    }]),
                }],
                bounds: Some(Box::new(AnswerBounds {
                    lower: Some(BoundEnd {
                        value: ResultValue::Number(Number::from(0_i64)),
                        inclusive: false,
                    }),
                    upper: None,
                    unit: Some("m".to_owned()),
                    sources: vec![SOURCE.to_owned()],
                    from: Some(vec![BoundFrom {
                        bound: "circle-circumference/positive".to_owned(),
                        kind: BoundKind::AtLeast,
                        inputs: vec![RADIUS.to_owned()],
                        ends: vec![BoundSide::Lower],
                    }]),
                })),
            },
        );

        let text = round_trips(not_reached);

        assert_eq!(statuses(&text), vec!["not_reached", "not_reached"]);
    }

    fn not_reached_text() -> String {
        let not_reached = answer(
            Combine::Order,
            AnswerBody::NotReached {
                missing_any_of: vec![FurtherInput {
                    add: vec![RADIUS.to_owned()],
                    rule: Some("circle-circumference/from-radius".to_owned()),
                    derivation: Some(vec!["circle-circumference/from-radius".to_owned()]),
                    between: None,
                    conditions: Some(vec![ReachableCondition {
                        rule: "circle-circumference/from-radius".to_owned(),
                        text: "r > 0".to_owned(),
                        parameters: Some(vec![ConditionParameter {
                            name: "r".to_owned(),
                            role: RADIUS.to_owned(),
                        }]),
                        holds: Some(false),
                    }]),
                }],
                bounds: Some(Box::new(AnswerBounds {
                    lower: None,
                    upper: Some(BoundEnd {
                        value: ResultValue::Number(Number::from(6_i64)),
                        inclusive: true,
                    }),
                    unit: None,
                    sources: vec![SOURCE.to_owned()],
                    from: Some(vec![
                        BoundFrom {
                            bound: "triangle-area/two-sides".to_owned(),
                            kind: BoundKind::AtMost,
                            inputs: vec!["side-a".to_owned(), "side-b".to_owned()],
                            ends: vec![BoundSide::Upper],
                        },
                        BoundFrom {
                            bound: "triangle-area/zero".to_owned(),
                            kind: BoundKind::StrictlyBetween,
                            inputs: vec!["side-a".to_owned()],
                            ends: vec![BoundSide::Lower, BoundSide::Upper],
                        },
                    ]),
                })),
            },
        );
        round_trips(not_reached)
    }

    #[test]
    fn further_input_conditions_and_bound_origins_round_trip_in_member_order() {
        let text = not_reached_text();
        let entry =
            &text[text.find("\"missing_any_of\"").unwrap()..text.find("\"bounds\"").unwrap()];
        let order: Vec<usize> = [
            "\"add\"",
            "\"rule\"",
            "\"derivation\"",
            "\"between\"",
            "\"conditions\"",
        ]
        .iter()
        .map(|key| entry.find(key).unwrap())
        .collect();
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(text.contains("\"kind\": \"strictly_between\",\n"));
    }

    fn remove_member(json: &mut Json, name: &str) {
        if let Json::Object(members) = json {
            members.retain(|(member, _)| member != name);
        }
    }

    fn member_mut<'json>(json: &'json mut Json, name: &str) -> &'json mut Json {
        let Json::Object(members) = json else {
            panic!("expected an object holding {name}");
        };
        &mut members
            .iter_mut()
            .find(|(member, _)| member == name)
            .unwrap()
            .1
    }

    fn version_3_of(text: &str) -> String {
        let mut document = crate::json::parse(text.as_bytes()).unwrap();
        *member_mut(&mut document, "version") = Json::Count(3);
        remove_member(member_mut(&mut document, "settings"), "units");
        let Json::Array(lines) = member_mut(&mut document, "lines") else {
            panic!("expected lines");
        };
        let answer = member_mut(member_mut(&mut lines[0], "outcome"), "answer");
        let Json::Array(entries) = member_mut(answer, "missing_any_of") else {
            panic!("expected missing_any_of");
        };
        for entry in entries {
            remove_member(entry, "derivation");
            remove_member(entry, "conditions");
        }
        remove_member(member_mut(answer, "bounds"), "from");
        crate::json::write_canonical(&document)
    }

    fn converted_not_reached() -> AnswerBody {
        let lines = load(&version_3_of(&not_reached_text())).unwrap();
        match lines[0].outcome() {
            Outcome::Answer(answer) => answer.body.clone(),
            other => panic!("expected an answer, found {other:?}"),
        }
    }

    #[test]
    fn version_3_entry_converts_with_derivation_not_recorded() {
        let AnswerBody::NotReached { missing_any_of, .. } = converted_not_reached() else {
            panic!("expected not reached");
        };
        assert_eq!(missing_any_of[0].derivation, None);
    }

    #[test]
    fn version_3_entry_converts_with_conditions_not_recorded() {
        let AnswerBody::NotReached { missing_any_of, .. } = converted_not_reached() else {
            panic!("expected not reached");
        };
        assert_eq!(missing_any_of[0].conditions, None);
    }

    #[test]
    fn version_3_entry_keeps_its_stored_rule() {
        let AnswerBody::NotReached { missing_any_of, .. } = converted_not_reached() else {
            panic!("expected not reached");
        };
        assert_eq!(
            missing_any_of[0].rule.as_deref(),
            Some("circle-circumference/from-radius")
        );
    }

    fn version_6_of(text: &str) -> String {
        let mut document = crate::json::parse(text.as_bytes()).unwrap();
        *member_mut(&mut document, "version") = Json::Count(6);
        let Json::Array(lines) = member_mut(&mut document, "lines") else {
            panic!("expected lines");
        };
        let answer = member_mut(member_mut(&mut lines[0], "outcome"), "answer");
        let Json::Array(entries) = member_mut(answer, "missing_any_of") else {
            panic!("expected missing_any_of");
        };
        for entry in entries {
            let Json::Array(conditions) = member_mut(entry, "conditions") else {
                panic!("expected conditions");
            };
            for condition in conditions {
                remove_member(condition, "parameters");
            }
        }
        crate::json::write_canonical(&document)
    }

    #[test]
    fn version_6_condition_converts_with_parameters_not_recorded() {
        let lines = load(&version_6_of(&not_reached_text())).unwrap();

        let Outcome::Answer(answer) = lines[0].outcome() else {
            panic!("expected an answer");
        };
        let AnswerBody::NotReached { missing_any_of, .. } = &answer.body else {
            panic!("expected not reached");
        };
        assert_eq!(
            missing_any_of[0].conditions.as_ref().unwrap()[0].parameters,
            None
        );
    }

    #[test]
    fn stored_condition_writes_its_parameters_between_text_and_holds() {
        let mut document = crate::json::parse(not_reached_text().as_bytes()).unwrap();
        let Json::Array(lines) = member_mut(&mut document, "lines") else {
            panic!("expected lines");
        };
        let answer = member_mut(member_mut(&mut lines[0], "outcome"), "answer");
        let Json::Array(entries) = member_mut(answer, "missing_any_of") else {
            panic!("expected missing_any_of");
        };
        let Json::Array(conditions) = member_mut(&mut entries[0], "conditions") else {
            panic!("expected conditions");
        };
        let Json::Object(members) = &conditions[0] else {
            panic!("expected a condition object");
        };

        let names: Vec<&str> = members.iter().map(|(name, _)| name.as_str()).collect();

        assert_eq!(names, ["rule", "text", "parameters", "holds"]);
    }

    #[test]
    fn version_3_bounds_convert_with_origins_not_recorded() {
        let AnswerBody::NotReached { bounds, .. } = converted_not_reached() else {
            panic!("expected not reached");
        };
        assert_eq!(bounds.unwrap().from, None);
    }

    #[test]
    fn converted_version_3_answer_saves_as_the_current_version_and_round_trips() {
        let lines = load(&version_3_of(&not_reached_text())).unwrap();
        let saved = save(&lines);

        assert!(saved.contains(&format!(
            "\"version\": {},",
            crate::session_file::FORMAT_VERSION
        )));
        assert_eq!(load(&saved).unwrap(), lines);
    }

    fn loaded_not_reached(text: &str) -> AnswerBody {
        match load(text).unwrap()[0].outcome() {
            Outcome::Answer(answer) => answer.body.clone(),
            other => panic!("expected an answer, found {other:?}"),
        }
    }

    fn first_entry(body: AnswerBody) -> FurtherInput {
        let AnswerBody::NotReached { missing_any_of, .. } = body else {
            panic!("expected not reached");
        };
        missing_any_of[0].clone()
    }

    fn origins(body: AnswerBody) -> Vec<BoundFrom> {
        let AnswerBody::NotReached { bounds, .. } = body else {
            panic!("expected not reached");
        };
        bounds.unwrap().from.unwrap()
    }

    #[test]
    fn condition_that_fails_is_written_with_holds_false() {
        assert!(not_reached_text().contains("\"holds\": false\n"));
    }

    #[test]
    fn condition_that_holds_is_read_with_holds_true() {
        let text = replaced(&not_reached_text(), "\"holds\": false", "\"holds\": true");
        let conditions = first_entry(loaded_not_reached(&text)).conditions.unwrap();
        assert_eq!(conditions[0].holds, Some(true));
    }

    #[test]
    fn undecided_condition_is_read_with_holds_null() {
        let text = replaced(&not_reached_text(), "\"holds\": false", "\"holds\": null");
        let conditions = first_entry(loaded_not_reached(&text)).conditions.unwrap();
        assert_eq!(conditions[0].holds, None);
    }

    #[test]
    fn condition_carries_its_rule_and_text() {
        let conditions = first_entry(loaded_not_reached(&not_reached_text()))
            .conditions
            .unwrap();
        assert_eq!(
            (conditions[0].rule.as_str(), conditions[0].text.as_str()),
            ("circle-circumference/from-radius", "r > 0")
        );
    }

    #[test]
    fn entry_carries_its_staged_derivation() {
        assert_eq!(
            first_entry(loaded_not_reached(&not_reached_text())).derivation,
            Some(vec!["circle-circumference/from-radius".to_owned()])
        );
    }

    #[test]
    fn bound_origin_carries_its_bound_and_kind() {
        let origin = &origins(loaded_not_reached(&not_reached_text()))[0];
        assert_eq!(
            (origin.bound.as_str(), origin.kind),
            ("triangle-area/two-sides", BoundKind::AtMost)
        );
    }

    #[test]
    fn bound_origin_carries_its_inputs() {
        assert_eq!(
            origins(loaded_not_reached(&not_reached_text()))[0].inputs,
            vec!["side-a".to_owned(), "side-b".to_owned()]
        );
    }

    #[test]
    fn bound_origin_carries_the_ends_it_sets() {
        assert_eq!(
            origins(loaded_not_reached(&not_reached_text()))[1].ends,
            vec![BoundSide::Lower, BoundSide::Upper]
        );
    }

    #[test]
    fn bound_origin_with_an_unknown_kind_is_rejected() {
        let text = not_reached_text();
        let changed = replaced(&text, "\"kind\": \"at_most\"", "\"kind\": \"below\"");
        assert_eq!(
            load(&changed).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "0", "outcome", "answer", "bounds", "from", "0", "kind"
            ])))
        );
    }

    #[test]
    fn bound_origins_out_of_identifier_order_are_rejected() {
        let text = not_reached_text();
        let changed = replaced(
            &text,
            "\"bound\": \"triangle-area/zero\"",
            "\"bound\": \"triangle-area/a\"",
        );
        assert_eq!(
            load(&changed).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "0", "outcome", "answer", "bounds", "from", "1", "bound"
            ])))
        );
    }

    #[test]
    fn bound_origin_with_ends_out_of_order_is_rejected() {
        let text = not_reached_text();
        let start = text.find("\"lower\",\n").unwrap();
        let changed = format!(
            "{}\"upper\",{}",
            &text[..start],
            text[start + "\"lower\",".len()..].replacen("\"upper\"", "\"lower\"", 1)
        );
        assert_eq!(
            load(&changed).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "0", "outcome", "answer", "bounds", "from", "1", "ends"
            ])))
        );
    }

    #[test]
    fn outcome_status_that_contradicts_the_answer_is_rejected() {
        let text = save(&solve_lines(ways_answer()));
        let changed = replaced(&text, "\"status\": \"ways\"", "\"status\": \"solved\"");

        assert_eq!(
            load(&changed).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "0", "outcome", "status"
            ])))
        );
    }

    #[test]
    fn solved_answer_without_a_record_is_rejected() {
        let text = save(&solve_lines(ways_answer()));
        let changed = text.replace("\"status\": \"ways\"", "\"status\": \"solved\"");

        assert_eq!(
            load(&changed).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "0", "outcome", "answer", "status"
            ])))
        );
    }

    #[test]
    fn other_tie_break_is_rejected() {
        let text = save(&solve_lines(ways_answer()));
        let changed = replaced(&text, "\"rule_id\",", "\"inputs\",");

        assert_eq!(
            load(&changed).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "0",
                "outcome",
                "answer",
                "criterion",
                "tie_break"
            ])))
        );
    }

    #[test]
    fn rank_out_of_listing_order_is_rejected() {
        let text = save(&solve_lines(ways_answer()));
        let changed = replaced(&text, "\"rank\": 2,", "\"rank\": 3,");

        assert_eq!(
            load(&changed).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines", "0", "outcome", "answer", "ways", "1", "rank"
            ])))
        );
    }

    #[test]
    fn truncated_flag_without_its_cause_is_rejected() {
        let text = save(&solve_lines(ways_answer()));
        let changed = replaced(&text, "\"truncated\": false,", "\"truncated\": true,");

        assert_eq!(
            load(&changed).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "0",
                "outcome",
                "answer",
                "truncated_by"
            ])))
        );
    }

    #[test]
    fn score_of_the_wrong_type_is_rejected() {
        let text = save(&solve_lines(ways_answer()));
        let changed = replaced(&text, "\"gate_count\": 3", "\"gate_count\": \"exact\"");

        assert_eq!(
            load(&changed).err(),
            Some(LoadError::InvalidValue(path(&[
                "lines",
                "0",
                "outcome",
                "answer",
                "ways",
                "0",
                "criterion",
                "gate_count"
            ])))
        );
    }

    #[test]
    fn way_record_depending_on_a_missing_line_is_rejected() {
        let mut evaluated = way(Some(1), None, "diameter");
        evaluated.record = Some(evaluated_record());
        let solved = answer(
            Combine::Order,
            AnswerBody::Solved {
                listing: listing(),
                ways: vec![evaluated],
            },
        );
        let text = save(&solve_lines(solved));
        let changed = replaced(
            &text,
            "\"dependencies\": [],",
            "\"dependencies\": [\"r1\", \"r9\"],",
        );

        assert_eq!(
            load(&changed).err(),
            Some(LoadError::MissingDependency(path(&[
                "lines",
                "0",
                "outcome",
                "answer",
                "ways",
                "0",
                "record",
                "dependencies",
                "1"
            ])))
        );
    }

    #[test]
    fn canonical_answer_text_parses_as_json() {
        let text = save(&solve_lines(ways_answer()));

        assert!(json::parse(text.as_bytes()).is_ok());
    }
}
