use std::process::Command;

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn answered(expression: &str) -> String {
    let output = Command::new(CALC)
        .args([expression, "--locale", "en"])
        .env_clear()
        .output()
        .expect("calc runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("output is UTF-8")
        .lines()
        .map(|line| {
            format!(
                "\n{}\n",
                line.split_whitespace().collect::<Vec<_>>().join(" ")
            )
        })
        .collect()
}

#[test]
fn a_pound_in_kilograms_is_its_defining_decimal() {
    let shown = answered("1 lb -> kg");

    assert!(shown.contains("\nvalue 0.45359237 kg\n"), "{shown}");
    assert!(shown.contains("\nnumber exact decimal\n"), "{shown}");
}

#[test]
fn a_quarter_mile_in_metres_stays_a_fraction() {
    let shown = answered("(1/4) mi -> m");

    assert!(shown.contains("\nvalue (50292/125) m\n"), "{shown}");
}

#[test]
fn twenty_degrees_celsius_in_kelvin_is_exact_to_two_places() {
    let shown = answered("from_celsius(20) -> K");

    assert!(shown.contains("\nvalue 293.15 K\n"), "{shown}");
    assert!(!shown.contains("rounded"), "{shown}");
}

#[test]
fn metres_and_centimetres_add_to_a_decimal() {
    let shown = answered("2 m + 30 cm");

    assert!(shown.contains("\nvalue 2.3 m\n"), "{shown}");
}

#[test]
fn a_percent_of_a_half_stays_a_fraction() {
    let shown = answered("(1/2) %");

    assert!(shown.contains("\nvalue 1/200\n"), "{shown}");
}

#[test]
fn a_maximum_answers_in_the_form_of_the_value_it_picks() {
    let shown = answered("max(1/4, 0.1)");

    assert!(shown.contains("\nvalue 1/4\n"), "{shown}");
}

#[test]
fn a_mean_of_decimals_answers_a_decimal() {
    let shown = answered("mean([0.2, 0.4])");

    assert!(shown.contains("\nvalue 0.3\n"), "{shown}");
}

#[test]
fn a_logarithm_of_a_decimal_answers_a_decimal() {
    let shown = answered("log(0.5, 4)");

    assert!(shown.contains("\nvalue -0.5\n"), "{shown}");
}

#[test]
fn pounds_and_kilograms_in_grams_add_to_a_decimal() {
    let shown = answered("1 lb + 1 kg -> g");

    assert!(shown.contains("\nvalue 1453.59237 g\n"), "{shown}");
}

#[test]
fn feet_and_inches_in_centimetres_add_to_a_decimal() {
    let shown = answered("5 ft + 3 in -> cm");

    assert!(shown.contains("\nvalue 160.02 cm\n"), "{shown}");
}
