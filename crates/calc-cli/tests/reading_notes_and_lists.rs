use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

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
fn the_rounded_block_of_a_reading_says_it_is_a_reading() {
    let text = shown(&calc(&[
        "to_celsius(from_fahrenheit(72))",
        "--locale",
        "en",
    ]));
    let rounded_block = text.split("\n\n").nth(1).unwrap_or_default();

    assert!(
        rounded_block.contains("this is a reading on the Celsius scale"),
        "{text}"
    );
}

#[test]
fn a_fraction_in_a_list_with_a_root_is_written_as_a_fraction() {
    assert_eq!(
        shown(&calc(&["[1/3, sqrt(2)]", "--terse"])),
        "[1/3, sqrt(2)] exact\n"
    );
}

#[test]
fn a_decimal_in_a_list_with_a_root_stays_a_decimal() {
    assert_eq!(
        shown(&calc(&["[0.5, sqrt(2)]", "--terse"])),
        "[0.5, sqrt(2)] exact\n"
    );
}

#[test]
fn a_matrix_with_a_root_writes_its_fractions_compactly() {
    assert_eq!(
        shown(&calc(&["[1, sqrt(2); 1/3, 2]", "--terse"])),
        "[1, sqrt(2); 1/3, 2] exact\n"
    );
}

#[test]
fn a_sum_before_a_unit_is_bracketed() {
    assert_eq!(
        shown(&calc(&["(3 + sqrt(2)) m", "--terse"])),
        "(3 + sqrt(2)) m exact\n"
    );
}

#[test]
fn a_fraction_before_a_unit_is_bracketed_so_it_reads_back_the_same() {
    assert_eq!(shown(&calc(&["(1/3) m", "--terse"])), "(1/3) m exact\n");
}

#[test]
fn a_plain_number_before_a_unit_is_not_bracketed() {
    assert_eq!(shown(&calc(&["5 m", "--terse"])), "5 m exact\n");
}
