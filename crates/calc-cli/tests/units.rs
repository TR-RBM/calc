use std::path::PathBuf;
use std::process::{Command, Output};

use calc_app::{FixedClock, KindUnit, Session, UnitOverride, UtcTimestamp, registered_backends};

const CALC: &str = env!("CARGO_BIN_EXE_calc");
const SPEED: &str = "100 km / 1 h";

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

fn value_row(output: &Output) -> String {
    standard_output(output)
        .lines()
        .find(|row| row.trim_start().starts_with("value"))
        .expect("a value row")
        .to_owned()
}

fn session_file(name: &str, input: &str, unit_override: Option<UnitOverride>) -> PathBuf {
    let directory = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("calc-units");
    std::fs::create_dir_all(&directory).expect("scratch directory is created");
    let clock = FixedClock::new(UtcTimestamp::from_milliseconds_since_unix_epoch(0), 1);
    let mut session = Session::new(Box::new(clock), registered_backends());
    session.enter(input).expect("line is entered");
    if let Some(unit_override) = unit_override {
        session
            .set_unit_override(unit_override)
            .expect("the override is structured");
    }
    let bytes = session.save_to_bytes().expect("session is saved");
    let path = directory.join(name);
    std::fs::write(&path, bytes).expect("session file is written");
    path
}

#[test]
fn a_unit_system_decides_the_unit_a_value_is_shown_in() {
    let output = calc(&[SPEED, "--units", "us-customary", "--locale", "en"]);

    assert!(output.status.success());
    assert!(
        value_row(&output).contains("mi/h"),
        "{}",
        value_row(&output)
    );
}

#[test]
fn a_unit_for_one_kind_decides_that_kind_alone() {
    let output = calc(&[SPEED, "--unit", "speed=mi/h", "--locale", "en"]);

    assert!(output.status.success());
    assert!(
        value_row(&output).contains("mi/h"),
        "{}",
        value_row(&output)
    );
}

#[test]
fn coherent_units_show_the_coherent_unit() {
    let output = calc(&[SPEED, "--coherent-units", "--locale", "en"]);

    assert!(output.status.success());
    assert!(value_row(&output).contains("m/s"), "{}", value_row(&output));
}

#[test]
fn a_session_override_wins_over_the_option() {
    let path = session_file(
        "override.calc",
        SPEED,
        Some(UnitOverride {
            system: None,
            overrides: vec![KindUnit {
                kind: "speed".to_owned(),
                unit: "mi/h".to_owned(),
            }],
        }),
    );
    let output = calc(&[
        path.to_str().expect("path is UTF-8"),
        "--unit",
        "speed=m/s",
        "--locale",
        "en",
    ]);

    assert!(output.status.success());
    assert!(
        value_row(&output).contains("mi/h"),
        "{}",
        value_row(&output)
    );
}

#[test]
fn the_json_form_is_the_same_with_and_without_the_options() {
    let path = session_file("json.calc", SPEED, None);
    let path = path.to_str().expect("path is UTF-8");
    let plain = calc(&[path, "--json"]);
    let chosen = calc(&[path, "--json", "--units", "us-customary"]);

    assert!(plain.status.success() && chosen.status.success());
    assert_eq!(plain.stdout, chosen.stdout);
}

