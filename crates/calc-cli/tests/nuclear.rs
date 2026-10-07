use std::cmp::Ordering;
use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");
const ORACLE: &str = include_str!("fixtures/nuclear_oracle.tsv");
const RANGE_START: &str = "between(";
const KILO_ELECTRON_VOLT: &str = " keV";

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn shown_in(expression: &str, locale: &str, terse: bool) -> String {
    let mut arguments = vec![expression, "--locale", locale];
    if terse {
        arguments.push("--terse");
    }
    let output = calc(&arguments);
    let mut shown = String::from_utf8(output.stdout).expect("output is UTF-8");
    shown.push_str(&String::from_utf8(output.stderr).expect("output is UTF-8"));
    shown
}

fn terse(expression: &str) -> String {
    shown_in(expression, "en", true)
}

fn full(expression: &str) -> String {
    shown_in(expression, "en", false)
}

fn scaled(decimal: &str, places: usize) -> i128 {
    let (is_negative, digits) = match decimal.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, decimal),
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let padded = format!("{whole}{fraction:0<places$}");
    let magnitude: i128 = padded.parse().expect("a decimal");
    if is_negative { -magnitude } else { magnitude }
}

fn compare(left: &str, right: &str) -> Ordering {
    let places = |value: &str| {
        value
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len())
    };
    let common = places(left).max(places(right));
    scaled(left, common).cmp(&scaled(right, common))
}

fn range_of(shown: &str) -> (String, String) {
    let start = shown.find(RANGE_START).expect("a range") + RANGE_START.len();
    let inside = &shown[start..shown[start..].find(')').expect("a closed range") + start];
    let (low, high) = inside.split_once(", ").expect("two ends");
    (
        low.trim_end_matches(KILO_ELECTRON_VOLT).to_string(),
        high.trim_end_matches(KILO_ELECTRON_VOLT).to_string(),
    )
}

fn q_value_of(reaction: &str) -> String {
    terse(&format!("q_value({reaction})"))
}

#[test]
fn every_q_value_of_the_independent_oracle_lies_in_calcs_range() {
    for line in ORACLE.lines().skip(1) {
        let [case, input, published, expected] = line.split('\t').collect::<Vec<_>>()[..] else {
            panic!("malformed oracle line {line}");
        };
        if expected != "contains" {
            continue;
        }
        let checked = terse(input);
        assert!(!checked.starts_with("error"), "{case}: {checked}");
        assert!(!checked.contains("does not balance"), "{case}: {checked}");
        let shown = q_value_of(input);
        let (low, high) = range_of(&shown);
        assert_ne!(
            compare(&low, published),
            Ordering::Greater,
            "{case}: {shown}"
        );
        assert_ne!(compare(&high, published), Ordering::Less, "{case}: {shown}");
    }
}

#[test]
fn every_refusal_of_the_oracle_names_the_law_it_breaks() {
    for line in ORACLE.lines().skip(1) {
        let [case, input, _, expected] = line.split('\t').collect::<Vec<_>>()[..] else {
            panic!("malformed oracle line {line}");
        };
        let shown = terse(input);
        match expected {
            "lepton" => assert!(
                shown.contains("electron lepton number is"),
                "{case}: {shown}"
            ),
            "charge" => assert!(shown.contains("the charge is"), "{case}: {shown}"),
            "mass number" => assert!(shown.contains("the mass number is"), "{case}: {shown}"),
            "impossible" => {
                let noted = full(&format!("q_value({input})"));
                assert!(
                    noted.contains("cannot happen for the neutral atom"),
                    "{case}: {noted}"
                );
            }
            _ => {}
        }
    }
}

#[test]
fn a_decay_without_its_antineutrino_meets_it_by_name() {
    let shown = terse("nuc'^14C -> ^14N + e-'");
    assert!(shown.contains("mass number and charge do"), "{shown}");
    assert!(
        shown.contains("ν̄_e on the right would balance it"),
        "{shown}"
    );
}

#[test]
fn a_line_that_fails_as_written_stays_a_failure_and_names_its_only_balance() {
    let shown = terse("nuc'p -> ^4He + e+ + ν_e'");
    assert!(
        shown.contains("the mass number is 1 on the left and 4 on the right"),
        "{shown}"
    );
    assert!(
        shown.contains("as arithmetic only, the one set of coefficients that balances these particles is 4 p -> ^4He + 2 e+ + 2 ν_e, which does not say that such a reaction happens"),
        "{shown}"
    );
    let absurd = terse("nuc'^238U -> ^234Th + ^3He'");
    assert!(absurd.starts_with("error"), "{absurd}");
    assert!(
        absurd.contains("the mass number is 238 on the left and 237 on the right"),
        "{absurd}"
    );
}

#[test]
fn a_nuclide_alone_answers_what_calc_read_and_whether_its_mass_was_measured() {
    let shown = full("nuc'C-14'");
    assert!(shown.contains("^14C: A 14, Z 6, N 8"), "{shown}");
    assert!(shown.contains("measured atomic mass"), "{shown}");
    assert!(full("nuc'^3Li'").contains("only an estimated mass"));
}

#[test]
fn a_q_value_names_its_tables_and_its_convention() {
    let shown = full("q_value(nuc'^18F -> ^18O + e+ + ν_e')");
    assert!(shown.contains("atomic masses AME2020"), "{shown}");
    assert!(shown.contains("an atomic-mass Q value"), "{shown}");
    assert!(shown.contains("before the positron annihilates"), "{shown}");
    let value = shown
        .lines()
        .find(|line| line.contains("value"))
        .expect("a value line");
    assert!(!value.contains('±'), "{shown}");
    assert!(
        shown.contains("number    a range holding every value its inputs' tolerances allow"),
        "{shown}"
    );
}

#[test]
fn a_sign_the_carried_masses_cannot_decide_is_said_to_be_undecided() {
    let shown = full("q_value(nuc'^163Dy -> ^163Ho + e- + ν̄_e')");
    assert!(shown.contains("not decided at this coverage"), "{shown}");
}

#[test]
fn a_nuclide_whose_mass_is_only_an_estimate_has_no_q_value() {
    let shown = q_value_of("nuc'^3Li -> ^3He + e+ + ν_e'");
    assert!(shown.contains("only an estimated mass"), "{shown}");
}

#[test]
fn nuclide_notation_in_chemistry_points_at_nuc() {
    assert!(terse("chem'^14C'").contains("nuc'"));
    assert!(terse("chem'e^+'").contains("nuc'"));
}

#[test]
fn a_nuclear_line_used_as_a_number_is_refused() {
    assert!(terse("2 * nuc'^14C'").contains("is a nuclide, not a number"));
    assert!(terse("1 + nuc'p -> n + e+ + ν_e'").contains("stands on a line of its own"));
}

#[test]
fn the_german_messages_address_the_reader_as_du() {
    let shown = shown_in("nuc'D'", "de", true);
    assert!(shown.contains("schreib d oder ^2H"), "{shown}");
}

#[test]
fn an_atomic_number_that_disagrees_is_named_with_the_symbols_own() {
    let shown = terse("nuc'^14_7C'");
    assert!(
        shown.contains("^14_7C writes the atomic number 7, and C has the atomic number 6"),
        "{shown}"
    );
}

#[test]
fn a_number_attached_to_a_species_names_the_whole_species() {
    let shown = terse("chem'2H2O'");
    assert!(
        shown.contains("2H2O starts with a number attached to it"),
        "{shown}"
    );
}
