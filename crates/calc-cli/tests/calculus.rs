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

fn terse(expression: &str) -> Output {
    calc(&[expression, "--terse", "--locale", "en"])
}

fn answers(expression: &str, expected: &str) {
    let output = terse(expression);

    assert!(output.status.success(), "{expression} failed");
    assert_eq!(standard_output(&output), format!("{expected}\n"));
}

fn refuses(expression: &str, wording: &str) {
    let output = terse(expression);

    assert!(!output.status.success(), "{expression} should be refused");
    assert!(
        standard_output(&output).contains(wording),
        "{expression} did not say {wording}"
    );
}

#[test]
fn the_integral_of_a_square_over_the_unit_interval_is_a_third() {
    answers("integral(x^2, x, 0, 1)", "1/3 exact");
}

#[test]
fn a_polynomial_integrates_term_by_term() {
    answers("integral(x^3 + 2*x, x, 0, 2)", "8 exact");
}

#[test]
fn an_integral_over_fractional_bounds_stays_exact() {
    answers("integral(x^2, x, 0, 1/2)", "1/24 exact");
}

#[test]
fn bounds_the_wrong_way_round_give_the_negative() {
    answers("integral(x^2, x, 1, 0)", "-1/3 exact");
}

#[test]
fn an_integrand_that_is_not_a_rational_function_says_so() {
    refuses(
        "integral(sin(x), x, 0, 1)",
        "a polynomial or a quotient of two polynomials with rational coefficients in the variable",
    );
}

#[test]
fn an_integral_without_bounds_answers_an_antiderivative() {
    answers("integral(x^2, x)", "x^3 / 3 exact");
}

#[test]
fn a_polynomial_limit_is_its_value_at_the_point() {
    answers("limit(x^2 + 1, x, 3)", "10 exact");
}

#[test]
fn a_zero_over_zero_quotient_is_cancelled() {
    answers("limit((x^2 - 1)/(x - 1), x, 1)", "2 exact");
}

#[test]
fn a_double_root_is_cancelled_twice() {
    answers("limit((x^3 - 3*x + 2)/(x^2 - 2*x + 1), x, 1)", "3 exact");
}

#[test]
fn a_limit_at_a_fraction_is_exact() {
    answers("limit(x^2, x, 1/3)", "1/9 exact");
}

#[test]
fn a_one_sided_limit_agrees_where_the_two_sided_one_exists() {
    answers("limit((x^2-1)/(x-1), x, 1, side=left)", "2 exact");
    answers("limit((x^2-1)/(x-1), x, 1, side=right)", "2 exact");
}

#[test]
fn a_pole_is_named_as_having_no_single_number() {
    refuses("limit(1/(x-1), x, 1)", "do not approach one number");
}

#[test]
fn a_limit_calc_cannot_justify_says_it_will_not_assume_continuity() {
    refuses("limit(abs(x)/x, x, 0)", "will not assume it");
}

#[test]
fn the_language_reference_no_longer_calls_anything_uncomputed() {
    let output = calc(&["help", "language", "--locale", "en"]);

    assert!(!standard_output(&output).contains("does not compute it yet"));
}

#[test]
fn a_pole_asked_about_from_one_side_is_refused_for_that_side() {
    refuses(
        "limit(1/x, x, 0, side=right)",
        "from the right the value passes every bound in size",
    );
}

#[test]
fn a_pole_asked_about_from_both_sides_keeps_its_two_sided_reason() {
    refuses(
        "limit(1/x, x, 0)",
        "the two sides do not approach one number",
    );
}

#[test]
fn a_one_sided_refusal_names_its_side_for_a_program() {
    let output = calc(&["limit(1/x, x, 0, side=right)", "--json"]);

    let text = standard_output(&output);
    assert!(text.contains("\"code\": \"limit_not_finite\""), "{text}");
    assert!(text.contains("\"side\": \"right\""), "{text}");
}

#[test]
fn a_rational_function_integrates_by_partial_fractions() {
    let output = calc(&["integral(1/(x^2-1), x)", "--terse", "--locale", "en"]);
    assert_eq!(
        standard_output(&output),
        "ln(abs(x - 1)) / 2 - ln(abs(x + 1)) / 2 exact\n"
    );
}

