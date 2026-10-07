use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn factor(entry: &str) -> Output {
    Command::new(CALC)
        .args(["--factor", entry, "--locale", "en"])
        .env_clear()
        .output()
        .expect("calc runs")
}

fn standard_output(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

#[test]
fn a_whole_number_is_written_as_a_product_of_prime_powers() {
    let output = factor("360");
    assert!(output.status.success());
    assert_eq!(standard_output(&output), "2^3 * 3^2 * 5\n");
}

#[test]
fn a_fraction_is_factored_above_and_below() {
    assert_eq!(standard_output(&factor("-12/35")), "-2^2 * 3 / (5 * 7)\n");
}

#[test]
fn a_prime_is_its_own_factorization() {
    assert_eq!(standard_output(&factor("97")), "97\n");
}

#[test]
fn the_strong_pseudoprime_to_twelve_bases_is_split() {
    assert_eq!(
        standard_output(&factor("318665857834031151167461")),
        "399165290221 * 798330580441\n"
    );
}

#[test]
fn a_prime_above_the_proven_bound_is_called_probable() {
    assert!(standard_output(&factor("2^89 - 1")).contains("is probably prime"));
}

#[test]
fn a_composite_rho_cannot_split_is_named_as_not_split() {
    let output = factor("(2^61-1)*(2^89-1)");
    assert!(!output.status.success());
    assert!(standard_output(&output).contains("is not prime, and calc did not find its factors"));
}

#[test]
fn a_line_that_is_not_rational_is_refused() {
    assert!(standard_output(&factor("sqrt(2)")).contains("calc factors an exact rational number"));
}

#[test]
fn a_factorization_names_each_factor_s_status_for_a_program() {
    let output = Command::new(CALC)
        .args(["--factor", "2^89 - 1", "--json"])
        .env_clear()
        .output()
        .expect("calc runs");
    assert!(standard_output(&output).contains("\"status\": \"probably_prime\""));
}

#[test]
fn a_polynomial_is_factored_into_irreducibles_over_the_rationals() {
    assert_eq!(
        standard_output(&factor("x^4 - 5*x^2 + 6")),
        "(x^2 - 3) * (x^2 - 2)\n"
    );
}

#[test]
fn an_irreducible_polynomial_is_its_own_factorization() {
    assert_eq!(standard_output(&factor("x^4 + 1")), "x^4 + 1\n");
}

#[test]
fn a_constant_factor_leads_the_product() {
    assert_eq!(
        standard_output(&factor("2*x^2 - 2")),
        "2 * (x - 1) * (x + 1)\n"
    );
}

#[test]
fn a_polynomial_that_splits_modulo_every_prime_stays_whole() {
    assert_eq!(
        standard_output(&factor("x^4 - 10*x^2 + 1")),
        "x^4 - 10 * x^2 + 1\n"
    );
}
