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
fn the_reference_lists_its_groups_and_entries_in_english() {
    let output = calc(&["help", "language", "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.starts_with("Numbers\n"));
    assert!(text.contains("\n  multiplication — the product of two values\n"));
    assert!(text.contains("\n    ascii    x * y  as in x * y\n"));
    assert!(text.contains("\n    unicode  x · y  as in x · y\n"));
}

#[test]
fn the_reference_is_in_german_under_the_german_locale() {
    let output = calc(&["help", "language", "--locale", "de"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.starts_with("Zahlen\n"));
    assert!(text.contains("wie in x · y\n"));
}

#[test]
fn the_same_entries_are_listed_in_both_locales() {
    let english = standard_output(&calc(&["help", "language", "--locale", "en"]));
    let german = standard_output(&calc(&["help", "language", "--locale", "de"]));

    assert_eq!(english.lines().count(), german.lines().count());
}

#[test]
fn the_json_form_names_keys_and_no_words() {
    let output = calc(&["help", "language", "--json"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.starts_with("{\n  \"groups\": [\n"));
    assert!(text.contains("\"heading_key\": \"common-reference-group-numbers\""));
    assert!(text.contains("\"name_key\": \"common-reference-operator-mul\""));
    assert!(text.contains("\"meaning_key\": \"common-reference-operator-mul-meaning\""));
    assert!(!text.contains("multiplication"));
}

#[test]
fn the_json_form_is_the_same_bytes_in_every_locale() {
    let english = standard_output(&calc(&["help", "language", "--json", "--locale", "en"]));
    let german = standard_output(&calc(&["help", "language", "--json", "--locale", "de"]));

    assert_eq!(english, german);
}

#[test]
fn the_json_form_carries_the_unit_and_prefix_tables() {
    let text = standard_output(&calc(&["help", "language", "--json"]));

    assert!(text.contains("\"units\": ["));
    assert!(text.contains("\"display_symbol\""));
    assert!(text.contains("\"scale_factor\""));
    assert!(text.contains("\"prefixes\": ["));
}

#[test]
fn an_unknown_help_topic_is_a_usage_error() {
    let output = calc(&["help", "language", "units", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn every_placeholder_and_argument_count_is_a_number() {
    let text = standard_output(&calc(&["help", "language", "--json"]));

    assert!(text.contains("\"placeholders\": ["));
    assert!(!text.contains("\"start\": null"));
    assert!(!text.contains("\"end\": null"));
    assert!(!text.contains("\"least\": null"));
    assert!(!text.contains("\"largest\": null"));
}
