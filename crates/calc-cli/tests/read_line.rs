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

fn session_file(name: &str, input: &str) -> PathBuf {
    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("calc-read");
    std::fs::create_dir_all(&directory).expect("scratch directory is created");
    let clock = FixedClock::new(UtcTimestamp::from_milliseconds_since_unix_epoch(0), 1);
    let mut session = Session::new(Box::new(clock), registered_backends());
    session.enter(input).expect("line is entered");
    let bytes = session.save_to_bytes().expect("session is saved");
    let path = directory.join(name);
    std::fs::write(&path, bytes).expect("session file is written");
    path
}

#[test]
fn read_prints_the_value_of_the_line_at_the_typed_coordinate() {
    let path = session_file("square.calc", "f(x) = x^2");
    let path_text = path.to_str().expect("path is UTF-8");

    let output = calc(&[
        "read", path_text, "r1", "--at", "1/2", "--size", "320x240", "--locale", "en",
    ]);

    assert!(output.status.success());
    assert!(standard_output(&output).contains("1/4"));
}

#[test]
fn committed_reading_appends_the_call_and_writes_the_session_back() {
    let path = session_file("committed.calc", "f(x) = x^2");
    let path_text = path.to_str().expect("path is UTF-8");

    let output = calc(&[
        "read", path_text, "r1", "--at", "1/2", "--commit", "--size", "320x240", "--locale", "en",
    ]);

    assert!(output.status.success());
    let written = std::fs::read_to_string(&path).expect("session file is read back");
    assert!(standard_output(&output).starts_with("r2  f(0.5)\n"));
    assert!(written.contains("\"reading\""));
}

#[test]
fn committed_reading_of_a_line_without_a_function_form_is_an_error() {
    let path = session_file("plain.calc", "x^2");
    let path_text = path.to_str().expect("path is UTF-8");

    let output = calc(&[
        "read", path_text, "r1", "--at", "1/2", "--commit", "--size", "320x240", "--locale", "en",
    ]);

    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn read_without_coordinates_is_a_usage_error() {
    let path = session_file("without.calc", "x^2");
    let path_text = path.to_str().expect("path is UTF-8");

    let output = calc(&["read", path_text, "r1", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn read_as_json_prints_the_record_of_the_reading() {
    let path = session_file("json.calc", "f(x) = x^2");
    let path_text = path.to_str().expect("path is UTF-8");

    let output = calc(&[
        "read", path_text, "r1", "--at", "1/2", "--json", "--size", "320x240",
    ]);

    assert!(output.status.success());
    assert!(standard_output(&output).contains("\"numerator\": \"1\""));
}

#[test]
fn plot_with_settle_writes_the_view_into_the_session() {
    let path = session_file("settled.calc", "x^2");
    let path_text = path.to_str().expect("path is UTF-8");
    let image = path.with_extension("png");
    let image_text = image.to_str().expect("path is UTF-8");

    let output = calc(&[
        "plot", path_text, "r1", "--output", image_text, "--view", "-1..1", "--settle", "--size",
        "320x240", "--locale", "en",
    ]);

    assert!(output.status.success());
    let written = std::fs::read_to_string(&path).expect("session file is read back");
    assert!(written.contains("\"picture\""));
}
