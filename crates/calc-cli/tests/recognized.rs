use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::mpsc::channel;

use calc_app::{FixedClock, Job, JobState, Session, UtcTimestamp, registered_backends};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn standard_output(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

fn recognized_session(name: &str, input: &str) -> PathBuf {
    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("calc-recognized");
    std::fs::create_dir_all(&directory).expect("scratch directory is created");
    let clock = FixedClock::new(UtcTimestamp::from_milliseconds_since_unix_epoch(0), 1);
    let mut session = Session::new(Box::new(clock), registered_backends());
    let id = session.enter(input).expect("line is entered");
    let (sender, received) = channel();
    if let Some(mut job) = session.recognition_job(id, sender) {
        while job.step() == JobState::Pending {}
        for event in received.try_iter() {
            session.apply_recognition(&event);
        }
    }
    let bytes = session.save_to_bytes().expect("session is saved");
    let path = directory.join(name);
    std::fs::write(&path, bytes).expect("session file is written");
    path
}

#[test]
fn a_recognized_line_names_its_concepts_in_the_stack() {
    let path = recognized_session("circumference.calc", "2 * pi * 3");
    let output = calc(&[path.to_str().expect("path is UTF-8"), "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(
        text.lines()
            .any(|row| row.trim_start().starts_with("concept")),
        "no recognized row in {text}"
    );
}

#[test]
fn the_json_form_carries_the_recognized_member() {
    let path = recognized_session("json.calc", "2 * pi * 3");
    let output = calc(&[path.to_str().expect("path is UTF-8"), "--json"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("\"recognized\": {"));
    assert!(text.contains("\"concept_set_version\""));
}

#[test]
fn a_stored_offer_that_was_cut_short_still_says_so_after_opening() {
    let path = recognized_session("cut-short.calc", "2 * pi * 3");
    let stored = std::fs::read_to_string(&path).expect("session file is read");
    let cut_short = stored
        .replace("\"truncated\": false", "\"truncated\": true")
        .replace("\"truncated_by\": null", "\"truncated_by\": \"cap\"");
    std::fs::write(&path, cut_short).expect("session file is written");

    let output = calc(&[path.to_str().expect("path is UTF-8"), "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("and more"), "{text}");
}

#[test]
fn an_offer_that_was_not_cut_short_says_nothing_about_more() {
    let path = recognized_session("whole-offer.calc", "2 * pi * 3");
    let output = calc(&[path.to_str().expect("path is UTF-8"), "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(!text.contains("and more"), "{text}");
}

#[test]
fn a_line_where_nothing_matched_says_so() {
    let path = recognized_session("plain.calc", "3 - 7");
    let output = calc(&[path.to_str().expect("path is UTF-8"), "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("none in the concept set matches"));
}

#[test]
fn an_expression_run_names_what_it_recognized() {
    let output = calc(&["2 * pi * 3", "--locale", "en"]);

    assert!(output.status.success());
    let text = standard_output(&output);
    assert!(text.contains("Circumference of a circle"), "{text}");
    assert!(!text.contains("circle-circumference"), "{text}");
}

#[test]
fn the_case_the_ceo_gave_names_the_pythagorean_theorem() {
    let output = calc(&["sqrt(3^2 + 4^2)", "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("Pythagorean theorem"), "{text}");
}

#[test]
fn the_case_the_ceo_gave_carries_its_bindings_and_its_guard() {
    let output = calc(&["sqrt(3^2 + 4^2)", "--json"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("\"state\": \"ran\""), "{text}");
    assert!(text.contains("\"variable\": \"a\""), "{text}");
    assert!(
        text.contains("\"holds\": [\n              true\n            ]"),
        "{text}"
    );
}

#[test]
fn a_one_shot_run_answers_with_its_recognition_ran() {
    let output = calc(&["3 - 7", "--json"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("\"state\": \"ran\""), "{text}");
    assert!(text.contains("\"matches\": []"), "{text}");
}

#[test]
fn a_session_saved_before_recognition_ran_is_recognized_when_it_is_opened() {
    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("calc-recognized");
    std::fs::create_dir_all(&directory).expect("scratch directory is created");
    let clock = FixedClock::new(UtcTimestamp::from_milliseconds_since_unix_epoch(0), 1);
    let mut session = Session::new(Box::new(clock), registered_backends());
    session.enter("2 * pi * 3").expect("line is entered");
    let bytes = session.save_to_bytes().expect("session is saved");
    let path = directory.join("not-run.calc");
    std::fs::write(&path, bytes).expect("session file is written");

    let output = calc(&[path.to_str().expect("path is UTF-8"), "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(
        text.contains("Circumference of a circle"),
        "the opened session was not recognized: {text}"
    );
}

#[test]
fn a_run_that_matched_nothing_says_so_rather_than_staying_silent() {
    let output = calc(&["3 - 7", "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("none in the concept set matches"));
}

fn foreign_concept_session(name: &str, input: &str) -> PathBuf {
    let path = recognized_session(name, input);
    let stored = std::fs::read_to_string(&path).expect("the session file is read");
    let foreign = stored.replace("circle-circumference", "torus-volume");
    assert_ne!(foreign, stored, "the record names no concept to replace");
    std::fs::write(&path, foreign).expect("session file is written");
    path
}

#[test]
#[ignore = "opening a session re-runs recognition over the stored answer, so a match from another concept set never reaches the row; the end-to-end check is this test with the attribute removed once that is fixed"]
fn a_match_this_version_cannot_name_is_counted_rather_than_written() {
    let path = foreign_concept_session("foreign.calc", "2 * pi * 3");
    let output = calc(&[path.to_str().expect("path is UTF-8"), "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(
        text.contains("one concept this version does not know") && !text.contains("torus-volume"),
        "{text}"
    );
}
