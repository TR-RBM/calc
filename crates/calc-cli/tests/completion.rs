use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");
const FILES_FIT: i32 = 5;

fn completed(words: &[&str]) -> Output {
    Command::new(CALC)
        .arg("--complete")
        .arg("--")
        .args(words)
        .env_clear()
        .env("LANG", "en_US.UTF-8")
        .output()
        .expect("calc runs")
}

fn offered(output: &Output) -> Vec<String> {
    String::from_utf8(output.stdout.clone())
        .expect("output is UTF-8")
        .lines()
        .map(|line| line.split('\t').next().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn the_first_word_lists_the_subcommands_beside_file_names() {
    let output = completed(&[""]);

    assert_eq!(output.status.code(), Some(FILES_FIT));
    assert_eq!(
        offered(&output),
        ["solve", "plot", "read", "concept", "asm", "help"]
    );
}

#[test]
fn each_candidate_carries_a_description_after_a_tab() {
    let output = completed(&["pl"]);
    let text = String::from_utf8(output.stdout).expect("output is UTF-8");

    assert_eq!(
        text,
        "plot\tdraw the picture of an expression into a PNG file\n"
    );
}

#[test]
fn a_plot_lists_only_the_options_of_plot() {
    let output = completed(&["plot", "x^2", "--"]);
    let offered = offered(&output);

    assert_eq!(output.status.code(), Some(0));
    assert!(offered.contains(&"--output".to_owned()), "{offered:?}");
    assert!(!offered.contains(&"--digits".to_owned()), "{offered:?}");
}

#[test]
fn the_locale_option_lists_the_locales() {
    let offered = offered(&completed(&["--locale", ""]));

    assert!(offered.contains(&"de".to_owned()), "{offered:?}");
    assert!(offered.contains(&"en".to_owned()), "{offered:?}");
}

#[test]
fn a_concept_lists_the_concept_identifiers() {
    let offered = offered(&completed(&["concept", "circle-a"]));

    assert_eq!(offered, ["circle-area"]);
}

#[test]
fn a_german_locale_describes_in_german() {
    let output = completed(&["--locale", "de", "--dig"]);
    let text = String::from_utf8(output.stdout).expect("output is UTF-8");

    assert_eq!(text, "--digits\tn Nachkommastellen zeigen, mit dem Rest\n");
}

#[test]
fn a_bare_double_dash_under_the_cursor_lists_the_main_options() {
    let output = completed(&["--"]);
    let offered = offered(&output);

    assert_eq!(output.status.code(), Some(0));
    assert!(offered.contains(&"--digits".to_owned()), "{offered:?}");
    assert!(!offered.contains(&"plot".to_owned()), "{offered:?}");
}
