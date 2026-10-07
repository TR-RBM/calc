use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::mpsc::Sender;

use calc_concepts::RecognitionPattern as ContentPattern;
use calc_core::{PatternRecognition, RecognitionLimits, RecognitionPattern, recognize};
use calc_expr::{ExprId, ExprPool};
use calc_syntax::{PrintMode, print_expression};

use crate::platform::{Job, JobState};
use crate::result_record::{
    LineId, RecognitionLimitsUsed, Recognized, RecognizedBinding, RecognizedConcept,
};
use crate::solve_answer::{AnswerBody, SolveAnswer};

const RULE_SEPARATOR: char = '/';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecognitionUnavailable {
    ConceptSetNotLoaded,
    PatternsNotBuilt,
    MatcherFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecognitionState {
    Running,
    Ready,
    Unavailable(RecognitionUnavailable),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecognitionEvent {
    Ready {
        line: LineId,
        generation: u64,
        recognized: Recognized,
    },
    Unavailable {
        line: LineId,
        generation: u64,
        reason: RecognitionUnavailable,
    },
}

impl RecognitionEvent {
    pub fn line(&self) -> LineId {
        match self {
            RecognitionEvent::Ready { line, .. } | RecognitionEvent::Unavailable { line, .. } => {
                *line
            }
        }
    }

    pub fn generation(&self) -> u64 {
        match self {
            RecognitionEvent::Ready { generation, .. }
            | RecognitionEvent::Unavailable { generation, .. } => *generation,
        }
    }
}

pub(crate) fn derivation_of(answer: &SolveAnswer) -> Vec<String> {
    let ways = match &answer.body {
        AnswerBody::Solved { ways, .. } | AnswerBody::Ways { ways, .. } => ways,
        AnswerBody::Front { front, .. } => front,
        AnswerBody::NotReached { .. } => return Vec::new(),
    };
    ways.iter()
        .find(|way| way.record.is_some())
        .or_else(|| ways.first())
        .map(|way| way.derivation.clone())
        .unwrap_or_default()
}

pub(crate) fn concept_of_rule(rule: &str) -> &str {
    rule.split_once(RULE_SEPARATOR)
        .map_or(rule, |(concept, _)| concept)
}

pub(crate) enum RecognitionWork {
    Match {
        pool: Box<ExprPool>,
        root: ExprId,
        patterns: Vec<RecognitionPattern>,
        variables: Vec<(String, Vec<String>)>,
        limits: RecognitionLimits,
    },
    Rules {
        rules: Vec<String>,
    },
}

pub(crate) fn matcher_patterns(
    content: &[ContentPattern],
) -> (Vec<RecognitionPattern>, Vec<(String, Vec<String>)>) {
    let patterns = content
        .iter()
        .map(|pattern| RecognitionPattern {
            concept: pattern.concept.clone(),
            identifier: pattern.identifier.clone(),
            parameters: pattern.variables.len(),
            function: pattern.function,
            conditions: pattern.conditions.clone(),
        })
        .collect();
    let variables = content
        .iter()
        .map(|pattern| (pattern.identifier.clone(), pattern.variables.clone()))
        .collect();
    (patterns, variables)
}

pub struct RecognitionJob {
    pub(crate) work: RecognitionWork,
    pub(crate) concept_set_version: String,
    pub(crate) line: LineId,
    pub(crate) generation: u64,
    pub(crate) cancellation: Arc<AtomicBool>,
    pub(crate) events: Sender<RecognitionEvent>,
}

fn printed(pool: &ExprPool, expression: ExprId) -> String {
    print_expression(pool, expression, PrintMode::Ascii).unwrap_or_default()
}

fn stored_matches(
    pool: &ExprPool,
    recognition: &PatternRecognition,
    variables: &[(String, Vec<String>)],
) -> Vec<RecognizedConcept> {
    let variables_of = |identifier: &str| {
        variables
            .iter()
            .find(|(pattern, _)| pattern == identifier)
            .map(|(_, names)| names.clone())
            .unwrap_or_default()
    };
    recognition
        .matches
        .iter()
        .map(|found| {
            let names = variables_of(&found.pattern);
            RecognizedConcept {
                concept: found.concept.clone(),
                pattern: found.pattern.clone(),
                coverage: found.coverage.clone(),
                site: printed(pool, found.site),
                bindings: found
                    .bindings
                    .iter()
                    .enumerate()
                    .map(|(position, binding)| RecognizedBinding {
                        variable: names
                            .get(position)
                            .cloned()
                            .unwrap_or_else(|| position.to_string()),
                        expression: printed(pool, *binding),
                    })
                    .collect(),
                holds: found.holds.clone(),
            }
        })
        .collect()
}

fn rule_matches(rules: &[String]) -> Vec<RecognizedConcept> {
    rules
        .iter()
        .map(|rule| RecognizedConcept {
            concept: concept_of_rule(rule).to_owned(),
            pattern: rule.clone(),
            coverage: calc_numbers::Number::from(0),
            site: String::new(),
            bindings: Vec::new(),
            holds: Vec::new(),
        })
        .collect()
}

impl Job for RecognitionJob {
    fn step(&mut self) -> JobState {
        if self.cancellation.load(AtomicOrdering::SeqCst) {
            return JobState::Finished;
        }
        let event = match &mut self.work {
            RecognitionWork::Rules { rules } => RecognitionEvent::Ready {
                line: self.line,
                generation: self.generation,
                recognized: Recognized {
                    concept_set_version: self.concept_set_version.clone(),
                    limits: None,
                    truncated: Some(false),
                    truncated_by: None,
                    matches: rule_matches(rules),
                },
            },
            RecognitionWork::Match {
                pool,
                root,
                patterns,
                variables,
                limits,
            } => match recognize(pool, *root, patterns.as_slice(), *limits, &printed) {
                Ok(recognition) => RecognitionEvent::Ready {
                    line: self.line,
                    generation: self.generation,
                    recognized: Recognized {
                        concept_set_version: self.concept_set_version.clone(),
                        limits: Some(RecognitionLimitsUsed {
                            cap: limits.cap,
                            work_budget: limits.work_budget,
                        }),
                        truncated: Some(recognition.truncated),
                        truncated_by: recognition.truncated_by,
                        matches: stored_matches(pool, &recognition, variables),
                    },
                },
                Err(_) => RecognitionEvent::Unavailable {
                    line: self.line,
                    generation: self.generation,
                    reason: RecognitionUnavailable::MatcherFailed,
                },
            },
        };
        if self.cancellation.load(AtomicOrdering::SeqCst) {
            return JobState::Finished;
        }
        self.events.send(event).ok();
        JobState::Finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::tests::session;
    use crate::session::{Outcome, Session};
    use std::sync::mpsc::{Receiver, channel};

    fn recognized_line(session: &mut Session, text: &str) -> (LineId, Receiver<RecognitionEvent>) {
        let id = session.enter(text).expect("a line");
        let (sender, receiver) = channel();
        let mut job = session.recognition_job(id, sender).expect("a job");
        assert_eq!(job.step(), JobState::Finished);
        (id, receiver)
    }

    fn summary_state(session: &Session, id: LineId) -> crate::summary::RecognitionSummary {
        let line = session.line(id).expect("a line");
        match session.line_summary(line).state {
            crate::summary::LineState::Result(record) => record.recognition,
            other => panic!("expected a result, found {other:?}"),
        }
    }

    fn stored(session: &Session, id: LineId) -> Option<Recognized> {
        match session.line(id)?.outcome() {
            Outcome::Result(record) => record.recognized().cloned(),
            _ => None,
        }
    }

    #[test]
    fn a_line_with_a_value_waits_for_its_recognition() {
        let mut computing = session();

        let id = computing.enter("2 * pi * 3").unwrap();

        assert_eq!(computing.pending_recognition(), [id]);
    }

    #[test]
    fn a_result_is_there_before_its_recognition_runs() {
        let mut computing = session();

        let id = computing.enter("2 * pi * 3").unwrap();

        assert!(matches!(
            computing.line(id).map(crate::session::Line::outcome),
            Some(Outcome::Result(_))
        ));
        assert_eq!(stored(&computing, id), None);
    }

    #[test]
    fn a_running_recognition_says_so() {
        let mut computing = session();
        let id = computing.enter("2 * pi * 3").unwrap();
        let (sender, _receiver) = channel();

        let _job = computing.recognition_job(id, sender).expect("a job");

        assert_eq!(
            computing.recognition_state(id),
            Some(RecognitionState::Running)
        );
    }

    #[test]
    fn a_circumference_is_recognized_as_its_concept() {
        let mut computing = session();
        let (id, receiver) = recognized_line(&mut computing, "2 * pi * 3");

        let event = receiver.try_recv().expect("an event");
        assert!(computing.apply_recognition(&event));

        let concepts: Vec<String> = stored(&computing, id)
            .expect("a recognition")
            .matches
            .into_iter()
            .map(|found| found.concept)
            .collect();
        assert!(
            concepts.contains(&"circle-circumference".to_owned()),
            "{concepts:?}"
        );
    }

    #[test]
    fn a_recognized_line_carries_the_concept_set_version() {
        let mut computing = session();
        let (id, receiver) = recognized_line(&mut computing, "2 * pi * 3");

        computing.apply_recognition(&receiver.try_recv().expect("an event"));

        let version = stored(&computing, id)
            .expect("a recognition")
            .concept_set_version;
        assert_eq!(version.len(), 16);
    }

    #[test]
    fn a_binding_is_named_by_its_pattern_variable() {
        let mut computing = session();
        let (id, receiver) = recognized_line(&mut computing, "2 * pi * 3");

        computing.apply_recognition(&receiver.try_recv().expect("an event"));

        let bindings = stored(&computing, id)
            .expect("a recognition")
            .matches
            .into_iter()
            .flat_map(|found| found.bindings)
            .map(|binding| (binding.variable, binding.expression))
            .collect::<Vec<(String, String)>>();
        assert!(
            bindings
                .iter()
                .any(|(variable, expression)| !variable.is_empty() && expression == "3"),
            "{bindings:?}"
        );
    }

    #[test]
    fn an_answer_of_an_older_generation_is_dropped() {
        let mut computing = session();
        let (id, receiver) = recognized_line(&mut computing, "2 * pi * 3");
        let event = receiver.try_recv().expect("an event");

        computing.edit(id, "2 * pi * 4").unwrap();

        assert!(!computing.apply_recognition(&event));
        assert_eq!(stored(&computing, id), None);
    }

    #[test]
    fn a_line_recomputed_waits_for_its_recognition_again() {
        let mut computing = session();
        let (id, receiver) = recognized_line(&mut computing, "2 * pi * 3");
        computing.apply_recognition(&receiver.try_recv().expect("an event"));

        computing.edit(id, "2 * pi * 4").unwrap();

        assert_eq!(computing.pending_recognition(), [id]);
    }

    #[test]
    fn a_failed_line_has_no_recognition() {
        let mut computing = session();
        let id = computing.enter("1/0").unwrap();
        let (sender, _receiver) = channel();

        assert!(computing.recognition_job(id, sender).is_none());
    }

    #[test]
    fn a_function_naming_has_no_recognition() {
        let mut computing = session();
        let id = computing.enter("f(x) = 2 * pi * x").unwrap();
        let (sender, _receiver) = channel();

        assert!(computing.recognition_job(id, sender).is_none());
    }

    #[test]
    fn a_solve_line_is_recognized_by_the_rules_of_its_derivation() {
        let mut computing = session();
        let request = crate::solve_request::parse_request(
            br#"{"phase": "evaluate", "object": "circle", "wanted": "circumference", "given": [{"name": "diameter", "value": "5", "unit": "cm"}]}"#,
        )
        .expect("a request");
        let id = computing.enter_solve(&request).expect("a solve line");
        let (sender, receiver) = channel();
        let mut job = computing.recognition_job(id, sender).expect("a job");

        assert_eq!(job.step(), JobState::Finished);

        let event = receiver.try_recv().expect("an event");
        let RecognitionEvent::Ready { recognized, .. } = &event else {
            panic!("expected a recognition, found {event:?}");
        };
        assert_eq!(
            recognized
                .matches
                .iter()
                .map(|found| (found.concept.as_str(), found.bindings.len()))
                .collect::<Vec<(&str, usize)>>(),
            [("circle-circumference", 0)]
        );
    }

    #[test]
    fn a_cancelled_recognition_sends_nothing() {
        let mut computing = session();
        let id = computing.enter("2 * pi * 3").unwrap();
        let (sender, receiver) = channel();
        let mut job = computing.recognition_job(id, sender).expect("a job");

        computing.cancel_recognition(id);
        job.step();

        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn the_pending_work_is_run_to_completion() {
        let mut computing = session();
        let id = computing.enter("sqrt(3^2 + 4^2)").unwrap();

        computing.run_pending_recognition();

        let concepts: Vec<String> = stored(&computing, id)
            .expect("a recognition")
            .matches
            .into_iter()
            .map(|found| found.concept)
            .collect();
        assert_eq!(concepts, ["pythagorean-theorem"]);
    }

    #[test]
    fn the_guard_of_the_pattern_holds_for_the_case_the_ceo_gave() {
        let mut computing = session();
        let id = computing.enter("sqrt(3^2 + 4^2)").unwrap();

        computing.run_pending_recognition();

        let holds: Vec<Option<bool>> = stored(&computing, id)
            .expect("a recognition")
            .matches
            .into_iter()
            .flat_map(|found| found.holds)
            .collect();
        assert_eq!(holds, [Some(true)]);
    }

    #[test]
    fn nothing_is_left_pending_once_the_work_has_run() {
        let mut computing = session();
        computing.enter("sqrt(3^2 + 4^2)").unwrap();

        computing.run_pending_recognition();

        assert!(computing.pending_recognition().is_empty());
    }

    #[test]
    fn a_line_that_matched_nothing_says_the_matcher_ran() {
        let mut computing = session();
        let id = computing.enter("3 - 7").unwrap();

        computing.run_pending_recognition();

        let recognized = stored(&computing, id).expect("a recognition");
        assert_eq!(
            (recognized.matches.len(), recognized.truncated),
            (0, Some(false))
        );
    }

    #[test]
    fn a_queued_line_reads_as_running_until_its_work_is_taken() {
        let mut computing = session();
        let id = computing.enter("sqrt(3^2 + 4^2)").unwrap();

        assert_eq!(
            summary_state(&computing, id),
            crate::summary::RecognitionSummary::Running
        );
    }

    #[test]
    fn a_line_reads_as_what_it_matched_once_its_work_has_run() {
        let mut computing = session();
        let id = computing.enter("sqrt(3^2 + 4^2)").unwrap();

        computing.run_pending_recognition();

        assert!(matches!(
            summary_state(&computing, id),
            crate::summary::RecognitionSummary::Matched(_)
        ));
    }

    #[test]
    fn the_queued_work_is_handed_over_as_jobs() {
        let mut computing = session();
        computing.enter("sqrt(3^2 + 4^2)").unwrap();
        let (sender, _receiver) = channel();

        let jobs = computing.start_pending_recognition(&sender);

        assert_eq!((jobs.len(), computing.pending_recognition().len()), (1, 0));
    }

    #[test]
    fn a_concept_of_a_rule_is_its_first_part() {
        assert_eq!(
            concept_of_rule("circle-circumference/from-diameter"),
            "circle-circumference"
        );
    }
}
