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
fn seven_hundred_sixty_torr_are_one_standard_atmosphere() {
    assert_eq!(terse("760 Torr -> atm"), "1 atm exact\n");
}

#[test]
fn a_torr_is_not_a_millimetre_of_mercury() {
    assert_eq!(
        terse("1 Torr -> mmHg"),
        "(24125000000/24125003437) mmHg exact\n"
    );
}

#[test]
fn the_lower_case_name_is_written_as_the_symbol() {
    assert_eq!(terse("1 torr"), "1 Torr exact\n");
}

#[test]
fn the_torr_takes_decimal_prefixes() {
    assert_eq!(terse("1 mTorr -> Pa"), "(4053/30400) Pa exact\n");
}
