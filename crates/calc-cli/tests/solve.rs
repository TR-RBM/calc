use std::io::Write;
use std::process::{Command, Output, Stdio};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

const WAYS_REQUEST: &str =
    r#"{"phase": "ways", "object": "circle", "wanted": "circumference", "cap": 3}"#;
const SOLVED_REQUEST: &str = r#"{"phase": "evaluate", "object": "circle", "wanted": "circumference", "given": [{"name": "diameter", "value": "2", "uncertainty": "0.01", "unit": "m"}], "only_obtainable": true}"#;
const NOT_REACHED_REQUEST: &str = r#"{"phase": "evaluate", "object": "triangle", "wanted": "area", "given": [{"name": "side-a", "value": "3", "unit": "m"}, {"name": "side-b", "value": "4", "unit": "m"}], "not_obtainable": ["side-c", "angle-gamma", "angle-alpha", "angle-beta", "height-b"]}"#;

fn solve_argument(request: &str) -> Output {
    Command::new(CALC)
        .args(["solve", "--json", "--request", request])
        .env_clear()
        .output()
        .expect("calc runs")
}

fn solve_standard_input(request: &str) -> Output {
    let mut child = Command::new(CALC)
        .args(["solve", "--json"])
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
        .write_all(request.as_bytes())
        .expect("request is written");
    child.wait_with_output().expect("calc finishes")
}

fn answer(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("answer is UTF-8")
}

fn member<'text>(text: &'text str, name: &str) -> &'text str {
    let key = format!("\"{name}\": ");
    let start = text.find(&key).expect("member is present") + key.len();
    let rest = &text[start..];
    &rest[..rest.find(",\n").unwrap_or(rest.len())]
}

#[test]
fn ways_request_prints_the_ranked_ways_and_exits_with_zero() {
    let output = solve_argument(WAYS_REQUEST);
    let text = answer(&output);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(text.starts_with("{\n  \"status\": \"ways\",\n  \"concept_set_version\": \""));
    assert_eq!(member(&text, "truncated_by"), "\"cap\"");
    assert_eq!(
        member(&text, "rule"),
        "\"circle-circumference/from-diameter\""
    );
    assert!(text.ends_with("}\n"));
}

#[test]
fn ways_request_gives_the_same_bytes_on_every_run() {
    assert_eq!(
        answer(&solve_argument(WAYS_REQUEST)),
        answer(&solve_standard_input(WAYS_REQUEST))
    );
}

#[test]
fn evaluated_request_prints_the_solved_record_and_exits_with_zero() {
    let output = solve_standard_input(SOLVED_REQUEST);
    let text = answer(&output);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(member(&text, "status"), "\"solved\"");
    assert!(text.contains("\"name\": \"way_evaluation\",\n"));
    assert!(text.contains("\"unit\": \"m\",\n"));
    assert!(text.contains("\"standard\": {\n"));
}

#[test]
fn not_reached_request_prints_the_bound_and_exits_with_four() {
    let output = solve_argument(NOT_REACHED_REQUEST);
    let text = answer(&output);
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(member(&text, "status"), "\"not_reached\"");
    assert!(text.contains("\"unit\": \"m^2\",\n"));
    assert!(text.contains("\"digits\": \"6\"\n"));
}

#[test]
fn invalid_request_prints_the_typed_error_and_exits_with_three() {
    let output = solve_argument(r#"{"phase": "ways", "wanted": "happiness"}"#);
    let text = answer(&output);
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(member(&text, "status"), "\"invalid_request\"");
    assert_eq!(member(&text, "code"), "\"unknown_wanted\"");
}

#[test]
fn solve_without_json_option_prints_no_answer_and_exits_with_one() {
    let output = Command::new(CALC)
        .args(["solve", "--request", WAYS_REQUEST])
        .env_clear()
        .output()
        .expect("calc runs");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .expect("error is UTF-8")
            .contains("\"code\": \"json_output_required\"")
    );
}
