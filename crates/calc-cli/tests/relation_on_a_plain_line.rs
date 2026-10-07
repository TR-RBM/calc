use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn terse(line: &str) -> String {
    let output = calc(&[line, "--terse"]);
    [output.stdout, output.stderr]
        .map(|bytes| String::from_utf8(bytes).expect("output is UTF-8"))
        .concat()
}

#[test]
fn a_comparison_of_numbers_names_check() {
    assert_eq!(
        terse("3 < 5"),
        "error 3 < 5 is a claim, not a value; calc decides it with --check: calc \"3 < 5\" --check\n"
    );
}

#[test]
fn a_claim_about_a_free_name_names_the_options_that_ask_it() {
    assert_eq!(
        terse("x < 5"),
        "error x < 5 is a claim about x, not a value; calc decides it for every value with --identity, looks for a counterexample with --find and solves it with --solve-for\n"
    );
}

#[test]
fn a_claim_on_a_plain_line_is_refused() {
    assert_eq!(calc(&["3 < 5"]).status.code(), Some(1));
}

#[test]
fn a_claim_on_a_plain_line_has_its_own_code_in_json() {
    let text = String::from_utf8(calc(&["3 < 5", "--json"]).stdout).expect("output is UTF-8");

    assert!(text.contains("\"code\": \"relation_is_a_claim\""), "{text}");
}

#[test]
fn a_naming_is_still_a_naming() {
    assert_eq!(terse("x = 5"), "5 exact\n");
}

#[test]
fn a_claim_is_quoted_as_the_person_wrote_it() {
    assert_eq!(
        terse("1 = 1"),
        "error 1 = 1 is a claim, not a value; calc decides it with --check: calc \"1 = 1\" --check\n"
    );
}
