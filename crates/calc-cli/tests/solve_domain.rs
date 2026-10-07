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
fn an_equation_with_only_complex_solutions_says_no_real_number_solves_it() {
    let output = solve(&["x^2 = -1"]);

    assert!(output.status.success());
    assert_eq!(
        shown(&output),
        "no real number solves this equation; its solutions are not real numbers, and --solve-for does not list them yet\n"
    );
}

#[test]
fn a_cube_root_says_the_other_solutions_are_not_real_and_explains_rootof() {
    assert_eq!(
        shown(&solve(&["x^3 = 2"])),
        "rootof(x^3 - 2, x, 1); its other solutions are not real numbers, and --solve-for does not list them yet\nrootof(p, x, k) is the k-th real root of p, counted from the smallest; --enclose gives its digits\n"
    );
}

#[test]
fn a_repeated_real_root_leaves_nothing_out() {
    assert_eq!(shown(&solve(&["(x-1)^3 = 0"])), "1\n");
}

#[test]
fn every_number_solving_an_equation_is_an_answer() {
    let output = solve(&["0*x = 0"]);

    assert!(output.status.success());
    assert_eq!(shown(&output), "every number solves this equation\n");
}

#[test]
fn a_program_is_told_that_non_real_solutions_were_left_out() {
    assert_eq!(
        shown(&solve(&["x^4 = 16", "--json"])),
        "{\"equation\": \"x^4 = 16\", \"solutions\": [\"-2\", \"2\"], \"non_real_left_out\": true}\n"
    );
}
