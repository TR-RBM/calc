use std::path::PathBuf;
use std::process::{Command, Output};

use calc_app::{FixedClock, Session, UtcTimestamp, registered_backends};

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

fn standard_error(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("output is UTF-8")
}

fn session_file(name: &str, entered: &str, stored: &str) -> PathBuf {
    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("calc-ambiguous");
    std::fs::create_dir_all(&directory).expect("scratch directory is created");
    let clock = FixedClock::new(UtcTimestamp::from_milliseconds_since_unix_epoch(0), 1);
    let mut session = Session::new(Box::new(clock), registered_backends());
    session.enter(entered).expect("line is entered");
    let bytes = session.save_to_bytes().expect("session is saved");
    let text = String::from_utf8(bytes).expect("the session file is UTF-8");
    let path = directory.join(name);
    std::fs::write(&path, text.replace(entered, stored)).expect("session file is written");
    path
}

fn data_members(json: &str) -> Vec<String> {
    let start = json.find("\"data\"").expect("a data member");
    let end = start + json[start..].find('}').expect("the end of the data");
    json[start..end]
        .lines()
        .filter(|line| line.contains(": "))
        .map(|line| line.trim().trim_end_matches(',').to_owned())
        .collect()
}

#[test]
fn a_fresh_ambiguous_application_names_the_same_members_as_a_stored_one() {
    let path = session_file("application.calc", "sin(pi)/2", "sin pi/2");
    let stored = calc(&[path.to_str().expect("path is UTF-8"), "--json"]);
    let fresh = calc(&["sin pi/2", "--json"]);

    assert_eq!(
        data_members(&standard_error(&fresh)),
        data_members(&standard_output(&stored))
    );
}

#[test]
fn a_fresh_temperature_sign_names_the_same_members_as_a_stored_one() {
    let path = session_file("temperature.calc", "from_celsius(20)", "20 \u{00B0}C");
    let stored = calc(&[path.to_str().expect("path is UTF-8"), "--json"]);
    let fresh = calc(&["20 \u{00B0}C", "--json"]);

    assert_eq!(
        data_members(&standard_error(&fresh)),
        data_members(&standard_output(&stored))
    );
}

#[test]
fn a_fresh_ambiguous_input_names_the_same_code_as_a_stored_one() {
    let path = session_file("code.calc", "sin(pi)/2", "sin pi/2");
    let stored = calc(&[path.to_str().expect("path is UTF-8"), "--json"]);
    let fresh = calc(&["sin pi/2", "--json"]);

    assert!(standard_output(&stored).contains("\"code\": \"ambiguous_application\""));
    assert!(standard_error(&fresh).contains("\"code\": \"ambiguous_application\""));
}
