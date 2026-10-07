use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");
const ORACLE: &str = include_str!("fixtures/chemistry_oracle.tsv");
const NOT_UNIQUE: &str = "not unique";

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn terse(expression: &str) -> String {
    terse_in(expression, "en")
}

fn terse_in(expression: &str, locale: &str) -> String {
    let output = calc(&[expression, "--terse", "--locale", locale]);
    let mut shown = String::from_utf8(output.stdout).expect("output is UTF-8");
    shown.push_str(&String::from_utf8(output.stderr).expect("output is UTF-8"));
    shown
}

fn full(expression: &str) -> String {
    let output = calc(&[expression, "--locale", "en"]);
    String::from_utf8(output.stdout).expect("output is UTF-8")
}

#[test]
fn every_case_of_the_independent_oracle_is_answered_as_it_expects() {
    for line in ORACLE.lines().skip(1) {
        let [case, input, expected] = line.split('\t').collect::<Vec<_>>()[..] else {
            panic!("malformed oracle line {line}");
        };
        let shown = terse(input);
        if expected == NOT_UNIQUE {
            assert!(shown.contains("has no single balance"), "{case}: {shown}");
        } else {
            assert_eq!(shown.trim_end(), expected, "{case}");
        }
    }
}

#[test]
fn a_balanced_reaction_carries_its_count_on_each_side() {
    let shown = full("chem'C3H8 + O2 -> CO2 + H2O'");
    assert!(shown.contains("C3H8 + 5 O2 -> 3 CO2 + 4 H2O"), "{shown}");
    assert!(shown.contains("each side holds H 8, C 3, O 10"), "{shown}");
}

#[test]
fn a_redox_between_ions_balances_through_the_charge() {
    let shown = full("chem'MnO4^- + Fe^2+ + H^+ -> Mn^2+ + Fe^3+ + H2O'");
    assert!(
        shown.contains("MnO4^- + 5 Fe^2+ + 8 H^+ -> Mn^2+ + 5 Fe^3+ + 4 H2O"),
        "{shown}"
    );
    assert!(shown.contains("the charge on each side is 17+"), "{shown}");
}

#[test]
fn a_reaction_that_is_not_unique_names_its_smallest_reactions_and_no_single_answer() {
    let shown = terse("chem'KMnO4 + H2O2 + H2SO4 -> K2SO4 + MnSO4 + O2 + H2O'");
    assert!(shown.contains("2 independent reactions"), "{shown}");
    assert!(shown.contains("2 H2O2 -> O2 + 2 H2O"), "{shown}");
    assert!(!shown.contains("exact"), "{shown}");
}

#[test]
fn a_reaction_that_cannot_be_balanced_is_refused_saying_so() {
    assert!(terse("chem'FeS + H2SO4 -> FeSO4 + H2O'").contains("cannot be balanced"));
}

#[test]
fn a_species_that_takes_no_part_or_belongs_on_the_other_side_is_named() {
    assert!(terse("chem'NaCl + H2O -> NaOH + HCl + O2'").contains("O2 cannot take part"));
    assert!(terse("chem'C3H8 + O2 + H2O -> CO2'").contains("H2O can take part"));
}

#[test]
fn written_coefficients_are_checked_with_a_left_out_one_counting_as_one() {
    assert_eq!(
        terse("chem'CH4 + 2 O2 -> CO2 + 2 H2O'"),
        "CH4 + 2 O2 -> CO2 + 2 H2O exact\n"
    );
    assert!(terse("chem'2 H2 + O2 -> H2O'").contains("H counts 4 on the left and 2 on the right"));
}

#[test]
fn a_substance_alone_answers_its_composition_and_charge() {
    assert_eq!(terse("chem'Ca(OH)2'"), "1 Ca, 2 O, 2 H exact\n");
    assert!(full("chem'SO4^2-'").contains("charge 2-"));
}

