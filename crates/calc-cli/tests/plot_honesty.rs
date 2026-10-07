#![cfg(target_arch = "x86_64")]

use std::path::PathBuf;
use std::process::Command;

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn plotted(input: &str, view: &str, name: &str, is_json: bool) -> String {
    plotted_at(input, view, name, is_json, "800x600")
}

fn plotted_at(input: &str, view: &str, name: &str, is_json: bool, size: &str) -> String {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("plot-honesty");
    std::fs::create_dir_all(&directory).expect("scratch directory is created");
    let path = directory.join(name);
    let mut arguments = vec![
        "plot",
        input,
        "--view",
        view,
        "--output",
        path.to_str().expect("path is UTF-8"),
        "--size",
        size,
        "--locale",
        "en",
    ];
    if is_json {
        arguments.push("--json");
    }
    let output = Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs");
    assert!(output.status.success());
    String::from_utf8(output.stdout).expect("output is UTF-8")
}

#[test]
fn a_cancelling_line_is_drawn_from_its_exact_values_with_no_limit() {
    let json = plotted("(x + 1) - 1", "1e-17..2e-17", "cancelling.png", true);

    assert!(
        json.contains("\"precision_limits\": [\n    null\n  ]"),
        "{json}"
    );
}

#[test]
fn a_sine_faster_than_the_columns_draws_a_band_and_no_limit() {
    let json = plotted("sin(1000*x)", "0..100", "fast-sine.png", true);

    assert!(
        json.contains("\"precision_limits\": [\n    null\n  ]"),
        "{json}"
    );
}

#[test]
fn columns_between_unresolved_samples_are_counted_in_the_notice() {
    let text = plotted(
        "(1 - cos(x))/x^2",
        "-1e-6..1e-6",
        "unresolved-columns.png",
        false,
    );

    assert!(
        text.contains("columns not resolved at this precision"),
        "{text}"
    );
}

#[test]
fn a_machine_line_that_varies_below_its_bounds_says_so() {
    let text = plotted(
        "to_f64(x) + 1 - 1",
        "1e-17..2e-17",
        "below-bounds.png",
        false,
    );

    assert!(
        text.contains("the sampled values vary less than their rounding bounds"),
        "{text}"
    );
}

#[test]
fn a_picture_with_every_column_state_still_fits_at_640_by_420() {
    let text = plotted_at(
        "(1 - cos(x))/x^2",
        "-1e-6..1e-6",
        "small-columns.png",
        false,
        "640x420",
    );

    assert!(
        text.contains("columns not resolved at this precision"),
        "{text}"
    );
}

#[test]
fn a_root_up_to_the_edge_of_its_domain_is_drawn_to_its_last_column() {
    let text = plotted_at(
        "sqrt(1 - x^2)",
        "0.9..1",
        "domain-edge.png",
        false,
        "400x300",
    );

    assert!(
        text.contains("may be hit: 1 of 272 columns not resolved at this width"),
        "{text}"
    );
}

#[test]
fn a_sine_with_whole_periods_in_every_column_is_proven_hit_throughout() {
    let text = plotted("sin(1000*x)", "0..100", "whole-periods.png", false);

    assert!(!text.contains("may be hit"), "{text}");
}

#[test]
fn a_line_that_holds_its_variable_twice_keeps_what_may_be_hit() {
    let text = plotted("x * sin(1000*x)", "0..100", "variable-twice.png", false);

    assert!(text.contains("may be hit"), "{text}");
}
