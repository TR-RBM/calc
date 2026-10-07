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
fn radix_sort_counts_its_passes_and_writes() {
    let text = shown(&calc(&[
        "radix_sort([170, 45, 75, 90, 802, 24, 2, 66], base=10)",
        "--locale",
        "en",
    ]));

    assert!(
        row(&text, "value").ends_with("[2, 24, 45, 66, 75, 90, 170, 802]"),
        "{text}"
    );
    assert!(
        row(&text, "comparisons").ends_with("0, counted on this input"),
        "{text}"
    );
    assert!(
        row(&text, "writes").ends_with("48, counted on this input"),
        "{text}"
    );
    assert!(
        row(&text, "counter updates").contains("75, counted on this input"),
        "{text}"
    );
    assert!(row(&text, "passes").contains("3 in base 10"), "{text}");
    assert!(
        row(&text, "most writes").contains("2*n in each pass"),
        "{text}"
    );
    assert!(
        row(&text, "lower bound").contains("does not apply"),
        "{text}"
    );
    assert!(!text.contains("key range"), "{text}");
}

#[test]
fn radix_sort_asks_for_its_base_and_refuses_a_base_below_two() {
    for (expression, expected) in [
        ("radix_sort([3, 1, 2])", "so name one"),
        ("radix_sort([3, 1, 2], base=1)", "at least 2"),
        ("radix_sort([3, 1, 2], base=70000)", "at most 65536"),
        (
            "radix_sort([3, 1, 2], base=x)",
            "base= takes a whole number",
        ),
    ] {
        let output = calc(&[expression, "--locale", "en"]);
        let errors = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

        assert!(!output.status.success(), "{expression}");
        assert!(errors.contains(expected), "{expression}: {errors}");
    }
}

#[test]
fn radix_sort_keeps_equal_keys_in_input_order_both_ways() {
    let increasing = shown(&calc(&[
        "radix_sort([12, 3, 12, 3], base=2)",
        "--locale",
        "en",
    ]));
    let decreasing = shown(&calc(&[
        "radix_sort([12, 3, 12, 3], base=2, order=decreasing)",
        "--locale",
        "en",
    ]));

    assert!(
        row(&increasing, "from positions").contains("[2, 4, 1, 3]"),
        "{increasing}"
    );
    assert!(
        row(&decreasing, "from positions").contains("[1, 3, 2, 4]"),
        "{decreasing}"
    );
}

#[test]
fn radix_sort_names_itself_when_it_refuses_a_key() {
    for (expression, expected) in [
        (
            "radix_sort([1/2, 1], base=10)",
            "radix_sort counts whole numbers, and 1 / 2 is not one",
        ),
        (
            "radix_sort([3 m, 1 m], base=10)",
            "radix_sort counts whole numbers, and 3 m carries a unit",
        ),
    ] {
        let output = calc(&[expression, "--locale", "en"]);
        let errors = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

        assert!(errors.contains(expected), "{expression}: {errors}");
        assert!(!errors.contains("counting"), "{expression}: {errors}");
    }
}

#[test]
fn radix_sort_trace_names_the_least_key_and_the_digit() {
    let text = shown(&calc(&[
        "radix_sort([12, 3], base=10)",
        "--trace",
        "--locale",
        "en",
    ]));

    assert!(
        text.contains(
            "counting sort by digit 1, counted from the lowest, of each key less the least key, 3"
        ),
        "{text}"
    );
    assert!(
        text.contains("count 12 in counter 10, the counter of digit 9"),
        "{text}"
    );
}

#[test]
fn radix_sort_with_no_pass_says_why() {
    let text = shown(&calc(&["radix_sort([5, 5, 5], base=10)", "--locale", "en"]));

    assert!(
        row(&text, "passes").contains("0 in base 10: every key equals the least"),
        "{text}"
    );
}

#[test]
fn bead_sort_rebuilds_the_list_and_counts_every_bead_twice() {
    let text = shown(&calc(&["bead_sort([3, 1, 2])", "--locale", "en"]));

    assert!(row(&text, "value").ends_with("[1, 2, 3]"), "{text}");
    assert!(
        row(&text, "comparisons").ends_with("0, counted on this input"),
        "{text}"
    );
    assert!(
        row(&text, "writes").ends_with("3, counted on this input"),
        "{text}"
    );
    assert!(
        row(&text, "counter updates").contains("12, counted on this input"),
        "{text}"
    );
    assert!(
        row(&text, "from positions").contains("none: bead sort rebuilds each value"),
        "{text}"
    );
    assert!(
        row(&text, "lower bound").contains("does not apply"),
        "{text}"
    );
}

#[test]
fn bead_sort_records_no_positions() {
    let text = shown(&calc(&["bead_sort([3, 1, 2])", "--json"]));
    let compact: String = text.split_whitespace().collect();

    assert!(
        compact.contains(r#""sort":{"from_positions":null,"comparisons":0,"writes":3"#),
        "{text}"
    );
}

#[test]
fn bead_sort_refuses_what_it_cannot_lay_out_as_beads() {
    for (expression, expected) in [
        ("bead_sort([3, -1])", "-1 is below 0"),
        (
            "bead_sort([3, 1/2])",
            "bead_sort counts whole numbers, and 1 / 2 is not one",
        ),
        (
            "bead_sort([3, 1], x |-> -x)",
            "cannot carry an entry by its key",
        ),
        (
            "bead_sort([1048576, 1])",
            "1048577 beads, more than the 1048576",
        ),
    ] {
        let output = calc(&[expression, "--locale", "en"]);
        let errors = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

        assert!(!output.status.success(), "{expression}");
        assert!(errors.contains(expected), "{expression}: {errors}");
    }
}

#[test]
fn bead_sort_traces_every_bead() {
    let text = shown(&calc(&["bead_sort([2, 1])", "--trace", "--locale", "en"]));

    for step in [
        "2 drops a bead onto pole 1",
        "2 drops a bead onto pole 2",
        "1 drops a bead onto pole 1",
        "the bead of pole 1 in row 2 from the bottom is counted for that row",
        "write 2, the beads of row 1 from the bottom, at position 2",
    ] {
        assert!(text.contains(step), "{step}: {text}");
    }
}

#[test]
fn bead_sort_reads_the_identity_key_as_no_key() {
    let keyed = shown(&calc(&["bead_sort([3, 1, 2], x |-> x)", "--locale", "en"]));
    let plain = shown(&calc(&["bead_sort([3, 1, 2])", "--locale", "en"]));

    assert_eq!(row(&keyed, "value"), row(&plain, "value"));
    assert_eq!(
        row(&keyed, "counter updates"),
        row(&plain, "counter updates")
    );
}

#[test]
fn a_key_with_a_unit_is_refused_without_speaking_of_counters() {
    for expression in [
        "bead_sort([3 m, 1 m])",
        "radix_sort([3 m, 1 m], base=10)",
        "counting_sort([3 m, 1 m])",
    ] {
        for (locale, expected, counters) in [
            (
                "en",
                "whether it is a whole number at all, and which one",
                "counters",
            ),
            (
                "de",
                "ob es überhaupt eine ganze Zahl ist und welche",
                "Zähler",
            ),
        ] {
            let output = calc(&[expression, "--locale", locale]);
            let errors = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

            assert!(errors.contains(expected), "{expression}: {errors}");
            assert!(!errors.contains(counters), "{expression}: {errors}");
        }
    }
}