#[test]
fn an_unknown_unit_system_is_a_usage_error_naming_the_systems() {
    let output = calc(&[SPEED, "--units", "metric-ish", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        standard_error(&output).contains("metric-ish is not a unit system"),
        "{}",
        standard_error(&output)
    );
}

#[test]
fn an_unknown_quantity_kind_is_a_usage_error() {
    let output = calc(&[SPEED, "--unit", "distance=km", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        standard_error(&output).contains("distance is not a kind that --unit can set"),
        "{}",
        standard_error(&output)
    );
}

#[test]
fn a_unit_that_is_not_of_its_kind_is_a_usage_error() {
    let output = calc(&[SPEED, "--unit", "length=kg", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        standard_error(&output).contains("kg is not a unit of length"),
        "{}",
        standard_error(&output)
    );
}

#[test]
fn an_override_without_a_unit_is_a_usage_error() {
    let output = calc(&[SPEED, "--unit", "length", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        standard_error(&output).contains("--unit needs <kind>=<unit>"),
        "{}",
        standard_error(&output)
    );
}

#[test]
fn the_same_kind_twice_is_a_usage_error() {
    let output = calc(&[
        SPEED,
        "--unit",
        "speed=mi/h",
        "--unit",
        "speed=m/s",
        "--locale",
        "en",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        standard_error(&output).contains("more than once"),
        "{}",
        standard_error(&output)
    );
}

#[test]
fn coherent_units_beside_a_chosen_unit_is_a_usage_error() {
    let output = calc(&[
        SPEED,
        "--coherent-units",
        "--unit",
        "speed=mi/h",
        "--locale",
        "en",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        standard_error(&output).contains("--coherent-units"),
        "{}",
        standard_error(&output)
    );
}

#[test]
fn the_help_example_shows_what_that_invocation_prints() {
    let example = calc(&[SPEED, "--units", "us-customary", "--locale", "en"]);
    let page = calc(&["--help", "--locale", "en"]);
    let shown = value_row(&example);
    let printed = shown.split_whitespace().skip(1).collect::<Vec<&str>>();
    let printed = printed[printed.len() - 2..].join(" ");

    assert!(
        standard_output(&page).contains(&printed),
        "the help does not show {printed}"
    );
}

#[test]
fn a_german_run_says_in_german_which_system_it_does_not_know() {
    let output = calc(&[SPEED, "--units", "metric-ish", "--locale", "de"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        standard_error(&output).contains("ist kein Einheitensystem"),
        "{}",
        standard_error(&output)
    );
}

#[test]
fn a_frequency_is_answered_in_the_hertz_that_was_written() {
    let written = calc(&["3 Hz", "--terse", "--locale", "en"]);
    let machine = calc(&["to_f64(3 Hz)", "--terse", "--locale", "en"]);
    let scaled = calc(&["pi * 1 Hz", "--terse", "--locale", "en"]);

    assert_eq!(
        (
            standard_output(&written),
            standard_output(&machine),
            standard_output(&scaled)
        ),
        (
            "3 Hz exact\n".to_owned(),
            "3 Hz machine\n".to_owned(),
            "pi Hz exact\n".to_owned()
        )
    );
}

#[test]
fn an_irradiance_is_answered_in_the_unit_that_was_written() {
    let output = calc(&["pi * 1 W/m^2", "--terse", "--locale", "en"]);

    assert_eq!(standard_output(&output), "pi W/m\u{b2} exact\n");
}

#[test]
fn a_value_with_no_written_unit_of_its_dimension_answers_in_the_coherent_one() {
    let output = calc(&["1/(0.5 s)", "--terse", "--locale", "en"]);

    assert_eq!(standard_output(&output), "2 s\u{207b}\u{b9} exact\n");
}

#[test]
fn a_complex_value_keeps_a_unit_that_needs_no_conversion() {
    let output = calc(&["(i) rad", "--terse", "--locale", "en"]);

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "(0 + 1 * i) rad machine\n");
}

#[test]
fn a_complex_value_keeps_a_unit_of_its_own_dimension() {
    let output = calc(&["(i) m", "--terse", "--locale", "en"]);

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "i m exact\n");
}

#[test]
fn a_complex_value_is_shown_in_the_coherent_unit_where_it_must_be_converted() {
    let output = calc(&["(2*i) km", "--terse", "--locale", "en"]);

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "2000 * i m exact\n");
}

fn terse(expression: &str) -> Output {
    calc(&[expression, "--terse", "--locale", "en"])
}

#[test]
fn decimal_byte_prefixes_add_exactly() {
    assert_eq!(
        standard_output(&terse("1 GB + 500 MB -> MB")),
        "1500 MB exact\n"
    );
}

#[test]
fn a_terabyte_in_gibibytes_is_exact() {
    assert_eq!(
        standard_output(&terse("1 TB -> GiB")),
        "(244140625/262144) GiB exact\n"
    );
}

#[test]
fn a_kibibyte_is_1024_bytes() {
    assert_eq!(standard_output(&terse("1 KiB -> B")), "1024 B exact\n");
}

#[test]
fn a_byte_is_eight_bits() {
    assert_eq!(standard_output(&terse("8 bit -> B")), "1 B exact\n");
}

#[test]
fn a_transfer_time_is_an_exact_duration() {
    assert_eq!(
        standard_output(&terse("1 TB / (100 MB/s) -> h")),
        "(25/9) h exact\n"
    );
}

#[test]
fn an_amount_of_data_does_not_add_to_a_bare_number() {
    let output = terse("1 GB + 3");
    assert!(!output.status.success());
    assert!(standard_output(&output).contains("cannot be added, compared or converted"));
}

#[test]
fn kb_is_refused_as_ambiguous_with_both_readings() {
    let output = terse("1 KB");
    let shown = format!("{}{}", standard_output(&output), standard_error(&output));
    assert!(!output.status.success());
    assert!(shown.contains("write one of kB, KiB"), "{shown}");
}

#[test]
fn b_behind_a_prefix_is_refused_as_bit_or_byte() {
    let output = terse("1 Mb");
    let shown = format!("{}{}", standard_output(&output), standard_error(&output));
    assert!(shown.contains("write one of Mbit, MB"), "{shown}");
}

#[test]
fn a_millibyte_is_not_a_unit() {
    let output = terse("1 mB");
    let shown = format!("{}{}", standard_output(&output), standard_error(&output));
    assert!(shown.contains("mB is not a unit"), "{shown}");
}

#[test]
fn a_unit_symbol_stays_free_as_a_name() {
    assert_eq!(standard_output(&terse("B = 5")), "5 exact\n");
}

#[test]
fn a_mismatch_names_both_quantities_and_asks_about_the_case() {
    let shown = standard_output(&terse("100 T -> kg"));
    assert!(
        shown.contains("T measures magnetic flux density"),
        "{shown}"
    );
    assert!(
        shown.contains("kg measures mass (units: g, kg, lb, oz_av, oz_t, t)"),
        "{shown}"
    );
    assert!(shown.contains("Did you mean t?"), "{shown}");
}

#[test]
fn a_mismatch_with_a_bare_number_says_so() {
    let shown = standard_output(&terse("2 kg + 3"));
    assert!(shown.contains("one side is a pure number"), "{shown}");
    assert!(!shown.contains("Did you mean"), "{shown}");
}

#[test]
fn a_mismatch_is_explained_in_german() {
    let output = calc(&["5 m -> s", "--terse", "--locale", "de"]);
    let shown = standard_output(&output);
    assert!(shown.contains("m misst Länge"), "{shown}");
    assert!(shown.contains("s misst Zeit"), "{shown}");
}

#[test]
fn a_case_variant_already_written_is_not_suggested() {
    let shown = standard_output(&terse("100 t -> T"));
    assert!(!shown.contains("Did you mean"), "{shown}");
}

#[test]
fn a_quotient_of_written_units_answers_in_those_units() {
    let output = calc(&["12 g / (1 mol)", "--terse", "--locale", "en"]);
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8"),
        "12 g/mol exact\n"
    );
    let output = calc(&["5 km / (2 h)", "--terse", "--locale", "en"]);
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8"),
        "(5/2) km/h exact\n"
    );
}

#[test]
fn a_composed_unit_that_repeats_a_quantity_is_not_used() {
    let output = calc(&["5 L/s * 1 h", "--terse", "--locale", "en"]);
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8"),
        "18000 L exact\n"
    );
}

#[test]
fn a_composed_unit_equal_to_the_coherent_one_keeps_its_name() {
    let output = calc(&["2.50 kg * 9.81 m/s^2", "--terse", "--locale", "en"]);
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8"),
        "24.525 N exact\n"
    );
}

