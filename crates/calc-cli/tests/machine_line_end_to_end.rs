use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str], standard_input: &str) -> Output {
    let mut child = Command::new(CALC)
        .args(arguments)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("calc starts");
    child
        .stdin
        .take()
        .expect("standard input is piped")
        .write_all(standard_input.as_bytes())
        .expect("input is written");
    child.wait_with_output().expect("calc finishes")
}

fn text(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

fn typed_and_asked_in(width: &str, expression: &str) -> String {
    let begun = calc(&["-", "--begin"], "");
    assert_eq!(begun.status.code(), Some(0), "{}", text(&begun));
    let entered = calc(&["-", "--enter", expression], &text(&begun));
    assert_eq!(entered.status.code(), Some(0), "{}", text(&entered));
    let asked = calc(&["-", "--machine-line", "r1", width], &text(&entered));
    assert_eq!(asked.status.code(), Some(0), "{}", text(&asked));
    text(&asked)
}

fn written_to(name: &str, session: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("calc-machine-line");
    std::fs::create_dir_all(&directory).expect("scratch directory is created");
    let path = directory.join(name);
    std::fs::write(&path, session).expect("session file is written");
    path
}

#[test]
fn a_line_asked_for_in_f32_answers_in_f32_and_writes_the_width_down() {
    let session = typed_and_asked_in("f32", "1/3");

    assert!(session.contains("\"input\": \"to_f32(r1)\""), "{session}");
    assert!(session.contains("\"type\": \"f32\""), "{session}");
    assert!(
        session.contains("\"decimal\": \"3.3333334e-1\""),
        "{session}"
    );
    assert!(session.contains("\"width\": \"f32\""), "{session}");
}

#[test]
fn the_same_line_asked_for_in_f64_answers_in_f64_and_writes_that_width_down() {
    let session = typed_and_asked_in("f64", "1/3");

    assert!(session.contains("\"input\": \"to_f64(r1)\""), "{session}");
    assert!(session.contains("\"type\": \"f64\""), "{session}");
    assert!(
        session.contains("\"decimal\": \"3.333333333333333e-1\""),
        "{session}"
    );
    assert!(session.contains("\"width\": \"f64\""), "{session}");
}

#[test]
fn opening_the_file_again_prints_the_f32_width_without_recomputing() {
    let path = written_to("f32.calc", &typed_and_asked_in("f32", "1/3"));
    let path_text = path.to_str().expect("path is UTF-8");

    let opened = calc(&[path_text, "--locale", "en"], "");

    assert_eq!(opened.status.code(), Some(0));
    let shown = text(&opened);
    assert!(shown.contains("machine float, 32-bit (f32)"), "{shown}");
    assert!(shown.contains("0.33333334"), "{shown}");
    assert!(shown.contains("replay: not run"), "{shown}");
}

#[test]
fn replaying_the_f32_file_recomputes_in_that_width_and_reports_no_difference() {
    let path = written_to("f32-replay.calc", &typed_and_asked_in("f32", "1/3"));
    let path_text = path.to_str().expect("path is UTF-8");

    let replayed = calc(&[path_text, "--replay", "--locale", "en"], "");

    assert_eq!(replayed.status.code(), Some(0));
    let shown = text(&replayed);
    assert!(shown.contains("machine float, 32-bit (f32)"), "{shown}");
    assert!(shown.contains("0.33333334"), "{shown}");
    assert!(shown.contains("0 differ"), "{shown}");
}

#[test]
fn replaying_a_file_that_stores_another_f32_value_reports_the_difference() {
    let stored = typed_and_asked_in("f32", "1/3")
        .replace("\"0x3eaaaaab\"", "\"0x3e800000\"")
        .replace("\"3.3333334e-1\"", "\"2.5e-1\"");
    let path = written_to("f32-changed.calc", &stored);
    let path_text = path.to_str().expect("path is UTF-8");

    let replayed = calc(&[path_text, "--replay", "--locale", "en"], "");

    assert_eq!(replayed.status.code(), Some(1));
    let shown = text(&replayed);
    assert!(shown.contains("1 differs"), "{shown}");
}

#[test]
fn replaying_a_file_whose_stored_width_disagrees_with_its_value_reports_the_difference() {
    let stored =
        typed_and_asked_in("f32", "1/3").replace("\"width\": \"f32\"", "\"width\": \"f64\"");
    let path = written_to("f32-width-changed.calc", &stored);
    let path_text = path.to_str().expect("path is UTF-8");

    let replayed = calc(&[path_text, "--replay", "--locale", "en"], "");

    assert_eq!(replayed.status.code(), Some(1));
    let shown = text(&replayed);
    assert!(shown.contains("1 differs"), "{shown}");
}

#[test]
fn replaying_the_f64_file_recomputes_in_that_width_and_reports_no_difference() {
    let path = written_to("f64-replay.calc", &typed_and_asked_in("f64", "1/3"));
    let path_text = path.to_str().expect("path is UTF-8");

    let replayed = calc(&[path_text, "--replay", "--locale", "en"], "");

    assert_eq!(replayed.status.code(), Some(0));
    let shown = text(&replayed);
    assert!(shown.contains("machine float, 64-bit (f64)"), "{shown}");
    assert!(shown.contains("0.3333333333333333"), "{shown}");
    assert!(shown.contains("0 differ"), "{shown}");
}

fn rounding_row(expression: &str) -> String {
    let output = Command::new(CALC)
        .arg(expression)
        .env_clear()
        .output()
        .expect("calc runs");
    text(&output)
        .lines()
        .find(|line| line.trim_start().starts_with("rounding error"))
        .expect("a rounding error row")
        .to_owned()
}

#[test]
fn atan2_with_a_zero_or_subnormal_ordinate_states_a_bound() {
    for expression in [
        "to_f64(atan2(0, -1))",
        "to_f64(atan2(2^-1074, -1))",
        "to_f64(atan2(2^-1074, 2^-1074))",
        "to_f64(atan2(3*2^-1074, -5*2^-1074))",
    ] {
        let row = rounding_row(expression);
        assert!(row.contains("at most"), "{expression}: {row}");
    }
}
