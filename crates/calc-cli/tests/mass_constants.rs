use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn terse_in(expression: &str, locale: &str) -> String {
    let output = calc(&[expression, "--terse", "--locale", locale]);
    let mut shown = String::from_utf8(output.stdout).expect("output is UTF-8");
    shown.push_str(&String::from_utf8(output.stderr).expect("output is UTF-8"));
    shown
}

fn terse(expression: &str) -> String {
    terse_in(expression, "en")
}

#[test]
fn the_electron_rest_energy_is_codata_2022_with_its_uncertainty() {
    assert_eq!(
        terse("m_e * c_0^2 -> keV").trim_end(),
        "510.99895069 keV ± 0.00000016 keV derived from 1 measured input"
    );
}

#[test]
fn the_atomic_mass_constant_is_codata_2022_with_its_uncertainty() {
    assert_eq!(
        terse("m_u -> kg").trim_end(),
        "1.66053906892e-27 kg ± 0.00000000052e-27 kg derived from 1 measured input"
    );
}

#[test]
fn the_atomic_mass_unit_is_refused_with_the_constant_to_write() {
    for line in ["1 u", "1 Da -> kg", "2e-26 kg -> u"] {
        let shown = terse(line);
        assert!(shown.contains("is the atomic mass unit"), "{line}: {shown}");
        assert!(shown.contains("m_u"), "{line}: {shown}");
    }
    assert!(terse_in("1 Da", "de").contains("schreib sie über die Konstante m_u"));
}

#[test]
fn a_name_a_person_defines_stays_theirs() {
    assert_eq!(terse("u = 3").trim_end(), "3 exact");
    assert_eq!(terse("m_e = 2").trim_end(), "2 exact");
}

#[test]
fn a_reciprocal_target_answers_what_the_negative_exponent_answers() {
    assert_eq!(terse("5 Hz -> 1/s"), terse("5 Hz -> s^-1"));
    let per_second = terse("3.828e26 W / (26730.9688 keV) -> 1/s");
    let hertz = terse("3.828e26 W / (26730.9688 keV) -> Hz");
    assert_eq!(per_second.replace("s⁻¹", "Hz"), hertz);
    assert!(terse("2 m/s -> 1/s").contains("cannot be added, compared or converted"));
}
