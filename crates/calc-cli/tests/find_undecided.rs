use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");
const EXIT_NOTHING_REACHED: i32 = 4;

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn shown(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

#[test]
fn a_counter_run_names_a_value_where_the_claim_has_no_truth_value() {
    let output = calc(&["--find", "n=0..5", "--counter", "1/n > 0"]);

    assert_eq!(
        shown(&output),
        "no value calc could decide in the range makes it fail\ncalc could not decide the claim at n = 0\n"
    );
}

#[test]
fn a_counter_run_with_an_undecided_value_does_not_exit_as_passed() {
    let output = calc(&["--find", "n=0..5", "--counter", "1/n > 0"]);

    assert_eq!(output.status.code(), Some(EXIT_NOTHING_REACHED));
}

#[test]
fn a_find_run_lists_its_matches_and_then_the_undecided_values() {
    let output = calc(&["--find", "n=0..2", "1/n > 0"]);

    assert!(output.status.success());
    assert_eq!(
        shown(&output),
        "n = 1\nn = 2\ncalc could not decide the claim at n = 0\n"
    );
}

#[test]
fn several_undecided_values_are_counted_with_the_first() {
    let output = calc(&["--find", "x=-3..3", "--counter", "sqrt(x)*sqrt(x) = x"]);

    assert!(
        shown(&output).contains("calc could not decide the claim at 3 values, the first x = -3"),
        "{}",
        shown(&output)
    );
}

#[test]
fn a_fully_decided_counter_run_is_unchanged() {
    let output = calc(&["--find", "n=1..5", "--counter", "1/n > 0"]);

    assert!(output.status.success());
    assert_eq!(shown(&output), "nothing in the range makes it fail\n");
}
