use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn checked(claim: &str) -> String {
    String::from_utf8(calc(&[claim, "--check", "--locale", "en"]).stdout).expect("output is UTF-8")
}

#[test]
fn an_order_with_a_non_real_side_is_refused_naming_the_side() {
    assert!(
        checked("1 < i")
            .starts_with("refused  i is not a real number, and the complex numbers have no order")
    );
}

#[test]
fn an_order_with_a_non_real_side_exits_as_refused() {
    assert_eq!(calc(&["1 < i", "--check"]).status.code(), Some(1));
}

#[test]
fn a_real_value_written_with_i_is_ordered() {
    assert_eq!(checked("i*i < 0"), "holds  i*i < 0\n");
}

#[test]
fn inequality_of_complex_values_is_still_decided() {
    assert_eq!(checked("i != 1"), "holds  i != 1\n");
}

#[test]
fn the_json_names_the_non_real_side() {
    let text = String::from_utf8(calc(&["1 < i", "--check", "--json"]).stdout).expect("UTF-8");

    assert!(text.contains("\"code\": \"order_not_real\""), "{text}");
    assert!(text.contains("\"side\": \"i\""), "{text}");
}
