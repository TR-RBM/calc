use std::process::Command;

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn answered(expression: &str) -> String {
    let output = Command::new(CALC)
        .args([expression, "--locale", "en"])
        .env_clear()
        .output()
        .expect("calc runs");
    let shown = String::from_utf8(output.stdout).expect("output is UTF-8");
    assert!(output.status.success(), "{shown}");
    rows(&shown)
}

fn rows(text: &str) -> String {
    text.lines()
        .map(|line| {
            format!(
                "\n{}\n",
                line.split_whitespace().collect::<Vec<_>>().join(" ")
            )
        })
        .collect()
}

#[test]
fn a_rounded_third_times_three_says_it_was_computed_after_rounding() {
    let shown = answered("to_f64(1/3) * 3");

    assert!(
        shown.contains(
            "\ncomputed exactly, after rounding each to_f64 or to_f32 argument to a machine value\n"
        ),
        "{shown}"
    );
}

#[test]
fn a_rounded_third_names_its_exact_difference_from_a_third() {
    let shown = answered("to_f64(1/3) * 3");

    assert!(
        shown.contains("the difference is -1/54043195528445952"),
        "{shown}"
    );
}

#[test]
fn a_line_whose_root_is_a_conversion_keeps_its_machine_record() {
    let shown = answered("to_f64(0.1)");

    assert!(
        shown.contains("\nnumber machine float, 64-bit (f64)\n"),
        "{shown}"
    );
    assert!(!shown.contains("after rounding each"), "{shown}");
}
