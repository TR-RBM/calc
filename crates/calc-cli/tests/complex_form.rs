use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn terse(line: &str) -> String {
    String::from_utf8(calc(&[line, "--terse"]).stdout).expect("output is UTF-8")
}

#[test]
fn a_fractional_imaginary_part_is_written_with_its_product_sign() {
    assert_eq!(terse("(1+2*i)/(3-i)"), "1/10 + 7/10 * i exact\n");
}

#[test]
fn a_zero_real_part_is_left_out() {
    assert_eq!(terse("(1+i)^2"), "2 * i exact\n");
}

#[test]
fn an_imaginary_part_of_minus_one_is_written_minus_i() {
    assert_eq!(terse("1/i"), "-i exact\n");
}

#[test]
fn a_negative_imaginary_part_is_subtracted() {
    assert_eq!(terse("11/5 - 2/5*i"), "11/5 - 2/5 * i exact\n");
}

#[test]
fn the_shown_form_reads_back_to_the_same_value() {
    assert_eq!(terse("1/10 + 7/10 * i"), "1/10 + 7/10 * i exact\n");
}

fn refusal(line: &str) -> String {
    let output = calc(&[line, "--terse"]);
    assert!(!output.status.success(), "{line} is refused");
    let mut text = String::from_utf8(output.stdout).expect("output is UTF-8");
    text.push_str(&String::from_utf8(output.stderr).expect("output is UTF-8"));
    text
}

#[test]
fn zero_to_an_exponent_with_positive_real_part_is_zero() {
    assert_eq!(terse("0^(1+i)"), "0 exact\n");
}

#[test]
fn zero_to_an_imaginary_exponent_is_refused_as_having_no_value() {
    let message = refusal("0^i");
    assert!(message.contains("has no value"), "{message}");
    assert!(!message.contains("yet"), "{message}");
}

#[test]
fn zero_to_an_exponent_with_negative_real_part_divides_by_zero() {
    assert!(refusal("0^(-1+i)").contains("divides by zero"));
}
