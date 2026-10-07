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
fn three_one_two_is_sorted_with_its_counts_formulas_and_bound() {
    let output = calc(&["insertion_sort([3, 1, 2])", "--locale", "en"]);
    let text = shown(&output);

    assert!(output.status.success());
    assert!(row(&text, "value").ends_with("[1, 2, 3]"));
    assert!(row(&text, "from positions").contains("[2, 3, 1], the place each entry"));
    assert!(row(&text, "comparisons").ends_with("3, counted on this input"));
    assert!(row(&text, "writes").ends_with("4, counted on this input"));
    assert!(row(&text, "fewest comparisons").contains("n - 1 = 2 at n = 3"));
    assert!(row(&text, "most comparisons").contains("n*(n - 1)/2 = 3 at n = 3"));
    assert!(
        row(&text, "average comparisons")
            .contains("n*(n - 1)/4 + n - sum(1/k, k, 1, n) = 8/3 at n = 3")
    );
    assert!(row(&text, "average writes").contains("= 7/2 at n = 3"));
    assert!(row(&text, "lower bound").contains("ceil(log2(n!)) = 3 at n = 3"));
    assert!(row(&text, "computed").contains("insertion sort, which is stable"));
}

#[test]
fn the_trace_names_seven_steps() {
    let text = shown(&calc(&[
        "insertion_sort([3, 1, 2])",
        "--trace",
        "--locale",
        "en",
    ]));

    let steps: Vec<&str> = text
        .lines()
        .skip_while(|line| line.trim() != "steps")
        .skip(1)
        .collect();
    assert_eq!(
        steps,
        vec![
            "  1  compare 1 (held aside from position 2) with 3 at position 1: 1 is smaller",
            "  2  move 3 from position 1 to position 2",
            "  3  put 1, held aside from position 2, into position 1",
            "  4  compare 2 (held aside from position 3) with 3 at position 2: 2 is smaller",
            "  5  move 3 from position 2 to position 3",
            "  6  compare 2 (held aside from position 3) with 1 at position 1: 2 is larger",
            "  7  put 2, held aside from position 3, into position 2",
        ]
    );
}

#[test]
fn a_key_sorts_by_its_value_and_keeps_equal_keys_in_input_order() {
    let text = shown(&calc(&[
        "insertion_sort([3, -1, 2, -3], x |-> abs(x))",
        "--locale",
        "en",
    ]));

    assert!(row(&text, "value").ends_with("[-1, 2, 3, -3]"));
    assert!(row(&text, "from positions").contains("[2, 3, 1, 4]"));
    assert!(row(&text, "key").contains("evaluated 4 times, once for each entry"));
}

#[test]
fn rows_of_a_matrix_are_sorted_by_a_key_of_the_row() {
    let text = shown(&calc(&[
        "insertion_sort([1, 30; 2, 10; 3, 20], r |-> at(r, 2), order=decreasing)",
        "--locale",
        "en",
    ]));

    assert!(row(&text, "value").ends_with("[1, 30; 3, 20; 2, 10]"));
    assert!(row(&text, "order").ends_with("by decreasing key"));
}

