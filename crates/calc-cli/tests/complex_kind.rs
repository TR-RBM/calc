use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn shown(line: &str) -> String {
    String::from_utf8(calc(&[line, "--locale", "en"]).stdout).expect("output is UTF-8")
}

#[test]
fn an_exact_complex_value_is_described_as_complex() {
    let text = shown("(1+2*i)/(3-i)");

    assert!(
        text.contains("  number    exact complex number, rational parts\n"),
        "{text}"
    );
}

#[test]
fn a_machine_complex_value_is_described_as_complex() {
    let text = shown("to_f64(i)");

    assert!(
        text.contains("complex number of two machine floats\n"),
        "{text}"
    );
}

#[test]
fn a_rational_value_is_still_a_fraction() {
    let text = shown("1/2");

    assert!(text.contains("  number    exact fraction\n"), "{text}");
}
