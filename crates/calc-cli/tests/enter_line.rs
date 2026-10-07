use std::io::Write;
use std::process::{Command, Output, Stdio};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

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

#[test]
fn a_line_is_entered_into_the_session_on_standard_input() {
    let output = calc(&["-", "--enter", "1/3 + 1/6"], EMPTY_SESSION);

    assert_eq!(output.status.code(), Some(0));
    let saved = text(&output);
    assert!(saved.contains("\"input\": \"1/3 + 1/6\""), "{saved}");
    assert!(saved.contains("\"id\": \"r1\""), "{saved}");
}

#[test]
fn a_second_entered_line_refers_to_the_first() {
    let first = calc(&["-", "--enter", "1/3 + 1/6"], EMPTY_SESSION);
    let second = calc(&["-", "--enter", "r1 * 2"], &text(&first));

    assert_eq!(second.status.code(), Some(0));
    let saved = text(&second);
    assert!(saved.contains("\"id\": \"r2\""), "{saved}");
    assert!(saved.contains("\"input\": \"r1 * 2\""), "{saved}");
    assert!(saved.contains("\"digits\": \"1\""), "{saved}");
}

#[test]
fn a_line_that_does_not_parse_is_refused_and_changes_nothing() {
    let output = calc(&["-", "--enter", "200 + 15%"], EMPTY_SESSION);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(text(&output), String::new());
    let reported = String::from_utf8(output.stderr).expect("errors are UTF-8");
    assert!(reported.contains("\"code\": \"parse_error\""), "{reported}");
}

#[test]
fn an_escape_time_request_is_entered_as_its_text() {
    let request = "escape_time {\"form\":\"quadratic_parameter\"}";
    let output = calc(&["-", "--enter", request], EMPTY_SESSION);

    assert_eq!(output.status.code(), Some(0));
    assert!(text(&output).contains("escape_time"), "{}", text(&output));
}

#[test]
fn a_session_is_begun_from_the_command_line_and_the_next_call_reads_it() {
    let begun = calc(&["-", "--begin"], "");

    assert_eq!(begun.status.code(), Some(0));
    let empty = text(&begun);
    assert!(empty.contains("\"lines\": []"), "{empty}");

    let entered = calc(&["-", "--enter", "0.1 + 0.2"], &empty);

    assert_eq!(entered.status.code(), Some(0));
    assert!(text(&entered).contains("\"input\": \"0.1 + 0.2\""));
}

#[test]
fn a_machine_line_is_reachable_from_a_begun_session() {
    let begun = calc(&["-", "--begin"], "");
    let entered = calc(&["-", "--enter", "0.1 + 0.2"], &text(&begun));
    let machine = calc(&["-", "--machine-line", "r1", "f64"], &text(&entered));

    assert_eq!(machine.status.code(), Some(0));
    let saved = text(&machine);
    assert!(saved.contains("\"input\": \"to_f64(r1)\""), "{saved}");
    assert!(
        saved.contains("\"bits\": \"0x3fd3333333333334\""),
        "{saved}"
    );
}
