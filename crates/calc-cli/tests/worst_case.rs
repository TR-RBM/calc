use std::io::Write;
use std::process::{Command, Output, Stdio};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str], input: Option<&str>) -> Output {
    let mut child = Command::new(CALC)
        .args(arguments)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("calc runs");
    let mut standard_input = child.stdin.take().expect("standard input");
    if let Some(input) = input {
        standard_input
            .write_all(input.as_bytes())
            .expect("input is written");
    }
    drop(standard_input);
    child.wait_with_output().expect("calc ends")
}

fn terse(expression: &str) -> String {
    let output = calc(&[expression, "--terse", "--locale", "en"], None);
    let mut shown = String::from_utf8(output.stdout).expect("UTF-8");
    shown.push_str(&String::from_utf8(output.stderr).expect("UTF-8"));
    shown
}

fn batch(lines: &str) -> String {
    let output = calc(&["--batch", "--terse", "--locale", "en"], Some(lines));
    String::from_utf8(output.stdout).expect("UTF-8")
}

#[test]
fn a_tolerance_answers_its_range_in_a_form_that_reads_back() {
    assert_eq!(
        terse("tolerance(25 Ω, 10%)"),
        "between((45/2) Ω, (55/2) Ω) range, both ends exact\n"
    );
}

#[test]
fn two_independent_ranges_span_every_combination() {
    assert_eq!(
        terse("between(1, 2) - between(3, 5)"),
        "between(-4, -1) range, both ends exact\n"
    );
}

#[test]
fn a_named_component_is_one_value_wherever_it_is_used() {
    let shown = batch("Rc = tolerance(25 Ω, 10%)\nU = 3.3 V * Rc / (Rc + 2 Ω)\n");
    assert!(
        shown.contains("between((297/98) V, (363/118) V)"),
        "{shown}"
    );
}

#[test]
fn the_relay_cable_length_is_bounded_exactly() {
    let shown = batch(
        "Rc = tolerance(25 Ω, 10%)\n(3.3/2.25 - 1) * Rc * pi * (0.25 mm)^2 / (2 * (1 Ω*mm^2/m) / 58) -> m\n",
    );
    assert!(
        shown.contains("between((609 * pi / 32) m, (2233 * pi / 96) m)"),
        "{shown}"
    );
}

#[test]
fn the_same_tolerance_written_twice_in_a_line_is_refused() {
    let shown = terse("3.3 V * tolerance(25 Ω, 10%) / (tolerance(25 Ω, 10%) + 2 Ω)");
    assert!(shown.contains("more than once"), "{shown}");
}

#[test]
fn a_function_that_turns_inside_its_range_is_refused_not_guessed() {
    let shown = terse("between(-1, 1)^2");
    assert!(
        shown.contains("rather than one that could be wrong"),
        "{shown}"
    );
}

#[test]
fn a_range_with_its_ends_reversed_is_refused() {
    assert!(terse("between(2, 1)").contains("lower end above its upper end"));
}

#[test]
fn two_names_with_the_same_text_are_two_components() {
    let shown = batch("R1 = tolerance(25 Ω, 10%)\nR2 = tolerance(25 Ω, 10%)\nR1 - R2\n");
    assert!(shown.contains("between(-5 Ω, 5 Ω)"), "{shown}");
}

#[test]
fn a_named_range_and_the_same_range_written_again_are_two_components() {
    let shown = batch("R = between(1, 2)\nR - between(1, 2)\n");
    assert!(shown.contains("between(-1, 1)"), "{shown}");
}

#[test]
fn a_name_used_twice_stays_one_component() {
    let shown = batch("R = between(1, 2)\nR - R\n");
    assert!(shown.contains("between(0, 0)"), "{shown}");
}

