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

fn solve(equation: &str) -> Output {
    calc(&["--solve-for", equation, "--locale", "en"])
}

fn solves(equation: &str, expected: &str) {
    let output = solve(equation);

    assert!(output.status.success(), "{equation} failed");
    assert_eq!(standard_output(&output), format!("{expected}\n"));
}

fn refuses(equation: &str, wording: &str) {
    let output = solve(equation);

    assert!(!output.status.success(), "{equation} should be refused");
    assert!(
        standard_output(&output).contains(wording),
        "{equation} did not say {wording}"
    );
}

#[test]
fn a_linear_equation_gives_one_solution() {
    solves("2*x + 6 = 0", "-3");
}

#[test]
fn a_fractional_solution_stays_a_fraction() {
    solves("3*x - 1 = 0", "1/3");
}

#[test]
fn a_square_gives_both_solutions_in_order() {
    solves("x^2 = 4", "-2, 2");
}

#[test]
fn a_factorable_quadratic_gives_both_roots() {
    solves("x^2 - 5*x + 6 = 0", "2, 3");
}

#[test]
fn a_double_root_is_named_once() {
    solves("x^2 - 2*x + 1 = 0", "1");
}

#[test]
fn an_irrational_root_is_written_with_a_square_root() {
    solves("x^2 = 2", "-sqrt(2), sqrt(2)");
}

#[test]
fn the_square_part_is_taken_out_of_the_radical() {
    solves("x^2 = 8", "-2 * sqrt(2), 2 * sqrt(2)");
}

#[test]
fn a_shifted_quadratic_keeps_both_parts_exact() {
    solves("x^2 - 2*x - 1 = 0", "1 - sqrt(2), 1 + sqrt(2)");
}

#[test]
fn a_fractional_coefficient_never_becomes_a_decimal() {
    solves("x^2 + x - 1 = 0", "(-1 - sqrt(5)) / 2, (-1 + sqrt(5)) / 2");
}

#[test]
fn a_solution_is_written_as_the_line_that_evaluates_it_is() {
    solves("x^2 + x - 1 = 0", "(-1 - sqrt(5)) / 2, (-1 + sqrt(5)) / 2");
    let answered = calc(&["(-1 - sqrt(5)) / 2", "--terse", "--locale", "en"]);
    assert_eq!(standard_output(&answered), "(-1 - sqrt(5)) / 2 exact\n");
}

#[test]
fn a_cubic_with_rational_roots_is_solved_completely() {
    solves("x^3 - 6*x^2 + 11*x - 6 = 0", "1, 2, 3");
}

#[test]
fn a_quadratic_with_no_real_root_says_no_real_number_solves_it() {
    solves(
        "x^2 + 1 = 0",
        "no real number solves this equation; its solutions are not real numbers, and --solve-for does not list them yet",
    );
}

#[test]
fn an_equation_that_every_number_solves_says_so() {
    solves("x + 1 = 1 + x", "every number solves this equation");
}

#[test]
fn a_cubic_whose_root_is_not_rational_is_answered_with_rootof() {
    solves(
        "x^3 = 2",
        "rootof(x^3 - 2, x, 1); its other solutions are not real numbers, and --solve-for does not list them yet\nrootof(p, x, k) is the k-th real root of p, counted from the smallest; --enclose gives its digits",
    );
}

#[test]
fn a_square_root_whose_primes_pass_trial_division_is_split_by_rho() {
    solves(
        "9223372036854775807*x^2 - 1 = 0",
        "-sqrt(188232082384791343) / 1317624576693539401, sqrt(188232082384791343) / 1317624576693539401",
    );
}

#[test]
fn a_radical_equation_with_an_unproven_square_part_is_refused_not_answered_empty() {
    refuses(
        "x^2 + sqrt(2)*x - 309485009821345068724781055 = 0",
        "could not split 618970019642690137449562111",
    );
}

#[test]
fn a_quadratic_rest_with_an_unproven_square_part_keeps_its_roots_by_refusing() {
    refuses(
        "(x - 1)*(x^2 - 10000000000004*x - 1) = 0",
        "could not split 5000000000004000000000001",
    );
}

#[test]
fn a_quadratic_factor_with_an_unproven_square_part_keeps_its_roots_by_refusing() {
    refuses(
        "(x^2 - 10000000000004*x - 1)*(x^3 - 2) = 0",
        "could not split 5000000000004000000000001",
    );
}

#[test]
fn a_square_part_that_rho_does_not_split_is_refused_with_the_number() {
    refuses(
        "x^2 + sqrt(2)*x - 150000000000620000000000240 = 0",
        "could not split 300000000001240000000000481",
    );
}

#[test]
fn a_radical_equation_whose_discriminant_needs_rho_answers_both_roots() {
    solves(
        "x^2 + sqrt(2)*x - 300000000001220000000000407 = 0",
        "(-sqrt(2) - sqrt(1200000000004880000000001630)) / 2, (-sqrt(2) + sqrt(1200000000004880000000001630)) / 2",
    );
}

#[test]
fn a_rational_root_beside_an_unsplit_quadratic_is_not_answered_alone() {
    refuses(
        "(x - 1)*(x^2 - 300000000001220000000000407) = 0",
        "could not split",
    );
}

#[test]
fn a_rootof_beside_an_unsplit_quadratic_is_not_answered_alone() {
    refuses(
        "(x^2 - 300000000001220000000000407)*(x^3 - 2) = 0",
        "could not split",
    );
}

#[test]
fn a_linear_equation_whose_root_passes_one_hundred_thousand_is_answered() {
    solves("x - 499290947 = 0", "499290947");
}

