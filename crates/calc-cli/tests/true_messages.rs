use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("output is UTF-8")
}

#[test]
fn a_fractional_power_of_a_unit_is_refused_as_not_whole() {
    let output = calc(&["1 kg^0.5", "--locale", "en"]);

    assert!(
        stderr(&output).contains("a unit takes only whole powers, and 0.5 is not a whole number"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_result_too_large_gives_its_digits_as_a_lower_bound() {
    let output = calc(&["1000000!", "--terse"]);

    assert!(
        stdout(&output).contains("would have at least 5403924 digits"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_descending_range_is_refused_with_the_range_to_write() {
    let output = calc(&["--find", "n=3..1", "--counter", "n > 0"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains(
            "the range n=3..1 runs downward, and a range is searched upward; write n=1..3"
        ),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_list_holding_a_machine_number_names_it_as_the_reason() {
    let output = calc(&["[1, 2] * to_f64(0.5)", "--terse"]);

    assert!(
        stdout(&output).contains(
            "to_f64(0.5) is a machine number, and calc does not compute a list that holds one yet"
        ),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_free_name_is_noted_as_free_rather_than_as_holding() {
    let output = calc(&["x + 1", "--locale", "en"]);

    assert!(
        stdout(&output).contains("written in the free names x, which no line gives a value"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_locale_calc_does_not_have_is_named_on_standard_error() {
    let output = calc(&["1/3", "--locale", "xx", "--terse"]);

    assert!(output.status.success());
    assert_eq!(stdout(&output), "1/3 exact\n");
    assert_eq!(
        stderr(&output),
        "calc has no language xx, so it answers in en; it has de, en\n"
    );
}

#[test]
fn a_regional_tag_of_a_shipped_language_needs_no_note() {
    let output = calc(&["1/3", "--locale", "de-AT", "--terse"]);

    assert_eq!(stderr(&output), "");
}
