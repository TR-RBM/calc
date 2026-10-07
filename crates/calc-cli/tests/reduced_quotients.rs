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
fn a_cancelled_factor_answers_with_its_condition_row() {
    let text = shown(&calc(&["(x^2-1)/(x-1)", "--locale", "en"]));

    assert!(text.contains("  value     x + 1\n"), "{text}");
    assert!(
        text.contains(
            "  valid     wherever x - 1 is not zero; where it is zero, the line has no value\n"
        ),
        "{text}"
    );
}

#[test]
fn a_difference_of_equal_quotients_keeps_its_condition() {
    assert_eq!(
        shown(&calc(&["1/x - 1/x", "--terse"])),
        "0 exact, wherever x is not zero\n"
    );
}

#[test]
fn a_sum_over_one_denominator_needs_no_condition() {
    assert_eq!(
        shown(&calc(&["1/(x-1) + 1/(x+1)", "--terse"])),
        "2 * x / (x^2 - 1) exact\n"
    );
}

#[test]
fn a_condition_names_the_denominator_as_written() {
    assert_eq!(
        shown(&calc(&["x/(x*y)", "--terse"])),
        "1 / y exact, wherever x * y is not zero\n"
    );
}

#[test]
fn expand_answers_the_same_reduced_form_and_condition() {
    assert_eq!(
        shown(&calc(&["--expand", "(x^2-1)/(x-1)"])),
        "x + 1, wherever x - 1 is not zero\n"
    );
}

#[test]
fn expand_gives_a_program_the_excluded_denominators() {
    assert_eq!(
        shown(&calc(&["--expand", "(x^2-1)/(x-1)", "--json"])),
        "{\"entry\": \"(x^2-1)/(x-1)\", \"expanded\": \"x + 1\", \"excluding\": [\"x - 1\"]}\n"
    );
}

#[test]
fn the_condition_row_reads_in_german() {
    let text = shown(&calc(&["(x^2-1)/(x-1)", "--locale", "de"]));

    assert!(
        text.contains(
            "überall, wo x - 1 nicht null ist; wo es null ist, hat die Zeile keinen Wert"
        ),
        "{text}"
    );
}
