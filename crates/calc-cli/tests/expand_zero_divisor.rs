use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn shown(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

#[test]
fn expand_names_a_division_by_an_identical_zero() {
    assert_eq!(
        shown(&calc(&["--expand", "x/(x - x)"])),
        "x / (x - x) divides by zero\n"
    );
}

#[test]
fn expand_of_a_division_by_an_identical_zero_is_refused() {
    assert_eq!(calc(&["--expand", "x/(x - x)"]).status.code(), Some(1));
}

#[test]
fn expand_json_names_the_division_by_zero() {
    assert_eq!(
        shown(&calc(&["--expand", "x/(x - x)", "--json"])),
        "{\"entry\": \"x/(x - x)\", \"refused\": \"division_by_zero\", \"reading\": \"x / (x - x)\"}\n"
    );
}
