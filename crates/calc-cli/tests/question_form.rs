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

#[test]
fn a_decimal_divided_by_three_answers_a_terminating_decimal() {
    let shown = answered("1.5 / 3");

    assert!(shown.contains("\nvalue 0.5\n"), "{shown}");
    assert!(shown.contains("\nnumber exact decimal\n"), "{shown}");
}

#[test]
fn a_price_divided_by_twelve_answers_in_decimals() {
    let shown = answered("47.76 / 12");

    assert!(shown.contains("\nvalue 3.98\n"), "{shown}");
    assert!(shown.contains("\nnumber exact decimal\n"), "{shown}");
}

#[test]
fn a_decimal_length_divided_by_three_keeps_its_decimals() {
    let shown = answered("1.2 m / 3");

    assert!(shown.contains("\nvalue 0.4 m\n"), "{shown}");
}

#[test]
fn a_decimal_distance_over_hours_answers_a_decimal_speed() {
    let shown = answered("5.5 km / 2 h");

    assert!(shown.contains("\nvalue 2.75 km/h\n"), "{shown}");
}

#[test]
fn a_decimal_distance_over_a_bracketed_time_answers_a_decimal_speed() {
    let shown = answered("5.5 km / (2 h)");

    assert!(shown.contains("\nvalue 2.75 km/h\n"), "{shown}");
}

#[test]
fn a_decimal_quotient_that_recurs_stays_a_fraction() {
    let shown = answered("0.1 / 3");

    assert!(shown.contains("\nvalue 1/30\n"), "{shown}");
    assert!(shown.contains("\nnumber exact fraction\n"), "{shown}");
}

#[test]
fn a_quotient_of_whole_numbers_stays_a_fraction_when_it_terminates() {
    let shown = answered("7 / 4");

    assert!(shown.contains("\nvalue 7/4\n"), "{shown}");
}

#[test]
fn a_list_element_written_as_a_decimal_stays_a_decimal() {
    let shown = answered("[0.5, 1/3]");

    assert!(shown.contains("\nvalue [0.5, 1/3]\n"), "{shown}");
}

#[test]
fn a_list_of_decimals_divided_by_three_keeps_its_terminating_element_decimal() {
    let shown = answered("[0.5, 1.5] / 3");

    assert!(shown.contains("\nvalue [1/6, 0.5]\n"), "{shown}");
}

#[test]
fn the_larger_of_a_decimal_and_a_fraction_keeps_its_decimal_form() {
    let shown = answered("max(0.5, 1/3)");

    assert!(shown.contains("\nvalue 0.5\n"), "{shown}");
}

#[test]
fn a_logarithm_whose_argument_and_base_share_a_base_is_a_fraction() {
    let shown = answered("log(10, 100)");

    assert!(shown.contains("\nvalue 1/2\n"), "{shown}");
}

#[test]
fn a_logarithm_of_eight_to_base_four_is_two_thirds() {
    let shown = answered("log(4, 8)");

    assert!(shown.contains("\nvalue 2/3\n"), "{shown}");
    assert!(shown.contains("\nnumber exact fraction\n"), "{shown}");
}

#[test]
fn a_factorial_shown_in_exponent_form_counts_every_digit() {
    let shown = answered("1000!");

    assert!(
        shown.contains("\ndigits 2568 digits, in full with --json\n"),
        "{shown}"
    );
}

#[test]
fn an_enclosure_of_a_refused_root_gives_the_root_its_own_reason() {
    let output = calc(&[
        "rootof(2*x^8+16*x^7+18*x^6+16*x^5-10*x^4+10*x^3+9*x^2-3*x-3, x, 3)",
        "--enclose",
        "30",
        "--locale",
        "en",
    ]);

    assert!(!output.status.success());
    let error = standard_error(&output);
    assert!(
        error.contains("this polynomial has 2 real roots"),
        "{error}"
    );
}

#[test]
fn a_decimal_minus_a_fraction_keeps_its_rounded_reading() {
    let shown = answered("0.5 - 1/3");

    assert!(shown.contains("1/6"), "{shown}");
    assert!(shown.contains("0.1667"), "{shown}");
}

#[test]
fn the_smaller_of_a_decimal_and_a_fraction_keeps_its_rounded_reading() {
    let shown = answered("min(0.5, 1/3)");

    assert!(shown.contains("1/3"), "{shown}");
    assert!(shown.contains("0.3333"), "{shown}");
}
