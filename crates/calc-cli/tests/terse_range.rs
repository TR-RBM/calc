use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn terse(expression: &str, locale: &str) -> Output {
    Command::new(CALC)
        .args([expression, "--terse", "--locale", locale])
        .env_clear()
        .output()
        .expect("calc runs")
}

fn shown(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

#[test]
fn a_range_says_it_is_a_range_in_one_line() {
    assert_eq!(
        shown(&terse("between(1, 2)", "en")),
        "between(1, 2) range, both ends exact\n"
    );
}

#[test]
fn a_range_says_it_is_a_range_in_german() {
    assert_eq!(
        shown(&terse("between(1, 2)", "de")),
        "between(1, 2) Bereich, beide Enden exakt\n"
    );
}

#[test]
fn a_nested_list_names_the_matrix_form_to_write() {
    let output = terse("eigenvalues([[2, 1], [1, 3]])", "en");

    assert!(
        shown(&output)
            .contains("a matrix is written with its rows separated by semicolons, as [2, 1; 1, 3]"),
        "{}",
        shown(&output)
    );
}
