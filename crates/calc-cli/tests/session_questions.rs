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

fn piped(arguments: &[&str], standard_input: &str) -> Output {
    let mut child = Command::new(CALC)
        .args(arguments)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("calc starts");
    child
        .stdin
        .take()
        .expect("standard input is piped")
        .write_all(standard_input.as_bytes())
        .expect("input is written");
    child.wait_with_output().expect("calc finishes")
}

fn standard_output(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

fn batch(option: &str, lines: &str) -> Output {
    piped(&["--batch", option, "--locale", "en"], lines)
}

const DEFINITION: &str = "p(x) = x^2 - 5*x + 6\n";

#[test]
fn a_definition_reaches_a_claim_about_the_name_it_defines() {
    let output = batch("--identity", &format!("{DEFINITION}p(x) = (x-2)*(x-3)\n"));

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        standard_output(&output),
        "defines  p(x) = x^2 - 5*x + 6\nholds for every value\n"
    );
}

#[test]
fn a_definition_reaches_an_expansion_that_uses_it() {
    let output = batch("--expand", &format!("{DEFINITION}p(x) * (x+1)\n"));

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        standard_output(&output),
        "defines  p(x) = x^2 - 5*x + 6\nx^3 - 4 * x^2 + x + 6\n"
    );
}

#[test]
fn a_definition_reaches_an_equation_written_with_it() {
    let output = batch("--solve-for", &format!("{DEFINITION}p(x) = 0\n"));

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        standard_output(&output),
        "defines  p(x) = x^2 - 5*x + 6\n2, 3\n"
    );
}

#[test]
fn a_reference_reaches_the_line_it_names_under_identity() {
    let output = batch("--identity", "2+2\n4 = r1\n");

    assert_eq!(
        standard_output(&output),
        "not a relation  2+2\nholds, with r1 = 4\n"
    );
}

#[test]
fn a_label_cannot_be_written_as_a_naming_header() {
    let output = batch("--identity", "2+2\nr1 = 4\n");

    assert_eq!(
        standard_output(&output),
        "not a relation  2+2\nunreadable  r1 = 4: r1 is a reserved name at column 1\n"
    );
}

#[test]
fn a_question_the_session_made_nameless_names_the_substitution() {
    let output = batch("--identity", "a = 3\na * 2 = 6\n");

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        standard_output(&output),
        "defines  a = 3\nholds, with a = 3\n"
    );
}

#[test]
fn a_claim_that_was_nameless_as_written_says_there_is_nothing_to_vary() {
    let output = calc(&["--identity", "sqrt(2) = 1.5", "--locale", "en"]);

    assert_eq!(
        standard_output(&output),
        "fails, and there is nothing in it to vary\n"
    );
}

#[test]
fn a_claim_naming_something_with_no_value_is_not_called_not_a_relation() {
    let output = batch("--check", "b + 1 = 4\n");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        standard_output(&output),
        "unknown name  b + 1 = 4: b is not defined\n"
    );
}

#[test]
fn an_unknown_name_carries_its_typed_error_for_a_program() {
    let output = piped(&["--batch", "--check", "--json"], "4 = r9\n");

    assert_eq!(
        standard_output(&output),
        "{\"claim\": \"4 = r9\", \"verdict\": \"unknown_name\", \
         \"error\": { \"code\": \"undefined_name\", \"data\": { \"name\": \"r9\" } }}\n"
    );
}

#[test]
fn a_claim_that_refers_to_its_own_line_says_that_rather_than_varying_the_label() {
    let output = calc(&["--identity", "sqrt(r1^2) = r1", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        standard_output(&output),
        "unknown name  sqrt(r1^2) = r1: r1 would depend on itself\n"
    );
}

#[test]
fn a_definition_is_a_definition_under_identity_for_a_program() {
    let output = piped(&["--batch", "--identity", "--json"], DEFINITION);

    assert_eq!(
        standard_output(&output),
        "{\"claim\": \"p(x) = x^2 - 5*x + 6\", \"verdict\": \"definition\"}\n"
    );
}

#[test]
fn a_definition_names_what_it_defines_under_expand_for_a_program() {
    let output = piped(&["--batch", "--expand", "--json"], DEFINITION);

    assert_eq!(
        standard_output(&output),
        "{\"entry\": \"p(x) = x^2 - 5*x + 6\", \"definition\": \"p\"}\n"
    );
}

#[test]
fn a_definition_names_what_it_defines_under_solve_for_a_program() {
    let output = piped(&["--batch", "--solve-for", "--json"], DEFINITION);

    assert_eq!(
        standard_output(&output),
        "{\"equation\": \"p(x) = x^2 - 5*x + 6\", \"definition\": \"p\"}\n"
    );
}

#[test]
fn a_definition_does_not_move_the_exit_status() {
    let output = batch("--solve-for", DEFINITION);

    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn a_verdict_name_is_the_same_under_another_locale() {
    let english = piped(
        &["--batch", "--identity", "--json", "--locale", "en"],
        DEFINITION,
    );
    let german = piped(
        &["--batch", "--identity", "--json", "--locale", "de"],
        DEFINITION,
    );

    assert_eq!(standard_output(&english), standard_output(&german));
}

#[test]
fn a_name_the_session_does_not_define_stays_a_free_name() {
    let output = calc(&["--identity", "(x+1)^2 = x^2 + 2*x + 1", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(standard_output(&output), "holds for every value\n");
}

#[test]
fn a_question_is_answered_against_the_session_as_it_stood() {
    let output = batch("--identity", "a * 2 = 6\na = 3\n");

    assert_eq!(
        standard_output(&output),
        "not an identity: the two sides are not equal as polynomials in their names\ndefines  a = 3\n"
    );
}
