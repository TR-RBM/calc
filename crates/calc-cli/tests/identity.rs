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

fn identity(claim: &str) -> Output {
    calc(&["--identity", claim, "--locale", "en"])
}

fn expand(text: &str) -> Output {
    calc(&["--expand", text, "--locale", "en"])
}

#[test]
fn a_binomial_identity_is_shown_to_hold_for_every_value() {
    let output = identity("(x+1)^2 = x^2 + 2*x + 1");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn a_three_variable_identity_is_shown_to_hold_for_every_value() {
    let output = identity("(a+b)^3 = a^3 + 3*a^2*b + 3*a*b^2 + b^3");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn a_wrong_identity_is_named_as_no_identity_at_all() {
    let output = identity("(x+1)^2 = x^2 + 1");

    assert!(!output.status.success());
    assert!(standard_output(&output).starts_with("not an identity"));
}

#[test]
fn an_identity_that_needs_more_than_algebra_says_it_is_undecided() {
    let output = identity("sqrt(x^2) = abs(x)");

    assert!(!output.status.success());
    assert!(standard_output(&output).starts_with("undecided"));
}

#[test]
fn an_inequality_that_holds_by_algebra_is_decided_everywhere() {
    let output = identity("x + 1 > x");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn a_false_arithmetic_claim_fails_and_has_nothing_to_vary() {
    let output = identity("2 + 2 = 5");

    assert!(!output.status.success());
    assert_eq!(
        standard_output(&output),
        "fails, and there is nothing in it to vary\n"
    );
}

#[test]
fn a_batch_of_claims_is_answered_line_by_line() {
    let output = calc(&["--identity", "--batch", "--locale", "en"]);

    assert!(output.status.success() || !output.status.success());
}

#[test]
fn a_power_of_a_sum_is_multiplied_out() {
    let output = expand("(x+1)^3");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "x^3 + 3 * x^2 + 3 * x + 1\n");
}

#[test]
fn a_difference_of_squares_is_multiplied_out_with_a_minus_sign() {
    let output = expand("(a+b)*(a-b)");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "a^2 - b^2\n");
}

#[test]
fn terms_that_cancel_leave_zero() {
    let output = expand("(x+1)^2 - x^2 - 2*x - 1");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "0\n");
}

#[test]
fn a_fractional_coefficient_stays_a_fraction() {
    let output = expand("(x+1)/2");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "x / 2 + 1 / 2\n");
}

#[test]
fn like_terms_are_collected_exactly() {
    let output = expand("x/3 + x/6");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "x / 2\n");
}

