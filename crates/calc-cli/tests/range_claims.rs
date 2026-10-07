use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn checked(claim: &str) -> Output {
    Command::new(CALC)
        .args([claim, "--check", "--locale", "en"])
        .env_clear()
        .output()
        .expect("calc runs")
}

fn shown(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

#[test]
fn a_claim_true_across_its_range_holds_throughout() {
    let output = checked("between(1, 2) < 3");

    assert!(output.status.success());
    assert_eq!(
        shown(&output),
        "holds for every value of its ranges  between(1, 2) < 3\n"
    );
}

#[test]
fn a_claim_true_at_one_end_only_holds_in_part() {
    let output = checked("between(1, 4) < 3");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        shown(&output),
        "FAILS: holds for part of its ranges only  between(1, 4) < 3\n"
    );
}

#[test]
fn a_claim_false_across_its_range_fails_throughout() {
    let output = checked("between(4, 5) < 3");

    assert_eq!(
        shown(&output),
        "FAILS for every value of its ranges  between(4, 5) < 3\n"
    );
}

#[test]
fn a_tolerance_is_checked_across_its_range() {
    let output = checked("tolerance(25, 10 %) > 20");

    assert!(output.status.success());
}

#[test]
fn an_inequality_that_may_meet_zero_inside_the_range_stays_undecided() {
    let output = checked("between(-1, 1) != 0");

    assert_eq!(output.status.code(), Some(4));
}
