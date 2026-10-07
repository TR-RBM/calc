use std::process::{Command, Output};

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

#[test]
fn a_square_root_of_two_large_primes_in_a_coefficient_survives_the_derivative() {
    let output = calc(&[
        "diff(x^2 + sqrt(4294967291)*sqrt(4294967279)*x, x)",
        "--terse",
        "--locale",
        "en",
    ]);

    assert_eq!(
        standard_output(&output),
        "2 * x + sqrt(18446743979220271189) exact\n"
    );
}

#[test]
fn a_derivative_at_a_point_is_exact() {
    let output = calc(&["diff(x^2, x, 3)", "--terse", "--locale", "en"]);

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "6 exact\n");
}

#[test]
fn the_derivative_of_a_sine_at_zero_is_one() {
    let output = calc(&["diff(sin(x), x, 0)", "--terse", "--locale", "en"]);

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "1 exact\n");
}

#[test]
fn the_derivative_of_a_logarithm_is_the_reciprocal_of_the_point() {
    let output = calc(&["diff(ln(x), x, 2)", "--terse", "--locale", "en"]);

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "1/2 exact\n");
}

#[test]
fn a_derivative_identity_holds_at_every_point_of_a_range() {
    let output = calc(&[
        "--find",
        "x=1..20",
        "--counter",
        "diff(x^3, x) = 3*x^2",
        "--locale",
        "en",
    ]);

    assert!(output.status.success());
    assert_eq!(
        standard_output(&output),
        "nothing in the range makes it fail\n"
    );
}

#[test]
fn a_wrong_derivative_identity_is_refuted_by_a_counterexample() {
    let output = calc(&[
        "--find",
        "x=1..10",
        "--counter",
        "diff(x^3, x) = 2*x^2",
        "--locale",
        "en",
    ]);

    assert!(!output.status.success());
    assert!(standard_output(&output).starts_with("x = 1"));
}

#[test]
fn a_derivative_with_no_rule_says_it_cannot_be_evaluated() {
    let output = calc(&["diff(abs(x), x, 1)", "--locale", "en"]);

    assert!(!output.status.success());
}

#[test]
fn the_language_reference_no_longer_calls_the_derivative_uncomputed() {
    let output = calc(&["help", "language", "--locale", "en"]);
    let shown = standard_output(&output);
    let derivative = shown
        .split("\n  ")
        .find(|entry| entry.starts_with("derivative"))
        .expect("the reference names the derivative");

    assert!(!derivative.contains("does not compute it yet"));
}

#[test]
fn a_factor_of_one_is_dropped_once_the_point_is_substituted() {
    let output = calc(&["diff(log10(x), x, 1)", "--terse", "--locale", "en"]);

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "1 / ln(10) exact\n");
}

#[test]
fn a_factor_that_is_not_one_stays_in_the_derivative() {
    let output = calc(&["diff(log10(x), x, 2)", "--terse", "--locale", "en"]);

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "1 / (2 * ln(10)) exact\n");
}

#[test]
fn the_product_rule_folds_both_factors_of_one_at_the_point() {
    let output = calc(&["diff(x*sin(x), x, 1)", "--terse", "--locale", "en"]);

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "sin(1) + cos(1) exact\n");
}

#[test]
fn a_derivative_of_a_list_is_taken_entry_by_entry() {
    let output = calc(&["diff([x, x^2], x, 2)", "--terse", "--locale", "en"]);

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "[1, 4] exact\n");
}

#[test]
fn a_derivative_of_a_matrix_keeps_its_shape() {
    let output = calc(&[
        "diff([x, x^2; x^3, x^4], x, 1)",
        "--terse",
        "--locale",
        "en",
    ]);

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "[1, 2; 3, 4] exact\n");
}

#[test]
fn a_derivative_of_a_list_at_the_variable_needs_the_variable() {
    let output = calc(&["diff([x, x^2], x)", "--json"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        standard_output(&output).contains("\"code\": \"undefined_name\""),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn a_derivative_at_a_list_of_points_answers_one_value_for_each() {
    let output = calc(&["diff(x^2, x, [1,2])", "--terse", "--locale", "en"]);

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "[2, 4] exact\n");
}

#[test]
fn a_matrix_of_points_keeps_its_shape() {
    let output = calc(&["diff(x^2, x, [1,2;3,4])", "--terse", "--locale", "en"]);

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "[2, 4; 6, 8] exact\n");
}

#[test]
fn a_point_the_derivative_refuses_answers_with_that_cells_refusal() {
    let output = calc(&["diff(abs(x), x, [0,1])", "--terse", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        standard_output(&output),
        "error diff(abs(x), x, 0) cannot be evaluated yet\n"
    );
}

#[test]
fn a_list_in_both_positions_is_refused_and_both_readings_are_named() {
    let output = calc(&["diff([x, x^2], x, [1,2])", "--terse", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        standard_output(&output),
        "error a derivative with a list in what is differentiated and a list of points \
         is ambiguous: it could pair each entry with the point beside it, or take every \
         entry at every point, so calc does not choose one; write one point for the whole \
         of it, as in diff([x, x^2], x, 2), or one entry for each point\n"
    );
}

#[test]
fn lists_of_different_lengths_meet_the_same_refusal() {
    let output = calc(&["diff([x, x^2], x, [1,2,3])", "--terse", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(standard_output(&output).contains("could pair each entry"));
}

#[test]
fn the_refusal_names_both_readings_in_german_too() {
    let output = calc(&["diff([x, x^2], x, [1,2])", "--terse", "--locale", "de"]);

    assert!(standard_output(&output).contains("jeden Eintrag an jeder Stelle"));
}
