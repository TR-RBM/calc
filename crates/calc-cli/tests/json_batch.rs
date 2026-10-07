use std::io::Write;
use std::process::{Command, Output, Stdio};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn batch(arguments: &[&str], input: &str) -> Output {
    let mut child = Command::new(CALC)
        .arg("--batch")
        .args(arguments)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("calc runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("input written");
    child.wait_with_output().expect("calc ends")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("output is UTF-8")
}

#[test]
fn a_refused_batch_line_names_its_number_and_input() {
    let output = batch(&["--terse", "--locale", "en"], "a = 1\na = 5\n");

    assert!(
        text(&output.stderr).contains("error: line 2, a = 5: a already exists"),
        "{}",
        text(&output.stderr)
    );
}

#[test]
fn a_self_reference_names_what_it_defines() {
    let output = batch(&["--terse", "--locale", "en"], "k = k + 1\n1/0\n");

    assert!(
        text(&output.stderr).contains("error: line 1, k = k + 1: k would depend on itself"),
        "{}",
        text(&output.stderr)
    );
}

#[test]
fn a_refused_batch_line_reaches_the_json() {
    let output = batch(&["--json"], "a = 1\na = 5\n");

    assert!(
        text(&output.stdout).contains(
            "{\"code\": \"name_exists\", \"input\": \"a = 5\", \"data\": {\"name\": \"a\"}, \"line_number\": \"2\"}"
        ),
        "{}",
        text(&output.stdout)
    );
}

#[test]
fn a_range_search_answers_in_json() {
    let output = Command::new(CALC)
        .args(["--find", "n=0..5", "--counter", "2^n > n^2", "--json"])
        .env_clear()
        .output()
        .expect("calc runs");

    assert_eq!(
        text(&output.stdout),
        "{\"claim\": \"2^n > n^2\", \"searched\": [\"n=0..5\"], \"asked\": \"fails\", \"found\": [{\"n\": \"2\"}, {\"n\": \"3\"}, {\"n\": \"4\"}], \"undecided\": null}\n"
    );
}

#[test]
fn a_fraction_before_a_unit_names_both_readings_in_json() {
    let output = Command::new(CALC)
        .args(["1/2 kg", "--json"])
        .env_clear()
        .output()
        .expect("calc runs");
    let shown = text(&output.stderr) + &text(&output.stdout);

    assert!(shown.contains("\"fraction\": \"(1/2) kg\""), "{shown}");
    assert!(shown.contains("\"reciprocal\": \"1/(2 kg)\""), "{shown}");
    assert!(shown.contains("\"input\": \"1/2 kg\""), "{shown}");
}
