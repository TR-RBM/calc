use std::path::PathBuf;
use std::process::{Command, Output};

use calc_app::{FixedClock, Session, UtcTimestamp, registered_backends};

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

fn standard_error(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("output is UTF-8")
}

fn session_file(name: &str, inputs: &[&str]) -> PathBuf {
    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("calc-instruments");
    std::fs::create_dir_all(&directory).expect("scratch directory is created");
    let clock = FixedClock::new(UtcTimestamp::from_milliseconds_since_unix_epoch(0), 1);
    let mut session = Session::new(Box::new(clock), registered_backends());
    for input in inputs {
        session.enter(input).expect("line is entered");
    }
    let bytes = session.save_to_bytes().expect("session is saved");
    let path = directory.join(name);
    std::fs::write(&path, bytes).expect("session file is written");
    path
}

#[test]
fn inspect_shows_the_record_of_the_line() {
    let output = calc(&["2 * pi * 3", "--inspect", "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(
        text.contains("value") && text.contains("computed"),
        "{text}"
    );
}

#[test]
fn inspect_says_whether_every_concept_was_tried() {
    let output = calc(&["2 * pi * 3", "--inspect", "--locale", "en"]);
    let text = standard_output(&output);

    assert!(text.contains("every concept was tried"), "{text}");
}

#[test]
fn a_run_without_the_flag_says_nothing_about_the_offer() {
    let output = calc(&["2 * pi * 3", "--locale", "en"]);

    assert!(!standard_output(&output).contains("every concept was tried"));
}

#[test]
fn inspect_shows_one_line_of_a_session_file() {
    let path = session_file("two-lines.calc", &["2 + 3", "4 * 5"]);
    let output = calc(&[
        path.to_str().expect("path is UTF-8"),
        "r2",
        "--inspect",
        "--locale",
        "en",
    ]);
    let text = standard_output(&output);

    assert!(output.status.success(), "{}", standard_error(&output));
    assert!(text.starts_with("r2  "), "{text}");
    assert!(!text.contains("r1  "), "{text}");
}

#[test]
fn inspect_is_refused_for_a_naming_the_terminal_shows_whole() {
    let output = calc(&["f(x) = x^2", "--inspect", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        standard_error(&output).contains("--inspect does not apply to r1"),
        "{}",
        standard_error(&output)
    );
}

#[test]
fn a_refusal_names_the_forms_the_line_does_have() {
    let output = calc(&["f(x) = x^2", "--inspect", "--locale", "en"]);

    assert!(
        standard_error(&output).contains("calc plot"),
        "{}",
        standard_error(&output)
    );
}

#[test]
fn a_line_with_no_other_form_says_so() {
    let output = calc(&["h(x, y, z) = x + y + z", "--inspect", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        standard_error(&output).contains("no other form"),
        "{}",
        standard_error(&output)
    );
}

#[test]
fn two_forms_of_one_line_is_a_usage_error() {
    let output = calc(&["2 + 3", "--inspect", "--digits", "3", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        standard_error(&output).contains("one form at a time"),
        "{}",
        standard_error(&output)
    );
}

#[test]
fn the_json_form_of_an_inspected_line_is_the_line_object() {
    let output = calc(&["2 + 3", "--inspect", "--json"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("\"input\": \"2 + 3\""), "{text}");
}
