use std::io::Write;
use std::process::{Command, Output, Stdio};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

const FROM_RADIUS: &str =
    r#"{"phase": "reachable", "given": [{"name": "radius", "value": "2", "unit": "m"}]}"#;
const WITH_OBTAINABLE: &str = r#"{"phase": "reachable", "object": "triangle", "given": [{"name": "side-a", "value": "3", "unit": "m"}], "obtainable": ["side-b", "angle-gamma"]}"#;
const EMPTY_SESSION: &str = "{\"format\": \"calc-session\", \"version\": 2, \"settings\": {\"precision\": \"f64\", \"backend\": \"automatic\"}, \"next_line_number\": 1, \"lines\": []}";

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

fn entry<'answer>(answer: &'answer str, name: &str) -> &'answer str {
    let start = answer
        .find(&format!("\"name\": \"{name}\",\n      \"steps\""))
        .expect("entry is listed");
    let rest = &answer[start..];
    &rest[..rest.find("\n    }").unwrap_or(rest.len())]
}

#[test]
fn reachable_request_from_the_radius_lists_its_quantities_with_exit_zero() {
    let output = calc(&["solve", "--json", "--request", FROM_RADIUS], "");
    let answer = text(&output);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(answer.starts_with("{\n  \"status\": \"reachable\",\n  \"concept_set_version\": \""));
    assert!(
        answer.contains(
            "\n  \"object\": \"circle\",\n  \"truncated\": false,\n  \"reachable_total\": "
        )
    );
    let area = entry(&answer, "area");
    assert!(area.contains("\"rule\": \"circle-area/from-radius\",\n"));
    assert!(area.contains("\"holds\": true\n"));
    assert!(area.contains("\"needs_obtainable\": false,\n"));
}

#[test]
fn reachable_request_with_obtainable_marks_shows_what_needs_them() {
    let output = calc(&["solve", "--json"], WITH_OBTAINABLE);
    let answer = text(&output);
    assert_eq!(output.status.code(), Some(0));
    let area = entry(&answer, "area");
    assert!(area.contains("\"needs_obtainable\": true,\n"));
    assert!(area.contains("\"side-b\""));
    assert!(answer.contains("\"name\": \"side-b\",\n      \"mark\": \"obtainable\",\n"));
}

#[test]
fn reachable_request_gives_the_same_bytes_on_every_run() {
    assert_eq!(
        text(&calc(&["solve", "--json"], WITH_OBTAINABLE)),
        text(&calc(
            &["solve", "--json", "--request", WITH_OBTAINABLE],
            ""
        ))
    );
}

#[test]
fn choose_wanted_on_a_saved_reachable_line_writes_the_way_search() {
    let with_line = calc(&["-", "--solve", FROM_RADIUS], EMPTY_SESSION);
    assert_eq!(with_line.status.code(), Some(0));
    let chosen = calc(&["-", "--choose-wanted", "r1", "area"], &text(&with_line));
    let session = text(&chosen);
    assert_eq!(chosen.status.code(), Some(0));
    assert!(session.contains(
        "\\\"phase\\\": \\\"ways\\\", \\\"object\\\": \\\"circle\\\", \\\"wanted\\\": \\\"area\\\""
    ));
    assert!(session.contains("\"status\": \"ways\",\n"));
    assert!(session.contains("\"rule\": \"circle-area/from-radius\",\n"));
}

#[test]
fn reachable_request_with_a_wanted_role_exits_with_three() {
    let output = calc(
        &[
            "solve",
            "--json",
            "--request",
            r#"{"phase": "reachable", "wanted": "area"}"#,
        ],
        "",
    );
    assert_eq!(output.status.code(), Some(3));
    assert!(text(&output).contains("\"code\": \"unexpected_wanted\",\n"));
}