#[test]
fn a_molar_mass_names_its_tables_and_the_kinds_of_its_ends() {
    let shown = full("molar_mass(chem'NaCl')");
    assert!(shown.contains("CIAAW 2024"), "{shown}");
    assert!(shown.contains("CODATA 2022"), "{shown}");
    assert!(
        shown.contains("natural variation that CIAAW gives: Cl"),
        "{shown}"
    );
    assert!(shown.contains("not a standard uncertainty: Na"), "{shown}");
    let value = shown
        .lines()
        .find(|line| line.trim_start().starts_with("value"))
        .expect("a value line");
    assert!(!value.contains('±'), "{value}");
}

#[test]
fn a_molar_mass_is_used_as_a_range_in_arithmetic() {
    let shown = terse("10 g / molar_mass(chem'H2O') -> mol");
    assert!(shown.starts_with("between("), "{shown}");
    assert!(shown.contains("mol)"), "{shown}");
}

#[test]
fn a_substance_used_as_a_number_is_refused_with_its_property() {
    assert!(terse("2 * chem'H2O'").contains(
        "is a substance, not a number; ask for one of its properties, as molar_mass(chem'H2O')"
    ));
    assert!(terse("chem'H2 + O2 -> H2O' + 1").contains("stands on a line of its own"));
}

#[test]
fn an_ambiguous_formula_is_refused_naming_what_to_write() {
    let shown = terse("chem'NH4+'");
    assert!(shown.contains("NH4+ does not say"), "{shown}");
    assert!(shown.contains("NH4^+"), "{shown}");
}

#[test]
fn a_nuclide_is_refused_as_not_read_yet() {
    assert!(terse("chem'e+'").contains("is a nuclide"));
}

#[test]
fn an_element_without_a_standard_atomic_weight_has_no_molar_mass() {
    assert!(terse("molar_mass(chem'Tc')").contains("Tc has no standard atomic weight"));
}

#[test]
fn molar_mass_is_a_reserved_name() {
    assert!(terse("molar_mass = 2").contains("reserved name"));
}

#[test]
fn a_chemistry_refusal_speaks_german_with_du() {
    let shown = terse_in("chem'NH4+'", "de");
    assert!(shown.contains("schreib die Ladung nach ^"), "{shown}");
}

#[test]
fn a_reaction_with_no_balance_on_its_written_sides_says_so() {
    let shown = terse("chem'O2 + O3 -> H2O + H2O2'");
    assert!(
        shown.contains("has no reaction with these sides"),
        "{shown}"
    );
}

#[test]
fn a_species_written_twice_is_refused_by_name() {
    assert!(terse("chem'H2 + H2 -> H4'").contains("H2 is written more than once"));
}

#[test]
fn a_number_before_the_literal_is_named_as_a_coefficient_outside_it() {
    assert!(terse("2 chem'H2O'").contains("a coefficient outside the formula"));
}

#[test]
fn an_oxidation_state_refusal_names_no_element_it_does_not_hold() {
    let shown = terse("chem'Fe(III)'");
    assert!(shown.contains("oxidation state"), "{shown}");
    assert!(!shown.contains("uranium"), "{shown}");
}

#[test]
fn an_ion_with_its_state_is_read() {
    assert_eq!(terse("chem'Na+(aq)'"), "1 Na exact\n");
}

#[test]
fn a_coefficient_of_zero_is_named_as_a_coefficient_and_not_a_letter() {
    let shown = terse("chem'0 H2 + O2 -> O2'");
    assert!(shown.contains("a coefficient of 0"), "{shown}");
    assert!(!shown.contains("letter O"), "{shown}");
}

#[test]
fn a_charge_of_zero_written_with_a_sign_is_refused() {
    assert!(terse("chem'Fe^0+'").contains("a charge of 0"));
}

#[test]
fn a_count_too_large_to_read_is_refused_and_not_dropped() {
    let shown = terse("chem'H99999999999999999999999'");
    assert!(shown.contains("above 9223372036854775807"), "{shown}");
    let checked = terse("chem'99999999999999999999999 H2 + O2 -> H2O'");
    assert!(checked.contains("above 9223372036854775807"), "{checked}");
}

#[test]
fn a_molar_mass_says_in_its_number_row_that_it_is_a_range() {
    let shown = full("molar_mass(chem'H2O')");
    assert!(
        shown.contains("number    a range holding every value its inputs' tolerances allow; both ends are exact"),
        "{shown}"
    );
}
