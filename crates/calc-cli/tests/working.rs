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

fn long_sum(terms: usize) -> String {
    (1..=terms)
        .map(|term| term.to_string())
        .collect::<Vec<String>>()
        .join(" + ")
}

#[test]
fn the_working_of_an_exact_line_names_every_step() {
    let output = calc(&["(2+3)*4", "--working", "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("addition      2, 3 = 5"), "{text}");
    assert!(text.contains("multiplication  5, 4 = 20"), "{text}");
}

#[test]
fn a_step_is_indented_by_its_depth_under_the_step_it_feeds() {
    let output = calc(&["(2+3)*4", "--working", "--locale", "en"]);
    let text = standard_output(&output);
    let rows: Vec<&str> = text.lines().skip(1).collect();

    let depth = |row: &str| row.len() - row.trim_start().len();
    assert!(depth(rows[0]) > depth(rows[1]), "{text}");
}

#[test]
fn the_working_is_never_shown_without_the_flag() {
    let output = calc(&["(2+3)*4", "--locale", "en"]);

    assert!(output.status.success());
    assert!(!standard_output(&output).contains("addition"));
}

#[test]
fn a_working_over_the_limit_says_how_many_steps_are_left_and_how_to_ask() {
    let output = calc(&[&long_sum(260), "--working", "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("further steps"));
    assert!(text.contains("--working 0."));
}

#[test]
fn a_path_asks_for_the_working_of_that_subexpression() {
    let output = calc(&["(2+3)*4", "--working", "0", "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("addition"));
    assert!(!text.contains("multiplication"));
}

#[test]
fn a_path_that_is_not_step_numbers_is_a_usage_error() {
    let output = calc(&["(2+3)*4", "--working", "0.x", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn the_json_form_carries_the_steps_and_what_is_left() {
    let output = calc(&["(2+3)*4", "--working", "--json"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("\"steps\": ["));
    assert!(text.contains("\"further_steps\": 0"));
    assert!(text.contains("\"further_paths\": []"));
}

#[test]
fn a_machine_line_names_its_format_in_the_json_form() {
    let output = calc(&["to_f64(1/3 + 1)", "--working", "--json"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("\"format\": \"f64\"") || text.contains("\"steps\": []"));
}
