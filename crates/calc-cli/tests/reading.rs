use std::path::PathBuf;
use std::process::{Command, Output};

use calc_app::{FixedClock, Session, UnitOverride, UtcTimestamp, registered_backends};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn standard_output(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

fn session_file(name: &str, inputs: &[&str]) -> PathBuf {
    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("calc-reading");
    std::fs::create_dir_all(&directory).expect("scratch directory is created");
    let clock = FixedClock::new(UtcTimestamp::from_milliseconds_since_unix_epoch(0), 1);
    let mut session = Session::new(Box::new(clock), registered_backends());
    session
        .set_unit_override(UnitOverride {
            system: Some("si".to_owned()),
            overrides: Vec::new(),
        })
        .expect("the si system is known");
    for input in inputs {
        session.enter(input).expect("line is entered");
    }
    let bytes = session.save_to_bytes().expect("session is saved");
    let path = directory.join(name);
    std::fs::write(&path, bytes).expect("session file is written");
    path
}

fn value_row(name: &str, input: &str) -> String {
    let path = session_file(name, &[input]);
    let output = calc(&[path.to_str().expect("path is UTF-8"), "--locale", "en"]);
    assert!(output.status.success());
    standard_output(&output)
        .lines()
        .find(|row| row.trim_start().starts_with("value"))
        .expect("a value row")
        .trim()
        .to_owned()
}

#[test]
fn a_length_of_five_hours_reads_in_kilometres() {
    assert_eq!(
        value_row("hours.calc", "100 km/h * 5 h"),
        "value     500 km"
    );
}

#[test]
fn a_length_of_five_seconds_keeps_its_fraction_and_gains_a_reading() {
    assert_eq!(
        value_row("seconds.calc", "100 km/h * 5 s"),
        "value           (1250/9) m"
    );
}

#[test]
fn a_length_of_ninety_seconds_reads_as_an_exact_decimal_without_a_reading() {
    assert_eq!(
        value_row("ninety.calc", "100 km/h * 90 s"),
        "value     2.5 km"
    );
}

#[test]
fn a_volume_written_in_litres_stays_in_litres() {
    assert_eq!(value_row("litres.calc", "5 l/s * 1 h"), "value     18000 L");
}

#[test]
fn the_reading_never_reaches_the_json_output() {
    let path = session_file("json.calc", &["100 km/h * 5 s"]);
    let output = calc(&[path.to_str().expect("path is UTF-8"), "--json"]);

    let text = standard_output(&output);
    assert!(output.status.success());
    assert!(!text.contains('\u{2248}'));
    assert!(text.contains("\"reading\": null"));
}

#[test]
fn a_machine_value_gets_no_reading() {
    let path = session_file("machine.calc", &["to_f64(1/3)"]);
    let output = calc(&[path.to_str().expect("path is UTF-8"), "--locale", "en"]);

    assert!(output.status.success());
    assert!(!standard_output(&output).contains('\u{2248}'));
}

#[test]
fn an_integer_gets_no_reading() {
    assert_eq!(value_row("integer.calc", "1 m + 1 m"), "value     2 m");
}
