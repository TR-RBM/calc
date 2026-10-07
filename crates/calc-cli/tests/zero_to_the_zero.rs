use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");
const NOTE: &str = "0^0 is taken as 1, the empty product, as the binomial theorem and power series need it; that x^y has no limit as x and y both go to 0 is a different question";

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn shown(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

#[test]
fn zero_to_the_zero_answers_one_with_its_convention() {
    let text = shown(&calc(&["0^0", "--locale", "en"]));

    assert!(text.contains("value     1\n"), "{text}");
    assert!(text.contains(NOTE), "{text}");
}

#[test]
fn a_check_of_zero_to_the_zero_names_the_convention() {
    assert_eq!(
        shown(&calc(&["0^0 = 1", "--check"])),
        format!("holds  0^0 = 1\nnote: {NOTE}\n")
    );
}

#[test]
fn an_identity_whose_base_may_be_zero_names_the_convention() {
    assert_eq!(
        shown(&calc(&["--identity", "x^0 = 1"])),
        format!("holds for every value\nnote: {NOTE}\n")
    );
}

#[test]
fn a_power_whose_base_is_never_zero_carries_no_note() {
    assert!(!shown(&calc(&["--identity", "pi^0 = 1"])).contains("note:"));
}

#[test]
fn a_program_is_given_the_note_by_its_code() {
    assert!(
        shown(&calc(&["0^0 = 1", "--check", "--json"]))
            .contains("\"notes\": [\"zero_to_the_zero\"]")
    );
}