#[test]
fn a_pair_that_cannot_be_ordered_stops_the_sort_with_its_counts() {
    let output = calc(&["insertion_sort([sin(1)^2 + cos(1)^2, 1])", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(shown(&output).contains(
        "calc could not decide which of sin(1)^2 + cos(1)^2 and 1 is larger, so it cannot order the list; until then it made 1 comparison, this one included, and 0 writes"
    ));
}

#[test]
fn rows_of_a_matrix_without_a_key_are_refused() {
    let output = calc(&["insertion_sort([1, 2; 3, 4])", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(shown(&output).contains("the rows of a matrix are sorted by a key"));
}

#[test]
fn the_json_record_carries_the_counts() {
    let text = shown(&calc(&["insertion_sort([3, 1, 2])", "--json"]));

    let compact: String = text.split_whitespace().collect();
    assert!(
        compact.contains(
            r#""sort":{"from_positions":[2,3,1],"comparisons":3,"writes":4,"key_evaluations":null,"tallies":null,"key_range":null,"draws":null,"flips":null}"#
        ),
        "{text}"
    );
}

#[test]
fn trace_on_a_line_that_is_not_a_sort_says_so() {
    let text = shown(&calc(&["1 + 1", "--trace", "--locale", "en"]));

    assert!(text.contains("--trace shows the steps of a named sort"));
}

#[test]
fn the_answer_is_given_in_german_with_du_where_it_asks_something() {
    let output = calc(&["insertion_sort([1, 2; 3, 4])", "--locale", "de"]);

    assert!(shown(&output).contains("schreib ihn als Funktion der Zeile"));
}

#[test]
fn a_sorted_list_keeps_the_form_its_entries_were_written_in() {
    for expression in [
        "insertion_sort([0.5, 1/3, 0.25])",
        "sorted([0.5, 1/3, 0.25])",
    ] {
        let text = shown(&calc(&[expression, "--terse", "--locale", "en"]));

        assert_eq!(text, "[0.25, 1/3, 0.5] exact\n", "{expression}");
    }
}

#[test]
fn quantities_keep_the_form_the_unsorted_list_gets() {
    let text = shown(&calc(&[
        "insertion_sort([3 m, 20 cm, 1 km])",
        "--terse",
        "--locale",
        "en",
    ]));

    assert_eq!(text, "[0.2, 3, 1000] m exact\n");
}

#[test]
fn a_complex_entry_is_refused_because_the_complex_numbers_have_no_order() {
    for expression in ["insertion_sort([1, i])", "sorted([1, i])"] {
        let output = calc(&[expression, "--locale", "en"]);

        assert_eq!(output.status.code(), Some(1), "{expression}");
        assert!(
            shown(&output).contains(
                "i is not a real number, and the complex numbers have no order, so the list cannot be ordered"
            ),
            "{expression}"
        );
    }
}

#[test]
fn entries_of_different_kinds_are_named_by_their_kinds() {
    let output = calc(&["insertion_sort([1 m, 1 s])", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(shown(&output).contains("m measures length"));
    assert!(shown(&output).contains("s measures time"));
}

#[test]
fn an_unknown_keyword_is_named_by_its_name() {
    let output = calc(&["insertion_sort([3, 1, 2], foo=1)", "--locale", "en"]);
    let errors = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

    assert!(errors.contains("unknown keyword argument foo"), "{errors}");
}

#[test]
fn a_wrong_order_names_the_keyword_and_the_values_it_takes() {
    let output = calc(&["insertion_sort([3, 1, 2], order=up)", "--locale", "en"]);
    let errors = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

    assert!(
        errors.contains(
            "up is not a value of the keyword argument order at column 33; order takes increasing, decreasing"
        ),
        "{errors}"
    );
}

#[test]
fn an_empty_list_says_from_which_length_a_formula_holds() {
    let text = shown(&calc(&["insertion_sort([])", "--locale", "en"]));

    assert!(
        row(&text, "fewest comparisons")
            .contains("n - 1, which holds from n = 1 on and so says nothing at n = 0")
    );
    assert!(row(&text, "most comparisons").contains("n*(n - 1)/2 = 0 at n = 0"));
}

#[test]
fn a_complex_number_written_as_a_sum_is_refused_as_not_real() {
    let output = calc(&["largest([2, 1 + i])", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        shown(&output).contains("1 + i is not a real number"),
        "{}",
        shown(&output)
    );
}

#[test]
fn an_entry_without_a_value_gets_its_own_refusal() {
    for expression in [
        "insertion_sort([1, sqrt(-1)])",
        "sorted([1, sqrt(-1)])",
        "largest([2, 1/0])",
        "insertion_sort([1, 2], x |-> 1/(x - 1))",
    ] {
        let output = calc(&[expression, "--locale", "en"]);
        let text = shown(&output);

        assert_eq!(output.status.code(), Some(1), "{expression}");
        assert!(!text.contains("could not decide"), "{expression}: {text}");
    }
}

#[test]
fn values_the_decimal_proof_cannot_separate_are_ordered_by_their_proven_enclosures() {
    let text = shown(&calc(&[
        "sorted([asin(1/2), asin(1/3)])",
        "--terse",
        "--locale",
        "en",
    ]));

    assert_eq!(text, "[asin(1 / 3), pi / 6] exact\n");
}

#[test]
fn entries_written_identically_are_equal() {
    let text = shown(&calc(&[
        "sorted([exp(1), 1, exp(1)])",
        "--terse",
        "--locale",
        "en",
    ]));

    assert_eq!(text, "[1, e, e] exact\n", "{text}");
}

#[test]
fn selection_sort_counts_every_pair_and_says_it_is_not_stable() {
    let text = shown(&calc(&["selection_sort([3, 1, 2])", "--locale", "en"]));

    assert!(row(&text, "value").ends_with("[1, 2, 3]"));
    assert!(row(&text, "comparisons").ends_with("3, counted on this input"));
    assert!(row(&text, "computed").contains("selection sort, which is not stable"));
    assert!(row(&text, "average writes").contains("2*(n - sum(1/k, k, 1, n)) = 7/3 at n = 3"));
    assert!(row(&text, "described in").contains("section 3.7"));
}

#[test]
fn binary_insertion_sort_states_its_costs_in_binary_logarithms() {
    let text = shown(&calc(&[
        "binary_insertion_sort([3, 1, 2])",
        "--locale",
        "en",
    ]));

    assert!(row(&text, "value").ends_with("[1, 2, 3]"));
    assert!(row(&text, "most comparisons").contains("sum(ceil(log2(k)), k, 1, n) = 3 at n = 3"));
    assert!(row(&text, "average comparisons").contains("= 8/3 at n = 3"));
}

#[test]
fn bubble_sort_names_its_form_and_says_which_average_it_does_not_state() {
    let text = shown(&calc(&[
        "bubble_sort([3, 1, 2], form=early_exit)",
        "--locale",
        "en",
    ]));

    assert!(row(&text, "form").contains("early_exit: shrinking passes"));
    assert!(row(&text, "average comparisons").contains("not stated"));
    assert!(row(&text, "most writes").contains("n*(n - 1) = 6 at n = 3"));
}

#[test]
fn bubble_sort_without_a_form_names_the_four() {
    let output = calc(&["bubble_sort([3, 1, 2])", "--locale", "en"]);
    let errors = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

    assert!(!output.status.success());
    for form in [
        "form=full",
        "form=shrinking",
        "form=early_exit",
        "form=last_exchange",
    ] {
        assert!(errors.contains(form), "{form}: {errors}");
    }
}

#[test]
fn a_form_on_a_sort_that_has_none_is_an_unknown_keyword() {
    let output = calc(&["selection_sort([3, 1, 2], form=full)", "--locale", "en"]);
    let errors = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

    assert!(errors.contains("unknown keyword argument form"), "{errors}");
}

#[test]
fn the_trace_of_a_bubble_sort_names_its_exchanges() {
    let text = shown(&calc(&[
        "bubble_sort([2, 1], form=full)",
        "--trace",
        "--locale",
        "en",
    ]));

    assert!(
        text.contains("exchange 2 at position 1 with 1 at position 2"),
        "{text}"
    );
}

#[test]
fn a_key_written_after_a_keyword_is_refused_saying_it_comes_first() {
    let output = calc(&[
        "bubble_sort([-3, 1, -2], form=last_exchange, x |-> abs(x))",
        "--locale",
        "en",
    ]);
    let errors = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

    assert!(!output.status.success());
    assert!(
        errors.contains("comes after a keyword argument; write every positional argument before the keyword arguments"),
        "{errors}"
    );
}

#[test]
fn merge_sort_states_its_worst_case_and_its_average_by_recurrence() {
    let text = shown(&calc(&["merge_sort([3, 1, 2])", "--locale", "en"]));

    assert!(row(&text, "value").ends_with("[1, 2, 3]"));
    assert!(row(&text, "writes").ends_with("10, counted on this input"));
    assert!(
        row(&text, "most comparisons")
            .contains("n*ceil(log2(n)) - 2^ceil(log2(n)) + 1 = 3 at n = 3")
    );
    assert!(row(&text, "average comparisons").contains("A(0) = A(1) = 0) = 8/3 at n = 3"));
    assert!(row(&text, "computed").contains("which is stable"));
}

#[test]
fn the_trace_of_a_merge_sort_names_its_scratch_places() {
    let text = shown(&calc(&["merge_sort([2, 1])", "--trace", "--locale", "en"]));

    assert!(
        text.contains("copy 1 from position 2 into scratch place 1"),
        "{text}"
    );
    assert!(
        text.contains("copy 1 from scratch place 1 back into position 1"),
        "{text}"
    );
}

#[test]
fn heap_sort_says_which_rows_it_does_not_state() {
    let text = shown(&calc(&["heap_sort([3, 1, 2])", "--locale", "en"]));

    assert!(row(&text, "value").ends_with("[1, 2, 3]"));
    assert!(row(&text, "most comparisons").contains("not stated"));
    assert!(row(&text, "computed").contains("heap sort, which is not stable"));
}

#[test]
fn quick_sort_with_lomuto_states_its_three_comparison_formulas() {
    let text = shown(&calc(&[
        "quick_sort([3, 1, 2], partition=lomuto, pivot=last)",
        "--locale",
        "en",
    ]));

    assert!(row(&text, "value").ends_with("[1, 2, 3]"));
    assert!(row(&text, "comparisons").ends_with("2, counted on this input"));
    assert!(
        row(&text, "average comparisons")
            .contains("2*(n + 1)*sum(1/k, k, 1, n) - 4*n = 8/3 at n = 3")
    );
    assert!(row(&text, "fewest writes").contains("fewest writes        0 at every n"));
}

#[test]
fn quick_sort_with_hoare_counts_its_two_scans() {
    let text = shown(&calc(&[
        "quick_sort([3, 1, 2], partition=hoare, pivot=first)",
        "--locale",
        "en",
    ]));

    assert!(row(&text, "value").ends_with("[1, 2, 3]"));
    assert!(row(&text, "comparisons").ends_with("9, counted on this input"));
    assert!(row(&text, "writes").ends_with("4, counted on this input"));
}

#[test]
fn quick_sort_without_its_keywords_says_what_to_write() {
    for (expression, expected) in [
        ("quick_sort([3, 1, 2])", "partition=lomuto, pivot=last"),
        (
            "quick_sort([3, 1, 2], partition=lomuto)",
            "with this partition write pivot=last",
        ),
        (
            "quick_sort([3, 1, 2], partition=lomuto, pivot=first)",
            "quick sort pairs partition=lomuto with pivot=last",
        ),
    ] {
        let output = calc(&[expression, "--locale", "en"]);
        let errors = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

        assert!(!output.status.success(), "{expression}");
        assert!(errors.contains(expected), "{expression}: {errors}");
    }
}

#[test]
fn counting_sort_counts_its_keys_and_makes_no_comparisons() {
    let text = shown(&calc(&["counting_sort([3, 1, 2, 1])", "--locale", "en"]));

    assert!(row(&text, "value").ends_with("[1, 1, 2, 3]"));
    assert!(row(&text, "from positions").contains("[2, 4, 3, 1]"));
    assert!(row(&text, "comparisons").ends_with("0, counted on this input"));
    assert!(row(&text, "writes").ends_with("8, counted on this input"));
    assert!(row(&text, "counter updates").contains("10, counted on this input"));
    assert!(row(&text, "key range").contains("3 values from the smallest key to the largest"));
    assert!(row(&text, "lower bound").contains("does not apply"));
}

#[test]
fn counting_sort_traces_every_count_prefix_sum_and_copy() {
    let text = shown(&calc(&[
        "counting_sort([2, 1])",
        "--trace",
        "--locale",
        "en",
    ]));

    for step in [
        "1  count 2 in counter 2",
        "2  count 1 in counter 1",
        "3  add counter 1 to counter 2",
        "4  take 1 from counter 1",
        "5  copy 1 from position 2 into scratch place 1",
        "6  take 1 from counter 2",
        "7  copy 2 from position 1 into scratch place 2",
        "8  copy 1 from scratch place 1 back into position 1",
        "9  copy 2 from scratch place 2 back into position 2",
    ] {
        assert!(text.contains(step), "{step} in\n{text}");
    }
}

#[test]
fn counting_sort_refuses_a_key_that_is_not_whole() {
    let output = calc(&["counting_sort([1/2, 1])", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(shown(&output).contains("counting_sort counts whole numbers, and 1 / 2 is not one"));
}

#[test]
fn counting_sort_refuses_a_range_wider_than_its_counters() {
    let output = calc(&["counting_sort([0, 2^20])", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(shown(&output).contains(
        "the keys span 1048577 values, and counting sort keeps at most 1048576 counters"
    ));
}

#[test]
fn counting_sort_records_its_counters_in_the_json_record() {
    let text = shown(&calc(&["counting_sort([3, 1, 2, 1])", "--json"]));

    let compact: String = text.split_whitespace().collect();
    assert!(
        compact.contains(
            r#""sort":{"from_positions":[2,4,3,1],"comparisons":0,"writes":8,"key_evaluations":null,"tallies":10,"key_range":3,"draws":null,"flips":null}"#
        ),
        "{text}"
    );
}

#[test]
fn counting_sort_refuses_a_key_with_a_unit_and_says_how_to_choose_one() {
    for expression in [
        "counting_sort([1 m, 50 cm])",
        "counting_sort([1 h, 30 min])",
    ] {
        let output = calc(&[expression, "--locale", "en"]);

        assert_eq!(output.status.code(), Some(1), "{expression}");
        assert!(
            shown(&output).contains("carries a unit"),
            "{expression}: {}",
            shown(&output)
        );
    }
}

#[test]
fn counting_sort_counts_in_the_unit_the_key_divides_by() {
    let text = shown(&calc(&[
        "counting_sort([1 m, 50 cm], x |-> x / (1 cm))",
        "--locale",
        "en",
    ]));

    assert!(row(&text, "from positions").contains("[2, 1]"), "{text}");
    assert!(row(&text, "key range").contains("51 values"), "{text}");
}

#[test]
fn a_key_range_of_one_value_is_named_in_the_singular() {
    let text = shown(&calc(&["counting_sort([5])", "--locale", "en"]));

    assert!(
        row(&text, "key range").contains("1 value, since the smallest key is also the largest")
    );
}

#[test]
fn counting_sort_refuses_a_machine_number_as_one() {
    let output = calc(&["counting_sort([to_f64(3), 1])", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        shown(&output).contains("is a machine number"),
        "{}",
        shown(&output)
    );
}

#[test]
fn counting_sort_names_the_key_function_as_the_source_of_a_machine_number() {
    let output = calc(&["counting_sort([1, 2], x |-> to_f64(x))", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    let text = shown(&output);
    assert!(text.contains("the key function gives"), "{text}");
    assert!(!text.contains("on a line of its own"), "{text}");
}

#[test]
fn counting_sort_refuses_entries_written_in_a_unit_without_a_dimension() {
    for expression in [
        "counting_sort([50 %, 20 %])",
        "counting_sort([100 %, 200 %])",
        "counting_sort([3 deg, 1 deg])",
        "counting_sort([2*(50 %), 20 %])",
        "counting_sort([(3 deg) + 0, 1 deg])",
        "counting_sort([3 * 1 deg, 1 deg])",
        "counting_sort([50 %, 20 %], x |-> 2*x)",
    ] {
        let output = calc(&[expression, "--locale", "en"]);

        assert_eq!(output.status.code(), Some(1), "{expression}");
        assert!(
            shown(&output).contains("carries a unit"),
            "{expression}: {}",
            shown(&output)
        );
    }
}

#[test]
fn counting_sort_counts_percentages_in_the_unit_the_key_divides_by() {
    let text = shown(&calc(&[
        "counting_sort([50 %, 20 %], x |-> x / (1 %))",
        "--locale",
        "en",
    ]));

    assert!(row(&text, "from positions").contains("[2, 1]"), "{text}");
    assert!(row(&text, "key range").contains("31 values"), "{text}");
}

#[test]
fn quick_sort_with_a_random_pivot_names_its_draws_and_seed() {
    let text = shown(&calc(&[
        "quick_sort([3, 1, 2], partition=lomuto, pivot=random, seed=7)",
        "--locale",
        "en",
    ]));

    assert!(row(&text, "value").ends_with("[1, 2, 3]"), "{text}");
    assert!(
        row(&text, "draws").contains("2 blocks of philox4x32_10 at seed 7"),
        "{text}"
    );
    assert!(
        row(&text, "average comparisons")
            .contains("the expected value when every pivot is drawn uniformly"),
        "{text}"
    );
    assert!(
        row(&text, "most comparisons").contains("every choice of pivots"),
        "{text}"
    );
}

#[test]
fn the_same_seed_gives_the_same_answer_and_the_record_carries_it() {
    let first = shown(&calc(&[
        "quick_sort([5, 1, 4, 2, 3], partition=lomuto, pivot=random, seed=7)",
        "--json",
    ]));
    let second = shown(&calc(&[
        "quick_sort([5, 1, 4, 2, 3], partition=lomuto, pivot=random, seed=7)",
        "--json",
    ]));
    let compact = |text: &str| text.split_whitespace().collect::<String>();
    let sort_member = |text: &str| {
        let text = compact(text);
        let start = text.find(r#""sort":{"#).expect("a sort member");
        let end = start + text[start..].find('}').expect("its end");
        text[start..=end].to_string()
    };

    assert_eq!(sort_member(&first), sort_member(&second));
    assert!(
        compact(&first).contains(r#""seed":{"value":"7","generator":"philox4x32_10_1"}"#),
        "{first}"
    );
    assert!(compact(&first).contains(r#""draws":"#), "{first}");
}

#[test]
fn the_trace_shows_each_draw_before_its_exchange() {
    let text = shown(&calc(&[
        "quick_sort([5, 1, 4, 2, 3], partition=lomuto, pivot=random, seed=7)",
        "--trace",
        "--locale",
        "en",
    ]));

    assert!(text.contains("draw a pivot for positions 1 to 5"), "{text}");
}

#[test]
fn a_random_pivot_without_its_seed_or_a_seed_without_it_is_refused() {
    for (expression, expected) in [
        (
            "quick_sort([3, 1, 2], partition=lomuto, pivot=random)",
            "so write seed=",
        ),
        (
            "quick_sort([3, 1, 2], partition=lomuto, pivot=last, seed=7)",
            "is for pivot=random, and this sort draws no pivots",
        ),
        (
            "quick_sort([3, 1, 2], partition=hoare, pivot=random, seed=7)",
            "partition=lomuto with pivot=last or pivot=random",
        ),
        (
            "quick_sort([3, 1, 2], partition=lomuto, pivot=random, seed=1.5)",
            "seed= takes a whole number from 0 to 2^64",
        ),
    ] {
        let output = calc(&[expression, "--locale", "en"]);
        let errors = String::from_utf8_lossy(&output.stderr).into_owned() + &shown(&output);

        assert!(!output.status.success(), "{expression}");
        assert!(errors.contains(expected), "{expression}: {errors}");
    }
}

#[test]
fn the_extreme_rows_of_a_random_pivot_claim_no_weighting() {
    let text = shown(&calc(&[
        "quick_sort([3, 1, 2], partition=lomuto, pivot=random, seed=7)",
        "--locale",
        "en",
    ]));

    for label in ["fewest comparisons", "most comparisons", "fewest writes"] {
        let line = row(&text, label);
        assert!(
            line.contains("with every sequence of pivot choices"),
            "{line}"
        );
        assert!(!line.contains("weighted"), "{line}");
        assert!(!line.contains("average"), "{line}");
    }
    assert!(row(&text, "average comparisons").contains("each weighted by its probability"));
}

#[test]
fn a_sort_refuses_keys_of_different_dimensions_before_it_compares() {
    for (expression, expected) in [
        (
            "merge_sort([2 m, 1 s, 3 m])",
            "2 m at position 1 and 1 s at position 2 cannot be put in one order",
        ),
        (
            "insertion_sort([2 m, 3 m, 1 s])",
            "2 m at position 1 and 1 s at position 3 cannot be put in one order",
        ),
        (
            "bitonic_sort([2 m, 1 s])",
            "2 m at position 1 and 1 s at position 2 cannot be put in one order",
        ),
    ] {
        let output = calc(&[expression, "--locale", "en"]);
        let text = shown(&output) + &String::from_utf8_lossy(&output.stderr);

        assert_eq!(output.status.code(), Some(1), "{expression}: {text}");
        assert!(text.contains(expected), "{expression}: {text}");
        assert!(text.contains("m measures length"), "{expression}: {text}");
        assert!(!text.contains("[1 s, 2 m]"), "{expression}: {text}");
    }
    let output = calc(&["merge_sort([2 m, 1 s, 3 m])", "--locale", "de"]);
    assert!(
        shown(&output).contains(
            "2 m an Stelle 1 und 1 s an Stelle 2 lassen sich nicht in eine Reihenfolge bringen"
        ),
        "{}",
        shown(&output)
    );
}

#[test]
fn a_sort_of_keys_in_units_of_one_dimension_still_sorts() {
    let text = shown(&calc(&["merge_sort([2 m, 150 cm, 1 m])", "--locale", "en"]));

    assert!(text.contains("[1, 1.5, 2] m"), "{text}");
}

#[test]
fn a_key_clash_names_the_keys_of_the_entries() {
    let output = calc(&[
        "insertion_sort([1, 2], x |-> x * (1 m)^x)",
        "--locale",
        "en",
    ]);
    let text = shown(&output) + &String::from_utf8_lossy(&output.stderr);

    assert!(
        text.contains(
            "the key 1 m of the entry at position 1 and the key 2 m^2 of the entry at position 2"
        ),
        "{text}"
    );
}

#[test]
fn smallest_largest_and_sorted_refuse_keys_of_different_dimensions() {
    for (expression, expected) in [
        (
            "smallest([1 s, 2 m])",
            "1 s at position 1 and 2 m at position 2 cannot be put in one order",
        ),
        (
            "largest([1 s, 2 m])",
            "1 s at position 1 and 2 m at position 2 cannot be put in one order",
        ),
        (
            "sorted([1 s, 2 m])",
            "1 s at position 1 and 2 m at position 2 cannot be put in one order",
        ),
        (
            "sorted([2 m, 3 m, 1])",
            "2 m at position 1 and 1 at position 3 cannot be put in one order",
        ),
        ("sorted([pi m, 1 s])", "cannot be put in one order"),
    ] {
        let output = calc(&[expression, "--locale", "en"]);
        let text = shown(&output) + &String::from_utf8_lossy(&output.stderr);

        assert_eq!(output.status.code(), Some(1), "{expression}: {text}");
        assert!(text.contains(expected), "{expression}: {text}");
    }
}
