use std::process::{Command, Output};

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

#[test]
fn the_version_names_the_build_the_target_and_the_file_format() {
    let output = calc(&["--version", "--locale", "en"]);
    let shown = standard_output(&output);
    let lines: Vec<&str> = shown.lines().collect();

    assert!(output.status.success());
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("calc "));
    assert!(lines[0].contains('(') && lines[0].ends_with(')'));
    assert!(lines[1].starts_with("target "));
    assert_eq!(
        lines[2],
        format!("session file format {}", calc_app::FORMAT_VERSION)
    );
}

#[test]
fn the_version_needs_no_expression() {
    let output = calc(&["--version"]);

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn the_file_format_the_version_names_is_the_one_the_library_writes() {
    let output = calc(&["--version", "--locale", "en"]);
    let shown = standard_output(&output);
    let named = shown
        .lines()
        .find_map(|line| line.strip_prefix("session file format "))
        .expect("the version names a format");

    assert_eq!(named, calc_app::FORMAT_VERSION.to_string());
}