#[test]
fn a_derivative_identity_is_decided_for_every_value() {
    let output = identity("diff(x^3, x) = 3*x^2");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn a_claim_in_two_names_is_not_told_that_it_differs_almost_everywhere() {
    let output = identity("x*y = 0");
    let shown = standard_output(&output);

    assert!(!output.status.success());
    assert!(shown.starts_with("not an identity"));
    assert!(!shown.contains("finitely many"));
    assert!(shown.contains("not equal as polynomials"));
}

#[test]
fn a_claim_algebra_cannot_decide_is_refuted_by_its_smallest_counterexample() {
    let output = identity("sqrt(x^2) = x");

    assert!(!output.status.success());
    assert_eq!(
        standard_output(&output),
        "not an identity: it fails at x = -1\n"
    );
}

#[test]
fn an_undecided_answer_names_the_range_it_searched() {
    let output = identity("abs(x*y) = abs(x)*abs(y)");

    assert!(!output.status.success());
    assert!(
        standard_output(&output).contains("no whole number from -16 to 16 refutes it"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn a_search_that_meets_a_point_it_cannot_decide_says_where_it_stopped() {
    let output = identity("atan(tan(x)) = x");

    assert!(!output.status.success());
    assert!(
        standard_output(&output).contains("stopped at x = 1, where the claim could not be decided"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn a_refutation_names_its_witness_for_a_program() {
    let output = calc(&["--identity", "abs(x) = x", "--json"]);

    assert!(!output.status.success());
    assert_eq!(
        standard_output(&output),
        "{\"claim\": \"abs(x) = x\", \"verdict\": \"not_an_identity\", \"witness\": {\"x\": \"-1\"}}\n"
    );
}

#[test]
fn an_undecided_verdict_says_what_was_searched_for_a_program() {
    let output = calc(&["--identity", "abs(x*y) = abs(x)*abs(y)", "--json"]);

    assert!(!output.status.success());
    assert!(
        standard_output(&output).contains("\"searched\": \"-16..16\""),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn a_stopped_search_names_its_point_for_a_program() {
    let output = calc(&["--identity", "atan(tan(x)) = x", "--json"]);

    assert!(!output.status.success());
    assert!(
        standard_output(&output)
            .contains("\"searched\": \"stopped\", \"stopped_at\": {\"x\": \"1\"}"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn a_claim_with_no_free_names_is_decided_at_its_only_instance() {
    let output = identity("sqrt(2) = 1.5");

    assert!(!output.status.success());
    assert_eq!(
        standard_output(&output),
        "fails, and there is nothing in it to vary\n"
    );
}

#[test]
fn a_true_claim_with_no_free_names_holds_and_says_so() {
    let output = identity("sqrt(2)*sqrt(3) = sqrt(6)");

    assert!(output.status.success());
    assert_eq!(
        standard_output(&output),
        "holds, and there is nothing in it to vary\n"
    );
}

#[test]
fn a_point_that_cannot_be_decided_is_no_counterexample() {
    let output = calc(&[
        "--find",
        "x=1..3",
        "--counter",
        "sin(x)^2 + cos(x)^2 = 1",
        "--locale",
        "en",
    ]);

    assert_eq!(output.status.code(), Some(4));
    assert_eq!(
        standard_output(&output),
        "no value calc could decide in the range makes it fail\ncalc could not decide the claim at 3 values, the first x = 1\n"
    );
}

#[test]
fn the_normal_form_keeps_its_fractional_coefficients() {
    let output = expand("x/3 + x/2");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "5 * x / 6\n");
}

#[test]
fn a_quotient_that_cancels_holds_wherever_its_denominator_is_not_zero() {
    let output = identity("(x^2-1)/(x-1) = x+1");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        standard_output(&output),
        "holds wherever x - 1 is not zero\n"
    );
}

#[test]
fn the_same_quotient_on_both_sides_no_longer_holds_for_every_value() {
    let output = identity("1/x = 1/x");

    assert_eq!(standard_output(&output), "holds wherever x is not zero\n");
}

#[test]
fn a_name_over_itself_is_decided_and_says_where() {
    let output = identity("x/x = 1");

    assert_eq!(standard_output(&output), "holds wherever x is not zero\n");
}

#[test]
fn every_denominator_of_the_claim_is_named_in_the_order_it_appears() {
    let output = identity("1/(x-1) + 1/(x+1) = 2*x/(x^2-1)");

    assert_eq!(
        standard_output(&output),
        "holds wherever x - 1, x + 1 and x^2 - 1 are not zero\n"
    );
}

#[test]
fn a_denominator_with_no_real_zero_is_excluded_anyway() {
    let output = identity("1/(x^2+1) = 1/(x^2+1)");

    assert_eq!(
        standard_output(&output),
        "holds wherever x^2 + 1 is not zero\n"
    );
}

#[test]
fn a_constant_denominator_leaves_the_verdict_unconditional() {
    let output = identity("x/2 = 0.5*x");

    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn quotients_that_differ_are_still_no_identity() {
    let output = identity("1/x = 2/x");

    assert_eq!(output.status.code(), Some(1));
    assert!(standard_output(&output).starts_with("not an identity"));
}

#[test]
fn the_conditional_verdict_names_its_exclusions_for_a_program() {
    let output = calc(&["--identity", "(x^2-1)/(x-1) = x+1", "--json"]);

    assert_eq!(
        standard_output(&output),
        "{\"claim\": \"(x^2-1)/(x-1) = x+1\", \"verdict\": \"holds_where_defined\", \
         \"excluding\": [\"x - 1\"]}\n"
    );
}

#[test]
fn a_claim_the_algebra_cannot_normalise_is_undecided_and_not_called_no_relation() {
    let output = identity("1/0 = 1/0");

    assert_eq!(output.status.code(), Some(4));
    assert!(standard_output(&output).starts_with("undecided"));
}

#[test]
fn a_relation_that_fails_where_it_is_defined_says_where_and_fails() {
    let output = identity("1/x != 1/x");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(standard_output(&output), "fails wherever x is not zero\n");
}

#[test]
fn an_inequality_between_equal_sides_fails_where_they_are_numbers() {
    let output = identity("1/x > 1/x");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(standard_output(&output), "fails wherever x is not zero\n");
}

#[test]
fn a_name_over_itself_is_not_one_where_the_relation_says_it_is_not() {
    let output = identity("x/x != 1");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(standard_output(&output), "fails wherever x is not zero\n");
}

#[test]
fn the_failing_verdict_names_its_exclusions_for_a_program() {
    let output = calc(&["--identity", "1/x != 1/x", "--json"]);

    assert_eq!(
        standard_output(&output),
        "{\"claim\": \"1/x != 1/x\", \"verdict\": \"fails_where_defined\", \
         \"excluding\": [\"x\"]}\n"
    );
}

#[test]
fn a_relation_that_fails_at_every_value_keeps_the_unconditional_verdict() {
    let output = identity("x/2 != 0.5*x");

    assert_eq!(standard_output(&output), "fails for every value\n");
}

#[test]
fn a_true_claim_about_constants_is_not_refuted_at_a_value_they_cannot_take() {
    let output = identity("pi = 4*atan(1)");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        standard_output(&output),
        "holds, and there is nothing in it to vary\n"
    );
}

#[test]
fn a_false_claim_about_a_constant_is_decided_at_its_one_instance() {
    let output = identity("3 = pi");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        standard_output(&output),
        "fails, and there is nothing in it to vary\n"
    );
}

#[test]
fn a_claim_over_constants_answers_the_same_under_check_and_identity() {
    let checked = calc(&["--check", "pi = 4*atan(1)", "--locale", "en"]);
    let decided = identity("pi = 4*atan(1)");

    assert_eq!(checked.status.code(), Some(0));
    assert_eq!(decided.status.code(), Some(0));
    assert!(standard_output(&checked).starts_with("holds"));
    assert!(standard_output(&decided).starts_with("holds"));
}

#[test]
fn what_the_algebra_proves_over_constants_stays_proved() {
    let output = identity("(pi+1)^2 = pi^2 + 2*pi + 1");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        standard_output(&output),
        "holds, and there is nothing in it to vary\n"
    );
}

#[test]
fn a_claim_over_constants_the_instance_cannot_settle_is_undecided() {
    let output = identity("tan(1)*cos(1) = sin(1)");

    assert_eq!(output.status.code(), Some(4));
    assert!(standard_output(&output).starts_with("undecided"));
}

#[test]
fn a_constant_beside_a_free_name_leaves_the_free_name_to_the_search() {
    let output = identity("3*x = pi*x");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        standard_output(&output),
        "not an identity: it fails at x = 1\n"
    );
}

#[test]
fn a_denominator_that_is_a_constant_takes_no_conditional_verdict() {
    let output = identity("1/pi = 1/pi");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        standard_output(&output),
        "holds, and there is nothing in it to vary\n"
    );
}

#[test]
fn the_german_answer_for_a_claim_over_constants_says_the_same() {
    let output = calc(&["--identity", "3 = pi", "--locale", "de"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        standard_output(&output),
        "gilt nicht, und darin kann nichts variieren\n"
    );
}

#[test]
fn a_claim_about_infinity_is_not_refuted_by_the_algebra() {
    let output = identity("inf + 1 = inf");

    assert_eq!(output.status.code(), Some(4));
    assert!(standard_output(&output).starts_with("undecided"));
}

#[test]
fn neither_route_decides_a_claim_about_infinity() {
    let checked = calc(&["--check", "inf > 0", "--locale", "en"]);
    let decided = identity("inf > 0");

    assert_eq!(checked.status.code(), Some(4));
    assert_eq!(decided.status.code(), Some(4));
    assert!(standard_output(&checked).starts_with("undecided"));
    assert!(standard_output(&decided).starts_with("undecided"));
}

#[test]
fn a_constant_radical_expands_to_the_value_the_line_evaluates_to() {
    let output = expand("(3+sqrt(8))^2");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "17 + 12 * sqrt(2)\n");
    let evaluated = calc(&["(3+sqrt(8))^2", "--terse", "--locale", "en"]);
    assert_eq!(standard_output(&evaluated), "17 + 12 * sqrt(2) exact\n");
}

#[test]
fn an_identity_with_a_radical_coefficient_holds_for_every_value() {
    let output = identity("(x+sqrt(2))^2 = x^2 + 2*sqrt(2)*x + 2");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn a_square_root_of_a_name_squared_stays_undecided() {
    let output = identity("sqrt(x)^2 = x");

    assert!(standard_output(&output).starts_with("undecided"));
}

#[test]
fn the_square_root_of_zero_is_zero_to_both_halves_of_calc() {
    let output = identity("sqrt(0)*x = 0");
    assert_eq!(standard_output(&output), "holds for every value\n");
    let evaluated = calc(&["sqrt(0) + 1", "--terse", "--locale", "en"]);
    assert_eq!(standard_output(&evaluated), "1 exact\n");
}

#[test]
fn a_derivative_of_a_named_polynomial_is_taken_in_the_name_it_carries() {
    let mut child = std::process::Command::new(CALC)
        .args(["--batch", "--identity", "--locale", "en"])
        .env_clear()
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("calc runs");
    use std::io::Write as _;
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"p = x^2\ndiff(p, x) = 2*x\n")
        .expect("written");
    let output = child.wait_with_output().expect("calc ends");
    assert_eq!(
        standard_output(&output),
        "defines  p = x^2\nholds for every value\n"
    );
}

#[test]
fn a_closed_part_is_evaluated_before_the_algebra_decides() {
    let output = identity("sin(pi/6)*x = x/2");

    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn a_whole_cube_root_expands_as_the_integer_it_is() {
    let output = expand("(x + 8^(1/3))^2");

    assert_eq!(standard_output(&output), "x^2 + 4 * x + 4\n");
}

#[test]
fn the_trigonometric_pythagoras_holds_for_every_value() {
    let output = identity("sin(x)^2 + cos(x)^2 = 1");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn the_trigonometric_pythagoras_holds_for_every_value_in_german() {
    let output = calc(&["--identity", "sin(x)^2 + cos(x)^2 = 1", "--locale", "de"]);

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "gilt für jeden Wert\n");
}

#[test]
fn an_inverse_function_is_not_on_the_rewrite_list() {
    let output = identity("asin(sin(x)) = x");

    assert!(standard_output(&output).starts_with("undecided"));
}

#[test]
fn a_double_angle_holds_for_every_value() {
    let output = identity("sin(2*x) = 2*sin(x)*cos(x)");

    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn an_angle_sum_holds_for_every_value() {
    let output = identity("cos(x + y) = cos(x)*cos(y) - sin(x)*sin(y)");

    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn a_quarter_turn_shifts_the_sine_into_the_cosine() {
    let output = identity("sin(x + pi/2) = cos(x)");

    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn a_false_double_angle_is_refuted() {
    let output = identity("sin(2*x) = 2*sin(x)");

    assert!(!output.status.success());
    assert!(standard_output(&output).starts_with("not an identity"));
}

#[test]
fn a_power_of_a_real_root_reduces_by_its_polynomial() {
    let output = identity("rootof(x^3 - 2, x, 1)^3 = 2");

    assert!(output.status.success());
    assert!(standard_output(&output).starts_with("holds"));
}

#[test]
fn a_cube_root_cubed_is_its_radicand() {
    let output = identity("(2^(1/3))^3 = 2");

    assert!(standard_output(&output).starts_with("holds"));
}

#[test]
fn two_powers_of_one_cube_root_multiply_to_the_radicand() {
    let output = identity("2^(1/3) * 2^(2/3) = 2");

    assert!(standard_output(&output).starts_with("holds"));
}

#[test]
fn a_binomial_with_a_cube_root_holds_for_every_value() {
    let output = identity("(x + 2^(1/3))^3 = x^3 + 3*2^(1/3)*x^2 + 3*2^(2/3)*x + 2");

    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn a_wrong_decimal_for_a_cube_root_still_fails() {
    let output = identity("2^(1/3) = 1.26");

    assert!(standard_output(&output).starts_with("fails"));
}

#[test]
fn a_square_of_an_exponential_is_the_exponential_of_the_double() {
    let output = identity("exp(x)^2 = exp(2*x)");

    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn the_exponential_of_a_sum_is_the_product_of_the_exponentials() {
    let output = identity("exp(x + y) = exp(x)*exp(y)");

    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn an_exponential_times_its_reciprocal_is_one() {
    let output = identity("exp(x)*exp(-x) = 1");

    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn euler_s_number_is_the_exponential_of_one() {
    let output = identity("exp(x+1) = e*exp(x)");

    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn a_false_exponential_identity_is_refuted() {
    let output = identity("exp(2*x) = 2*exp(x)");

    assert!(standard_output(&output).starts_with("not an identity"));
}

#[test]
fn euler_s_number_is_decided_to_be_the_exponential_of_one() {
    let output = identity("e = exp(1)");

    assert!(output.status.success());
    assert!(standard_output(&output).starts_with("holds"));
}

#[test]
fn a_log_of_a_product_of_rationals_is_the_sum_of_their_logs() {
    let output = identity("ln(2) + ln(3) = ln(6)");

    assert!(output.status.success());
    assert!(standard_output(&output).starts_with("holds"));
}

#[test]
fn a_log_of_a_power_is_a_multiple_of_the_log() {
    let output = identity("ln(8) = 3*ln(2)");

    assert!(standard_output(&output).starts_with("holds"));
}

#[test]
fn a_log_of_a_quotient_is_a_difference_of_logs() {
    let output = identity("ln(1/2) = -ln(2)");

    assert!(standard_output(&output).starts_with("holds"));
}

#[test]
fn a_log_of_a_product_of_names_is_not_split() {
    let output = identity("ln(x*y) = ln(x) + ln(y)");

    assert!(standard_output(&output).starts_with("undecided"));
}

#[test]
fn a_tangent_is_a_quotient_that_holds_wherever_its_cosine_is_not_zero() {
    let output = identity("tan(x) = sin(x)/cos(x)");

    assert!(output.status.success());
    assert_eq!(
        standard_output(&output),
        "holds wherever cos(x) is not zero\n"
    );
}

#[test]
fn a_false_tangent_identity_is_refuted() {
    let output = identity("tan(x) = sin(x)");

    assert!(standard_output(&output).starts_with("not an identity"));
}

#[test]
fn a_logarithm_of_a_product_of_large_primes_splits_into_its_primes() {
    for claim in [
        "ln(1000003*1000033) = ln(1000003) + ln(1000033)",
        "ln(18446743979220271189) - ln(4294967291) = ln(4294967279)",
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_calc"))
            .args(["--identity", claim, "--terse", "--locale", "en"])
            .env_clear()
            .output()
            .expect("calc runs");
        let shown = String::from_utf8(output.stdout).expect("UTF-8");
        assert!(shown.starts_with("holds"), "{claim}: {shown}");
    }
}
