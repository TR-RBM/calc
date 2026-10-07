use std::io::Write;
use std::process::{Command, Output, Stdio};

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

fn standard_output(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

const RUN_TIME_LABELS: [&str; 2] = ["run time", "Laufzeit"];

fn without_run_time(output: &Output) -> String {
    standard_output(output)
        .lines()
        .filter(|line| {
            !RUN_TIME_LABELS
                .iter()
                .any(|label| line.trim_start().starts_with(label))
        })
        .map(|line| format!("{line}\n"))
        .collect()
}

#[test]
fn sum_of_fractions_prints_its_record_as_a_fraction() {
    let output = calc(&["1/4 + 1/8", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        without_run_time(&output),
        "r1  1/4 + 1/8\n\
         \x20 value     3/8\n\
         \x20 number    exact fraction\n\
         \x20 computed  exactly, without rounding\n\
         \x20 concept   none in the concept set matches\n"
    );
}

#[test]
fn sum_of_fractions_stays_exact_under_the_f64_setting() {
    let output = calc(&["1/3 + 1/6", "--json"]);
    let text = standard_output(&output);
    assert!(output.status.success());
    assert!(text.contains("\"kind\": \"exact_rational\",\n"));
    assert!(text.contains(
        "\"value\": {\n        \"type\": \"rational\",\n        \"numerator\": \"1\",\n        \"denominator\": \"2\"\n      },\n"
    ));
}

#[test]
fn decimal_sum_stays_exact_under_the_f64_setting() {
    let output = calc(&["0.1 + 0.2", "--json"]);
    let text = standard_output(&output);
    assert!(output.status.success());
    assert!(text.contains(
        "\"value\": {\n        \"type\": \"rational\",\n        \"numerator\": \"3\",\n        \"denominator\": \"10\"\n      },\n"
    ));
}

#[test]
fn a_factorial_under_a_root_answers_as_the_number_it_stands_for_does() {
    let over_factorial = calc(&["sqrt(3!)", "--terse", "--locale", "en"]);
    let over_six = calc(&["sqrt(6)", "--terse", "--locale", "en"]);
    assert!(over_factorial.status.success());
    assert_eq!(standard_output(&over_factorial), standard_output(&over_six));
}

fn terse(expression: &str) -> String {
    let output = calc(&[expression, "--terse", "--locale", "en"]);
    assert!(output.status.success(), "{expression} is answered");
    standard_output(&output)
}

#[test]
fn the_error_of_a_machine_value_is_a_line_a_person_can_type() {
    assert_eq!(terse("1/3 - to_f64(1/3)"), "1/54043195528445952 exact\n");
}

#[test]
fn one_quantity_at_two_widths_stands_in_one_line() {
    assert_eq!(
        terse("to_f64(1/3) - to_f32(1/3)"),
        "-178956971/18014398509481984 exact\n"
    );
}

#[test]
fn a_claim_over_two_machine_results_is_decided() {
    let holds = calc(&[
        "to_f64(sqrt(3!)) = to_f64(sqrt(6))",
        "--check",
        "--locale",
        "en",
    ]);
    let fails = calc(&[
        "to_f64(sqrt(2)) = to_f64(sqrt(3))",
        "--check",
        "--locale",
        "en",
    ]);

    assert_eq!(
        (standard_output(&holds), standard_output(&fails)),
        (
            "holds  to_f64(sqrt(3!)) = to_f64(sqrt(6))\n".to_owned(),
            "FAILS  to_f64(sqrt(2)) = to_f64(sqrt(3))\n".to_owned()
        )
    );
}

#[test]
fn a_conversion_answers_in_the_width_it_names() {
    assert_eq!(
        (terse("to_f32(1/3)"), terse("to_f64(1/3)")),
        (
            "0.33333334 machine\n".to_owned(),
            "0.3333333333333333 machine\n".to_owned()
        )
    );
}

#[test]
fn a_function_of_an_angle_is_not_written_in_the_angle_unit() {
    assert_eq!(terse("cos(30 deg)"), "sqrt(3) / 2 exact\n");
}

#[test]
fn every_function_of_an_angle_keeps_the_value_it_stored() {
    assert_eq!(
        (
            terse("tan(30 deg)"),
            terse("exp(1 deg)"),
            terse("sqrt(4 deg)"),
            terse("to_f64(sin(30 deg))")
        ),
        (
            "sqrt(3) / 3 exact\n".to_owned(),
            "exp(pi / 180) exact\n".to_owned(),
            "sqrt(pi / 45) exact\n".to_owned(),
            "0.49999999999999994 machine\n".to_owned()
        )
    );
}

#[test]
fn a_number_beside_an_angle_is_not_written_in_the_angle_unit() {
    assert_eq!(terse("sqrt(2)/2 + 0 * 1 deg"), "sqrt(2) / 2 exact\n");
}

#[test]
fn an_angle_is_still_written_in_the_unit_it_was_written_in() {
    assert_eq!(
        (terse("30 deg"), terse("30 deg + 15 deg")),
        (
            "30 \u{b0} exact\n".to_owned(),
            "45 \u{b0} exact\n".to_owned()
        )
    );
}

#[test]
fn a_wrong_unit_does_not_travel_through_a_session() {
    let mut running = Command::new(CALC)
        .args(["--batch", "--terse", "--locale", "en"])
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("calc runs");
    running
        .stdin
        .take()
        .expect("standard input is open")
        .write_all(b"a = cos(30 deg)\na * 2\n")
        .expect("the lines are written");
    let output = running.wait_with_output().expect("calc answers");

    assert!(output.status.success());
    assert_eq!(
        standard_output(&output),
        "sqrt(3) / 2 exact\nsqrt(3) exact\n"
    );
}

#[test]
fn the_text_and_the_json_of_one_line_agree_about_its_unit() {
    let text = calc(&["cos(30 deg)", "--locale", "en"]);
    let json = calc(&["cos(30 deg)", "--json"]);

    assert!(standard_output(&text).contains("value           sqrt(3) / 2\n"));
    assert!(standard_output(&json).contains("\"unit\": null"));
}

#[test]
fn a_photon_energy_is_written_in_the_exponent_form() {
    let output = calc(&["h_P * 5e14 Hz", "--terse", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(standard_output(&output), "3.313035075e-19 J exact\n");
}

#[test]
fn an_integer_that_is_mostly_zeros_is_written_in_the_exponent_form() {
    let output = calc(&["N_A", "--terse", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        standard_output(&output),
        "6.02214076e23 mol\u{207b}\u{00b9} exact\n"
    );
}

#[test]
fn a_fraction_keeps_its_form_and_reads_as_a_rounded_decimal() {
    let output = calc(&["500000/3", "--locale", "en"]);
    assert!(output.status.success());
    assert!(
        without_run_time(&output)
            .contains("value           500000/3\n  number          exact fraction")
    );
}

#[test]
fn the_endpoints_of_an_enclosure_are_values_a_later_line_can_use() {
    assert_eq!(
        (
            terse("enclosure_lower(pi, 12)"),
            terse("enclosure_upper(pi, 12)"),
            terse("enclosure_upper(pi, 12) - enclosure_lower(pi, 12)")
        ),
        (
            "157079632679/50000000000 exact\n".to_owned(),
            "314159265359/100000000000 exact\n".to_owned(),
            "1/100000000000 exact\n".to_owned()
        )
    );
}

#[test]
fn an_endpoint_is_the_number_the_view_shows() {
    let view = calc(&["pi", "--enclose", "12", "--json"]);
    let shown = standard_output(&view);

    assert!(shown.contains("\"numerator\": \"157079632679\""), "{shown}");
    assert_eq!(
        terse("enclosure_lower(pi, 12)"),
        "157079632679/50000000000 exact\n"
    );
}

#[test]
fn an_endpoint_carries_the_unit_of_its_value() {
    assert_eq!(
        terse("enclosure_lower(2 m * pi, 12)"),
        "(628318530717/100000000000) m exact\n"
    );
}

#[test]
fn a_claim_about_a_proven_bound_is_decided() {
    let below = calc(&["enclosure_lower(pi, 12) < pi", "--check", "--locale", "en"]);
    let equal = calc(&[
        "enclosure_lower(0.5, 12) <= 0.5",
        "--check",
        "--locale",
        "en",
    ]);

    assert_eq!(
        (standard_output(&below), standard_output(&equal)),
        (
            "holds  enclosure_lower(pi, 12) < pi\n".to_owned(),
            "holds  enclosure_lower(0.5, 12) <= 0.5\n".to_owned()
        )
    );
}

#[test]
fn an_endpoint_of_a_machine_value_bounds_the_double_it_names() {
    assert_eq!(
        (
            terse("enclosure_lower(to_f64(2*pi), 20)"),
            terse("enclosure_lower(2*pi, 20)")
        ),
        (
            "62831853071795862319/10000000000000000000 exact\n".to_owned(),
            "62831853071795864769/10000000000000000000 exact\n".to_owned()
        )
    );
}

#[test]
fn an_argument_the_view_will_not_enclose_is_refused_in_the_views_words() {
    let refusal = "this takes no enclosure: it must be a real value without an uncertainty whose operations all have a proven enclosure";
    for argument in [
        "enclosure_lower([1,2], 12)",
        "enclosure_lower(2 +- 0.1, 12)",
        "enclosure_lower(complex(1,2), 12)",
    ] {
        let output = calc(&[argument, "--locale", "en"]);
        assert!(
            standard_output(&output).contains(refusal),
            "{argument}: {}",
            standard_output(&output)
        );
    }
}

#[test]
fn a_digit_count_is_reported_as_it_was_written() {
    let fraction = calc(&["enclosure_lower(pi, 1.5)", "--locale", "en"]);
    let negative = calc(&["enclosure_lower(pi, -3)", "--locale", "en"]);

    assert!(
        standard_output(&fraction)
            .contains("the number of significant digits must be a whole number from 1 to 5000"),
        "{}",
        standard_output(&fraction)
    );
    assert!(
        standard_output(&negative).contains("-3 significant digits is outside 1 to 5000"),
        "{}",
        standard_output(&negative)
    );
}

#[test]
fn an_endpoint_refuses_where_the_view_refuses_and_in_its_words() {
    let digits = calc(&["enclosure_lower(pi, 5001)", "--locale", "en"]);
    let view = calc(&["pi", "--enclose", "5001", "--locale", "en"]);
    let reach = calc(&["enclosure_lower(asin(0.3), 12)", "--locale", "en"]);

    assert!(
        standard_output(&digits).contains("5001 significant digits is outside 1 to 5000"),
        "{}",
        standard_output(&digits)
    );
    assert!(
        standard_error(&view).contains("5001 significant digits is outside 1 to 5000"),
        "{}",
        standard_error(&view)
    );
    assert!(
        standard_output(&reach).contains(
            "this takes no enclosure: it must be a real value without an uncertainty whose operations all have a proven enclosure"
        ),
        "{}",
        standard_output(&reach)
    );
}

#[test]
fn an_overflow_from_finite_operands_says_so_on_the_line() {
    let output = calc(&["to_f64(product(k, k, 1, 200))", "--locale", "en"]);

    assert!(output.status.success());
    let shown = without_run_time(&output);
    assert!(shown.contains("value           inf\n"), "{shown}");
    assert!(
        shown.contains(
            "notes           every operand was finite and the result is not: the machine format cannot hold a number this large\n"
        ),
        "{shown}"
    );
}

#[test]
fn an_infinity_handed_in_by_an_earlier_line_is_not_noted_as_one_made_here() {
    let mut running = Command::new(CALC)
        .args(["--batch", "--locale", "en"])
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("calc runs");
    running
        .stdin
        .take()
        .expect("standard input is open")
        .write_all(b"to_f64(1e308 * 10)\nr1 + 1\n")
        .expect("the lines are written");
    let output = running.wait_with_output().expect("calc answers");
    let shown = String::from_utf8(output.stdout).expect("output is UTF-8");

    assert_eq!(shown.matches("notes").count(), 1, "{shown}");
    let (first, second) = shown.split_once("r2").expect("two lines");
    assert!(first.contains("notes"), "{first}");
    assert!(!second.contains("notes"), "{second}");
}

#[test]
fn an_infinity_a_person_wrote_is_not_noted_as_one_calc_made() {
    let output = calc(&["inf - inf", "--locale", "en"]);
    let shown = standard_output(&output);

    assert!(shown.contains("nan"), "{shown}");
    assert!(!shown.contains("notes"), "{shown}");
}

#[test]
fn a_cancelling_machine_line_names_the_exact_route() {
    let machine = calc(&["to_f64(sqrt(1e16 + 1) - sqrt(1e16))", "--locale", "en"]);
    let shown = without_run_time(&machine);

    assert!(shown.contains("value           0\n"), "{shown}");
    assert!(
        shown.contains(
            "notes           the rounding error is not smaller than the value, so these digits say nothing about it; the same line without to_f64 is evaluated exactly\n"
        ),
        "{shown}"
    );
}

#[test]
fn the_exact_route_is_the_line_without_the_conversion() {
    let exact = calc(&["sqrt(1e16 + 1) - sqrt(1e16)", "--locale", "en"]);
    let shown = without_run_time(&exact);

    assert!(shown.contains("value           5.000e-9\n"), "{shown}");
    assert!(!shown.contains("notes"), "{shown}");
}

#[test]
fn square_root_prints_its_record_in_german() {
    let output = calc(&["sqrt(2)", "--locale", "de"]);
    assert!(output.status.success());
    assert_eq!(
        without_run_time(&output),
        "r1  sqrt(2)\n\
         \x20 Wert            sqrt(2)\n\
         \x20 Zahl            exakt, mit Wurzeln (algebraisch)\n\
         \x20 gerechnet       exakt, ohne Rundung\n\
         \n\
         \x20 Wert            1.414\n\
         \x20 Zahl            Dezimalzahl, auf 4 gültige Stellen gerundet\n\
         \x20 gerechnet       aus dem exakten Wert oben, kaufmännisch gerundet\n\
         \x20 Rundungsfehler  höchstens 0.0005\n\
         \n\
         \x20 Konzept         keines aus der Konzeptmenge passt\n"
    );
}

#[test]
fn undefined_name_prints_the_failed_line_and_exits_with_failure() {
    let output = calc(&["r9 + 1", "--locale", "en"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        standard_output(&output),
        "r1  r9 + 1\n  r9 is not defined\n"
    );
}

#[test]
fn a_temperature_difference_line_says_it_is_not_a_reading() {
    let output = calc(&["20 degC -> K", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        without_run_time(&output),
        "r1  20 degC -> K\n\
         \x20 value     20 K\n\
         \x20 number    exact integer\n\
         \x20 computed  exactly, without rounding\n\
         \x20 notes     this is a temperature difference in \u{b0}C, not a reading on that \
         scale; for a reading write from_celsius(20)\n\
         \x20 concept   none in the concept set matches\n"
    );

    let machine = calc(&["20 degC -> K", "--json"]);
    let text = standard_output(&machine);
    assert!(text.contains("\"code\": \"temperature_difference\""));
    assert!(text.contains("\"operator\": \"from_celsius\""));
}

#[test]
fn an_undefined_constant_symbol_keeps_the_code_and_names_the_constant() {
    let output = calc(&["c", "--locale", "en"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        standard_output(&output),
        "r1  c\n  c is not defined; the physical constant is written c_0\n"
    );

    let machine = calc(&["c", "--json"]);
    let text = standard_output(&machine);
    assert!(text.contains("\"code\": \"undefined_name\""));
    assert!(text.contains("\"constant\": \"c_0\""));
    assert!(text.contains("\"name\": \"c\""));
}

#[test]
fn a_compound_unit_denominator_parses_and_reads_back() {
    let written = calc(&["4186 J/(kg*K)", "--terse", "--locale", "en"]);
    assert!(written.status.success());
    assert_eq!(standard_output(&written), "4186 J/(K\u{b7}kg) exact\n");

    let displayed = calc(&["4186 J/(K\u{b7}kg)", "--terse", "--locale", "en"]);
    assert!(displayed.status.success());
    assert_eq!(standard_output(&displayed), "4186 J/(K\u{b7}kg) exact\n");
}

#[test]
fn json_output_is_the_session_file_line_object() {
    let output = calc(&["2^10", "--json"]);
    let text = standard_output(&output);
    assert!(output.status.success());
    assert!(text.starts_with("{\n  \"id\": \"r1\",\n  \"name\": null,\n  \"input\": \"2^10\",\n"));
    assert!(text.contains("\"digits\": \"1024\"\n"));
    assert!(text.ends_with("}\n"));
}

#[test]
fn parse_error_goes_to_standard_error() {
    let output = calc(&["2 +", "--locale", "en"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).expect("errors are UTF-8"),
        "error: the input ends too early at column 4\n"
    );
}

#[test]
fn quantity_product_prints_its_value_in_newtons() {
    let output = calc(&["2.50 kg * 9.81 m/s^2", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        without_run_time(&output),
        "r1  2.50 kg * 9.81 m/s^2\n\
         \x20 value     24.525 N\n\
         \x20 number    exact decimal\n\
         \x20 computed  exactly, without rounding\n\
         \x20 concept   none in the concept set matches\n"
    );
}

#[test]
fn large_exact_integer_states_that_it_was_evaluated_exactly() {
    let output = calc(&["10^134+5", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        without_run_time(&output),
        "r1  10^134+5\n\
         \x20 value     100000000000\u{2026}000000000005\n\
         \x20 digits    135 digits, in full with --json\n\
         \x20 number    exact integer\n\
         \x20 computed  exactly, without rounding\n\
         \x20 concept   Addition\n"
    );
}

#[test]
fn a_long_fraction_is_shortened_at_both_ends_with_both_counts() {
    let output = calc(&["sum(i^2/(i+1), i, 1, 1000)", "--locale", "en"]);
    assert!(output.status.success());
    let shown = without_run_time(&output);
    assert!(shown.contains("value           499500\n"), "{shown}");
    assert!(shown.contains("digits          439 and 433 digits, in full with --json\n"));
}

#[test]
fn a_shortened_value_is_whole_in_the_json() {
    let output = calc(&["sum(i^2/(i+1), i, 1, 1000)", "--json"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(!text.contains('\u{2026}'));
    assert!(text.contains("\"numerator\": \"3560914445864965140882560404513059496451736753"));
}

#[test]
fn a_value_within_the_row_is_written_in_full() {
    let output = calc(&["2^100", "--terse", "--locale", "en"]);

    assert_eq!(
        standard_output(&output),
        "1267650600228229401496703205376 exact\n"
    );
}

#[test]
fn a_sum_asked_for_as_a_machine_value_names_its_method_and_backend() {
    let output = calc(&["to_f64(sum(i^2/(i+1), i, 1, 10))", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        without_run_time(&output),
        "r1  to_f64(sum(i^2/(i+1), i, 1, 10))\n\
         \x20 value           47.019877344877344\n\
         \x20 number          machine float, 64-bit (f64)\n\
         \x20 rounding error  at most 4.973799150320702e-14, absolute\n\
         \x20 computed        in machine arithmetic\n\
         \x20 ran on          CPU\n\
         \x20 concept         none in the concept set matches\n"
    );
}

#[test]
fn square_root_answers_with_its_exact_value_and_its_reading() {
    let output = calc(&["sqrt(2)", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        without_run_time(&output),
        "r1  sqrt(2)\n\
         \x20 value           sqrt(2)\n\
         \x20 number          exact, with roots (algebraic)\n\
         \x20 computed        exactly, without rounding\n\
         \n\
         \x20 value           1.414\n\
         \x20 number          decimal, rounded to 4 significant digits\n\
         \x20 computed        from the exact value above, rounded half away from zero\n\
         \x20 rounding error  at most 0.0005\n\
         \n\
         \x20 concept         none in the concept set matches\n"
    );
}

#[test]
fn the_digits_row_spells_the_unit_as_the_value_row_does() {
    let output = calc(&["2 N * 3 m", "--digits", "3", "--locale", "en"]);
    assert!(output.status.success());
    assert!(
        standard_output(&output).contains("truncated       6 N\u{00b7}m\n"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn the_enclosure_rows_spell_the_unit_as_the_value_row_does() {
    let output = calc(&["2 N * 3 m", "--enclose", "3", "--locale", "en"]);
    assert!(output.status.success());
    assert!(
        standard_output(&output).contains("lower bound         6 N\u{00b7}m\n"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn an_angle_written_in_radians_after_pi_converts_to_degrees() {
    let output = calc(&["pi rad -> deg", "--terse", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(standard_output(&output), "180 \u{00b0} exact\n");
}

#[test]
fn an_exact_and_a_machine_answer_spell_a_reciprocal_unit_alike() {
    let exact = calc(&["1/(2 s)", "--terse", "--locale", "en"]);
    let machine = calc(&["to_f64(1/(2 s))", "--terse", "--locale", "en"]);
    assert!(exact.status.success() && machine.status.success());
    assert_eq!(
        (
            standard_output(&exact).trim_end().to_owned(),
            standard_output(&machine).trim_end().to_owned()
        ),
        (
            "(1/2) s\u{207b}\u{00b9} exact".to_owned(),
            "0.5 s\u{207b}\u{00b9} machine".to_owned()
        )
    );
}

#[test]
fn the_digits_row_spells_a_reciprocal_unit_for_a_person() {
    let output = calc(&["1/(2 s)", "--digits", "3", "--locale", "en"]);
    assert!(output.status.success());
    assert!(
        standard_output(&output).contains("truncated       0.5 s\u{207b}\u{00b9}\n"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn the_digits_row_takes_the_unit_the_value_is_shown_in() {
    let output = calc(&["1.5 km", "--digits", "3", "--locale", "en"]);
    assert!(output.status.success());
    assert!(
        standard_output(&output).contains("truncated       1.5 km\n"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn the_digits_row_takes_the_unit_the_run_asked_for() {
    let output = calc(&[
        "100 km / 1 h",
        "--unit",
        "speed=mi/h",
        "--digits",
        "2",
        "--locale",
        "en",
    ]);
    assert!(output.status.success());
    assert!(
        standard_output(&output).contains("truncated       62.13 mi/h\n"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn the_enclosure_rows_take_the_unit_the_run_asked_for() {
    let output = calc(&[
        "100 km / 1 h",
        "--unit",
        "speed=mi/h",
        "--enclose",
        "4",
        "--locale",
        "en",
    ]);
    assert!(output.status.success());
    assert!(
        standard_output(&output).contains("lower bound         62.13 mi/h\n"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn coherent_units_reach_the_digits_row() {
    let output = calc(&[
        "100 km / 1 h",
        "--coherent-units",
        "--digits",
        "3",
        "--locale",
        "en",
    ]);
    assert!(output.status.success());
    assert!(
        standard_output(&output).contains("truncated       27.777 m/s\n"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn arcsine_states_its_machine_width_rounding_and_backend() {
    let output = calc(&["asin(0.3)", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        without_run_time(&output),
        "r1  asin(0.3)\n\
         \x20 value           0.3046926540153975\n\
         \x20 number          machine float, 64-bit (f64)\n\
         \x20 rounding error  at most 1.665334536937735e-16, absolute\n\
         \x20 computed        in machine arithmetic\n\
         \x20 ran on          CPU\n\
         \x20 concept         none in the concept set matches\n"
    );
}

#[test]
fn arccosine_answers_with_its_value() {
    let output = calc(&["acos(0.3)", "--locale", "en"]);
    assert!(output.status.success());
    assert!(without_run_time(&output).contains("value           1.2661036727794992\n"));
}

#[test]
fn arctangent_answers_with_its_value() {
    let output = calc(&["atan(2)", "--locale", "en"]);
    assert!(output.status.success());
    assert!(without_run_time(&output).contains("value           1.107\n"));
}

#[test]
fn the_digits_of_a_measured_value_state_its_uncertainty() {
    let output = calc(&[
        "10 kg * (3600 +- 150) kcal_th/kg / ((2000 +- 300) kcal_th/d) -> d",
        "--digits",
        "1",
        "--locale",
        "en",
    ]);
    assert!(output.status.success());
    assert!(
        standard_output(&output).contains("uncertainty     \u{b1} 2.8 d, cut off at this place\n"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn an_uncertainty_below_the_last_place_is_not_printed_as_zero() {
    let output = calc(&["(1000 +- 0.001) * 1", "--digits", "1", "--locale", "en"]);
    assert!(output.status.success());
    assert!(
        standard_output(&output).contains("uncertainty     smaller than the last place shown\n"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn a_symbolic_value_gives_its_decimal_places() {
    let output = calc(&["pi/3", "--digits", "10", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        without_run_time(&output),
        "r1  pi/3\n\
         \x20 decimal places  10\n\
         \x20 digits of       the exact value\n\
         \x20 truncated       1.0471975511\n\
         \x20 expansion       not known to recur; the digits come from a proven enclosure\n"
    );
}

#[test]
fn a_symbolic_value_can_be_enclosed() {
    let output = calc(&["pi", "--enclose", "12", "--locale", "en"]);
    assert!(output.status.success());
    assert!(
        standard_output(&output).contains("lower bound         3.14159265358\n"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn an_enclosure_says_it_encloses_a_machine_value() {
    let output = calc(&["to_f64(2*pi)", "--enclose", "20", "--locale", "en"]);
    assert!(output.status.success());
    assert!(
        standard_output(&output).contains("encloses            the machine value\n"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn measured_product_states_its_propagated_uncertainty() {
    let output = calc(&["(2.5 +- 0.01 kg) * (9.81 +- 0.02 m/s^2)", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        without_run_time(&output),
        "r1  (2.5 +- 0.01 kg) * (9.81 +- 0.02 m/s^2)\n\
         \x20 value        24.52 N\n\
         \x20 uncertainty  ± 0.11 N, standard uncertainty, coverage factor k = 1\n\
         \x20 propagation  first order from 2 uncorrelated inputs\n\
         \x20 number       derived from 2 measured inputs\n\
         \x20 computed     exactly, without rounding\n\
         \x20 concept      none in the concept set matches\n"
    );
}

#[test]
fn the_terse_form_of_a_measured_line_says_what_the_record_says() {
    let terse = calc(&["(2 +- 0.1) * 3", "--terse", "--locale", "en"]);
    let full = calc(&["(2 +- 0.1) * 3", "--locale", "en"]);

    assert!(terse.status.success());
    assert_eq!(
        standard_output(&terse),
        "6.00 \u{b1} 0.30 derived from 1 measured input\n"
    );
    let full = standard_output(&full);
    assert!(full.contains("\u{b1} 0.30, standard uncertainty"));
    assert!(full.contains("number       derived from 1 measured input\n"));
}

#[test]
fn the_terse_form_of_a_measured_quantity_carries_its_unit() {
    let output = calc(&["(2 +- 0.1) * 1 m", "--terse", "--locale", "en"]);

    assert!(output.status.success());
    assert_eq!(
        standard_output(&output),
        "2.00 m \u{b1} 0.10 m derived from 1 measured input\n"
    );
}

#[test]
fn the_terse_form_of_a_line_that_is_exact_still_says_exact() {
    let exact = calc(&["1/3 + 1/6", "--terse", "--locale", "en"]);
    let machine = calc(&["to_f64(1/3)", "--terse", "--locale", "en"]);

    assert_eq!(
        (standard_output(&exact), standard_output(&machine)),
        (
            "1/2 exact\n".to_owned(),
            "0.3333333333333333 machine\n".to_owned()
        )
    );
}

#[test]
fn a_measured_result_states_the_digits_its_uncertainty_supports() {
    let output = calc(&["G_N * 5.97e24 kg / (6.371e6 m)^2", "--locale", "en"]);
    assert!(output.status.success());
    let shown = without_run_time(&output);
    assert!(
        shown.contains("value        9.81668 m/s\u{b2}\n"),
        "{shown}"
    );
    assert!(shown.contains("uncertainty  \u{b1} 0.00022 m/s\u{b2}, standard uncertainty"));
}

#[test]
fn a_measured_time_is_written_to_two_figures_of_its_uncertainty() {
    let output = calc(&[
        "10 kg * (3600 +- 150) kcal_th/kg / ((2000 +- 300) kcal_th/d) -> d",
        "--locale",
        "en",
    ]);
    assert!(output.status.success());
    let shown = without_run_time(&output);
    assert!(shown.contains("value        18.0 d\n"), "{shown}");
    assert!(shown.contains("uncertainty  \u{b1} 2.8 d, standard uncertainty"));
}

#[test]
fn two_measurements_written_alike_propagate_as_two_inputs() {
    let output = calc(&["(10 +- 1) * (10 +- 1)", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        without_run_time(&output),
        "r1  (10 +- 1) * (10 +- 1)\n\
         \x20 value        100\n\
         \x20 uncertainty  ± 14, standard uncertainty, coverage factor k = 1\n\
         \x20 propagation  first order from 2 uncorrelated inputs\n\
         \x20 number       derived from 2 measured inputs\n\
         \x20 computed     exactly, without rounding\n\
         \x20 concept      none in the concept set matches\n"
    );
}

#[test]
fn non_terminating_fraction_is_shown_and_named_as_a_fraction() {
    let output = calc(&["1/3", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        without_run_time(&output),
        "r1  1/3\n\
         \x20 value           1/3\n\
         \x20 number          exact fraction\n\
         \x20 computed        exactly, without rounding\n\
         \n\
         \x20 value           0.3333\n\
         \x20 number          decimal, rounded to 4 significant digits\n\
         \x20 computed        from the exact value above, rounded half away from zero\n\
         \x20 rounding error  1/30000 below the exact value\n\
         \n\
         \x20 concept         Division\n"
    );
}

#[test]
fn written_kilometres_print_in_kilometres() {
    let output = calc(&["5 km", "--locale", "en"]);
    assert!(output.status.success());
    assert!(standard_output(&output).contains("\n  value     5 km\n"));
}

#[test]
fn a_record_states_the_run_time_it_measured() {
    let output = calc(&["1/4 + 1/8", "--locale", "en"]);
    assert!(output.status.success());
    assert!(
        standard_output(&output)
            .lines()
            .any(|line| line.trim_start().starts_with("run time"))
    );
}

#[test]
fn a_function_naming_reports_the_defined_status() {
    let output = calc(&["f(x) = sin(x)", "--json"]);
    assert!(output.status.success());
    assert!(standard_output(&output).contains("\"outcome\": {\n    \"status\": \"defined\"\n  },"));
}

#[test]
fn a_function_naming_says_what_it_defines() {
    let output = calc(&["f(x) = sin(x)", "--locale", "en"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        standard_output(&output),
        "r1  f(x) = sin(x)\n  defines a function for use in other lines\n"
    );
}

#[test]
fn json_keeps_written_kilometres_in_coherent_metres() {
    let output = calc(&["5 km", "--json"]);
    let text = standard_output(&output);
    assert!(output.status.success());
    assert!(text.contains("\"digits\": \"5000\""));
    assert!(text.contains("\"unit\": \"m\""));
}

#[test]
fn sum_of_decimal_literals_prints_its_record_as_a_decimal() {
    let output = calc(&["0.25 + 0.125", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        without_run_time(&output),
        "r1  0.25 + 0.125\n\
         \x20 value     0.375\n\
         \x20 number    exact decimal\n\
         \x20 computed  exactly, without rounding\n\
         \x20 concept   none in the concept set matches\n"
    );
}

#[test]
fn digits_option_prints_the_truncated_decimal_with_its_remainder() {
    let output = calc(&["1/3", "--digits", "5", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        standard_output(&output),
        "r1  1/3\n\
         \x20 decimal places  5\n\
         \x20 digits of       the exact value\n\
         \x20 truncated       0.33333\n\
         \x20 remainder       1/300000\n\
         \x20 expansion       recurring from place 1, period length 1\n"
    );
}

#[test]
fn enclose_option_prints_a_proven_interval() {
    let output = calc(&["sqrt(2)", "--enclose", "10", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        standard_output(&output),
        "r1  sqrt(2)\n\
         \x20 significant digits  10\n\
         \x20 encloses            the exact value\n\
         \x20 lower bound         1.414213562\n\
         \x20 upper bound         1.414213563\n\
         \x20 width               1e-9\n\
         \x20 precision           reached\n"
    );
}

#[test]
fn digits_option_with_json_prints_the_outcome_members() {
    let output = calc(&["1/8", "--digits", "2", "--json"]);
    assert!(output.status.success());
    assert!(standard_output(&output).contains("\"terminates\": false,\n"));
}

#[test]
fn view_of_a_refused_line_reports_the_line_its_own_refusal() {
    let output = calc(&["1/0", "--digits", "2", "--locale", "en"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).expect("errors are UTF-8"),
        "error: 1 / 0 divides by zero\n"
    );
}

#[test]
fn enclosure_above_its_limit_names_the_limit() {
    let output = calc(&["sqrt(2)", "--enclose", "5001", "--locale", "en"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr)
            .expect("errors are UTF-8")
            .lines()
            .next(),
        Some("error: 5001 significant digits is outside 1 to 5000")
    );
}

#[test]
fn decimal_speed_prints_in_its_written_unit_as_a_decimal() {
    let output = calc(&["1.5 km/h", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        without_run_time(&output),
        "r1  1.5 km/h\n\
         \x20 value     1.5 km/h\n\
         \x20 number    exact decimal\n\
         \x20 computed  exactly, without rounding\n\
         \x20 concept   none in the concept set matches\n"
    );
}

fn scratch_path(name: &str) -> std::path::PathBuf {
    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("calc-plot");
    std::fs::create_dir_all(&directory).expect("scratch directory is created");
    directory.join(name)
}

#[test]
fn plot_writes_a_png_image_and_names_it() {
    let path = scratch_path("square.png");
    let path_text = path.to_str().expect("path is UTF-8");
    let output = calc(&[
        "plot", "x^2", "--output", path_text, "--size", "320x240", "--locale", "en",
    ]);
    assert!(output.status.success());
    assert_eq!(
        standard_output(&output),
        format!("{path_text}, 320 by 240 pixels\n")
    );
    let bytes = std::fs::read(&path).expect("image is written");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn plot_of_a_line_with_three_axis_variables_writes_no_image() {
    let path = scratch_path("too-many.png");
    let path_text = path.to_str().expect("path is UTF-8");
    let _ = std::fs::remove_file(&path);
    let output = calc(&["plot", "x*y*z", "--output", path_text, "--locale", "en"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!path.exists());
}

#[test]
fn plot_of_an_escape_time_line_with_a_fixed_limit_writes_its_image() {
    let path = scratch_path("mandelbrot.png");
    let path_text = path.to_str().expect("path is UTF-8");
    let output = calc(&[
        "plot",
        "escape_time {\"form\":\"quadratic_parameter\"}",
        "--output",
        path_text,
        "--limit",
        "fixed:50",
        "--size",
        "480x360",
        "--locale",
        "en",
    ]);
    assert!(output.status.success());
    let bytes = std::fs::read(&path).expect("image is written");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn plot_of_a_julia_set_line_writes_its_image() {
    let path = scratch_path("julia.png");
    let path_text = path.to_str().expect("path is UTF-8");
    let output = calc(&[
        "plot",
        "escape_time {\"form\":\"quadratic_initial\",\"c\":{\"real\":{\"type\":\"rational\",\"numerator\":\"-4\",\"denominator\":\"5\"},\"imaginary\":{\"type\":\"rational\",\"numerator\":\"39\",\"denominator\":\"250\"}}}",
        "--output",
        path_text,
        "--limit",
        "following:64,16,256",
        "--size",
        "480x360",
        "--locale",
        "en",
    ]);
    assert!(output.status.success());
    let bytes = std::fs::read(&path).expect("image is written");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
}

#[test]
fn a_limit_on_a_line_that_is_not_an_escape_time_line_writes_no_image() {
    let path = scratch_path("limited.png");
    let path_text = path.to_str().expect("path is UTF-8");
    let _ = std::fs::remove_file(&path);
    let output = calc(&[
        "plot", "x^2", "--output", path_text, "--limit", "fixed:50", "--locale", "en",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!path.exists());
}

#[test]
fn plot_without_output_is_a_usage_error() {
    let output = calc(&["plot", "x^2", "--locale", "en"]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn a_sum_of_squares_is_exact_and_exits_zero() {
    let output = calc(&["sum(i^2, i, 1, 10)", "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("value     385"), "{text}");
    assert!(text.contains("exact integer"), "{text}");
}

#[test]
fn a_product_of_two_hundred_whole_numbers_is_an_exact_integer() {
    let output = calc(&["product(k, k, 1, 200)", "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("exact integer"), "{text}");
    assert!(!text.contains("inf"), "{text}");
}

#[test]
fn a_result_that_is_not_a_number_does_not_exit_zero() {
    let output = calc(&["to_f64(0) / to_f64(0)", "--locale", "en"]);

    assert!(!output.status.success());
}

#[test]
fn an_infinite_result_still_exits_zero() {
    let output = calc(&["inf", "--locale", "en"]);

    assert!(output.status.success());
}

#[test]
fn a_finite_result_still_exits_zero() {
    let output = calc(&["2 + 3", "--locale", "en"]);

    assert!(output.status.success());
}

#[test]
fn a_radical_sum_is_written_over_one_denominator() {
    let output = calc(&["(2 + sqrt(2))/4", "--terse", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(standard_output(&output), "(2 + sqrt(2)) / 4 exact\n");
}

#[test]
fn a_radical_sum_takes_the_least_common_denominator() {
    let output = calc(&["1/3 + sqrt(2)/2", "--terse", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(standard_output(&output), "(2 + 3 * sqrt(2)) / 6 exact\n");
}

#[test]
fn a_factor_of_one_is_not_written() {
    let output = calc(&["1 * ln(10)", "--terse", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(standard_output(&output), "ln(10) exact\n");
}

#[test]
fn a_negated_product_reads_the_same_by_either_route() {
    let built = calc(&["-1 * (2 * ln(10))", "--terse", "--locale", "en"]);
    let written = calc(&["-(2 * ln(10))", "--terse", "--locale", "en"]);
    assert!(built.status.success() && written.status.success());
    assert_eq!(
        (standard_output(&built), standard_output(&written)),
        (
            "-2 * ln(10) exact\n".to_owned(),
            "-2 * ln(10) exact\n".to_owned()
        )
    );
}

#[test]
fn a_plain_rational_keeps_the_form_its_own_rule_gives_it() {
    let output = calc(&["1.99/0.5", "--terse", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(standard_output(&output), "3.98 exact\n");
}

#[test]
fn an_arctangent_of_two_arguments_is_answered_exactly() {
    let output = calc(&["atan2(1, 2)", "--terse", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(standard_output(&output), "atan2(1, 2) exact\n");
}

#[test]
fn an_arctangent_of_two_arguments_encloses_a_second_quadrant_angle() {
    let output = calc(&["atan2(1, -1)", "--enclose", "12", "--locale", "en"]);
    let text = standard_output(&output);
    assert!(output.status.success());
    assert!(text.contains("2.35619449"), "{text}");
}

#[test]
fn the_imaginary_unit_is_answered_exactly() {
    let output = calc(&["i", "--terse", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(standard_output(&output), "i exact\n");
}

#[test]
fn a_complex_value_built_from_exact_parts_is_answered_exactly() {
    let output = calc(&["i * sqrt(4)", "--terse", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(standard_output(&output), "2 * i exact\n");
}

#[test]
fn a_complex_value_with_a_machine_part_is_answered_as_a_machine_value() {
    let output = calc(&["i * to_f64(sqrt(2))", "--terse", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(
        standard_output(&output),
        "0 + 1.4142135623730951 * i machine\n"
    );
}

#[test]
fn a_part_asked_for_in_a_machine_width_stays_a_machine_value() {
    let output = calc(&["to_f64(1) * i", "--terse", "--locale", "en"]);
    assert!(output.status.success());
    assert_eq!(standard_output(&output), "0 + 1 * i machine\n");
}

#[test]
fn an_imaginary_part_written_as_a_decimal_is_answered_as_a_decimal() {
    let output = calc(&["i * 0.5", "--terse", "--locale", "en"]);
    assert_eq!(standard_output(&output), "0.5 * i exact\n");
}

#[test]
fn a_root_of_a_negative_number_names_the_complex_root() {
    let output = calc(&["sqrt(-4)", "--terse", "--locale", "en"]);
    assert_eq!(
        standard_output(&output),
        "error sqrt needs a number of at least 0, and -4 is below 0; \
         for the complex root write i * sqrt(4)\n"
    );
}

#[test]
fn a_root_of_a_negative_quantity_is_refused_without_a_complex_suggestion() {
    let output = calc(&["sqrt(-4 m^2)", "--terse", "--locale", "en"]);
    assert_eq!(
        standard_output(&output),
        "error sqrt needs a number of at least 0, and -4 is below 0\n"
    );
}

#[test]
fn a_logarithm_of_a_negative_number_names_the_complex_logarithm() {
    let output = calc(&["ln(-4)", "--terse", "--locale", "en"]);
    assert_eq!(
        standard_output(&output),
        "error ln needs a number above 0, and -4 is not above 0; \
         for the complex logarithm write ln(4) + i*pi\n"
    );
}

#[test]
fn a_logarithm_of_zero_is_refused_without_a_complex_suggestion() {
    let output = calc(&["ln(0)", "--terse", "--locale", "en"]);
    assert_eq!(
        standard_output(&output),
        "error ln needs a number above 0, and 0 is not above 0\n"
    );
}

#[test]
fn a_line_in_a_free_name_answers_its_normal_form_and_names_the_name() {
    let output = calc(&["(x+1)^2", "--locale", "en"]);
    assert!(output.status.success());
    let shown = standard_output(&output);
    assert!(shown.contains("  value     x^2 + 2 * x + 1\n"), "{shown}");
    assert!(
        shown.contains("  notes     written in the free names x, which no line gives a value\n"),
        "{shown}"
    );
}

#[test]
fn a_named_polynomial_carries_its_free_name_into_a_derivative() {
    let mut child = Command::new(CALC)
        .args(["--batch", "--terse", "--locale", "en"])
        .env_clear()
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("calc runs");
    use std::io::Write as _;
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"p = x^2 - 5*x + 6\nq = p*(x+1)\ndiff(q, x)\n")
        .expect("written");
    let output = child.wait_with_output().expect("calc ends");
    assert_eq!(
        standard_output(&output),
        "x^2 - 5 * x + 6 exact\nx^3 - 4 * x^2 + x + 6 exact\n3 * x^2 - 8 * x + 1 exact\n"
    );
}

#[test]
fn a_view_of_a_line_holding_a_free_name_names_the_free_name() {
    for flag in [
        ["--digits", "5"].as_slice(),
        &["--enclose", "5"],
        &["--working"],
    ] {
        let mut arguments = vec!["(x+1)^2"];
        arguments.extend_from_slice(flag);
        arguments.extend_from_slice(&["--locale", "en"]);
        let output = calc(&arguments);
        assert!(!output.status.success(), "{flag:?}");
        let shown = String::from_utf8(output.stderr.clone()).unwrap_or_default()
            + &standard_output(&output);
        assert!(
            shown.contains("r1 holds the free name x and so has no single value to show this way"),
            "{flag:?}: {shown}"
        );
    }
}

#[test]
fn a_real_root_is_an_exact_number_written_as_it_was_entered() {
    let output = calc(&["rootof(x^3 - 2, x, 1)", "--terse", "--locale", "en"]);
    assert_eq!(standard_output(&output), "rootof(x^3 - 2, x, 1) exact\n");
}

#[test]
fn a_rational_real_root_is_answered_as_the_rational_it_is() {
    let output = calc(&["rootof(x^2 - 5*x + 6, x, 2)", "--terse", "--locale", "en"]);
    assert_eq!(standard_output(&output), "3 exact\n");
}

#[test]
fn the_digits_of_a_real_root_come_from_its_isolating_interval() {
    let output = calc(&["rootof(x^3 - 2, x, 1)", "--digits", "30", "--locale", "en"]);
    assert!(
        standard_output(&output).contains("1.259921049894873164767210607278"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn a_real_root_is_compared_exactly_with_a_decimal() {
    let output = calc(&["--check", "rootof(x^3 - 2, x, 1) > 1.25", "--locale", "en"]);
    assert_eq!(
        standard_output(&output),
        "holds  rootof(x^3 - 2, x, 1) > 1.25\n"
    );
    let output = calc(&["--check", "rootof(x^3 - 2, x, 1) > 1.26", "--locale", "en"]);
    assert_eq!(
        standard_output(&output),
        "FAILS  rootof(x^3 - 2, x, 1) > 1.26\n"
    );
}

#[test]
fn a_root_past_the_last_real_one_is_refused_with_the_count() {
    let output = calc(&["rootof(x^2 - 2, x, 3)", "--locale", "en"]);
    assert!(standard_output(&output).contains(
        "this polynomial has 2 real roots, so the third argument of rootof runs from 1 to 2"
    ));
}

#[test]
fn a_root_of_a_body_that_is_not_a_polynomial_is_refused() {
    let output = calc(&["rootof(sin(x), x, 1)", "--locale", "en"]);
    assert!(
        standard_output(&output)
            .contains("a root is taken here only where the body is a polynomial")
    );
}

#[test]
fn a_function_of_a_written_fraction_keeps_the_fraction() {
    let output = calc(&["atan(1/2)", "--locale", "en"]);

    assert!(standard_output(&output).contains("value           atan(1 / 2)\n"));
}

#[test]
fn a_function_of_a_written_decimal_keeps_the_decimal() {
    let output = calc(&["atan(0.5)", "--locale", "en"]);

    assert!(standard_output(&output).contains("value           atan(0.5)\n"));
}

#[test]
fn a_view_of_a_refused_line_gives_the_line_its_own_reason() {
    let output = calc(&[
        "rootof(2*x^8+16*x^7+18*x^6+16*x^5-10*x^4+10*x^3+9*x^2-3*x-3, x, 3)",
        "--digits",
        "30",
        "--locale",
        "en",
    ]);

    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).expect("output is UTF-8");
    assert!(
        error.contains("this polynomial has 2 real roots"),
        "{error}"
    );
}

#[test]
fn an_enclosure_of_a_division_by_zero_says_it_divides_by_zero() {
    let output = calc(&["1/0", "--enclose", "10", "--locale", "en"]);

    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).expect("output is UTF-8");
    assert!(error.contains("divides by zero"), "{error}");
}

#[test]
fn one_plus_a_percentage_has_one_reading_and_is_answered() {
    let output = calc(&["100 * (1 + 19 %)", "--locale", "en"]);

    assert!(output.status.success(), "{}", standard_error(&output));
    assert!(standard_output(&output).contains("value     119\n"));
}

#[test]
fn a_percentage_added_to_another_number_names_both_readings() {
    let output = calc(&["100 + 19 %", "--locale", "en"]);

    assert!(!output.status.success());
    assert!(standard_error(&output).contains("can mean"));
}

#[test]
fn a_recurring_value_of_a_decimal_question_keeps_its_reading() {
    let output = calc(&["0.1 / 3", "--locale", "en"]);

    assert!(standard_output(&output).contains("value           0.03333\n"));
}

#[test]
fn a_difference_is_not_taken_as_a_reading() {
    let output = calc(&["to_celsius(1 degF)", "--locale", "en"]);

    assert!(!output.status.success());
    assert!(
        standard_output(&output).contains("to_celsius(from_fahrenheit(1))"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn a_reading_below_absolute_zero_is_refused() {
    let output = calc(&["from_celsius(-300)", "--locale", "en"]);

    assert!(!output.status.success());
    assert!(standard_output(&output).contains("below absolute zero, -273.15 °C"));
}

#[test]
fn a_converted_difference_advises_a_reading_that_works() {
    let output = calc(&["1 degF -> degC", "--locale", "en"]);

    assert!(
        standard_output(&output).contains("for a reading write to_celsius(from_fahrenheit(1))")
    );
}

#[test]
fn a_reading_plus_a_difference_is_a_reading_in_kelvin() {
    let output = calc(&["from_celsius(20) + 8 degC", "--locale", "en"]);
    let shown = standard_output(&output);

    assert!(shown.contains("value     301.15 K\n"), "{shown}");
    assert!(!shown.contains("rounded"), "{shown}");
    assert!(!shown.contains("temperature difference"), "{shown}");
}

#[test]
fn a_reading_on_a_scale_says_so() {
    let output = calc(&["to_celsius(from_fahrenheit(1))", "--locale", "en"]);

    assert!(standard_output(&output).contains("this is a reading on the Celsius scale"));
}

#[test]
fn a_conversion_inside_an_exact_line_is_named_with_its_rounding() {
    let output = calc(&["to_f64(0.1) + 1/3", "--locale", "en"]);
    let shown = standard_output(&output);

    assert!(
        shown.contains(
            "computed        exactly, after rounding each to_f64 or to_f32 argument to a machine value"
        ),
        "{shown}"
    );
    assert!(
        shown.contains("to_f64(0.1) rounds 0.1 to the machine value"),
        "{shown}"
    );
    assert!(!shown.contains("without rounding"), "{shown}");
}

#[test]
fn a_square_root_that_denests_is_answered_in_the_field() {
    assert_eq!(terse("sqrt(17 + 12*sqrt(2))"), "3 + 2 * sqrt(2) exact\n");
    assert_eq!(
        terse("sqrt(2 + sqrt(3))"),
        "(sqrt(2) + sqrt(6)) / 2 exact\n"
    );
}

#[test]
fn a_square_root_that_does_not_denest_is_kept_as_written() {
    assert_eq!(terse("sqrt(1 + sqrt(2))"), "sqrt(1 + sqrt(2)) exact\n");
}

#[test]
fn an_algebraic_value_is_taken_apart() {
    assert_eq!(terse("rational_part(17 + 12*sqrt(2))"), "17 exact\n");
    assert_eq!(
        terse("coefficient_of(17 + 12*sqrt(2), sqrt(2))"),
        "12 exact\n"
    );
}

#[test]
fn a_multiple_of_a_square_root_is_refused_as_a_term_with_the_root_to_ask_for() {
    let output = calc(&["coefficient_of(17, sqrt(8))", "--terse", "--locale", "en"]);

    assert!(!output.status.success());
    assert!(standard_output(&output).contains("so ask for sqrt(2)"));
}

#[test]
fn a_denested_equality_holds_under_both_claim_options() {
    let checked = calc(&[
        "sqrt(17 + 12*sqrt(2)) = 3 + 2*sqrt(2)",
        "--check",
        "--terse",
        "--locale",
        "en",
    ]);
    let identity = calc(&[
        "--identity",
        "sqrt(17 + 12*sqrt(2)) = 3 + 2*sqrt(2)",
        "--terse",
        "--locale",
        "en",
    ]);

    assert!(standard_output(&checked).starts_with("holds"));
    assert!(standard_output(&identity).starts_with("holds"));
}

#[test]
fn a_machine_number_is_refused_as_a_value_to_take_apart() {
    for line in [
        "rational_part(f64'1.5')",
        "rational_part(to_f64(1.5))",
        "coefficient_of(to_f64(sqrt(2)), sqrt(2))",
    ] {
        let output = calc(&[line, "--terse", "--locale", "en"]);

        assert!(!output.status.success(), "{line}");
        assert!(
            standard_output(&output).contains("is not one"),
            "{line}: {}",
            standard_output(&output)
        );
    }
}

#[test]
fn a_free_name_is_refused_as_a_value_to_take_apart() {
    for line in [
        "rational_part(x)",
        "rational_part(x + 1)",
        "coefficient_of(x + sqrt(2), sqrt(2))",
    ] {
        let output = calc(&[line, "--terse", "--locale", "en"]);

        assert!(!output.status.success(), "{line}");
        assert!(
            standard_output(&output).contains("is not one"),
            "{line}: {}",
            standard_output(&output)
        );
    }
}

#[test]
fn arithmetic_on_exact_complex_numbers_is_exact() {
    assert_eq!(terse("i*i"), "-1 exact\n");
    assert_eq!(terse("(1+i)^2"), "2 * i exact\n");
    assert_eq!(terse("(1+2*i)/(3-i)"), "1/10 + 7/10 * i exact\n");
    assert_eq!(terse("(1+i)*(1-i)"), "2 exact\n");
    assert_eq!(terse("complex(1,2) * complex(3,-1)"), "5 + 5 * i exact\n");
}

#[test]
fn a_complex_value_with_a_square_root_part_is_written_with_times_i() {
    assert_eq!(terse("i*sqrt(2)"), "sqrt(2) * i exact\n");
}

#[test]
fn a_remainder_of_decimals_is_exact_and_written_as_a_decimal() {
    assert_eq!(terse("mod(7.5, 2)"), "1.5 exact\n");
    assert_eq!(terse("mod(7, 2.5)"), "2 exact\n");
}

#[test]
fn a_remainder_calc_does_not_compute_says_what_it_takes() {
    let output = calc(&["mod(pi, 1)", "--terse", "--locale", "en"]);

    assert!(!output.status.success());
    assert!(
        standard_output(&output)
            .contains("only where both arguments are rational numbers or square roots of them")
    );
}

#[test]
fn the_modulus_and_parts_of_an_exact_complex_number_are_exact() {
    assert_eq!(terse("abs(3+4*i)"), "5 exact\n");
    assert_eq!(terse("re(3+4*i)"), "3 exact\n");
    assert_eq!(terse("im(3+4*i)"), "4 exact\n");
    assert_eq!(terse("conj(3+4*i)"), "3 - 4 * i exact\n");
    assert_eq!(terse("im(3)"), "0 exact\n");
}

#[test]
fn a_part_of_a_value_holding_i_outside_the_exact_route_is_not_taken_as_real() {
    let part = calc(&["im(pi*i)", "--terse", "--locale", "en"]);
    assert_ne!(standard_output(&part), "0 exact\n");
    let output = calc(&["--identity", "im(x*i) = 0", "--locale", "en"]);
    assert!(!standard_output(&output).starts_with("holds"));
}

#[test]
fn a_negative_base_to_an_irrational_power_is_refused_and_has_no_imaginary_part_of_zero() {
    let output = calc(&["im((-2)^pi)", "--terse", "--locale", "en"]);

    assert!(!output.status.success());
}

#[test]
fn a_negative_constant_base_to_an_irrational_power_is_refused() {
    for line in [
        "im((-pi)^e)",
        "im((-e)^pi)",
        "im((1-e)^pi)",
        "im((1-e)^(1/2))",
    ] {
        let output = calc(&[line, "--terse", "--locale", "en"]);
        assert!(!output.status.success(), "{line}");
    }
}

#[test]
fn a_complex_value_with_a_unit_stays_exact() {
    assert_eq!(terse("(1+2*i) m + (3 - i) m"), "(4 + i) m exact\n");
}

#[test]
fn an_expansion_with_i_uses_its_square() {
    let output = calc(&["--expand", "(1+i)^3", "--locale", "en"]);

    assert_eq!(standard_output(&output), "2 * i - 2\n");
}

#[test]
fn zero_to_an_exponent_proven_positive_only_past_its_first_enclosure_is_zero() {
    assert_eq!(
        terse("0^(e - 2.718281828459045235360287471352662497757)"),
        "0 exact\n"
    );
}

#[test]
fn zero_to_an_exponent_of_undecided_sign_is_refused_and_never_one() {
    let output = calc(&["0^(e - e)", "--terse", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        standard_output(&output)
            .contains("calc could prove the exponent neither above 0 nor below 0")
    );
}
