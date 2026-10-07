use calc_core::{Combine, Criterion, Diagnostic, ParameterValue};
use calc_exec::{BackendKind, Preference};

use crate::json::{self, Json};
use crate::messages::json_path_text;
use crate::session_file::JsonPath;
use crate::solve_answer::{COMBINES, CRITERIA, SolveCriterion, TIE_BREAK, criterion_json, name_of};

pub(crate) const SOLVE_PREFIX: &str = "solve ";
const REQUEST_OPENING: char = '{';
const DEFAULT_CAP: u32 = 5;
const LARGEST_CAP: u32 = 1000;
const DEFAULT_WORK_BUDGET: u64 = 100_000;
const LARGEST_WORK_BUDGET: u64 = 10_000_000;
const PATH_DATA: &str = "path";
const DEFAULT_REACHABLE_CAP: u32 = 20;
const PHASES: [(&str, Phase); 3] = [
    ("ways", Phase::Ways),
    ("evaluate", Phase::Evaluate),
    ("reachable", Phase::Reachable),
];
const REACHABLE_EXCLUDED_MEMBERS: [usize; 5] = [5, 6, 7, 9, 10];
const BACKENDS: [(&str, Preference); 4] = [
    ("automatic", Preference::Automatic),
    ("cpu", Preference::Only(BackendKind::Cpu)),
    ("simd", Preference::Only(BackendKind::Simd)),
    ("gpu", Preference::Only(BackendKind::Gpu)),
];
const REQUEST_MEMBERS: [&str; 11] = [
    "phase",
    "object",
    "wanted",
    "given",
    "obtainable",
    "not_obtainable",
    "only_obtainable",
    "criterion",
    "cap",
    "work_budget",
    "backend",
];
const GIVEN_MEMBERS: [&str; 5] = ["name", "value", "uncertainty", "coverage_factor", "unit"];
const CRITERION_MEMBERS: [&str; 3] = ["by", "combine", "tie_break"];
const ERROR_CODES: [(&str, RequestErrorCode); 16] = [
    ("missing_wanted", RequestErrorCode::MissingWanted),
    ("unexpected_wanted", RequestErrorCode::UnexpectedWanted),
    ("unexpected_member", RequestErrorCode::UnexpectedMember),
    ("ambiguous_object", RequestErrorCode::AmbiguousObject),
    ("unknown_object", RequestErrorCode::UnknownObject),
    ("unknown_wanted", RequestErrorCode::UnknownWanted),
    ("ambiguous_wanted", RequestErrorCode::AmbiguousWanted),
    ("unknown_role", RequestErrorCode::UnknownRole),
    ("unknown_criterion", RequestErrorCode::UnknownCriterion),
    ("invalid_tie_break", RequestErrorCode::InvalidTieBreak),
    ("contradictory_marks", RequestErrorCode::ContradictoryMarks),
    ("invalid_value", RequestErrorCode::InvalidValue),
    ("dimension_mismatch", RequestErrorCode::DimensionMismatch),
    ("invalid_cap", RequestErrorCode::InvalidCap),
    ("invalid_work_budget", RequestErrorCode::InvalidWorkBudget),
    ("malformed_request", RequestErrorCode::MalformedRequest),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Ways,
    Evaluate,
    Reachable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GivenInput {
    pub name: String,
    pub value: String,
    pub uncertainty: Option<String>,
    pub coverage_factor: Option<String>,
    pub unit: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolveRequest {
    pub phase: Phase,
    pub object: Option<String>,
    pub wanted: Option<String>,
    pub given: Vec<GivenInput>,
    pub obtainable: Vec<String>,
    pub not_obtainable: Vec<String>,
    pub only_obtainable: bool,
    pub criterion: SolveCriterion,
    pub cap: u32,
    pub work_budget: u64,
    pub backend: Preference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestErrorCode {
    UnknownObject,
    UnknownWanted,
    AmbiguousWanted,
    UnknownRole,
    UnknownCriterion,
    InvalidTieBreak,
    ContradictoryMarks,
    InvalidValue,
    DimensionMismatch,
    InvalidCap,
    InvalidWorkBudget,
    MalformedRequest,
    MissingWanted,
    UnexpectedWanted,
    UnexpectedMember,
    AmbiguousObject,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestError {
    pub code: RequestErrorCode,
    pub path: JsonPath,
}

impl RequestError {
    pub(crate) fn new(code: RequestErrorCode, path: JsonPath) -> Self {
        Self { code, path }
    }

    pub fn typed_json(&self) -> Vec<u8> {
        crate::solve::typed_error_json(
            name_of(&ERROR_CODES, self.code),
            &[(PATH_DATA, &json_path_text(&self.path))],
        )
    }

    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic {
            code: name_of(&ERROR_CODES, self.code).to_owned(),
            data: [(
                PATH_DATA.to_owned(),
                ParameterValue::Identifier(json_path_text(&self.path)),
            )]
            .into_iter()
            .collect(),
        }
    }
}

impl SolveCriterion {
    pub fn program_default() -> Self {
        Self {
            by: vec![
                Criterion::Exactness,
                Criterion::ErrorBound,
                Criterion::GateCount,
            ],
            combine: Combine::Order,
        }
    }
}

impl SolveRequest {
    pub fn with_defaults(wanted: String) -> Self {
        Self {
            phase: Phase::Ways,
            object: None,
            wanted: Some(wanted),
            given: Vec::new(),
            obtainable: Vec::new(),
            not_obtainable: Vec::new(),
            only_obtainable: false,
            criterion: SolveCriterion::program_default(),
            cap: DEFAULT_CAP,
            work_budget: DEFAULT_WORK_BUDGET,
            backend: Preference::Automatic,
        }
    }

    pub fn line_text(&self) -> String {
        format!("{SOLVE_PREFIX}{}", json::write_one_line(&self.to_json()))
    }

    pub(crate) fn has_mark_conflict(&self) -> Option<JsonPath> {
        let root = JsonPath::default();
        self.not_obtainable
            .iter()
            .position(|name| {
                self.obtainable.contains(name) || self.given.iter().any(|given| &given.name == name)
            })
            .map(|position| root.member("not_obtainable").index(position))
    }

    fn to_json(&self) -> Json {
        let names =
            |names: &[String]| Json::Array(names.iter().map(|name| Json::string(name)).collect());
        let optional_text =
            |text: &Option<String>| Json::optional(text.as_deref().map(Json::string));
        let given = self
            .given
            .iter()
            .map(|given| {
                Json::object(vec![
                    ("name", Json::string(&given.name)),
                    ("value", Json::string(&given.value)),
                    ("uncertainty", optional_text(&given.uncertainty)),
                    ("coverage_factor", optional_text(&given.coverage_factor)),
                    ("unit", optional_text(&given.unit)),
                ])
            })
            .collect();
        let mut members = vec![
            ("phase", Json::string(name_of(&PHASES, self.phase))),
            ("object", optional_text(&self.object)),
        ];
        if self.phase != Phase::Reachable {
            members.push(("wanted", optional_text(&self.wanted)));
        }
        members.push(("given", Json::Array(given)));
        members.push(("obtainable", names(&self.obtainable)));
        if self.phase != Phase::Reachable {
            members.push(("not_obtainable", names(&self.not_obtainable)));
            members.push(("only_obtainable", Json::Boolean(self.only_obtainable)));
            members.push(("criterion", criterion_json(&self.criterion)));
        }
        members.push(("cap", Json::Count(u64::from(self.cap))));
        if self.phase != Phase::Reachable {
            members.push(("work_budget", Json::Count(self.work_budget)));
            members.push(("backend", Json::string(name_of(&BACKENDS, self.backend))));
        }
        Json::object(members)
    }
}

fn malformed(path: &JsonPath) -> RequestError {
    RequestError::new(RequestErrorCode::MalformedRequest, path.clone())
}

fn object_members<'json>(
    json: &'json Json,
    path: &JsonPath,
    allowed: &[&str],
) -> Result<Vec<Option<&'json Json>>, RequestError> {
    let Json::Object(object) = json else {
        return Err(malformed(path));
    };
    let mut found: Vec<Option<&Json>> = vec![None; allowed.len()];
    for (name, member) in object {
        let position = allowed
            .iter()
            .position(|allowed_name| allowed_name == name)
            .ok_or_else(|| malformed(&path.member(name)))?;
        if found[position].replace(member).is_some() {
            return Err(malformed(&path.member(name)));
        }
    }
    Ok(found)
}

fn present(member: Option<&Json>) -> Option<&Json> {
    member.filter(|json| **json != Json::Null)
}

fn text(json: &Json, path: &JsonPath) -> Result<String, RequestError> {
    match json {
        Json::String(text) if !text.is_empty() => Ok(text.clone()),
        _ => Err(malformed(path)),
    }
}

fn texts(member: Option<&Json>, path: &JsonPath) -> Result<Vec<String>, RequestError> {
    let Some(json) = present(member) else {
        return Ok(Vec::new());
    };
    let Json::Array(elements) = json else {
        return Err(malformed(path));
    };
    elements
        .iter()
        .enumerate()
        .map(|(position, element)| text(element, &path.index(position)))
        .collect()
}

fn choice<T: Copy>(json: &Json, path: &JsonPath, choices: &[(&str, T)]) -> Result<T, RequestError> {
    let name = text(json, path)?;
    choices
        .iter()
        .find(|(choice_name, _)| *choice_name == name)
        .map(|(_, choice)| *choice)
        .ok_or_else(|| malformed(path))
}

fn optional_text(member: Option<&Json>, path: &JsonPath) -> Result<Option<String>, RequestError> {
    present(member).map(|json| text(json, path)).transpose()
}

fn given(json: &Json, path: &JsonPath) -> Result<GivenInput, RequestError> {
    let found = object_members(json, path, &GIVEN_MEMBERS)?;
    let required = |position: usize| {
        present(found[position]).ok_or_else(|| malformed(&path.member(GIVEN_MEMBERS[position])))
    };
    Ok(GivenInput {
        name: text(required(0)?, &path.member("name"))?,
        value: text(required(1)?, &path.member("value"))?,
        uncertainty: optional_text(found[2], &path.member("uncertainty"))?,
        coverage_factor: optional_text(found[3], &path.member("coverage_factor"))?,
        unit: optional_text(found[4], &path.member("unit"))?,
    })
}

fn criterion(member: Option<&Json>, path: &JsonPath) -> Result<SolveCriterion, RequestError> {
    let Some(json) = present(member) else {
        return Ok(SolveCriterion::program_default());
    };
    let found = object_members(json, path, &CRITERION_MEMBERS)?;
    let by_path = path.member("by");
    let unknown = |path: JsonPath| RequestError::new(RequestErrorCode::UnknownCriterion, path);
    let Some(Json::Array(by_json)) = present(found[0]) else {
        return Err(unknown(by_path));
    };
    let mut by = Vec::new();
    for (position, element) in by_json.iter().enumerate() {
        let element_path = by_path.index(position);
        let criterion =
            choice(element, &element_path, &CRITERIA).map_err(|_| unknown(element_path.clone()))?;
        if by.contains(&criterion) {
            return Err(unknown(element_path));
        }
        by.push(criterion);
    }
    if by.is_empty() {
        return Err(unknown(by_path));
    }
    let combine = match present(found[1]) {
        Some(json) => choice(json, &path.member("combine"), &COMBINES)?,
        None => Combine::Order,
    };
    if let Some(json) = present(found[2]) {
        let tie_break_path = path.member("tie_break");
        let invalid =
            || RequestError::new(RequestErrorCode::InvalidTieBreak, tie_break_path.clone());
        let listed = texts(Some(json), &tie_break_path).map_err(|_| invalid())?;
        if listed != TIE_BREAK {
            return Err(invalid());
        }
    }
    Ok(SolveCriterion { by, combine })
}

pub fn parse_request(bytes: &[u8]) -> Result<SolveRequest, RequestError> {
    let root = JsonPath::default();
    let document = json::parse(bytes).map_err(|_| malformed(&root))?;
    let found = object_members(&document, &root, &REQUEST_MEMBERS)?;
    let phase_path = root.member("phase");
    let phase = choice(
        present(found[0]).ok_or_else(|| malformed(&phase_path))?,
        &phase_path,
        &PHASES,
    )?;
    let wanted_path = root.member("wanted");
    let wanted = optional_text(found[2], &wanted_path)?;
    match (phase, &wanted) {
        (Phase::Reachable, Some(_)) => {
            return Err(RequestError::new(
                RequestErrorCode::UnexpectedWanted,
                wanted_path,
            ));
        }
        (Phase::Ways | Phase::Evaluate, None) => {
            return Err(RequestError::new(
                RequestErrorCode::MissingWanted,
                wanted_path,
            ));
        }
        _ => {}
    }
    if phase == Phase::Reachable
        && let Some(position) = REACHABLE_EXCLUDED_MEMBERS
            .iter()
            .find(|position| found[**position].is_some())
    {
        return Err(RequestError::new(
            RequestErrorCode::UnexpectedMember,
            root.member(REQUEST_MEMBERS[*position]),
        ));
    }
    let given_path = root.member("given");
    let given = match present(found[3]) {
        None => Vec::new(),
        Some(Json::Array(elements)) => elements
            .iter()
            .enumerate()
            .map(|(position, element)| given(element, &given_path.index(position)))
            .collect::<Result<_, _>>()?,
        Some(_) => return Err(malformed(&given_path)),
    };
    let only_obtainable = match present(found[6]) {
        None => false,
        Some(Json::Boolean(flag)) => *flag,
        Some(_) => return Err(malformed(&root.member("only_obtainable"))),
    };
    let cap_path = root.member("cap");
    let cap = match present(found[8]) {
        None if phase == Phase::Reachable => DEFAULT_REACHABLE_CAP,
        None => DEFAULT_CAP,
        Some(Json::Count(cap)) => u32::try_from(*cap)
            .ok()
            .filter(|cap| (1..=LARGEST_CAP).contains(cap))
            .ok_or_else(|| RequestError::new(RequestErrorCode::InvalidCap, cap_path.clone()))?,
        Some(_) => return Err(RequestError::new(RequestErrorCode::InvalidCap, cap_path)),
    };
    let budget_path = root.member("work_budget");
    let work_budget = match present(found[9]) {
        None => DEFAULT_WORK_BUDGET,
        Some(Json::Count(budget)) if (1..=LARGEST_WORK_BUDGET).contains(budget) => *budget,
        Some(_) => {
            return Err(RequestError::new(
                RequestErrorCode::InvalidWorkBudget,
                budget_path,
            ));
        }
    };
    let request = SolveRequest {
        phase,
        object: optional_text(found[1], &root.member("object"))?,
        wanted,
        given,
        obtainable: texts(found[4], &root.member("obtainable"))?,
        not_obtainable: texts(found[5], &root.member("not_obtainable"))?,
        only_obtainable,
        criterion: criterion(found[7], &root.member("criterion"))?,
        cap,
        work_budget,
        backend: match present(found[10]) {
            None => Preference::Automatic,
            Some(json) => choice(json, &root.member("backend"), &BACKENDS)?,
        },
    };
    if let Some(conflict) = request.has_mark_conflict() {
        return Err(RequestError::new(
            RequestErrorCode::ContradictoryMarks,
            conflict,
        ));
    }
    if phase == Phase::Reachable
        && let Some(position) = request
            .obtainable
            .iter()
            .position(|name| request.given.iter().any(|given| &given.name == name))
    {
        return Err(RequestError::new(
            RequestErrorCode::ContradictoryMarks,
            root.member("obtainable").index(position),
        ));
    }
    Ok(request)
}

pub(crate) fn request_from_line_text(text: &str) -> Option<Result<SolveRequest, RequestError>> {
    text.strip_prefix(SOLVE_PREFIX)
        .filter(|request| request.trim_start().starts_with(REQUEST_OPENING))
        .map(|request| parse_request(request.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_file::PathSegment;

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

    fn error(text: &str) -> RequestError {
        parse_request(text.as_bytes()).unwrap_err()
    }

    #[test]
    fn smallest_request_takes_the_program_defaults() {
        let request = parse_request(br#"{"phase": "ways", "wanted": "area"}"#).unwrap();
        assert_eq!(
            (
                request.cap,
                request.work_budget,
                request.criterion.clone(),
                request.backend
            ),
            (
                5,
                100_000,
                SolveCriterion::program_default(),
                Preference::Automatic
            )
        );
    }

    #[test]
    fn line_text_writes_every_member_on_one_line() {
        let request = parse_request(br#"{"phase": "ways", "wanted": "area"}"#).unwrap();
        assert_eq!(
            request.line_text(),
            "solve {\"phase\": \"ways\", \"object\": null, \"wanted\": \"area\", \"given\": [], \"obtainable\": [], \"not_obtainable\": [], \"only_obtainable\": false, \"criterion\": {\"by\": [\"exactness\", \"error_bound\", \"gate_count\"], \"combine\": \"order\", \"tie_break\": [\"exactness\", \"error_bound\", \"gate_count\", \"rule_id\", \"inputs\"]}, \"cap\": 5, \"work_budget\": 100000, \"backend\": \"automatic\"}"
        );
    }

    #[test]
    fn line_text_parses_back_to_the_same_request() {
        let request = parse_request(
            br#"{"phase": "evaluate", "object": "circle", "wanted": "circumference", "given": [{"name": "diameter", "value": "2", "uncertainty": "0.01", "coverage_factor": null, "unit": "cm"}], "obtainable": ["radius"], "criterion": {"by": ["fewest_measurements"], "combine": "front"}, "cap": 7}"#,
        )
        .unwrap();
        let reparsed = request_from_line_text(&request.line_text())
            .unwrap()
            .unwrap();
        assert_eq!(reparsed, request);
    }

    #[test]
    fn text_that_is_not_json_is_malformed() {
        assert_eq!(
            error("area?"),
            RequestError::new(RequestErrorCode::MalformedRequest, JsonPath::default())
        );
    }

    #[test]
    fn unknown_member_is_malformed_at_its_path() {
        assert_eq!(
            error(r#"{"phase": "ways", "wanted": "area", "speed": 1}"#),
            RequestError::new(RequestErrorCode::MalformedRequest, path(&["speed"]))
        );
    }

    #[test]
    fn ways_request_without_wanted_is_missing_wanted() {
        assert_eq!(
            error(r#"{"phase": "ways"}"#),
            RequestError::new(RequestErrorCode::MissingWanted, path(&["wanted"]))
        );
    }

    #[test]
    fn reachable_request_with_wanted_is_unexpected_wanted() {
        assert_eq!(
            error(r#"{"phase": "reachable", "wanted": "area"}"#),
            RequestError::new(RequestErrorCode::UnexpectedWanted, path(&["wanted"]))
        );
    }

    #[test]
    fn reachable_request_with_a_criterion_is_unexpected_member() {
        assert_eq!(
            error(
                r#"{"phase": "reachable", "object": "circle", "criterion": {"by": ["exactness"]}}"#
            ),
            RequestError::new(RequestErrorCode::UnexpectedMember, path(&["criterion"]))
        );
    }

    #[test]
    fn reachable_request_with_a_given_role_also_obtainable_is_contradictory() {
        assert_eq!(
            error(
                r#"{"phase": "reachable", "given": [{"name": "radius", "value": "1"}], "obtainable": ["radius"]}"#
            ),
            RequestError::new(
                RequestErrorCode::ContradictoryMarks,
                path(&["obtainable", "0"])
            )
        );
    }

    #[test]
    fn reachable_request_takes_a_cap_of_twenty() {
        let request = parse_request(br#"{"phase": "reachable", "object": "circle"}"#).unwrap();
        assert_eq!((request.cap, request.wanted), (20, None));
    }

    #[test]
    fn reachable_line_text_writes_only_the_reachable_members() {
        let request = parse_request(
            br#"{"phase": "reachable", "object": "circle", "obtainable": ["radius"]}"#,
        )
        .unwrap();
        assert_eq!(
            request.line_text(),
            "solve {\"phase\": \"reachable\", \"object\": \"circle\", \"given\": [], \"obtainable\": [\"radius\"], \"cap\": 20}"
        );
        assert_eq!(
            request_from_line_text(&request.line_text()),
            Some(Ok(request))
        );
    }

    #[test]
    fn unknown_criterion_is_reported_at_its_position() {
        assert_eq!(
            error(r#"{"phase": "ways", "wanted": "area", "criterion": {"by": ["cheapest"]}}"#),
            RequestError::new(
                RequestErrorCode::UnknownCriterion,
                path(&["criterion", "by", "0"])
            )
        );
    }

    #[test]
    fn other_tie_break_is_invalid() {
        assert_eq!(
            error(
                r#"{"phase": "ways", "wanted": "area", "criterion": {"by": ["exactness"], "tie_break": ["rule_id"]}}"#
            ),
            RequestError::new(
                RequestErrorCode::InvalidTieBreak,
                path(&["criterion", "tie_break"])
            )
        );
    }

    #[test]
    fn role_both_obtainable_and_not_obtainable_is_contradictory() {
        assert_eq!(
            error(
                r#"{"phase": "ways", "wanted": "area", "obtainable": ["radius"], "not_obtainable": ["radius"]}"#
            ),
            RequestError::new(
                RequestErrorCode::ContradictoryMarks,
                path(&["not_obtainable", "0"])
            )
        );
    }

    #[test]
    fn cap_above_one_thousand_is_invalid() {
        assert_eq!(
            error(r#"{"phase": "ways", "wanted": "area", "cap": 1001}"#),
            RequestError::new(RequestErrorCode::InvalidCap, path(&["cap"]))
        );
    }

    #[test]
    fn zero_work_budget_is_invalid() {
        assert_eq!(
            error(r#"{"phase": "ways", "wanted": "area", "work_budget": 0}"#),
            RequestError::new(RequestErrorCode::InvalidWorkBudget, path(&["work_budget"]))
        );
    }

    #[test]
    fn error_diagnostic_names_code_and_path() {
        let diagnostic =
            RequestError::new(RequestErrorCode::UnknownRole, path(&["given", "0", "name"]))
                .diagnostic();
        assert_eq!(
            (
                diagnostic.code.as_str(),
                diagnostic.data.get("path").cloned()
            ),
            (
                "unknown_role",
                Some(ParameterValue::Identifier("given[0].name".to_owned()))
            )
        );
    }
}
