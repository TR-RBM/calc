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
fn an_arcminute_is_pi_over_10800_radians() {
    assert_eq!(terse("1 arcmin -> rad"), "(pi / 10800) rad exact\n");
}

#[test]
fn sixty_arcseconds_are_one_arcminute() {
    assert_eq!(terse("60 arcsec -> arcmin"), "1 arcmin exact\n");
}

#[test]
fn a_turn_is_6400_nato_mils() {
    assert_eq!(terse("6400 mil_NATO -> deg"), "360 ° exact\n");
}

#[test]
fn a_thou_is_a_thousandth_of_an_inch() {
    assert_eq!(terse("1 thou -> mm"), "0.0254 mm exact\n");
}

#[test]
fn moa_names_its_two_readings() {
    assert_eq!(
        terse("1 MOA"),
        "error: MOA at column 3 is written for more than one unit; write one of arcmin, IPHY, which each mean one\n"
    );
}

#[test]
fn mil_names_its_three_readings() {
    assert_eq!(
        terse("1 mil"),
        "error: mil at column 3 is written for more than one unit; write one of mrad, mil_NATO, thou, which each mean one\n"
    );
}