#[test]
fn an_indefinite_integral_says_it_is_one_antiderivative() {
    let output = calc(&["integral(x^2, x)", "--locale", "en"]);
    let shown = standard_output(&output);
    assert!(shown.contains("x^3 / 3"), "{shown}");
    assert!(
        shown.contains("every other differs from it by a constant"),
        "{shown}"
    );
}

#[test]
fn an_irreducible_quadratic_integrates_to_an_arctangent() {
    let output = calc(&["integral(1/(x^2+1), x)", "--terse", "--locale", "en"]);
    assert_eq!(standard_output(&output), "atan(x) exact\n");
}

#[test]
fn a_definite_rational_integral_is_exact() {
    let output = calc(&["integral(1/(x^2+1), x, 0, 1)", "--terse", "--locale", "en"]);
    assert_eq!(standard_output(&output), "pi / 4 exact\n");
}

#[test]
fn a_pole_between_the_bounds_leaves_the_integral_refused() {
    let output = calc(&["integral(1/x^2, x, -1, 1)", "--locale", "en"]);
    assert!(standard_output(&output).contains("has a pole between the bounds"));
}

#[test]
fn the_taylor_polynomial_of_sine_about_zero_is_exact() {
    let output = calc(&["taylor(sin(x), x, 0, 5)", "--terse", "--locale", "en"]);
    assert_eq!(standard_output(&output), "x^5 / 120 - x^3 / 6 + x exact\n");
}

#[test]
fn the_taylor_polynomial_of_exp_about_zero_is_exact() {
    let output = calc(&["taylor(exp(x), x, 0, 3)", "--terse", "--locale", "en"]);
    assert_eq!(
        standard_output(&output),
        "x^3 / 6 + x^2 / 2 + x + 1 exact\n"
    );
}

#[test]
fn a_taylor_polynomial_about_another_point_is_in_powers_of_the_shift() {
    let output = calc(&["taylor(ln(x), x, 1, 3)", "--terse", "--locale", "en"]);
    assert_eq!(
        standard_output(&output),
        "x - 1 - (x - 1)^2 / 2 + (x - 1)^3 / 3 exact\n"
    );
}

#[test]
fn a_taylor_polynomial_says_it_is_not_the_function() {
    let output = calc(&["taylor(cos(x), x, 0, 4)", "--locale", "en"]);
    let shown = standard_output(&output);
    assert!(shown.contains("is not the function itself"), "{shown}");
}

#[test]
fn a_taylor_polynomial_where_the_body_is_undefined_is_refused() {
    let output = calc(&["taylor(ln(x), x, 0, 2)", "--locale", "en"]);
    assert!(standard_output(&output).contains("not defined at the point"));
}

#[test]
fn a_taylor_order_above_the_bound_is_refused() {
    let output = calc(&["taylor(sin(x), x, 0, 100)", "--locale", "en"]);
    assert!(standard_output(&output).contains("at most 64"));
}

#[test]
fn limits_of_analytic_quotients_go_through_their_leading_orders() {
    for (line, expected) in [
        ("limit(sin(x)/x, x, 0)", "1 exact\n"),
        ("limit((1 - cos(x))/x^2, x, 0)", "1/2 exact\n"),
        ("limit((exp(x) - 1)/x, x, 0)", "1 exact\n"),
        ("limit((x - sin(x))/x^3, x, 0)", "1/6 exact\n"),
    ] {
        let output = calc(&[line, "--terse", "--locale", "en"]);
        assert_eq!(standard_output(&output), expected, "{line}");
    }
}

#[test]
fn an_analytic_quotient_with_a_lone_vanishing_denominator_has_no_finite_limit() {
    let output = calc(&["limit(sin(x)/x^2, x, 0)", "--locale", "en"]);
    assert!(standard_output(&output).contains("do not approach one number"));
}

#[test]
fn a_limit_of_a_square_root_is_still_refused() {
    let output = calc(&["limit(sqrt(x), x, 0)", "--locale", "en"]);
    assert!(standard_output(&output).contains("will not assume it"));
}
