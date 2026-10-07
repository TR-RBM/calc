use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn standard_error(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("output is UTF-8")
}

#[test]
fn an_exact_run_says_nothing_about_a_graphics_adapter() {
    let output = calc(&["2 + 3", "--locale", "en"]);

    assert!(output.status.success());
    assert!(standard_error(&output).is_empty());
}

#[test]
fn a_machine_run_keeps_its_value_and_its_exit_code_whatever_the_probe_answers() {
    let output = calc(&["to_f64(1/3)", "--locale", "en"]);
    let text = String::from_utf8(output.stdout.clone()).expect("output is UTF-8");

    assert!(output.status.success());
    assert!(text.contains("0.3333333333333333"));
}

#[test]
fn a_refusal_is_a_line_on_standard_error_and_not_on_standard_output() {
    let output = calc(&["to_f64(1/3)", "--locale", "en"]);
    let text = String::from_utf8(output.stdout.clone()).expect("output is UTF-8");

    assert!(!text.contains("graphics adapter"));
    let errors = standard_error(&output);
    assert!(errors.is_empty() || errors.contains("graphics adapter"));
}

#[test]
fn a_machine_run_under_json_keeps_standard_output_parseable() {
    let output = calc(&["to_f64(1/3)", "--json"]);
    let text = String::from_utf8(output.stdout.clone()).expect("output is UTF-8");

    assert!(output.status.success());
    assert!(text.starts_with("{\n"));
    assert!(text.trim_end().ends_with('}'));
}

#[test]
fn a_failed_run_that_reached_a_backend_still_says_what_the_probe_answered() {
    let output = calc(&["sqrt(2)", "--enclose", "5001", "--locale", "en"]);
    let errors = standard_error(&output);

    assert_eq!(output.status.code(), Some(1));
    assert!(errors.starts_with("error: "));
    assert!(errors.lines().count() == 1 || errors.contains("graphics adapter"));
}

#[test]
fn a_usage_mistake_never_probes() {
    let output = calc(&["--enclose", "5", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(!standard_error(&output).contains("graphics adapter"));
}