#[test]
fn a_corner_names_the_component_by_its_name() {
    let output = calc(
        &["--batch", "--locale", "en"],
        Some("R1 = between(1, 2)\nR2 = between(3, 5)\nR1 - R2\n"),
    );
    let shown = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(shown.contains("R1 = 2"), "{shown}");
    assert!(shown.contains("R2 = 3"), "{shown}");
}

#[test]
fn ends_of_two_dimensions_are_refused() {
    assert!(terse("between(1 m, 2 s)").contains("measure different quantities"));
    assert!(terse("between(1, 2 m)").contains("measure different quantities"));
}

#[test]
fn an_angle_range_in_degrees_is_answered() {
    let shown = terse("between(0 deg, 90 deg)");
    assert!(shown.starts_with("between("), "{shown}");
    assert!(!shown.contains("error"), "{shown}");
}

#[test]
fn a_tolerance_on_pi_is_answered() {
    let shown = terse("tolerance(pi, 10%)");
    assert!(shown.starts_with("between("), "{shown}");
}

#[test]
fn an_end_calc_cannot_bound_is_refused_with_that_reason() {
    let shown = terse("tolerance(sqrt(2), 10%)");
    assert!(shown.contains("cannot bound the ends"), "{shown}");
    assert!(!shown.contains("must be exact"), "{shown}");
}

#[test]
fn dividing_by_a_range_that_holds_zero_is_refused() {
    let shown = batch("R = between(-1, 1)\n0 / R\n1 + 0/R\n");
    assert_eq!(
        shown.matches("divides by a value that can be zero").count(),
        2,
        "{shown}"
    );
}

#[test]
fn eight_ranges_are_taken_and_nine_are_refused_naming_the_limit() {
    let eight = (1..=8)
        .map(|index| format!("between({index}, {})", index + 1))
        .collect::<Vec<_>>()
        .join(" + ");
    assert!(terse(&eight).starts_with("between("), "{eight}");
    let nine = format!("{eight} + between(9, 10)");
    assert!(terse(&nine).contains("at most 8 toleranced values"));
}

#[test]
fn a_range_in_a_function_body_is_refused_as_ambiguous() {
    let shown = batch("f(x) = x + between(1, 2)\nf(1) - f(2)\n");
    assert!(
        shown.contains("one part for every call or one part per call"),
        "{shown}"
    );
}

#[test]
fn a_named_range_used_in_a_function_body_is_one_component() {
    let shown = batch("R = between(1, 2)\nf(x) = x + R\nf(1) - f(2)\n");
    assert!(shown.contains("between(-1, -1)"), "{shown}");
}

#[test]
fn an_end_that_cancels_is_written_reduced() {
    let shown = batch("x = between(e, 3)\nx - e\n");
    assert!(shown.contains("between(0, "), "{shown}");
    assert!(!shown.contains("e - e"), "{shown}");
}

fn number_row(expression: &str, locale: &str) -> String {
    let output = calc(&[expression, "--locale", locale], None);
    let shown = String::from_utf8(output.stdout).expect("UTF-8");
    shown
        .lines()
        .nth(2)
        .expect("a number row")
        .trim()
        .to_string()
}

#[test]
fn a_range_says_in_its_number_row_that_it_is_a_range_of_allowed_values() {
    for expression in [
        "between(1, 2)",
        "tolerance(25 Ω, 10%)",
        "2 * between(1, 2) + 1",
    ] {
        assert_eq!(
            number_row(expression, "en"),
            "number    a range holding every value its inputs' tolerances allow; both ends are exact",
            "{expression}"
        );
    }
    assert!(
        number_row("between(1, 2)", "de").ends_with(
            "ein Bereich, der jeden Wert enthält, den die Toleranzen der Eingaben zulassen; beide Enden sind exakt"
        )
    );
}

#[test]
fn the_record_keeps_the_kind_of_a_range_as_symbolic() {
    let output = calc(&["between(1, 2)", "--json"], None);
    let shown = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(shown.contains("\"kind\": \"symbolic\""), "{shown}");
}
