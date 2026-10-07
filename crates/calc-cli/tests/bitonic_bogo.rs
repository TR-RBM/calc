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
fn bitonic_sort_makes_the_comparisons_of_its_network() {
    let text = shown(&calc(&["bitonic_sort([5, 3, 8, 1])", "--locale", "en"]));

    assert!(row(&text, "value").ends_with("[1, 3, 5, 8]"), "{text}");
    assert!(
        row(&text, "comparisons").ends_with("6, counted on this input"),
        "{text}"
    );
    assert!(
        row(&text, "most comparisons").contains(
            "= 6 at n = 4, taken over every order of n distinct entries, for n a power of two"
        ),
        "{text}"
    );
    assert!(row(&text, "average writes").contains("fitted"), "{text}");
    assert!(row(&text, "most writes").contains("not"), "{text}");
}

#[test]
fn bitonic_sort_refuses_a_length_that_is_not_a_power_of_two() {
    for locale in ["en", "de"] {
        let output = calc(&["bitonic_sort([3, 1, 2])", "--locale", locale]);
        let errors = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

        assert!(!output.status.success(), "{errors}");
        assert!(errors.contains("2^k"), "{errors}");
        assert!(
            errors.contains(" 3,") && errors.contains(" 2 ") && errors.contains(" 4;"),
            "{errors}"
        );
    }
}

#[test]
fn bitonic_sort_traces_each_merge_stage() {
    let text = shown(&calc(&[
        "bitonic_sort([4, 3, 2, 1])",
        "--trace",
        "--locale",
        "en",
    ]));

    for stage in [
        "merge in blocks of 2, rising and falling in turn: compare entries 1 apart",
        "merge in blocks of 4, rising and falling in turn: compare entries 2 apart",
        "merge in blocks of 4, rising and falling in turn: compare entries 1 apart",
    ] {
        assert!(text.contains(stage), "{stage}: {text}");
    }
}

#[test]
fn bogo_sort_sorts_and_counts_its_shuffles() {
    let text = shown(&calc(&[
        "bogo_sort([3, 1, 2], seed=7, limit=100)",
        "--locale",
        "en",
    ]));

    assert!(row(&text, "value").ends_with("[1, 2, 3]"), "{text}");
    assert!(row(&text, "shuffles").contains("of at most 100"), "{text}");
    assert!(
        row(&text, "expected shuffles").contains("6 = n! at n = 3"),
        "{text}"
    );
    assert!(
        row(&text, "fewest comparisons").contains("= 2 at n = 3"),
        "{text}"
    );
    assert!(
        row(&text, "most writes").contains("the shuffles have no bound"),
        "{text}"
    );
    assert!(row(&text, "draws").contains("seed 7, stream 1"), "{text}");
    assert!(
        row(&text, "draws").contains("each position of each shuffle"),
        "{text}"
    );
}

#[test]
fn bogo_sort_gives_the_same_answer_for_the_same_seed() {
    let first = shown(&calc(&[
        "bogo_sort([4, 2, 3, 1], seed=11, limit=1000)",
        "--locale",
        "en",
    ]));
    let second = shown(&calc(&[
        "bogo_sort([4, 2, 3, 1], seed=11, limit=1000)",
        "--locale",
        "en",
    ]));

    assert_eq!(row(&first, "comparisons"), row(&second, "comparisons"));
    assert_eq!(row(&first, "shuffles"), row(&second, "shuffles"));
}

#[test]
fn bogo_sort_stopped_at_its_limit_shows_no_list_and_exits_with_4() {
    let output = calc(&["bogo_sort([3, 1, 2], seed=1, limit=0)", "--locale", "en"]);
    let text = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

    assert_eq!(output.status.code(), Some(4), "{text}");
    assert!(
        text.contains(
            "stopped at its limit of 0 shuffles without sorting, so there is no list to show"
        ),
        "{text}"
    );
    assert!(text.contains("comparisons 1, writes 0, draws 0"), "{text}");
    assert!(!text.contains("[1, 2, 3]"), "{text}");
}

#[test]
fn bogo_sort_asks_for_its_seed_and_limit() {
    for (expression, expected) in [
        ("bogo_sort([3, 1, 2], limit=10)", "write seed="),
        ("bogo_sort([3, 1, 2], seed=1)", "limit="),
        (
            "bogo_sort([3, 1, 2], seed=1, limit=x)",
            "limit= takes a whole number",
        ),
    ] {
        let output = calc(&[expression, "--locale", "en"]);
        let errors = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

        assert!(!output.status.success(), "{expression}");
        assert!(errors.contains(expected), "{expression}: {errors}");
    }
}

#[test]
fn bogo_sort_states_expected_shuffles_for_distinct_keys_and_for_short_lists() {
    let three = shown(&calc(&[
        "bogo_sort([2, 1, 1], seed=3, limit=100)",
        "--locale",
        "en",
    ]));
    let one = shown(&calc(&[
        "bogo_sort([5], seed=1, limit=10)",
        "--locale",
        "en",
    ]));

    assert!(
        row(&three, "expected shuffles").contains("n distinct keys"),
        "{three}"
    );
    assert!(
        row(&three, "expected shuffles").contains("n!/(m1!·m2!·…)"),
        "{three}"
    );
    assert!(
        row(&one, "expected shuffles").contains("0: a list of no more than one entry"),
        "{one}"
    );
}

#[test]
fn a_sort_refusal_keeps_its_words_inside_a_larger_expression() {
    let stopped = calc(&[
        "2 * bogo_sort([5, 4, 3, 2, 1], seed=2, limit=3)",
        "--locale",
        "en",
    ]);
    let text = shown(&stopped) + &String::from_utf8_lossy(&stopped.stderr);
    assert_eq!(stopped.status.code(), Some(4), "{text}");
    assert!(
        text.contains("stopped at its limit of 3 shuffles"),
        "{text}"
    );

    for (expression, expected) in [
        ("2 * bitonic_sort([3, 1, 2])", "a network for 2^k entries"),
        ("2 * bead_sort([-1, 2])", "is below 0"),
        (
            "2 * insertion_sort([2 m, 1 s])",
            "2 m at position 1 and 1 s at position 2",
        ),
        (
            "bitonic_sort([3, 1, 2]) + [1, 1, 1]",
            "a network for 2^k entries",
        ),
    ] {
        let output = calc(&[expression, "--locale", "en"]);
        let text = shown(&output) + &String::from_utf8_lossy(&output.stderr);

        assert_eq!(output.status.code(), Some(1), "{expression}: {text}");
        assert!(text.contains(expected), "{expression}: {text}");
        assert!(!text.contains("not defined"), "{expression}: {text}");
    }
}
