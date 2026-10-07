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
fn a_divisor_that_is_zero_for_every_value_divides_by_zero() {
    let output = terse("x/(x - x)");

    assert!(!output.status.success());
    assert_eq!(shown(&output), "error x / (x - x) divides by zero\n");
}

#[test]
fn a_zero_divisor_inside_a_sum_is_named() {
    assert_eq!(
        shown(&terse("1/(sin(x) - sin(x)) + 2")),
        "error 1 / (sin(x) - sin(x)) divides by zero\n"
    );
}

#[test]
fn a_divisor_with_zeros_at_some_values_is_still_answered() {
    assert_eq!(shown(&terse("x/y")), "x / y exact\n");
}
