use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

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
fn a_quotient_that_cancels_to_one_is_an_exact_integer() {
    let text = shown(&calc(&["x/x", "--locale", "en"]));

    assert!(text.contains("  number    exact integer\n"), "{text}");
    assert!(!text.contains("symbolic"), "{text}");
    assert!(!text.contains("rounded"), "{text}");
}

#[test]
fn a_quotient_that_cancels_to_one_keeps_its_condition() {
    assert_eq!(
        shown(&calc(&["x/x", "--terse"])),
        "1 exact, wherever x is not zero\n"
    );
}

#[test]
fn a_quotient_that_cancels_to_a_fraction_is_written_as_a_fraction() {
    assert_eq!(
        shown(&calc(&["(2*x)/(4*x)", "--terse"])),
        "1/2 exact, wherever 4 * x is not zero\n"
    );
}

#[test]
fn a_quotient_that_cancels_to_zero_is_an_exact_integer() {
    let text = shown(&calc(&["0/x", "--locale", "en"]));

    assert!(text.contains("  number    exact integer\n"), "{text}");
    assert!(!text.contains("symbolic"), "{text}");
}

#[test]
fn a_power_to_zero_is_one_with_the_zero_to_the_zero_note() {
    let text = shown(&calc(&["x^0", "--locale", "en"]));

    assert!(text.contains("  number    exact integer\n"), "{text}");
    assert!(text.contains("0^0 is taken as 1"), "{text}");
}

#[test]
fn a_quotient_that_keeps_a_name_stays_symbolic() {
    let text = shown(&calc(&["(x^2-1)/(x-1)", "--locale", "en"]));

    assert!(text.contains("symbolic"), "{text}");
}
