use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc_in(language: &str, arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .env("LANG", language)
        .output()
        .expect("calc runs")
}

fn first_error_line(output: &Output) -> String {
    String::from_utf8(output.stderr.clone())
        .expect("output is UTF-8")
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn a_usage_error_follows_the_locale_written_on_the_same_line() {
    let english = calc_in(
        "de_DE.UTF-8",
        &["--ode", "diff(y, t) = -y", "--locale", "en"],
    );
    let german = calc_in("C", &["--unknown", "--locale", "de"]);

    assert_eq!(english.status.code(), Some(2));
    assert!(first_error_line(&english).starts_with("error: "));
    assert_eq!(german.status.code(), Some(2));
    assert!(first_error_line(&german).starts_with("Fehler: "));
}

#[test]
fn a_usage_error_without_a_locale_follows_lang() {
    let german = calc_in("de_DE.UTF-8", &["--unknown"]);

    assert!(first_error_line(&german).starts_with("Fehler: "));
}

#[test]
fn a_subcommand_usage_error_follows_the_locale_written_on_the_same_line() {
    let german = calc_in("C", &["plot", "x", "--bogus", "--locale", "de"]);

    assert!(first_error_line(&german).starts_with("Fehler: "));
}
