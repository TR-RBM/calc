use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn terse(expression: &str) -> Output {
    Command::new(CALC)
        .args([expression, "--terse", "--locale", "en"])
        .env_clear()
        .output()
        .expect("calc runs")
}

fn shown(output: &Output) -> String {
    let mut text = String::from_utf8(output.stdout.clone()).expect("output is UTF-8");
    text.push_str(&String::from_utf8(output.stderr.clone()).expect("output is UTF-8"));
    text
}

#[test]
fn a_bare_calorie_is_refused_with_both_readings() {
    let output = terse("1 cal -> J");

    assert!(!output.status.success());
    assert!(
        shown(&output).contains("write one of cal_th, cal_IT"),
        "{}",
        shown(&output)
    );
}

#[test]
fn a_kilocalorie_is_refused_with_both_prefixed_readings() {
    assert!(shown(&terse("1 kcal -> J")).contains("write one of kcal_th, kcal_IT"));
}

#[test]
fn a_thermochemical_kilocalorie_is_exact() {
    assert_eq!(shown(&terse("1 kcal_th -> J")), "4184 J exact\n");
}

#[test]
fn an_ounce_names_its_masses_and_its_volumes() {
    assert!(shown(&terse("1 oz")).contains("write one of oz_av, oz_t, floz_US, floz_imp"));
}

#[test]
fn a_troy_ounce_is_exact_in_grams() {
    assert_eq!(shown(&terse("1 oz_t -> g")), "31.1034768 g exact\n");
}

#[test]
fn a_year_offers_the_julian_year_and_calendar_days() {
    assert!(shown(&terse("1 yr")).contains("write one of a, 365 d, 366 d"));
}

#[test]
fn the_metric_horsepower_does_not_shadow_the_petasiemens() {
    assert_eq!(shown(&terse("1 PS -> S")), "1e15 S exact\n");
}
