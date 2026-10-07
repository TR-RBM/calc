use std::io::Write;
use std::process::{Command, Output, Stdio};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn piped(arguments: &[&str], input: &str) -> Output {
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
        .as_mut()
        .expect("stdin is piped")
        .write_all(input.as_bytes())
        .expect("the input is written");
    child.wait_with_output().expect("calc finishes")
}

fn standard_output(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

#[test]
fn an_identity_answers_in_json_when_asked() {
    let output = calc(&[
        "--identity",
        "(x+1)^2 = x^2+2*x+1",
        "--json",
        "--locale",
        "en",
    ]);

    assert_eq!(
        standard_output(&output),
        "{\"claim\": \"(x+1)^2 = x^2+2*x+1\", \"verdict\": \"holds_everywhere\"}\n"
    );
}

#[test]
fn a_batch_of_identities_answers_one_object_per_line() {
    let output = piped(
        &["--identity", "--batch", "--json", "--locale", "en"],
        "(x+1)^2 = x^2+2*x+1\n(x+1)^2 = x^2+1\n",
    );
    let shown = standard_output(&output);
    let lines: Vec<&str> = shown.lines().collect();

    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("\"verdict\": \"holds_everywhere\""));
    assert!(lines[1].contains("\"verdict\": \"not_an_identity\""));
}

#[test]
fn a_solution_set_answers_as_a_json_array() {
    let output = calc(&["--solve-for", "x^2 = 4", "--json", "--locale", "en"]);

    assert_eq!(
        standard_output(&output),
        "{\"equation\": \"x^2 = 4\", \"solutions\": [\"-2\", \"2\"]}\n"
    );
}

#[test]
fn a_refused_solution_names_its_reason_and_its_detail() {
    let output = calc(&["--solve-for", "x^40 = 2", "--json", "--locale", "en"]);

    assert_eq!(
        standard_output(&output),
        "{\"equation\": \"x^40 = 2\", \"refused\": \"degree_too_high\", \"detail\": \"40\"}\n"
    );
}

#[test]
fn an_unproven_square_part_is_refused_with_the_number_as_its_detail() {
    let output = calc(&[
        "--solve-for",
        "x^2 = 618970019642690137449562111",
        "--json",
        "--locale",
        "en",
    ]);

    assert_eq!(
        standard_output(&output),
        "{\"equation\": \"x^2 = 618970019642690137449562111\", \"refused\": \"square_part_unknown\", \"detail\": \"618970019642690137449562111\"}\n"
    );
}

#[test]
fn an_expansion_answers_in_json_when_asked() {
    let output = calc(&["--expand", "(x+1)^3", "--json", "--locale", "en"]);

    assert_eq!(
        standard_output(&output),
        "{\"entry\": \"(x+1)^3\", \"expanded\": \"x^3 + 3 * x^2 + 3 * x + 1\"}\n"
    );
}

#[test]
fn an_expansion_of_a_relation_expands_both_sides() {
    let output = calc(&["--expand", "(x+1)^2 = x^2+1", "--locale", "en"]);

    assert_eq!(standard_output(&output), "x^2 + 2 * x + 1 == x^2 + 1\n");
}

#[test]
fn what_an_expansion_prints_can_be_read_back() {
    let output = calc(&["--identity", "x^2 + 2 * x + 1 == x^2 + 1", "--locale", "en"]);

    assert!(standard_output(&output).starts_with("not an identity"));
}

#[test]
fn a_claim_that_cannot_be_read_says_so_and_says_why() {
    let output = calc(&[
        "--check",
        "35 commits - 11 commits = 24 commits",
        "--locale",
        "en",
    ]);

    assert!(!output.status.success());
    assert_eq!(
        standard_output(&output),
        "unreadable  35 commits - 11 commits = 24 commits: commits is not a unit at column 4\n"
    );
}

#[test]
fn a_line_that_was_read_and_is_not_a_relation_still_says_that() {
    let output = calc(&["--check", "x + 1", "--locale", "en"]);

    assert!(!output.status.success());
    assert_eq!(standard_output(&output), "not a relation  x + 1\n");
}

#[test]
fn an_unreadable_claim_carries_the_fault_typed_in_json() {
    let output = calc(&["--check", "10 us = 10 us", "--json", "--locale", "en"]);

    assert_eq!(
        standard_output(&output),
        "{\"claim\": \"10 us = 10 us\", \"verdict\": \"unreadable\", \"error\": { \"code\": \"parse_error\", \"data\": { \"kind\": \"not_a_unit\", \"column\": \"4\" } }}\n"
    );
}

