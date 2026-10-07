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

#[test]
fn a_reading_and_a_tick_label_write_one_power_of_ten_form() {
    let label = calc_render::tick_label(3142, 6, '.').expect("the label fits");

    let output = calc(&["pi * 10^9", "--locale", "en"]);

    assert!(output.status.success());
    assert!(
        standard_output(&output).contains(&format!("value           {label}\n")),
        "the reading of pi * 10^9 does not end in {label}"
    );
}

#[test]
fn a_negative_reading_and_its_tick_label_agree() {
    let label = calc_render::tick_label(-3142, -12, '.').expect("the label fits");

    let output = calc(&["-pi * 10^-9", "--locale", "en"]);

    assert!(output.status.success());
    assert!(
        standard_output(&output).contains(&format!("value           {label}\n")),
        "the reading of -pi * 10^-9 does not end in {label}"
    );
}
