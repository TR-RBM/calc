use std::process::{Command, Output};

use calc_app::shipped_unit_systems;
use calc_i18n::Locale;

const CALC: &str = env!("CARGO_BIN_EXE_calc");

const SUBCOMMANDS: [&str; 6] = ["solve", "plot", "read", "concept", "asm", "help language"];

const OPTIONS: [&str; 28] = [
    "--given",
    "--function",
    "--begin",
    "--inspect",
    "--units",
    "--unit ",
    "--coherent-units",
    "--json",
    "--locale",
    "--help",
    "--digits",
    "--enclose",
    "--solve",
    "--choose-wanted",
    "--machine-line",
    "--request",
    "--output",
    "--view",
    "--param",
    "--size",
    "--limit",
    "--settle",
    "--at",
    "--layer",
    "--commit",
    "--how-it-ran",
    "--working",
    "--trace",
];

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn help(locale: &str) -> String {
    let output = calc(&["--help", "--locale", locale]);
    assert!(output.status.success());
    String::from_utf8(output.stdout).expect("output is UTF-8")
}

#[test]
fn every_option_the_command_line_accepts_is_described() {
    for locale in ["en", "de"] {
        let page = help(locale);
        for option in OPTIONS {
            assert!(
                page.contains(option),
                "{option} is missing from the {locale} help"
            );
        }
    }
}

#[test]
fn every_subcommand_is_named() {
    for locale in ["en", "de"] {
        let page = help(locale);
        for subcommand in SUBCOMMANDS {
            assert!(
                page.contains(&format!("calc {subcommand}")),
                "{subcommand} is missing from the {locale} help"
            );
        }
    }
}

#[test]
fn the_page_has_the_sections_of_a_manual() {
    let page = help("en");

    assert!(page.starts_with("calc — "));
    for heading in [
        "Synopsis:",
        "Description:",
        "Options:",
        "Exit status:",
        "Examples:",
    ] {
        assert!(page.contains(heading), "{heading} is missing");
    }
}

#[test]
fn the_german_page_has_its_own_sections() {
    let page = help("de");

    assert!(page.starts_with("calc — "));
    for heading in [
        "Aufruf:",
        "Beschreibung:",
        "Optionen:",
        "Rückgabewerte:",
        "Beispiele:",
    ] {
        assert!(page.contains(heading), "{heading} is missing");
    }
}

#[test]
fn every_exit_code_the_command_line_returns_is_explained() {
    let page = help("en");

    for code in ["0  ", "1  ", "2  ", "3  ", "4  "] {
        assert!(page.contains(code), "exit code {code} is missing");
    }
}

#[test]
fn the_examples_show_the_work_and_not_a_corner_of_it() {
    for locale in ["en", "de"] {
        let page = help(locale);
        for example in [
            "calc --identity",
            "calc --check",
            "calc --find",
            "calc --expand",
            "calc --batch",
            "calc \"1/3 + 1/6\"",
            "calc \"integral(x^2, x, 0, 1)\"",
        ] {
            assert!(
                page.contains(example),
                "{example} is missing from the {locale} examples"
            );
        }
    }
}

#[test]
fn the_examples_teach_what_stays_exact() {
    let page = help("en");

    assert!(page.contains("calc \"0.1 + 0.2\""));
    assert!(page.contains("calc \"to_f64(0.1 + 0.2)\""));
}

#[test]
fn every_unit_system_the_content_ships_is_named_in_the_help() {
    let page = help("en");

    for system in shipped_unit_systems(&Locale::source()) {
        assert!(
            page.contains(&system.identifier),
            "unit system {} is missing from the help",
            system.identifier
        );
    }
}

#[test]
fn a_usage_error_prints_the_short_synopsis_and_not_the_page() {
    let output = calc(&["--locale", "en"]);
    let errors = String::from_utf8(output.stderr).expect("output is UTF-8");

    assert_eq!(output.status.code(), Some(2));
    assert!(errors.contains("Usage:"));
    assert!(!errors.contains("Exit status:"));
}

#[test]
fn no_line_of_the_page_is_wider_than_the_terminal() {
    for locale in ["en", "de"] {
        for row in help(locale).lines() {
            assert!(
                row.chars().count() <= 80,
                "a {locale} row is {} columns: {row}",
                row.chars().count()
            );
        }
    }
}
