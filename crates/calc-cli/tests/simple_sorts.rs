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
fn every_simple_sort_sorts_and_counts_on_this_input() {
    for (expression, comparisons, writes) in [
        ("double_selection_sort([3, 1, 2])", "3", "4"),
        ("cocktail_shaker_sort([3, 1, 2], form=full)", "4", "4"),
        ("cocktail_shaker_sort([3, 1, 2], form=shrinking)", "3", "4"),
        (
            "cocktail_shaker_sort([3, 1, 2], form=last_exchange)",
            "3",
            "4",
        ),
        ("gnome_sort([3, 1, 2])", "5", "4"),
        ("odd_even_sort([3, 1, 2], form=until_sorted)", "6", "4"),
        ("odd_even_sort([3, 1, 2], form=fixed_passes)", "3", "4"),
        ("comb_sort([3, 1, 2], form=lacey_box)", "5", "4"),
        ("cycle_sort([3, 1, 2])", "10", "3"),
        ("pancake_sort([3, 1, 2])", "3", "4"),
    ] {
        let text = shown(&calc(&[expression, "--locale", "en"]));

        assert!(
            row(&text, "value").ends_with("[1, 2, 3]"),
            "{expression}: {text}"
        );
        assert!(
            row(&text, "comparisons").ends_with(&format!("{comparisons}, counted on this input")),
            "{expression}: {text}"
        );
        assert!(
            row(&text, "writes").ends_with(&format!("{writes}, counted on this input")),
            "{expression}: {text}"
        );
    }
}

#[test]
fn a_fitted_row_says_it_was_fitted_and_not_derived() {
    let text = shown(&calc(&["cycle_sort([3, 1, 2])", "--locale", "en"]));

    assert!(
        row(&text, "most comparisons").contains("fitted to the counts")
            && row(&text, "most comparisons").contains("not derived for every n"),
        "{text}"
    );
    assert!(
        row(&text, "average writes").contains("derived here"),
        "{text}"
    );
}

#[test]
fn pancake_sort_counts_and_traces_its_flips() {
    let text = shown(&calc(&[
        "pancake_sort([3, 1, 2])",
        "--trace",
        "--locale",
        "en",
    ]));

    assert!(
        row(&text, "flips").contains(
            "2, counted on this input; a flip reverses the first k entries of the list, for some k"
        ),
        "{text}"
    );
    assert!(
        row(&text, "average flips").contains("2*n + 1 - 3*sum(1/k, k, 1, n) = 3/2"),
        "{text}"
    );
    assert!(text.contains("flip the first 3 entries"), "{text}");
}

#[test]
fn a_sort_taught_in_forms_that_count_differently_asks_for_one() {
    for (expression, expected) in [
        (
            "cocktail_shaker_sort([3, 1, 2])",
            "form=full scans the whole list",
        ),
        (
            "odd_even_sort([3, 1, 2])",
            "form=fixed_passes makes exactly n phases",
        ),
        (
            "comb_sort([3, 1, 2])",
            "form=lacey_box shrinks the gap by 1.3",
        ),
        (
            "comb_sort([3, 1, 2], form=dobosiewicz)",
            "form takes lacey_box",
        ),
    ] {
        let output = calc(&[expression, "--locale", "en"]);
        let errors = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

        assert!(!output.status.success(), "{expression}");
        assert!(errors.contains(expected), "{expression}: {errors}");
    }
}

#[test]
fn the_json_record_carries_the_flips() {
    let text = shown(&calc(&["pancake_sort([3, 1, 2])", "--json"]));

    let compact: String = text.split_whitespace().collect();
    assert!(compact.contains(r#""draws":null,"flips":2}"#), "{text}");
}

#[test]
fn a_stable_simple_sort_keeps_equal_keys_in_input_order() {
    for expression in [
        "cocktail_shaker_sort([2, 1, 2, 1], x |-> x, form=full)",
        "gnome_sort([2, 1, 2, 1])",
        "odd_even_sort([2, 1, 2, 1], form=until_sorted)",
    ] {
        let text = shown(&calc(&[expression, "--locale", "en"]));

        assert!(
            row(&text, "from positions").contains("[2, 4, 1, 3]"),
            "{expression}: {text}"
        );
    }
}

#[test]
fn gnome_sort_derives_its_most_comparisons() {
    let text = shown(&calc(&["gnome_sort([3, 1, 2])", "--locale", "en"]));

    assert!(
        row(&text, "most comparisons").contains("n*(n - 1) = 6 at n = 3")
            && row(&text, "most comparisons").contains("derived here"),
        "{text}"
    );
}

#[test]
fn shell_sort_names_its_gaps_and_the_ones_it_used() {
    let text = shown(&calc(&[
        "shell_sort([5, 4, 3, 2, 1], gaps=shell)",
        "--trace",
        "--locale",
        "en",
    ]));

    assert!(row(&text, "value").ends_with("[1, 2, 3, 4, 5]"), "{text}");
    assert!(
        row(&text, "gaps used").contains("2, 1 at n = 5, the gaps the rule above gives at this n"),
        "{text}"
    );
    assert!(
        row(&text, "fewest comparisons").contains("sum(n - h over the gaps h) = 7 at n = 5"),
        "{text}"
    );
    assert!(
        text.contains("insertion sort of the entries 2 apart"),
        "{text}"
    );
}

#[test]
fn shell_sort_without_gaps_names_the_sequences() {
    let output = calc(&["shell_sort([3, 1, 2])", "--locale", "en"]);
    let errors = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

    assert!(!output.status.success());
    assert!(
        errors.contains("gaps=knuth uses (3^k − 1)/2 up to ceil(n/3)"),
        "{errors}"
    );
}

#[test]
fn a_list_too_short_for_a_pass_says_no_gap_was_used() {
    let text = shown(&calc(&["shell_sort([1], gaps=shell)", "--locale", "en"]));

    assert!(
        row(&text, "gaps used").contains("none at n = 1: the rule above gives no gap at this n"),
        "{text}"
    );
}

#[test]
fn the_knuth_rule_says_a_gap_must_be_smaller_than_n() {
    let text = shown(&calc(&["shell_sort([1], gaps=knuth)", "--locale", "en"]));

    assert!(row(&text, "gaps ").contains("and smaller than n"), "{text}");
    assert!(row(&text, "gaps used").contains("none at n = 1"), "{text}");
}
