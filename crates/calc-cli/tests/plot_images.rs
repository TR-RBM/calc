#![cfg(target_arch = "x86_64")]

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn written_image(input: &str, name: &str) -> Vec<u8> {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("plot-images");
    std::fs::create_dir_all(&directory).expect("scratch directory is created");
    let path = directory.join(name);
    let output = Command::new(CALC)
        .args([
            "plot",
            input,
            "--output",
            path.to_str().expect("path is UTF-8"),
            "--size",
            "400x300",
            "--locale",
            "en",
        ])
        .env_clear()
        .output()
        .expect("calc runs");
    assert!(output.status.success());
    std::fs::read(&path).expect("image is written")
}

fn written_view_image(input: &str, view: (&str, &str), name: &str) -> Vec<u8> {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("plot-images");
    std::fs::create_dir_all(&directory).expect("scratch directory is created");
    let path = directory.join(name);
    let mut arguments = vec!["plot", input, "--view", view.0];
    if !view.1.is_empty() {
        arguments.push(view.1);
    }
    arguments.extend([
        "--output",
        path.to_str().expect("path is UTF-8"),
        "--size",
        "400x300",
        "--locale",
        "en",
    ]);
    let output = Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs");
    assert!(output.status.success());
    std::fs::read(&path).expect("image is written")
}

fn written_param_image(input: &str, parameter: &str, name: &str) -> Vec<u8> {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("plot-images");
    std::fs::create_dir_all(&directory).expect("scratch directory is created");
    let path = directory.join(name);
    let output = Command::new(CALC)
        .args([
            "plot",
            input,
            "--param",
            parameter,
            "--output",
            path.to_str().expect("path is UTF-8"),
            "--size",
            "400x300",
            "--locale",
            "en",
        ])
        .env_clear()
        .output()
        .expect("calc runs");
    assert!(output.status.success());
    std::fs::read(&path).expect("image is written")
}

fn first_difference(left: &[u8], right: &[u8]) -> Option<usize> {
    left.iter()
        .zip(right)
        .position(|(left, right)| left != right)
        .or_else(|| (left.len() != right.len()).then(|| left.len().min(right.len())))
}

#[test]
fn curve_of_sine_matches_its_golden_image() {
    assert_eq!(
        first_difference(
            &written_image("sin(x)", "curve.png"),
            include_bytes!("plots/curve.png")
        ),
        None
    );
}

#[test]
fn grid_of_a_product_matches_its_golden_image() {
    assert_eq!(
        first_difference(
            &written_image("x*y", "grid.png"),
            include_bytes!("plots/grid.png")
        ),
        None
    );
}

#[test]
fn complex_grid_matches_its_golden_image() {
    assert_eq!(
        first_difference(
            &written_image("z*z + i", "complex_grid.png"),
            include_bytes!("plots/complex_grid.png")
        ),
        None
    );
}

#[test]
fn curve_over_an_axis_in_seconds_matches_its_golden_image() {
    assert_eq!(
        first_difference(
            &written_view_image("x * 1 m/s", ("0..2", "s"), "axis_unit.png"),
            include_bytes!("plots/axis_unit.png")
        ),
        None
    );
}

#[test]
fn curve_of_a_function_head_with_a_parameter_matches_its_golden_image() {
    assert_eq!(
        first_difference(
            &written_param_image(
                "psi(x) = sqrt(2)*sin(n*pi*x)",
                "n=3",
                "param_function_head.png"
            ),
            include_bytes!("plots/param_function_head.png")
        ),
        None
    );
}

const BREAK_EVEN: &str = "50940.16*x + 134583.33*min(x,36) \
+ 67291.67*(min(max(x-84,0),36) + min(max(x-168,0),36)) - 169097.22*x";

#[test]
fn curve_of_an_expression_without_units_keeps_the_names_it_had() {
    assert_eq!(
        first_difference(
            &written_view_image(BREAK_EVEN, ("0..180", ""), "break_even.png"),
            include_bytes!("plots/break_even.png")
        ),
        None
    );
}

fn plot_to(path: &str) -> std::process::Output {
    Command::new(CALC)
        .args(["plot", "x^2", "--output", path, "--locale", "en"])
        .env_clear()
        .output()
        .expect("calc runs")
}

#[test]
fn an_output_path_whose_directory_is_missing_is_refused_by_name() {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("plot-images")
        .join("no-such-directory")
        .join("m.png");
    let output = plot_to(path.to_str().expect("path is UTF-8"));

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("there is no such directory"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn an_output_path_that_may_not_be_written_is_refused_by_name() {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("plot-read-only");
    std::fs::create_dir_all(&directory).expect("scratch directory is created");
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500))
        .expect("permissions are set");
    let output = plot_to(directory.join("m.png").to_str().expect("path is UTF-8"));
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
        .expect("permissions are restored");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("permission denied"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
