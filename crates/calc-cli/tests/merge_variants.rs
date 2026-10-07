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
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn row<'a>(text: &'a str, label: &str) -> &'a str {
    text.lines()
        .find(|line| line.trim_start().starts_with(label))
        .unwrap_or_else(|| panic!("no row {label} in\n{text}"))
}

#[test]
fn bottom_up_merge_sort_copies_every_entry_on_every_pass() {
    let text = shown(&calc(&[
        "bottom_up_merge_sort([3, 1, 2])",
        "--locale",
        "en",
    ]));

    assert!(row(&text, "value").ends_with("[1, 2, 3]"), "{text}");
    assert!(
        row(&text, "writes").ends_with("12, counted on this input"),
        "{text}"
    );
    assert!(
        row(&text, "average writes").contains("= 12 at n = 3"),
        "{text}"
    );
    assert!(
        row(&text, "most comparisons").contains("= 3 at n = 3"),
        "{text}"
    );
}

#[test]
fn natural_merge_sort_leaves_a_sorted_list_after_one_scan() {
    let text = shown(&calc(&[
        "natural_merge_sort([1, 2, 3, 4])",
        "--locale",
        "en",
    ]));

    assert!(
        row(&text, "comparisons").ends_with("3, counted on this input"),
        "{text}"
    );
    assert!(
        row(&text, "writes").ends_with("0, counted on this input"),
        "{text}"
    );
    assert!(
        row(&text, "most writes").contains("= 16 at n = 4"),
        "{text}"
    );
}

#[test]
fn both_merge_variants_keep_equal_keys_in_input_order() {
    for expression in [
        "bottom_up_merge_sort([2, 1, 2, 1])",
        "natural_merge_sort([2, 1, 2, 1])",
    ] {
        let text = shown(&calc(&[expression, "--locale", "en"]));

        assert!(
            row(&text, "from positions").contains("[2, 4, 1, 3]"),
            "{expression}: {text}"
        );
    }
}

#[test]
fn no_row_states_a_value_for_log2_of_zero() {
    for expression in [
        "merge_sort([])",
        "bottom_up_merge_sort([])",
        "natural_merge_sort([])",
    ] {
        let text = shown(&calc(&[expression, "--locale", "en"]));

        for line in text.lines().filter(|line| line.contains("log2(n)")) {
            assert!(
                line.contains("says nothing at n = 0"),
                "{expression}: {line}"
            );
        }
    }
}
