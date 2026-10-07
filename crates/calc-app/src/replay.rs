use calc_core::ResultValue;
use calc_expr::ExprPool;

use crate::json::{Json, write_one_line};
use crate::result_record::{BackendUse, LineId, ResultRecord, UtcTimestamp};
use crate::session::Outcome;
use crate::session_file::record_json;
use crate::value_equality::values_equal;

const STATUS_MEMBER: &str = "status";
const ERROR_MEMBER: &str = "error";
const ANSWER_MEMBER: &str = "answer";
const ORBIT_MEMBER: &str = "orbit";
const RECORD_MEMBER: &str = "record";
const MEMBER_SEPARATOR: char = '.';

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Difference {
    pub member: String,
    pub stored: String,
    pub replayed: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparedState {
    Verified,
    Differs,
    NotCompared,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineComparison {
    pub line: LineId,
    pub state: ComparedState,
    pub differences: Vec<Difference>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReplayReport {
    pub lines: Vec<LineComparison>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayRun {
    pub at: UtcTimestamp,
    pub report: ReplayReport,
}

impl ReplayReport {
    pub fn counted(&self, state: ComparedState) -> u64 {
        let counted = self
            .lines
            .iter()
            .filter(|comparison| comparison.state == state)
            .count();
        u64::try_from(counted).unwrap_or(u64::MAX)
    }

    pub fn differing(&self) -> u64 {
        self.counted(ComparedState::Differs)
    }
}

fn status_name(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Result(_) => "result",
        Outcome::Error(_) => "error",
        Outcome::NotEvaluated | Outcome::Reachable(_) => "not_evaluated",
        Outcome::Picture => "picture",
        Outcome::Defined => "defined",
        Outcome::EscapeTimeReading(_) => "escape_time_reading",
        Outcome::Answer(_) => "answer",
    }
}

fn member_of(json: Option<&Json>, name: &str) -> String {
    let Some(Json::Object(members)) = json else {
        return String::new();
    };
    members
        .iter()
        .find(|(member, _)| member == name)
        .map(|(_, value)| write_one_line(value))
        .unwrap_or_default()
}

fn number_value(number: &calc_numbers::Number) -> ResultValue {
    ResultValue::Number(number.clone())
}

fn rounding_errors_equal(
    stored: &calc_core::RoundingError,
    replayed: &calc_core::RoundingError,
) -> bool {
    use calc_core::RoundingError::{Bound, None, Unknown};
    match (stored, replayed) {
        (None, None) | (Unknown, Unknown) => true,
        (Bound(stored), Bound(replayed)) => values_equal(stored, replayed),
        _ => false,
    }
}

fn uncertainties_equal(
    stored: Option<&calc_core::Uncertainty>,
    replayed: Option<&calc_core::Uncertainty>,
) -> bool {
    match (stored, replayed) {
        (Option::None, Option::None) => true,
        (Some(stored), Some(replayed)) => {
            values_equal(stored.standard(), replayed.standard())
                && values_equal(
                    &number_value(stored.coverage_factor()),
                    &number_value(replayed.coverage_factor()),
                )
        }
        _ => false,
    }
}

fn widths_equal(stored: &[BackendUse], replayed: &[BackendUse]) -> bool {
    stored
        .iter()
        .zip(replayed)
        .all(|(stored, replayed)| stored.width == replayed.width)
}

fn compared_members(stored: &ResultRecord, replayed: &ResultRecord) -> Vec<&'static str> {
    let stored_computed = stored.computed();
    let replayed_computed = replayed.computed();
    let equal: [(&'static str, bool); 12] = [
        ("kind", stored_computed.kind() == replayed_computed.kind()),
        (
            "value",
            values_equal(stored_computed.value(), replayed_computed.value()),
        ),
        ("unit", stored_computed.unit() == replayed_computed.unit()),
        (
            "rounding_error",
            rounding_errors_equal(
                stored_computed.rounding_error(),
                replayed_computed.rounding_error(),
            ),
        ),
        (
            "uncertainty",
            uncertainties_equal(
                stored_computed.uncertainty(),
                replayed_computed.uncertainty(),
            ),
        ),
        (
            "method",
            stored_computed.method() == replayed_computed.method(),
        ),
        (
            "corpus_references",
            stored_computed
                .corpus_references()
                .eq(replayed_computed.corpus_references()),
        ),
        ("sources", stored.sources().eq(replayed.sources())),
        ("recognized", stored.recognized() == replayed.recognized()),
        ("seed", stored_computed.seed() == replayed_computed.seed()),
        (
            "dependencies",
            stored.dependencies() == replayed.dependencies(),
        ),
        ("width", widths_equal(stored.backend(), replayed.backend())),
    ];
    equal
        .into_iter()
        .filter(|(_, equal)| !equal)
        .map(|(member, _)| member)
        .collect()
}

pub(crate) fn compare_outcomes(
    pool: &ExprPool,
    line: LineId,
    stored: &Outcome,
    replayed: &Outcome,
) -> LineComparison {
    let differences = outcome_differences(pool, line, stored, replayed);
    let state = match (stored, differences.is_empty()) {
        (Outcome::NotEvaluated, _) => ComparedState::NotCompared,
        (_, true) => ComparedState::Verified,
        (_, false) => ComparedState::Differs,
    };
    LineComparison {
        line,
        state,
        differences,
    }
}

fn outcome_differences(
    pool: &ExprPool,
    line: LineId,
    stored: &Outcome,
    replayed: &Outcome,
) -> Vec<Difference> {
    if matches!(stored, Outcome::NotEvaluated) {
        return Vec::new();
    }
    if status_name(stored) != status_name(replayed) {
        return vec![Difference {
            member: STATUS_MEMBER.to_owned(),
            stored: status_name(stored).to_owned(),
            replayed: status_name(replayed).to_owned(),
        }];
    }
    match (stored, replayed) {
        (Outcome::Result(stored), Outcome::Result(replayed)) => {
            record_differences(pool, line, stored, replayed)
        }
        (Outcome::Error(stored_error), Outcome::Error(replayed_error)) => {
            if stored_error == replayed_error {
                Vec::new()
            } else {
                vec![Difference {
                    member: ERROR_MEMBER.to_owned(),
                    stored: stored_error.code.clone(),
                    replayed: replayed_error.code.clone(),
                }]
            }
        }
        (Outcome::Answer(stored_answer), Outcome::Answer(replayed_answer)) => {
            different_member(ANSWER_MEMBER, stored_answer != replayed_answer)
        }
        (Outcome::EscapeTimeReading(stored_orbit), Outcome::EscapeTimeReading(replayed_orbit)) => {
            different_member(ORBIT_MEMBER, stored_orbit != replayed_orbit)
        }
        _ => Vec::new(),
    }
}

fn different_member(member: &str, differs: bool) -> Vec<Difference> {
    if differs {
        vec![Difference {
            member: member.to_owned(),
            stored: String::new(),
            replayed: String::new(),
        }]
    } else {
        Vec::new()
    }
}

fn record_differences(
    pool: &ExprPool,
    line: LineId,
    stored: &ResultRecord,
    replayed: &ResultRecord,
) -> Vec<Difference> {
    let stored_json = record_json(pool, stored, line).ok();
    let replayed_json = record_json(pool, replayed, line).ok();
    compared_members(stored, replayed)
        .into_iter()
        .map(|member| Difference {
            member: format!("{RECORD_MEMBER}{MEMBER_SEPARATOR}{member}"),
            stored: member_of(stored_json.as_ref(), member),
            replayed: member_of(replayed_json.as_ref(), member),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result_record::{BackendUse, CalculatorVersion, UtcTimestamp};
    use calc_core::{ComputedResult, Method, ResultKind};
    use calc_exec::Domain;
    use calc_exec::{BackendKind, Preference};
    use calc_numbers::{Integer, Number};
    use std::time::Duration;

    fn line() -> LineId {
        LineId::from_number(1).unwrap()
    }

    fn computed(value: i64) -> ComputedResult {
        ComputedResult::new(
            ResultKind::ExactRational,
            ResultValue::Number(Number::Integer(Integer::from(value))),
            None,
            Method::named("exact_evaluation"),
        )
        .unwrap()
    }

    fn record(value: i64) -> ResultRecord {
        ResultRecord::new(
            computed(value),
            Vec::new(),
            [],
            UtcTimestamp::from_milliseconds_since_unix_epoch(1_000),
            Duration::from_nanos(250),
            CalculatorVersion::current(),
        )
    }

    fn outcome(record: ResultRecord) -> Outcome {
        Outcome::Result(Box::new(record))
    }

    #[test]
    fn a_record_that_differs_only_in_its_context_members_is_verified() {
        let pool = ExprPool::new();
        let stored = outcome(record(7));
        let replayed = outcome(ResultRecord::new(
            computed(7),
            vec![BackendUse {
                selected: Some(BackendKind::Cpu),
                ..BackendUse::machine_evaluation(Preference::Automatic, Domain::F64)
            }],
            [],
            UtcTimestamp::from_milliseconds_since_unix_epoch(2_000),
            Duration::from_nanos(9_000),
            CalculatorVersion {
                major: 0,
                minor: 0,
                patch: 1,
            },
        ));

        let comparison = compare_outcomes(&pool, line(), &stored, &replayed);

        assert_eq!(comparison.state, ComparedState::Verified);
    }

    fn recognized(version: &str) -> crate::result_record::Recognized {
        crate::result_record::Recognized {
            concept_set_version: version.to_owned(),
            limits: None,
            truncated: Some(false),
            truncated_by: None,
            matches: Vec::new(),
        }
    }

    #[test]
    fn a_differing_concept_set_version_names_the_recognized_member() {
        let pool = ExprPool::new();
        let stored = outcome(record(7).with_recognized(recognized("0123456789abcdef")));
        let replayed = outcome(record(7).with_recognized(recognized("fedcba9876543210")));

        let comparison = compare_outcomes(&pool, line(), &stored, &replayed);

        assert_eq!(
            comparison
                .differences
                .iter()
                .map(|difference| difference.member.as_str())
                .collect::<Vec<&str>>(),
            ["record.recognized"]
        );
    }

    #[test]
    fn a_recognition_cut_short_in_one_run_only_differs() {
        let pool = ExprPool::new();
        let mut cut = recognized("0123456789abcdef");
        cut.truncated = Some(true);
        cut.truncated_by = Some(calc_core::RecognitionTruncation::Cap);
        let stored = outcome(record(7).with_recognized(recognized("0123456789abcdef")));
        let replayed = outcome(record(7).with_recognized(cut));

        let comparison = compare_outcomes(&pool, line(), &stored, &replayed);

        assert_eq!(
            comparison
                .differences
                .iter()
                .map(|difference| difference.member.as_str())
                .collect::<Vec<&str>>(),
            ["record.recognized"]
        );
    }

    #[test]
    fn a_record_recognized_on_one_run_only_differs() {
        let pool = ExprPool::new();
        let stored = outcome(record(7));
        let replayed = outcome(record(7).with_recognized(recognized("0123456789abcdef")));

        let comparison = compare_outcomes(&pool, line(), &stored, &replayed);

        assert_eq!(comparison.state, ComparedState::Differs);
    }

    #[test]
    fn a_differing_value_names_its_member() {
        let pool = ExprPool::new();

        let comparison = compare_outcomes(&pool, line(), &outcome(record(7)), &outcome(record(8)));

        assert_eq!(
            comparison
                .differences
                .iter()
                .map(|difference| difference.member.as_str())
                .collect::<Vec<&str>>(),
            ["record.value"]
        );
    }

    #[test]
    fn a_differing_value_holds_both_sides() {
        let pool = ExprPool::new();

        let comparison = compare_outcomes(&pool, line(), &outcome(record(7)), &outcome(record(8)));

        assert_eq!(
            comparison
                .differences
                .first()
                .map(|difference| (difference.stored.as_str(), difference.replayed.as_str())),
            Some((
                "{\"type\": \"integer\", \"digits\": \"7\"}",
                "{\"type\": \"integer\", \"digits\": \"8\"}"
            ))
        );
    }

    #[test]
    fn a_different_status_is_one_difference() {
        let pool = ExprPool::new();

        let comparison =
            compare_outcomes(&pool, line(), &outcome(record(7)), &Outcome::NotEvaluated);

        assert_eq!(
            comparison
                .differences
                .iter()
                .map(|difference| difference.member.as_str())
                .collect::<Vec<&str>>(),
            ["status"]
        );
    }

    #[test]
    fn a_line_that_was_not_evaluated_is_not_compared() {
        let pool = ExprPool::new();

        let comparison =
            compare_outcomes(&pool, line(), &Outcome::NotEvaluated, &outcome(record(7)));

        assert_eq!(comparison.state, ComparedState::NotCompared);
    }

    fn session() -> crate::session::Session {
        crate::session::Session::new(
            Box::new(crate::SystemClock::new()),
            vec![Box::new(calc_exec_cpu::CpuBackend::new())],
        )
    }

    #[test]
    fn a_second_run_of_a_session_finds_no_difference() {
        let mut session = session();
        session.enter("2 + 3").unwrap();
        session.enter("sqrt(2)").unwrap();

        let report = session.replay();

        assert_eq!(report.differing(), 0);
    }

    #[test]
    fn a_second_run_of_a_session_verifies_every_line() {
        let mut session = session();
        session.enter("2 + 3").unwrap();
        session.enter("sqrt(2)").unwrap();

        let report = session.replay();

        assert_eq!(report.counted(ComparedState::Verified), 2);
    }

    #[test]
    fn a_session_counts_no_differences_before_a_replay() {
        let mut session = session();
        session.enter("2 + 3").unwrap();

        assert_eq!(session.status().differing, Option::None);
    }

    #[test]
    fn a_session_counts_differences_after_a_replay() {
        let mut session = session();
        session.enter("2 + 3").unwrap();
        session.replay();

        assert_eq!(session.status().differing, Some(0));
    }

    #[test]
    fn a_replay_just_run_is_stated_as_today() {
        let mut session = session();
        session.enter("2 + 3").unwrap();
        session.replay();

        assert!(matches!(
            session.status().replay,
            crate::summary::ReplayState::Verified { today: true, .. }
        ));
    }

    #[test]
    fn the_report_counts_the_lines_that_differ() {
        let report = ReplayReport {
            lines: vec![
                LineComparison {
                    line: line(),
                    state: ComparedState::Differs,
                    differences: Vec::new(),
                },
                LineComparison {
                    line: LineId::from_number(2).unwrap(),
                    state: ComparedState::Verified,
                    differences: Vec::new(),
                },
            ],
        };

        assert_eq!(report.differing(), 1);
    }
}
