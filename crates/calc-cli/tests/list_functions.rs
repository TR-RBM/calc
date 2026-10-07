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

fn terse(expression: &str) -> Output {
    calc(&[expression, "--terse", "--locale", "en"])
}

fn answers(expression: &str, expected: &str) {
    let output = terse(expression);

    assert!(output.status.success(), "{expression} failed");
    assert_eq!(standard_output(&output), format!("{expected}\n"));
}

#[test]
fn a_list_can_be_counted() {
    answers("count([1, 2, 3])", "3 exact");
}

#[test]
fn a_list_can_be_totalled_exactly() {
    answers("total([1/2, 1/3, 1/6])", "1 exact");
}

#[test]
fn the_mean_of_fractions_stays_a_fraction() {
    answers("mean([1/2, 1/3, 1/6])", "1/3 exact");
}

#[test]
fn the_median_of_an_odd_list_is_the_middle_entry() {
    answers("median([3, 1, 2])", "2 exact");
}

#[test]
fn the_median_of_an_even_list_is_the_mean_of_the_middle_two() {
    answers("median([4, 1, 3, 2])", "5/2 exact");
}

#[test]
fn an_entry_is_taken_by_its_position_counting_from_one() {
    answers("at([5, 6, 7], 2)", "6 exact");
}

#[test]
fn a_matrix_can_be_transposed() {
    answers("transpose([1, 2; 3, 4])", "[1, 3; 2, 4] exact");
}

#[test]
fn a_determinant_of_a_small_matrix_is_exact() {
    answers("determinant([1, 2; 3, 4])", "-2 exact");
}

#[test]
fn a_determinant_of_a_singular_matrix_is_zero() {
    answers("determinant([2, 0, 1; 1, 3, 2; 1, 1, 1])", "0 exact");
}

#[test]
fn a_determinant_over_fractions_collects_no_rounding_error() {
    answers(
        "determinant([1, 1/2, 1/3, 1/4; 1/2, 1/3, 1/4, 1/5; 1/3, 1/4, 1/5, 1/6; 1/4, 1/5, 1/6, 1/7])",
        "1/6048000 exact",
    );
}

#[test]
fn a_reduction_can_stand_inside_a_larger_expression() {
    answers("mean([1, 2, 3]) + 10", "12 exact");
}

#[test]
fn a_determinant_of_a_matrix_that_is_not_square_is_refused() {
    let output = terse("determinant([1, 2; 3, 4; 5, 6])");

    assert!(!output.status.success());
    assert!(standard_output(&output).contains("square"));
}

#[test]
fn an_entry_beyond_the_end_of_a_list_is_refused() {
    let output = terse("at([1, 2], 5)");

    assert!(!output.status.success());
    assert!(standard_output(&output).contains("no entry at that position"));
}

#[test]
fn the_smallest_entry_of_a_list_is_found() {
    answers("smallest([3, 1, 2])", "1 exact");
}

#[test]
fn the_largest_entry_of_fractions_stays_a_fraction() {
    answers("largest([1/2, 1/3, 2/3])", "2/3 exact");
}

#[test]
fn a_list_can_be_sorted_into_increasing_order() {
    answers("sorted([3, 1, 2])", "[1, 2, 3] exact");
}

#[test]
fn the_spread_of_a_list_composes_from_two_reductions() {
    answers("largest([1, 2, 3]) - smallest([1, 2, 3])", "2 exact");
}

#[test]
fn the_total_of_an_empty_list_is_the_empty_sum() {
    answers("total([])", "0 exact");
}

#[test]
fn an_empty_list_sorted_is_itself() {
    answers("sorted([])", "[] exact");
}

#[test]
fn the_mean_of_an_empty_list_is_refused() {
    let output = terse("mean([])");

    assert!(!output.status.success());
    assert!(standard_output(&output).contains("no entries"));
}

#[test]
fn the_least_entry_of_an_empty_list_is_refused() {
    let output = terse("smallest([])");

    assert!(!output.status.success());
    assert!(standard_output(&output).contains("no entries"));
}

#[test]
fn an_empty_list_is_a_value_of_its_own() {
    answers("[ ]", "[] exact");
}

#[test]
fn transposing_a_list_is_refused_for_being_a_list() {
    let output = terse("transpose([1,2,3])");

    assert!(!output.status.success());
    assert_eq!(
        standard_output(&output),
        "error transpose takes a matrix, and this is a list\n"
    );
}

#[test]
fn a_determinant_of_a_list_is_refused_for_being_a_list() {
    let output = terse("determinant([1])");

    assert!(!output.status.success());
    assert_eq!(
        standard_output(&output),
        "error a determinant takes a matrix, and this is a list\n"
    );
}

#[test]
fn transposing_an_empty_list_says_there_is_nothing_to_transpose() {
    let output = terse("transpose([])");

    assert!(!output.status.success());
    assert!(
        standard_output(&output).contains("nothing to transpose"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn a_determinant_of_an_empty_list_says_there_is_nothing_to_take_one_of() {
    let output = terse("determinant([])");

    assert!(!output.status.success());
    assert!(
        standard_output(&output).contains("nothing to take a determinant of"),
        "{}",
        standard_output(&output)
    );
}

#[test]
fn an_empty_array_refusal_carries_one_code_and_names_its_operation() {
    let output = calc(&["transpose([])", "--json"]);

    let shown = standard_output(&output);
    assert!(shown.contains("\"code\": \"array_empty\""), "{shown}");
    assert!(shown.contains("\"operation\": \"transpose\""), "{shown}");
}

#[test]
fn a_matrix_that_is_not_square_still_says_so() {
    let output = terse("determinant([1,2,3;4,5,6])");

    assert!(!output.status.success());
    assert!(
        standard_output(&output).contains("as many rows as columns"),
        "{}",
        standard_output(&output)
    );
}
