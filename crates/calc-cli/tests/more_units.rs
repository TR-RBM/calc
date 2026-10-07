use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn terse(expression: &str) -> String {
    let output: Output = Command::new(CALC)
        .args([expression, "--terse"])
        .env_clear()
        .output()
        .expect("calc runs");
    String::from_utf8(output.stdout).expect("output is UTF-8")
}

#[test]
fn kilowatt_hours_are_a_unit() {
    assert_eq!(terse("1 kW * 2 h -> kWh"), "2 kWh exact\n");
}

#[test]
fn a_megawatt_hour_is_three_point_six_gigajoules() {
    assert_eq!(terse("1 MWh -> GJ"), "3.6 GJ exact\n");
}

#[test]
fn a_square_kilometre_is_a_hundred_hectares() {
    assert_eq!(terse("1 km^2 -> ha"), "100 ha exact\n");
}

#[test]
fn a_parsec_holds_pi_in_astronomical_units() {
    assert_eq!(terse("1 pc -> au"), "(648000 / pi) au exact\n");
}

#[test]
fn a_light_year_is_exact_in_metres() {
    assert_eq!(terse("1 ly -> m"), "9460730472580800 m exact\n");
}

#[test]
fn ten_knots_are_exact_in_kilometres_per_hour() {
    assert_eq!(terse("10 kn -> km/h"), "18.52 km/h exact\n");
}

#[test]
fn a_millimetre_of_mercury_is_not_a_torr() {
    assert_ne!(terse("760 mmHg -> atm"), "1 atm exact\n");
}

#[test]
fn a_kilonewton_is_not_a_knot() {
    assert_eq!(terse("1 kN -> N"), "1000 N exact\n");
}
