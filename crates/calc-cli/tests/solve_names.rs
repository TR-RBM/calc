use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn solve(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .arg("--solve-for")
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn shown(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

#[test]
fn a_named_side_is_solved_rather_than_defined() {
    let output = solve(&["x = 2"]);

    assert!(output.status.success());
    assert_eq!(shown(&output), "2\n");
}

#[test]
fn a_named_side_with_a_constant_answers_the_constant() {
    assert_eq!(shown(&solve(&["x = pi"])), "pi\n");
}

#[test]
fn a_constant_coefficient_above_the_first_degree_is_refused_for_what_it_is() {
    let output = solve(&["x^2 = pi"]);

    assert!(!output.status.success());
    assert_eq!(
        shown(&output),
        "with pi or e in its coefficients, calc solves an equation of degree 1, and this one is of degree 2; it does not solve such equations yet\n"
    );
}

#[test]
fn a_coefficient_of_pi_and_e_that_is_not_proven_nonzero_is_refused_for_that() {
    let output = solve(&["(pi - e - 0.4233108251307480031)*x = 1"]);

    assert!(!output.status.success());
    assert!(shown(&output).starts_with("a coefficient of this equation holds pi and e together"));
}

#[test]
fn a_coefficient_with_another_constant_is_refused_naming_it() {
    let output = solve(&["sin(1)*x = pi"]);

    assert!(!output.status.success());
    assert_eq!(
        shown(&output),
        "calc solves an equation with constants in its coefficients only where they are built from rational numbers, square roots of rational numbers, pi and e by sums, products and quotients, and sin(1) is not; it does not solve with such a coefficient yet\n"
    );
}

#[test]
fn a_constant_written_as_a_divisor_is_solved() {
    assert_eq!(shown(&solve(&["x/pi = 2"])), "2 * pi\n");
}

#[test]
fn a_function_definition_stays_a_definition() {
    assert!(shown(&solve(&["f(x) = x^2"])).starts_with("defines"));
}

#[test]
fn every_number_is_an_answer_for_a_program() {
    assert_eq!(
        shown(&solve(&["0*x = 0", "--json"])),
        "{\"equation\": \"0*x = 0\", \"every_number\": true}\n"
    );
}

#[test]
fn a_linear_equation_with_pi_or_e_as_a_coefficient_is_solved() {
    assert_eq!(shown(&solve(&["pi*x = 1"])), "1 / pi\n");
    assert_eq!(shown(&solve(&["x - pi = 0"])), "pi\n");
    assert_eq!(shown(&solve(&["e*x = 1"])), "1 / e\n");
}

#[test]
fn a_solution_over_pi_is_reduced() {
    assert_eq!(shown(&solve(&["(pi - 1)*x = pi^2 - 1"])), "pi + 1\n");
    assert_eq!(shown(&solve(&["2*pi*x = pi"])), "1/2\n");
}

#[test]
fn a_coefficient_in_pi_and_e_is_proven_nonzero_by_its_enclosure() {
    assert_eq!(shown(&solve(&["(pi - e)*x = 1"])), "1 / (pi - e)\n");
}

#[test]
fn an_equation_with_a_constant_and_no_unknown_left_says_so() {
    assert_eq!(
        shown(&solve(&["0*x = pi"])),
        "no number solves this equation\n"
    );
}
