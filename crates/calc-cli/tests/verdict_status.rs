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

fn piped(arguments: &[&str], standard_input: &str) -> Output {
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

#[test]
fn a_claim_that_holds_exits_zero() {
    assert_eq!(calc(&["--identity", "x = x"]).status.code(), Some(0));
}

#[test]
fn a_claim_proved_false_exits_one() {
    assert_eq!(calc(&["--identity", "x = 2*x"]).status.code(), Some(1));
}

#[test]
fn an_undecided_claim_exits_four() {
    assert_eq!(
        calc(&["--identity", "sqrt(x^2) = abs(x)"]).status.code(),
        Some(4)
    );
}

#[test]
fn a_line_that_is_not_a_claim_exits_three() {
    assert_eq!(calc(&["--identity", "((("]).status.code(), Some(3));
    assert_eq!(calc(&["--check", "2+2"]).status.code(), Some(3));
}

#[test]
fn a_search_that_reaches_nothing_exits_four_and_a_counter_that_does_exits_zero() {
    assert_eq!(calc(&["--find", "x=1..5", "x = 99"]).status.code(), Some(4));
    assert_eq!(
        calc(&["--find", "x=1..5", "--counter", "x = x"])
            .status
            .code(),
        Some(0)
    );
}

#[test]
fn a_counterexample_found_exits_one() {
    assert_eq!(
        calc(&["--find", "x=1..5", "--counter", "x = 2"])
            .status
            .code(),
        Some(1)
    );
}

#[test]
fn a_run_of_many_lines_takes_the_most_consequential_status() {
    let proved_false_beats_undecided =
        piped(&["--check", "--batch"], "1+1 = 3\nsin(1)^2+cos(1)^2 = 1\n");
    let unreadable_beats_undecided = piped(&["--check", "--batch"], "(((\nsin(1)^2+cos(1)^2 = 1\n");

    assert_eq!(proved_false_beats_undecided.status.code(), Some(1));
    assert_eq!(unreadable_beats_undecided.status.code(), Some(3));
}

#[test]
fn a_definition_does_not_move_the_status() {
    let output = piped(&["--check", "--batch"], "f(x) = x^2\nf(3) = 9\n");

    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn a_search_above_the_limit_is_not_a_refutation() {
    let output = calc(&["--find", "x=1..100000000", "--counter", "x > 0"]);

    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn a_claim_between_a_length_and_a_time_is_refused_with_the_mismatch() {
    let output = calc(&["1 m = 1 s", "--check", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stdout).starts_with(
            "refused  1 m = 1 s: m measures length (units: au, ft, in, ly, m, mi, nmi, pc, thou, yd) and s measures time"
        )
    );
}

#[test]
fn an_identity_between_a_length_and_a_time_is_refused_for_a_program() {
    let output = calc(&["--identity", "1 m = 1 s", "--json"]);

    assert!(String::from_utf8_lossy(&output.stdout).contains("\"verdict\": \"refused\""));
    assert!(String::from_utf8_lossy(&output.stdout).contains("\"code\": \"dimension_mismatch\""));
}

#[test]
fn zero_to_a_half_is_zero_and_the_claim_holds() {
    let output = calc(&["0^(1/2) = 0", "--check", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn a_claim_between_exact_complex_values_holds() {
    let output = calc(&["i*i = -1", "--check", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "holds  i*i = -1\n");
}

#[test]
fn a_claim_and_a_sorted_list_agree_on_two_arcsines() {
    let claim = calc(&["asin(1/2) > asin(1/3)", "--check", "--locale", "en"]);
    let sorted = calc(&[
        "sorted([asin(1/2), asin(1/3)])",
        "--terse",
        "--locale",
        "en",
    ]);

    assert_eq!(claim.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&claim.stdout),
        "holds  asin(1/2) > asin(1/3)\n"
    );
    assert_eq!(
        String::from_utf8_lossy(&sorted.stdout),
        "[asin(1 / 3), pi / 6] exact\n"
    );
}
