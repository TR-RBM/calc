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

fn standard_error(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("output is UTF-8")
}

fn answered(expression: &str) -> String {
    let output = calc(&[expression, "--locale", "en"]);
    assert!(output.status.success(), "{}", standard_error(&output));
    rows(&standard_output(&output))
}

fn rows(text: &str) -> String {
    text.lines()
        .map(|line| {
            format!(
                "\n{}\n",
                line.split_whitespace().collect::<Vec<_>>().join(" ")
            )
        })
        .collect()
}

fn refused(expression: &str) -> String {
    let output = calc(&[expression, "--locale", "en"]);
    assert!(!output.status.success(), "{}", standard_output(&output));
    rows(&(standard_error(&output) + &standard_output(&output)))
}

#[test]
fn a_celsius_difference_is_not_taken_as_a_fahrenheit_reading() {
    let shown = refused("to_fahrenheit(20 degC)");

    assert!(shown.contains("to_fahrenheit(from_celsius(20))"), "{shown}");
}

#[test]
fn a_fahrenheit_reading_below_absolute_zero_is_refused() {
    let shown = refused("from_fahrenheit(-500)");

    assert!(shown.contains("below absolute zero"), "{shown}");
}

#[test]
fn a_negative_kelvin_value_is_refused_as_a_celsius_reading() {
    let shown = refused("to_celsius(-5 K)");

    assert!(shown.contains("below absolute zero"), "{shown}");
}

#[test]
fn absolute_zero_on_the_celsius_scale_is_zero_kelvin() {
    let shown = answered("from_celsius(-273.15) -> K");

    assert!(shown.contains("\nvalue 0 K\n"), "{shown}");
}

#[test]
fn a_hundredth_of_a_degree_below_absolute_zero_is_refused() {
    let shown = refused("from_celsius(-273.16)");

    assert!(shown.contains("below absolute zero"), "{shown}");
}

#[test]
fn absolute_zero_on_the_fahrenheit_scale_is_zero_kelvin() {
    let shown = answered("from_fahrenheit(-459.67) -> K");

    assert!(shown.contains("\nvalue 0 K\n"), "{shown}");
}

#[test]
fn a_sum_of_differences_on_two_scales_names_no_scale_in_its_note() {
    let shown = answered("1 degF + 1 degC");

    assert!(
        shown.contains(
            "this value is a temperature difference, not a reading on a temperature scale"
        ),
        "{shown}"
    );
    assert!(!shown.contains("difference in °F"), "{shown}");
}

#[test]
fn a_kelvin_value_converted_to_celsius_advises_the_reading() {
    let shown = answered("300 K -> degC");

    assert!(shown.contains("to_celsius(300 K)"), "{shown}");
}

#[test]
fn three_hundred_kelvin_reads_as_26_85_degrees_celsius() {
    let shown = answered("to_celsius(300 K)");

    assert!(
        shown.contains("\nvalue 537/20") || shown.contains("\nvalue 26.85"),
        "{shown}"
    );
    assert!(
        shown.contains("this is a reading on the Celsius scale"),
        "{shown}"
    );
}

#[test]
fn the_difference_of_two_celsius_readings_is_a_difference_in_kelvin() {
    let shown = answered("from_celsius(20) - from_celsius(12)");

    assert!(shown.contains("\nvalue 8 K\n"), "{shown}");
}

#[test]
fn twenty_degrees_celsius_reads_as_68_degrees_fahrenheit() {
    let shown = answered("to_fahrenheit(from_celsius(20))");

    assert!(shown.contains("\nvalue 68\n"), "{shown}");
    assert!(
        shown.contains("this is a reading on the Fahrenheit scale"),
        "{shown}"
    );
}

#[test]
fn a_celsius_difference_converted_to_fahrenheit_advises_a_reading_that_works() {
    let shown = answered("20 degC -> degF");

    assert!(shown.contains("to_fahrenheit(from_celsius(20))"), "{shown}");
}

#[test]
fn a_difference_in_a_list_beside_a_kelvin_value_is_named_with_its_position() {
    let text = answered("[20 degC, 300 K]");
    assert!(
        text.contains("the entry 20 °C at position 1 is a temperature difference in °C")
            && text.contains("for a reading write from_celsius(20)"),
        "{text}"
    );
}

#[test]
fn a_difference_sorted_beside_a_kelvin_value_is_named_with_its_position() {
    let text = answered("insertion_sort([300 K, 20 degC])");
    assert!(
        text.contains("the entry 20 °C at position 2 is a temperature difference in °C")
            && text.contains("for a reading write from_celsius(20)"),
        "{text}"
    );
}

#[test]
fn a_fahrenheit_difference_beside_a_celsius_difference_is_named_too() {
    let text = answered("insertion_sort([68 degF, 20 degC])");
    assert!(
        text.contains("the entry 68 °F at position 1")
            && text.contains("from_fahrenheit(68)")
            && text.contains("the entry 20 °C at position 2"),
        "{text}"
    );
}

#[test]
fn a_list_of_differences_in_one_unit_carries_one_note() {
    let text = answered("[20 degC, 5 degC]");
    assert!(
        text.contains("the entries in °C are temperature differences")
            && !text.contains("at position"),
        "{text}"
    );
}

#[test]
fn a_list_of_readings_carries_no_difference_note() {
    let text = answered("[from_celsius(20), 300 K]");
    assert!(!text.contains("temperature difference"), "{text}");
}

#[test]
fn the_entry_note_is_in_german_too() {
    let output = calc(&["[20 degC, 300 K]", "--locale", "de"]);
    assert!(output.status.success(), "{}", standard_error(&output));
    let text = rows(&standard_output(&output));
    assert!(
        text.contains("der Eintrag 20 °C an Position 1 ist eine Temperaturdifferenz in °C")
            && text.contains("für eine Ablesung schreib from_celsius(20)"),
        "{text}"
    );
}
