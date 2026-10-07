use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn error_output(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("output is UTF-8")
}

#[test]
fn two_thousand_parentheses_are_refused_with_an_error_and_not_a_crash() {
    let written = format!("{}1{}", "(".repeat(2000), ")".repeat(2000));
    let output = calc(&[&written, "--locale", "en"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        error_output(&output),
        "error: the expression goes more than 64 levels deep; \
         split it into named lines, at column 65\n"
    );
}

#[test]
fn ten_thousand_terms_are_refused_with_an_error_and_not_a_crash() {
    let written = format!("{}1", "1+".repeat(10000));
    let output = calc(&[&written, "--locale", "en"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        error_output(&output),
        "error: the expression joins more than 512 operations in one chain; \
         split it into named lines, at column 1026\n"
    );
}

#[test]
fn a_chain_within_the_limit_still_answers() {
    let written = format!("{}1", "1+".repeat(512));
    let output = calc(&[&written, "--locale", "en"]);
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .expect("output is UTF-8")
            .contains("value     513")
    );
}

#[test]
fn nesting_that_multiplies_a_chain_is_refused_with_an_error_and_not_a_crash() {
    let mut written = String::from("1");
    for _ in 0..60 {
        written = format!("({}{})", written, "+1".repeat(400));
    }
    let output = calc(&[&written, "--locale", "en"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(error_output(&output).starts_with(
        "error: brackets and chains together put this expression more than 768 levels deep"
    ));
}

#[test]
fn two_brackets_holding_many_terms_say_it_is_the_two_counted_together() {
    let written = format!("(({}1){})", "1+".repeat(400), "+1".repeat(399));
    let output = calc(&[&written, "--locale", "en"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        error_output(&output),
        "error: brackets and chains together put this expression more than 768 levels deep; \
         split it into named lines, at column 1\n"
    );
}

#[test]
fn a_program_reads_the_limit_it_crossed_and_which_one_it_was() {
    let brackets = format!("{}1{}", "(".repeat(2000), ")".repeat(2000));
    let chain = format!("{}1", "1+".repeat(600));
    let tree = format!("(({}1){})", "1+".repeat(400), "+1".repeat(399));
    let read = |written: &str| {
        let output = calc(&[written, "--json", "--locale", "en"]);
        let text = String::from_utf8(output.stderr).expect("output is UTF-8");
        let kind = text
            .lines()
            .find_map(|line| line.trim().strip_prefix("\"kind\": \""))
            .map(|rest| rest.trim_end_matches("\",").trim_matches('"').to_string());
        let limit = text
            .lines()
            .find_map(|line| line.trim().strip_prefix("\"limit\": \""))
            .map(|rest| rest.trim_end_matches('"').to_string());
        (kind, limit)
    };
    assert_eq!(
        (read(&brackets), read(&chain), read(&tree)),
        (
            (
                Some("nested_too_deeply".to_string()),
                Some("64".to_string())
            ),
            (Some("chain_too_long".to_string()), Some("512".to_string())),
            (
                Some("expression_too_deep".to_string()),
                Some("768".to_string())
            )
        )
    );
}

#[test]
fn two_thousand_nested_calls_are_refused_with_an_error_and_not_a_crash() {
    let written = format!("{}1{}", "sin(".repeat(2000), ")".repeat(2000));
    let output = calc(&[&written, "--locale", "en"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        error_output(&output).starts_with("error: the expression goes more than 64 levels deep")
    );
}

#[test]
fn nesting_within_the_limit_still_answers() {
    let written = format!("{}1 + 1{}", "(".repeat(30), ")".repeat(30));
    let output = calc(&[&written, "--locale", "en"]);
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .expect("output is UTF-8")
            .contains("value     2")
    );
}
