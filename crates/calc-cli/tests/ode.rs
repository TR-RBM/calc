use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn calc_in(language: &str, arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .env("LANG", language)
        .output()
        .expect("calc runs")
}

fn text(output: &Output) -> String {
    let mut text = String::from_utf8(output.stdout.clone()).expect("output is UTF-8");
    text.push_str(&String::from_utf8(output.stderr.clone()).expect("output is UTF-8"));
    text
}

fn solved(system: &str, initial: &str, at: &str) -> String {
    let output = calc(&["--ode", system, "--initial", initial, "--at", at]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    text(&output)
}

fn bounds(answer: &str, name: &str) -> Vec<(String, String)> {
    answer
        .lines()
        .filter_map(|line| {
            let rest = line.trim_start().strip_prefix(name)?.trim_start();
            let rest = rest.strip_prefix("between ")?;
            let (lower, upper) = rest.split_once(" and ")?;
            let number = |text: &str| text.split_whitespace().next().map(str::to_owned);
            Some((number(lower)?, number(upper)?))
        })
        .collect()
}

fn holds(claim: &str) -> bool {
    let output = calc(&["--check", claim]);
    output.status.code() == Some(0)
}

fn encloses(bound: &(String, String), closed_form: &str) {
    let (lower, upper) = bound;
    assert!(
        holds(&format!("{lower} <= {closed_form}")),
        "{lower} is above {closed_form}"
    );
    assert!(
        holds(&format!("{closed_form} <= {upper}")),
        "{upper} is below {closed_form}"
    );
}

#[test]
fn the_oscillator_at_ten_is_enclosed_around_cosine_and_sine() {
    let answer = solved(
        "diff(x, t) = v; diff(v, t) = -x",
        "t = 0; x = 1; v = 0",
        "t = 10",
    );
    encloses(&bounds(&answer, "x")[0], "cos(10)");
    encloses(&bounds(&answer, "v")[0], "-sin(10)");
}

#[test]
fn a_projectile_with_linear_drag_carries_its_units_and_meets_its_closed_form() {
    let answer = solved(
        "diff(y, t) = w; diff(w, t) = -9.81 m/s^2 - w/(2 s)",
        "t = 0 s; y = 0 m; w = 10 m/s",
        "t = 1 s, 2 s",
    );
    assert!(answer.contains(" m and "), "{answer}");
    assert!(answer.contains(" m/s and "), "{answer}");
    let heights = bounds(&answer, "y");
    encloses(&heights[0], "2*(10 + 981/50)*(1 - exp(-1/2)) - 981/50");
    encloses(&heights[1], "2*(10 + 981/50)*(1 - exp(-1)) - 981/25");
}

#[test]
fn a_time_in_another_unit_is_answered_in_the_unit_it_was_asked_in() {
    let answer = solved("diff(y, t) = -y/(1 min)", "t = 0 s; y = 3 kg", "t = 1 min");
    assert!(answer.contains("at t = 1 min"), "{answer}");
    encloses(&bounds(&answer, "y")[0], "3*exp(-1)");
}

#[test]
fn both_bounds_are_named_and_say_what_they_are() {
    let answer = solved("diff(y, t) = -y", "t = 0; y = 1", "t = 1");
    assert!(answer.contains("interval bound y"), "{answer}");
    assert!(
        answer.contains("the width of the enclosure at t = 1"),
        "{answer}"
    );
    assert!(answer.contains("step bound y"), "{answer}");
    assert!(
        answer.contains("the largest truncation one step adds"),
        "{answer}"
    );
    assert!(answer.contains("one step"), "{answer}");
}

#[test]
fn the_json_form_names_the_step_bound_and_the_interval_bound() {
    let output = calc(&[
        "--ode",
        "diff(y, t) = -y",
        "--initial",
        "t = 0; y = 1",
        "--at",
        "t = 1",
        "--json",
    ]);
    let json = text(&output);
    assert_eq!(output.status.code(), Some(0), "{json}");
    assert!(json.contains("\"step_bound\""), "{json}");
    assert!(json.contains("\"interval_bound\""), "{json}");
    assert!(json.contains("\"lohner_interval_taylor\""), "{json}");
}

#[test]
fn a_right_side_in_the_wrong_unit_is_refused_naming_both_units() {
    let output = calc(&[
        "--ode",
        "diff(x, t) = v; diff(v, t) = -x",
        "--initial",
        "t = 0 s; x = 1 m; v = 0 m/s",
        "--at",
        "t = 1 s",
    ]);
    let message = text(&output);
    assert_eq!(output.status.code(), Some(3), "{message}");
    assert!(
        message.contains("has the unit m, and v per t has the unit m/s²"),
        "{message}"
    );
}

#[test]
fn a_solution_that_blows_up_is_refused_where_it_stops() {
    let output = calc(&[
        "--ode",
        "diff(y, t) = y^2",
        "--initial",
        "t = 0; y = 1",
        "--at",
        "t = 2",
    ]);
    let message = text(&output);
    assert_eq!(output.status.code(), Some(4), "{message}");
    assert!(message.contains("up to t = 0.99999999999995"), "{message}");
    assert!(
        message.contains("rounded down to 15 significant digits"),
        "{message}"
    );
}

#[test]
fn ode_without_its_times_is_a_usage_error() {
    let output = calc(&["--ode", "diff(y, t) = -y", "--initial", "t = 0; y = 1"]);
    let message = text(&output);
    assert_eq!(output.status.code(), Some(2), "{message}");
    assert!(
        message.contains("are needed together, and --at is missing"),
        "{message}"
    );
}

#[test]
fn a_name_without_an_equation_is_refused_by_name() {
    let output = calc(&[
        "--ode",
        "diff(y, t) = -k*y",
        "--initial",
        "t = 0; y = 1",
        "--at",
        "t = 1",
    ]);
    let message = text(&output);
    assert_eq!(output.status.code(), Some(3), "{message}");
    assert!(message.contains("uses k"), "{message}");
}

#[test]
fn the_rows_read_in_german() {
    let output = calc_in(
        "de_DE.UTF-8",
        &[
            "--ode",
            "diff(y, t) = -y",
            "--initial",
            "t = 0; y = 1",
            "--at",
            "t = 1",
        ],
    );
    let answer = text(&output);
    assert!(answer.contains("Schrittschranke y"), "{answer}");
    assert!(answer.contains("Intervallschranke y"), "{answer}");
}

#[test]
fn an_ambiguous_right_side_says_what_it_can_mean() {
    let output = calc(&[
        "--ode",
        "diff(x, t) = v; diff(v, t) = -981/100 m/s^2",
        "--initial",
        "t = 0 s; x = 0 m; v = 10 m/s",
        "--at",
        "t = 1 s",
    ]);
    let message = text(&output);
    assert_eq!(output.status.code(), Some(3), "{message}");
    assert!(message.contains("can mean"), "{message}");
}

#[test]
fn two_forms_of_one_time_are_each_answered_as_written() {
    let answer = solved(
        "diff(y, t) = -y/(1 min)",
        "t = 0 s; y = 3 kg",
        "t = 1 min, 60 s",
    );
    assert!(answer.contains("at t = 1 min"), "{answer}");
    assert!(answer.contains("at t = 60 s"), "{answer}");
}