#[test]
fn a_chosen_unit_system_wins_over_the_composed_unit() {
    let output = calc(&[
        "100 km / (2 h)",
        "--units",
        "us-customary",
        "--terse",
        "--locale",
        "en",
    ]);
    assert!(
        String::from_utf8(output.stdout)
            .expect("UTF-8")
            .contains("mi/h")
    );
}

#[test]
fn a_composed_unit_whose_coherent_unit_has_no_name_keeps_its_written_units() {
    for (expression, answer) in [
        ("8.314 J / (1 mol * 1 K)", "8.314 J/(K·mol) exact\n"),
        ("1 V / (1 m)", "1 V/m exact\n"),
        ("20 degC / (1 s)", "20 °C/s exact\n"),
    ] {
        let output = calc(&[expression, "--terse", "--locale", "en"]);
        assert_eq!(
            String::from_utf8(output.stdout).expect("UTF-8"),
            answer,
            "{expression}"
        );
    }
}

#[test]
fn a_group_that_opens_with_a_number_ends_the_unit() {
    let output = calc(&["298.15 K/(1 bar)", "--terse", "--locale", "en"]);
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8"),
        "298.15 K/bar exact\n"
    );
}

#[test]
fn a_name_joined_to_a_unit_is_refused_saying_to_write_spaces() {
    let output = calc(&["1 mol*N_A", "--terse", "--locale", "en"]);
    let shown = String::from_utf8(output.stdout).expect("UTF-8")
        + &String::from_utf8(output.stderr).expect("UTF-8");
    assert!(
        shown.contains("put a space on each side of the * or /"),
        "{shown}"
    );
}

#[test]
fn a_value_with_its_unit_takes_an_uncertainty_with_its_unit() {
    for expression in ["1 kg ± 0.1 kg", "1 kg ± 100 g"] {
        let output = calc(&[expression, "--terse", "--locale", "en"]);
        assert!(
            String::from_utf8(output.stdout)
                .expect("UTF-8")
                .starts_with("1.00 kg ± 0.10 kg"),
            "{expression}"
        );
    }
}
