use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn terse(expression: &str) -> Output {
    Command::new(CALC)
        .args([expression, "--terse", "--locale", "en"])
        .env_clear()
        .output()
        .expect("calc runs")
}

fn shown(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

#[test]
fn a_list_of_irrational_values_is_sorted() {
    assert_eq!(
        shown(&terse("sorted([sqrt(2), 1, pi])")),
        "[1, sqrt(2), pi] exact\n"
    );
}

#[test]
fn the_largest_of_a_root_and_a_whole_number_is_found() {
    assert_eq!(shown(&terse("largest([sqrt(2), 1])")), "sqrt(2) exact\n");
}

#[test]
fn the_median_of_irrational_values_is_found() {
    assert_eq!(shown(&terse("median([sqrt(2), 1, pi])")), "sqrt(2) exact\n");
}

#[test]
fn a_pair_that_cannot_be_ordered_is_named() {
    let output = terse("sorted([sin(1)^2 + cos(1)^2, 1])");

    assert!(!output.status.success());
    assert!(
        shown(&output).contains(
            "calc could not decide which of sin(1)^2 + cos(1)^2 and 1 is larger, so it cannot order the list"
        ),
        "{}",
        shown(&output)
    );
}
