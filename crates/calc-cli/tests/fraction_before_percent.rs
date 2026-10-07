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
    let output = calc(&[line, "--terse"]);
    [output.stdout, output.stderr]
        .map(|bytes| String::from_utf8(bytes).expect("output is UTF-8"))
        .concat()
}

#[test]
fn a_fraction_before_a_percent_sign_names_both_readings() {
    assert_eq!(
        terse("1/2 %"),
        "error: 1/2 % can mean (1/2) % or 1/(2 %); write the one you mean, at column 1\n"
    );
}

#[test]
fn a_fraction_after_a_factor_names_both_readings() {
    assert_eq!(
        terse("100 * 1/2 %"),
        "error: 1/2 % can mean 100 * (1/2) % or 100 * 1/(2 %); write the one you mean, at column 7\n"
    );
}

#[test]
fn a_fraction_after_a_factor_before_a_unit_names_both_readings() {
    assert_eq!(
        terse("100 * 1/2 kg"),
        "error: 1/2 kg can mean 100 * (1/2) kg or 100 * 1/(2 kg); write the one you mean, at column 7\n"
    );
}

#[test]
fn a_bracketed_fraction_before_a_percent_sign_is_half_a_percent() {
    assert_eq!(terse("(1/2) %"), "1/200 exact\n");
}

#[test]
fn a_quantity_divided_by_a_quantity_is_still_a_rate() {
    assert_eq!(terse("5.5 km / 2 h"), "2.75 km/h exact\n");
}

#[test]
fn a_sum_before_a_unit_names_both_readings() {
    assert_eq!(
        terse("(1+1)/2 kg"),
        "error: (1+1)/2 kg can mean ((1+1)/2) kg or (1+1)/(2 kg); write the one you mean, at column 1\n"
    );
}

#[test]
fn a_name_before_a_percent_sign_names_both_readings() {
    assert_eq!(
        terse("x/2 %"),
        "error: x/2 % can mean (x/2) % or x/(2 %); write the one you mean, at column 1\n"
    );
}

#[test]
fn a_constant_before_a_percent_sign_names_both_readings() {
    assert_eq!(
        terse("pi/2 %"),
        "error: pi/2 % can mean (pi/2) % or pi/(2 %); write the one you mean, at column 1\n"
    );
}

#[test]
fn a_sum_of_quantities_divided_by_a_quantity_is_a_rate() {
    assert_eq!(terse("(2 m + 3 m)/2 s"), "(5/2) m/s exact\n");
}

#[test]
fn a_quantity_factor_before_a_fraction_keeps_the_fraction_ambiguous() {
    assert_eq!(
        terse("3 kg * 2/4 m"),
        "error: 2/4 m can mean 3 kg * (2/4) m or 3 kg * 2/(4 m); write the one you mean, at column 8\n"
    );
}
