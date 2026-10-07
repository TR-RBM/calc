use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn identity(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .arg("--identity")
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn shown(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

#[test]
fn a_claim_that_fails_only_between_whole_numbers_is_refuted_at_a_half() {
    let output = identity(&["sin(pi*x) = 0"]);

    assert!(!output.status.success());
    assert_eq!(shown(&output), "not an identity: it fails at x = 1/2\n");
}

#[test]
fn an_inequality_that_fails_between_zero_and_one_is_refuted() {
    let output = identity(&["x^2 >= x"]);

    assert_eq!(shown(&output), "not an identity: it fails at x = 1/2\n");
}

#[test]
fn a_fraction_witness_is_written_as_a_fraction_for_a_program() {
    let output = identity(&["floor(x) = x", "--json"]);

    assert_eq!(
        shown(&output),
        "{\"claim\": \"floor(x) = x\", \"verdict\": \"not_an_identity\", \"witness\": {\"x\": \"1/2\"}}\n"
    );
}

#[test]
fn an_undecided_answer_names_the_fractions_it_tried() {
    let output = identity(&["x^2 + 1 > 0"]);

    assert!(
        shown(&output).contains(
            "neither a whole number from -16 to 16 nor a fraction with a denominator from 2 to 4 between -2 and 2 refutes it"
        ),
        "{}",
        shown(&output)
    );
}

#[test]
fn a_fraction_search_that_cannot_decide_claims_only_the_whole_numbers() {
    let output = identity(&["gcd(x, 1) = 1"]);

    assert!(
        shown(&output).contains("no whole number from -16 to 16 refutes it"),
        "{}",
        shown(&output)
    );
}

#[test]
fn a_program_is_told_which_fractions_were_tried() {
    let output = identity(&["x^2 + 1 > 0", "--json"]);

    assert!(
        shown(&output).contains("\"fractions\": {\"denominators\": [2, 3, 4], \"bound\": 2}"),
        "{}",
        shown(&output)
    );
}