#[test]
fn the_typed_fault_is_the_same_in_every_locale() {
    let english = calc(&["--check", "10 us = 10 us", "--json", "--locale", "en"]);
    let german = calc(&["--check", "10 us = 10 us", "--json", "--locale", "de"]);

    assert_eq!(standard_output(&english), standard_output(&german));
}

#[test]
fn every_other_verdict_carries_a_null_error_so_the_shape_is_one_shape() {
    let output = calc(&["--check", "2 + 2 = 5", "--json", "--locale", "en"]);

    assert_eq!(
        standard_output(&output),
        "{\"claim\": \"2 + 2 = 5\", \"verdict\": \"fails\", \"error\": null}\n"
    );
}

#[test]
fn a_batch_writes_one_object_per_line_for_every_outcome() {
    let output = piped(
        &["--batch", "--json", "--locale", "en"],
        "1/3+1/6\n200 + 15%\nr9 + 1\n",
    );
    let shown = standard_output(&output);
    let lines: Vec<&str> = shown.lines().collect();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("\"input\": \"1/3+1/6\""), "{shown}");
    assert!(
        lines[1].starts_with("{\"code\": \"parse_error\", \"input\": \"200 + 15%\""),
        "{shown}"
    );
    assert!(lines[2].contains("\"status\": \"error\""), "{shown}");
}

#[test]
fn a_batch_writes_its_parse_error_where_the_other_lines_go() {
    let output = piped(&["--batch", "--json", "--locale", "en"], "200 + 15%\n");

    assert_eq!(
        String::from_utf8(output.stderr.clone()).expect("errors are UTF-8"),
        String::new()
    );
    assert_eq!(standard_output(&output).lines().count(), 1);
}

#[test]
fn a_form_flag_applies_to_every_line_of_a_batch() {
    let output = piped(
        &["--batch", "--digits", "3", "--locale", "en"],
        "1/7\n2/7\n",
    );
    let shown = standard_output(&output);

    assert!(output.status.success());
    assert!(shown.contains("truncated       0.142"), "{shown}");
    assert!(shown.contains("truncated       0.285"), "{shown}");
}

#[test]
fn a_form_flag_under_batch_and_json_writes_one_object_per_line() {
    let output = piped(&["--batch", "--digits", "3", "--json"], "1/7\n2/7\n");
    let shown = standard_output(&output);

    assert!(output.status.success());
    assert_eq!(shown.lines().count(), 2);
    assert!(
        shown.lines().all(|line| line.starts_with("{\"places\": 3")),
        "{shown}"
    );
}

#[test]
fn an_enclosure_flag_applies_to_every_line_of_a_batch() {
    let output = piped(
        &["--batch", "--enclose", "5", "--locale", "en"],
        "sqrt(2)\n",
    );

    assert!(output.status.success());
    assert!(
        standard_output(&output).contains("lower bound         1.4142"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn an_instrument_under_batch_refuses_the_line_it_does_not_apply_to() {
    let output = piped(
        &["--batch", "--inspect", "--locale", "en"],
        "2 * pi * 3\nf(x) = x^2\n",
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--inspect does not apply to r2"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_solved_system_carries_each_name_and_the_free_ones() {
    let output = calc(&[
        "--solve-for",
        "[x + y = 3, 2*x + 2*y = 6]",
        "--json",
        "--locale",
        "en",
    ]);

    assert_eq!(
        standard_output(&output),
        "{\"equation\": \"[x + y = 3, 2*x + 2*y = 6]\", \"solutions\": {\"x\": \"-y + 3\"}, \"free\": [\"y\"]}\n"
    );
}

#[test]
fn a_solved_inequality_carries_each_interval_and_its_ends() {
    let output = calc(&[
        "--solve-for",
        "x^2 - 5*x + 6 <= 0",
        "--json",
        "--locale",
        "en",
    ]);

    assert_eq!(
        standard_output(&output),
        "{\"equation\": \"x^2 - 5*x + 6 <= 0\", \"intervals\": [{\"from\": \"2\", \"from_included\": true, \"to\": \"3\", \"to_included\": true}]}\n"
    );
}
