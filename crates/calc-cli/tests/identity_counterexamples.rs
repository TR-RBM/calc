use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");
const REFUTED: i32 = 1;
const UNDECIDED: i32 = 4;

fn identity(claim: &str) -> Output {
    Command::new(CALC)
        .args(["--identity", claim, "--locale", "en"])
        .env_clear()
        .output()
        .expect("calc runs")
}

fn shown(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

fn assert_refuted_between_whole_numbers(claim: &str) {
    let output = identity(claim);
    let text = shown(&output);

    assert_eq!(output.status.code(), Some(REFUTED), "{text}");
    assert!(
        text.starts_with("not an identity: it fails at x = "),
        "{text}"
    );
    assert!(text.contains('/'), "{text}");
}

#[test]
fn a_square_of_a_cosine_that_is_one_at_whole_numbers_is_refuted() {
    assert_refuted_between_whole_numbers("cos(pi*x)^2 = 1");
}

#[test]
fn a_product_of_neighbours_that_dips_below_zero_is_refuted() {
    assert_refuted_between_whole_numbers("x*(x - 1) >= 0");
}

#[test]
fn a_distance_from_one_half_that_reaches_zero_is_refuted() {
    assert_refuted_between_whole_numbers("abs(x - 1/2) >= 1/4");
}

#[test]
fn a_claim_in_two_names_says_it_tried_only_whole_numbers() {
    let output = identity("abs(x*y) = abs(x)*abs(y)");
    let text = shown(&output);

    assert_eq!(output.status.code(), Some(UNDECIDED), "{text}");
    assert!(
        text.contains("no whole number from -16 to 16 refutes it"),
        "{text}"
    );
    assert!(!text.contains("fraction"), "{text}");
}

#[test]
fn a_claim_in_two_names_false_only_between_whole_numbers_is_not_said_to_hold() {
    let output = identity("floor(x) + floor(y) = x + y");
    let text = shown(&output);

    assert_eq!(output.status.code(), Some(UNDECIDED), "{text}");
    assert!(text.starts_with("undecided"), "{text}");
}
