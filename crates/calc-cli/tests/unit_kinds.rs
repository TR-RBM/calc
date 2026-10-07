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
fn a_frequency_and_an_activity_are_not_added() {
    let output = terse("1 Hz + 1 Bq");

    assert!(!output.status.success());
    assert!(
        shown(&output).contains("one side is frequency and the other activity"),
        "{}",
        shown(&output)
    );
}

#[test]
fn an_equivalent_dose_is_not_converted_to_an_absorbed_dose() {
    assert!(
        shown(&terse("1 Sv -> Gy"))
            .contains("one side is equivalent dose and the other absorbed dose")
    );
}

#[test]
fn a_luminous_flux_is_not_a_luminous_intensity() {
    assert!(shown(&terse("1 lm -> cd")).contains("they share a unit but are different quantities"));
}

#[test]
fn prefixed_units_of_one_kind_still_add() {
    assert_eq!(shown(&terse("1 mSv + 2 Sv")), "2.001 Sv exact\n");
}

#[test]
fn energy_and_torque_are_not_told_apart() {
    assert_eq!(shown(&terse("1 J + 1 N*m")), "2 J exact\n");
}

#[test]
fn an_area_lists_its_coherent_unit() {
    assert!(shown(&terse("1 m^2 -> m")).contains("m^2 measures area (units: m^2, ha)"));
}

#[test]
fn the_katal_names_catalytic_activity() {
    assert!(shown(&terse("1 Sv + 1 kat")).contains("kat measures catalytic activity"));
}
