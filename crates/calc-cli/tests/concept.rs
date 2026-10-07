use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn standard_output(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("output is UTF-8")
}

fn standard_error(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("output is UTF-8")
}

#[test]
fn a_concept_prints_its_name_statement_and_lenses() {
    let output = calc(&["concept", "circle-circumference", "--locale", "en"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.starts_with("circle-circumference  "));
    assert!(text.contains("statement"));
    assert!(text.contains("has something in"));
}

#[test]
fn the_concept_is_in_the_display_locale() {
    let output = calc(&["concept", "circle-circumference", "--locale", "de"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("Aussage"));
    assert!(text.contains("hat etwas in"));
}

#[test]
fn a_lens_with_nothing_to_show_is_not_an_error() {
    let output = calc(&[
        "concept",
        "circle-circumference",
        "--lens",
        "train",
        "--locale",
        "en",
    ]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("has nothing to show for this concept yet"));
    assert!(text.contains("has something in"));
}

#[test]
fn an_unknown_concept_is_a_typed_failure() {
    let output = calc(&["concept", "nonesuch", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(standard_error(&output).contains("nonesuch"));
}

#[test]
fn an_unknown_lens_is_a_usage_error() {
    let output = calc(&["concept", "circle-circumference", "--lens", "sideways"]);

    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn the_command_needs_an_identifier() {
    let output = calc(&["concept", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn the_json_form_names_identifiers_and_message_keys() {
    let output = calc(&["concept", "circle-circumference", "--json"]);
    let text = standard_output(&output);

    assert!(output.status.success());
    assert!(text.contains("\"identifier\": \"circle-circumference\""));
    assert!(text.contains("\"name_key\": \"common-lens-explore\""));
    assert!(text.contains("\"locale\":"));
}

#[test]
fn the_json_form_is_the_same_in_every_locale_but_for_the_wording() {
    let english = standard_output(&calc(&[
        "concept",
        "circle-circumference",
        "--json",
        "--locale",
        "en",
    ]));
    let german = standard_output(&calc(&[
        "concept",
        "circle-circumference",
        "--json",
        "--locale",
        "de",
    ]));

    assert_eq!(
        english
            .lines()
            .filter(|row| row.contains("name_key"))
            .count(),
        german
            .lines()
            .filter(|row| row.contains("name_key"))
            .count()
    );
    assert!(german.contains("\"locale\": \"de\""));
}

#[test]
fn no_row_is_wider_than_the_terminal() {
    for locale in ["en", "de"] {
        let text = standard_output(&calc(&[
            "concept",
            "circle-circumference",
            "--locale",
            locale,
        ]));
        for row in text.lines() {
            assert!(
                row.chars().count() <= 80,
                "a {locale} row is {} columns",
                row.chars().count()
            );
        }
    }
}

#[test]
fn a_concept_is_found_by_the_name_the_recognized_row_prints() {
    let recognized = calc(&["sqrt(3^2 + 4^2)", "--locale", "en"]);
    let row = standard_output(&recognized)
        .lines()
        .find(|row| row.trim_start().starts_with("concept"))
        .expect("a recognized row")
        .to_owned();
    let name = row
        .split_whitespace()
        .skip(1)
        .collect::<Vec<&str>>()
        .join(" ");

    let output = calc(&["concept", &name, "--locale", "en"]);

    assert!(output.status.success(), "{}", standard_error(&output));
    assert!(standard_output(&output).starts_with("pythagorean-theorem  "));
}

#[test]
fn a_concept_is_found_by_its_name_in_the_display_locale() {
    let output = calc(&["concept", "Satz des Pythagoras", "--locale", "de"]);

    assert!(output.status.success(), "{}", standard_error(&output));
    assert!(standard_output(&output).starts_with("pythagorean-theorem  "));
}

#[test]
fn a_name_of_another_locale_is_not_a_concept_of_this_one() {
    let output = calc(&["concept", "Satz des Pythagoras", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(standard_error(&output).contains("there is no concept"));
}

#[test]
fn no_row_of_a_learning_text_begins_with_a_sign() {
    let concepts = std::fs::read_dir(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../content/concepts"
    ))
    .expect("the concepts are readable")
    .filter_map(|entry| entry.ok()?.file_name().into_string().ok());
    for concept in concepts {
        for locale in ["de", "en"] {
            let output = calc(&["concept", &concept, "--lens", "learn", "--locale", locale]);
            for row in standard_output(&output).lines() {
                let first = row.split_whitespace().next().unwrap_or_default();
                assert!(
                    !calc_app::OPERATOR_SIGNS.contains(&first),
                    "{concept} {locale}: {row}"
                );
            }
        }
    }
}

#[test]
fn a_concept_names_its_activities() {
    let output = calc(&["concept", "shape-matching", "--locale", "en"]);
    let text = standard_output(&output)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    assert!(text.contains("activities"), "{text}");
    assert!(
        text.contains("matching shapes with circle, square, triangle, turned"),
        "{text}"
    );
}

#[test]
fn a_concept_names_its_activities_in_german() {
    let output = calc(&["concept", "shape-matching", "--locale", "de"]);

    assert!(standard_output(&output).contains("Aktivitäten"));
}

#[test]
fn a_concept_carries_its_activities_for_a_program() {
    let output = calc(&["concept", "shape-matching", "--json"]);

    assert!(standard_output(&output).contains("\"kind\": \"shape_matching\""));
}
