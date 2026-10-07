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

#[test]
fn a_list_keeps_every_entry_exact() {
    let output = terse("[1/2, 1/3, 1/6]");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "[1/2, 1/3, 1/6] exact\n");
}

#[test]
fn a_matrix_keeps_its_rows() {
    let output = terse("[1, 2; 3, 4]");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "[1, 2; 3, 4] exact\n");
}

#[test]
fn two_lists_are_added_entry_by_entry() {
    let output = terse("[1, 2] + [3, 4]");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "[4, 6] exact\n");
}

#[test]
fn a_list_times_a_number_multiplies_every_entry() {
    let output = terse("[1, 2, 3] * 2");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "[2, 4, 6] exact\n");
}

#[test]
fn a_number_minus_a_list_subtracts_every_entry_from_the_number() {
    let output = terse("10 - [1, 2, 3]");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "[9, 8, 7] exact\n");
}

#[test]
fn two_matrices_are_multiplied_as_matrices() {
    let output = terse("[1, 2; 3, 4] * [5, 6; 7, 8]");

    assert!(output.status.success());
    assert_eq!(standard_output(&output), "[19, 22; 43, 50] exact\n");
}

#[test]
fn lists_of_different_length_are_refused() {
    let output = terse("[1, 2] + [3, 4, 5]");

    assert!(!output.status.success());
    assert!(standard_output(&output).contains("different shapes"));
}

#[test]
fn a_product_of_two_lists_names_both_readings_instead_of_choosing() {
    let output = terse("[1, 2] * [3, 4]");
    let shown = standard_output(&output);

    assert!(!output.status.success());
    assert!(shown.contains("ambiguous"));
    assert!(shown.contains("matching entries"));
    assert!(shown.contains("scalar product"));
}

#[test]
fn the_language_reference_no_longer_calls_a_list_uncomputed() {
    let output = calc(&["help", "language", "--locale", "en"]);
    let shown = standard_output(&output);
    let list = shown
        .split("\n  ")
        .find(|entry| entry.starts_with("list "))
        .expect("the reference names the list");

    assert!(!list.contains("does not compute it yet"));
}

#[test]
fn entries_of_different_kinds_name_the_kind_each_measures() {
    let output = terse("[1 m, 2 kg]");

    assert!(!output.status.success());
    assert!(standard_output(&output).contains("m measures length"));
    assert!(standard_output(&output).contains("kg measures mass"));
}

#[test]
fn a_list_times_a_matrix_says_the_ranks_differ_rather_than_naming_a_scalar_product() {
    let output = terse("[1, 2] * [1, 2; 3, 4]");
    let shown = standard_output(&output);

    assert!(!output.status.success());
    assert!(shown.contains("list and the other a matrix"));
    assert!(!shown.contains("scalar product"));
}

#[test]
fn a_nested_list_says_so_instead_of_leaking_an_inner_expression() {
    let output = terse("[[1, 2], [3, 4]] + [[1, 2], [3, 4]]");
    let shown = standard_output(&output);

    assert!(!output.status.success());
    assert!(shown.contains("entries are themselves lists"));
    assert!(!shown.contains("[1, 2] cannot"));
}

#[test]
fn a_matrix_raised_to_a_power_is_refused_by_name() {
    let output = terse("[1, 2; 3, 4]^2");

    assert!(!output.status.success());
    assert!(standard_output(&output).contains("to a power"));
}

fn answer(expression: &str) -> String {
    standard_output(&terse(expression))
}

#[test]
fn the_eigenvalues_of_the_hilbert_matrix_of_order_four_are_answered() {
    let polynomial = "6048000 * t^4 - 10137600 * t^3 + 1603680 * t^2 - 10496 * t + 1";
    let roots: Vec<String> = (1..=4)
        .map(|index| format!("rootof({polynomial}, t, {index})"))
        .collect();
    assert_eq!(
        answer(
            "eigenvalues([1, 1/2, 1/3, 1/4; 1/2, 1/3, 1/4, 1/5; 1/3, 1/4, 1/5, 1/6; 1/4, 1/5, 1/6, 1/7])"
        ),
        format!("[{}] exact\n", roots.join(", "))
    );
}

#[test]
fn the_trace_of_a_matrix_is_the_sum_of_its_diagonal() {
    assert_eq!(answer("trace([1, 2; 3, 4])"), "5 exact\n");
}

#[test]
fn an_inverse_is_exact() {
    assert_eq!(
        answer("inverse([1, 2; 3, 4])"),
        "[-2, 1; 3/2, -1/2] exact\n"
    );
}

#[test]
fn a_singular_matrix_has_no_inverse() {
    assert!(answer("inverse([1, 2; 2, 4])").contains("this matrix is not invertible"));
}

#[test]
fn the_rank_counts_independent_rows() {
    assert_eq!(answer("rank([1, 2; 2, 4])"), "1 exact\n");
}

#[test]
fn rational_eigenvalues_are_listed_in_order() {
    assert_eq!(answer("eigenvalues([2, 1; 1, 2])"), "[1, 3] exact\n");
}

#[test]
fn irrational_eigenvalues_are_written_with_square_roots() {
    assert_eq!(
        answer("eigenvalues([1, 1; 1, 0])"),
        "[(1 - sqrt(5)) / 2, (1 + sqrt(5)) / 2] exact\n"
    );
}

#[test]
fn a_rotation_has_eigenvalues_that_are_not_real() {
    assert!(answer("eigenvalues([0, -1; 1, 0])").contains("eigenvalues that are not real numbers"));
}

#[test]
fn a_list_holding_a_square_root_is_answered_exactly() {
    assert_eq!(answer("[sqrt(2), 1]"), "[sqrt(2), 1] exact\n");
}

#[test]
fn the_kernel_of_a_singular_matrix_is_a_basis_of_its_null_space() {
    assert_eq!(answer("kernel([1, 2; 2, 4])"), "[-2; 1] exact\n");
}

#[test]
fn an_invertible_matrix_has_the_empty_basis() {
    assert_eq!(answer("kernel([1, 0; 0, 1])"), "[] exact\n");
}

#[test]
fn a_kernel_of_two_dimensions_has_two_columns() {
    assert_eq!(
        answer("kernel([1, 1, 1; 1, 1, 1])"),
        "[-1, -1; 1, 0; 0, 1] exact\n"
    );
}

#[test]
fn a_matrix_sends_its_kernel_to_zero() {
    assert_eq!(
        answer("[1, 1, 1; 1, 1, 1] * kernel([1, 1, 1; 1, 1, 1])"),
        "[0, 0; 0, 0] exact\n"
    );
}

#[test]
fn eigenvectors_are_columns_in_the_order_of_the_eigenvalues() {
    assert_eq!(
        answer("eigenvectors([2, 1; 1, 2])"),
        "[-1, 1; 1, 1] exact\n"
    );
}

#[test]
fn eigenvectors_of_irrational_eigenvalues_hold_square_roots() {
    assert_eq!(
        answer("eigenvectors([1, 1; 1, 0])"),
        "[(1 - sqrt(5)) / 2, (1 + sqrt(5)) / 2; 1, 1] exact\n"
    );
}

#[test]
fn a_repeated_eigenvalue_gives_a_column_for_each_dimension_of_its_eigenspace() {
    assert_eq!(answer("eigenvectors([2, 0; 0, 2])"), "[1, 0; 0, 1] exact\n");
}

#[test]
fn an_eigenvalue_written_as_rootof_leaves_eigenvectors_refused() {
    assert!(answer("eigenvectors([1, 2, 3; 4, 5, 6; 7, 8, 10])").contains("rootof"));
}
