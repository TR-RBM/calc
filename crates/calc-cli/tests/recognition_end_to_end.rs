use std::process::Command;

const CALC: &str = env!("CARGO_BIN_EXE_calc");
const LOCALE: &str = "en";
const RECOGNIZED_INPUT: &str = "sqrt(3^2 + 4^2)";
const RECOGNIZED_CONCEPT: &str = "pythagorean-theorem";
const UNRECOGNIZED_INPUT: &str = "3 - 7";
const SMALLEST_CAP: i64 = 1;
const LARGEST_CAP: i64 = 100;
const SMALLEST_WORK_BUDGET: i64 = 1;
const LARGEST_WORK_BUDGET: i64 = 10_000_000;

fn run(arguments: &[&str]) -> String {
    let output = Command::new(CALC)
        .args(arguments)
        .args(["--locale", LOCALE])
        .output()
        .expect("the calc binary runs");
    assert!(
        output.status.success(),
        "calc {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("calc writes UTF-8")
}

fn concept_name() -> String {
    let printed = run(&["concept", RECOGNIZED_CONCEPT]);
    let first = printed.lines().next().unwrap_or_default().to_string();
    first
        .split_once(RECOGNIZED_CONCEPT)
        .map(|(_, name)| name.trim().to_string())
        .unwrap_or(first)
}

fn json_of(input: &str) -> String {
    run(&[input, "--json"])
}

fn member_after(json: &str, member: &str) -> Option<String> {
    let start = json.find(&format!("\"{member}\""))? + member.len() + 3;
    let rest = json.get(start..)?.trim_start_matches([':', ' ']);
    let end = rest.find([',', '\n', '}'])?;
    Some(rest.get(..end)?.trim().trim_matches('"').to_string())
}

fn recognized_of(json: &str) -> String {
    let start = json
        .find("\"recognized\"")
        .expect("the record carries recognized");
    let end = json[start..]
        .find("\"seed\"")
        .map_or(json.len(), |offset| start + offset);
    json[start..end].to_string()
}

#[test]
fn the_text_output_names_the_recognized_concept_as_the_concept_command_does() {
    let name = concept_name();

    let printed = run(&[RECOGNIZED_INPUT]);

    assert!(
        printed.contains(&name),
        "the row names no concept called {name}: {printed}"
    );
}

#[test]
fn the_text_output_of_a_line_that_matches_nothing_says_so() {
    let printed = run(&[UNRECOGNIZED_INPUT]);

    let says_so = printed.lines().any(|line| {
        line.trim_start().starts_with("concept") && line.split_whitespace().count() > 1
    });

    assert!(says_so, "no row says that nothing matched: {printed}");
}

#[test]
fn the_json_output_carries_the_recognized_concept_of_the_line() {
    let recognized = recognized_of(&json_of(RECOGNIZED_INPUT));

    let concept = member_after(&recognized, "concept");

    assert_eq!(concept, Some(RECOGNIZED_CONCEPT.to_string()));
}

#[test]
fn the_json_output_says_that_recognition_ran() {
    let recognized = recognized_of(&json_of(RECOGNIZED_INPUT));

    let state = member_after(&recognized, "state");

    assert_eq!(state, Some("ran".to_string()));
}

#[test]
fn a_line_that_matches_nothing_says_recognition_ran_with_no_match() {
    let recognized = recognized_of(&json_of(UNRECOGNIZED_INPUT));

    let state = member_after(&recognized, "state");

    assert!(
        state == Some("ran".to_string()) && recognized.contains("\"matches\": []"),
        "recognized of a line that matches nothing: {recognized}"
    );
}

#[test]
fn a_recognition_that_ran_names_the_limits_it_ran_under() {
    let recognized = recognized_of(&json_of(RECOGNIZED_INPUT));

    let cap = member_after(&recognized, "cap").and_then(|value| value.parse::<i64>().ok());
    let budget =
        member_after(&recognized, "work_budget").and_then(|value| value.parse::<i64>().ok());

    assert!(
        cap.is_some_and(|cap| (SMALLEST_CAP..=LARGEST_CAP).contains(&cap))
            && budget.is_some_and(|budget| {
                (SMALLEST_WORK_BUDGET..=LARGEST_WORK_BUDGET).contains(&budget)
            }),
        "limits of the recognition: {recognized}"
    );
}

#[test]
fn a_recognition_that_was_not_cut_short_names_no_cause() {
    let recognized = recognized_of(&json_of(RECOGNIZED_INPUT));

    let truncated = member_after(&recognized, "truncated");
    let cause = member_after(&recognized, "truncated_by");

    assert_eq!(
        (truncated, cause),
        (Some("false".to_string()), Some("null".to_string()))
    );
}

#[test]
fn the_json_output_names_the_concept_without_its_rendered_words() {
    let name = concept_name();

    let json = json_of(RECOGNIZED_INPUT);

    assert!(
        json.contains(RECOGNIZED_CONCEPT) && !json.contains(&name),
        "the JSON carries the rendered name {name}"
    );
}

#[test]
fn a_sum_of_three_whole_numbers_is_an_addition() {
    let recognized = recognized_of(&json_of("1 + 2 + 3"));

    assert_eq!(
        member_after(&recognized, "concept"),
        Some("addition".to_string())
    );
}

#[test]
fn a_sum_whose_written_addends_are_not_whole_is_no_addition() {
    let recognized = recognized_of(&json_of("0.5 + 0.5 + 2"));

    assert!(recognized.contains("\"matches\": []"), "{recognized}");
}

#[test]
fn a_difference_whose_written_numbers_are_not_whole_is_no_subtraction() {
    let recognized = recognized_of(&json_of("2.5 - 0.5 - 1"));

    assert!(recognized.contains("\"matches\": []"), "{recognized}");
}