#[test]
fn a_cubic_whose_constant_passes_one_hundred_thousand_is_answered() {
    solves(
        "x^3 - 100001 = 0",
        "rootof(x^3 - 100001, x, 1); its other solutions are not real numbers, and --solve-for does not list them yet\nrootof(p, x, k) is the k-th real root of p, counted from the smallest; --enclose gives its digits",
    );
}

#[test]
fn three_irrational_roots_are_answered_in_ascending_order() {
    solves(
        "x^3 - 3*x + 1 = 0",
        "rootof(x^3 - 3 * x + 1, x, 1), rootof(x^3 - 3 * x + 1, x, 2), rootof(x^3 - 3 * x + 1, x, 3)\nrootof(p, x, k) is the k-th real root of p, counted from the smallest; --enclose gives its digits",
    );
}

#[test]
fn an_equation_in_two_names_is_refused_rather_than_choosing_one() {
    refuses("x + y = 3", "more than one name");
}

#[test]
fn an_equation_that_is_not_a_polynomial_is_refused() {
    refuses("sin(x) = 0", "polynomial in one name");
}

#[test]
fn every_solution_it_gives_survives_its_own_check() {
    let output = calc(&[
        "(-1/2 + sqrt(5)/2)^2 + (-1/2 + sqrt(5)/2) - 1 = 0",
        "--check",
        "--locale",
        "en",
    ]);

    assert!(output.status.success());
    assert!(standard_output(&output).starts_with("holds"));
}

#[test]
fn a_radical_coefficient_is_solved_as_the_evaluator_writes_it() {
    solves("sqrt(2)*x = 1", "sqrt(2) / 2");
}

#[test]
fn a_quadratic_with_radical_coefficients_gives_both_roots() {
    solves("x^2 - 2*sqrt(2)*x + 1 = 0", "-1 + sqrt(2), 1 + sqrt(2)");
}

#[test]
fn a_cubic_with_a_radical_coefficient_says_its_degree() {
    refuses("sqrt(2)*x^3 = 1", "this one is of degree 3");
}

#[test]
fn a_list_of_linear_equations_is_solved_as_a_system() {
    solves("[x + y = 3, x - y = 1]", "x = 2, y = 1");
}

#[test]
fn a_system_in_three_names_is_solved_exactly() {
    solves(
        "[a + 2*b + 3*c = 14, 2*a - b + c = 3, 3*a + b - c = 2]",
        "a = 1, b = 2, c = 3",
    );
}

#[test]
fn a_system_with_no_solution_says_so() {
    solves(
        "[x + y = 1, x + y = 2]",
        "no numbers solve all of these equations at once",
    );
}

#[test]
fn a_dependent_system_names_the_name_left_free() {
    solves(
        "[x + y = 3, 2*x + 2*y = 6]",
        "x = -y + 3, for every value of y",
    );
}

#[test]
fn a_system_that_is_not_linear_is_refused() {
    refuses(
        "[x*y = 1, x + y = 2]",
        "calc solves a list of equations only where each is linear in its names",
    );
}

#[test]
fn a_quartic_that_splits_into_quadratics_is_solved_with_square_roots() {
    solves(
        "x^4 - 5*x^2 + 6 = 0",
        "-sqrt(3), -sqrt(2), sqrt(2), sqrt(3)",
    );
}

#[test]
fn square_roots_and_rootof_are_ordered_together() {
    solves(
        "(x^2 - 2)*(x^3 - 3) = 0",
        "-sqrt(2), sqrt(2), rootof(x^3 - 3, x, 1); its other solutions are not real numbers, and --solve-for does not list them yet\nrootof(p, x, k) is the k-th real root of p, counted from the smallest; --enclose gives its digits",
    );
}

#[test]
fn a_quadratic_inequality_is_answered_as_two_rays() {
    solves("x^2 - 2 > 0", "x < -sqrt(2) or x > sqrt(2)");
}

#[test]
fn a_closed_inequality_includes_its_ends() {
    solves("x^2 - 5*x + 6 <= 0", "2 <= x <= 3");
}

#[test]
fn a_double_root_splits_an_open_inequality_without_changing_the_sign() {
    solves("(x - 1)^2 > 0", "x < 1 or x > 1");
}

#[test]
fn an_inequality_every_number_solves_says_so() {
    solves("x^2 + 1 > 0", "every number solves this inequality");
}

#[test]
fn an_inequality_no_number_solves_says_so() {
    solves("x^2 + 1 < 0", "no number solves this inequality");
}

#[test]
fn an_inequality_whose_boundary_is_irrational_names_it_with_rootof() {
    solves("x^3 - 2 >= 0", "x >= rootof(x^3 - 2, x, 1)");
}

#[test]
fn a_rational_root_of_an_equation_in_whole_numbers_stays_a_fraction() {
    solves("4*x^2 - 9 = 0", "-3/2, 3/2");
}

#[test]
fn a_rational_root_of_an_equation_in_decimals_is_a_decimal() {
    solves("0.5*x = 1.25", "2.5");
}

#[test]
fn a_root_whose_decimal_does_not_end_stays_a_fraction() {
    solves("0.3*x = 1", "10/3");
}

#[test]
fn an_inequality_in_whole_numbers_keeps_its_bound_a_fraction() {
    solves("2*x < 7", "x < 7/2");
}

#[test]
fn a_system_in_whole_numbers_keeps_its_values_fractions() {
    solves("[x + y = 1, x - y = 2]", "x = 3/2, y = -1/2");
}
