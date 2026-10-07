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
fn a_block_names_its_generator_and_says_it_is_not_for_cryptography() {
    let text = shown(&calc(&["philox4x32_10(1, 2, 3)", "--locale", "en"]));

    assert!(
        row(&text, "value").ends_with("[3725115218, 1715404623, 2270807266, 3184911688]"),
        "{text}"
    );
    assert!(text.contains("by Philox4x32-10"), "{text}");
    assert!(text.contains("DOI 10.1145/2063384.2063405"), "{text}");
    assert!(text.contains("the seed 1 as two 32-bit words"), "{text}");
    assert!(text.contains("not suitable for cryptography"), "{text}");
}

#[test]
fn the_published_vectors_come_back_through_seed_stream_and_index() {
    for (expression, expected) in [
        (
            "philox4x32_10(0, 0, 0)",
            "[1713891541, 3781805453, 3159862348, 2600524760]",
        ),
        (
            "philox4x32_10(2^64 - 1, 2^64 - 1, 2^64 - 1)",
            "[1083123565, 1103641358, 2718681030, 1834242557]",
        ),
    ] {
        let text = shown(&calc(&[expression, "--locale", "en"]));

        assert!(
            row(&text, "value").ends_with(expected),
            "{expression}: {text}"
        );
    }
}

#[test]
fn the_record_carries_the_seed_and_its_generator() {
    let text = shown(&calc(&["philox4x32_10(1, 2, 3)", "--json"]));

    let compact: String = text.split_whitespace().collect();
    assert!(
        compact.contains(r#""seed":{"value":"1","generator":"philox4x32_10_1"}"#),
        "{text}"
    );
}

#[test]
fn an_argument_outside_its_range_is_refused_by_name() {
    for (expression, expected) in [
        ("philox4x32_10(2^64, 0, 0)", "the seed of philox4x32_10"),
        ("philox4x32_10(0, -1, 0)", "the stream of philox4x32_10"),
        ("philox4x32_10(0, 0, 1/2)", "the index of philox4x32_10"),
        ("philox4x32_10(1 m, 0, 0)", "the seed of philox4x32_10"),
    ] {
        let output = calc(&[expression, "--locale", "en"]);

        assert_eq!(output.status.code(), Some(1), "{expression}");
        assert!(
            shown(&output).contains(expected),
            "{expression}: {}",
            shown(&output)
        );
    }
}

#[test]
fn a_block_is_a_list_inside_an_expression_and_keeps_its_notes() {
    let text = shown(&calc(&["at(philox4x32_10(1, 2, 3), 2)", "--locale", "en"]));

    assert!(row(&text, "value").ends_with("1715404623"), "{text}");
    assert!(text.contains("not suitable for cryptography"), "{text}");
}

#[test]
fn a_later_line_uses_the_block_through_its_line_reference() {
    let output = Command::new(CALC)
        .args(["--batch", "--locale", "en"])
        .env_clear()
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(b"philox4x32_10(1, 2, 3)\nat(r1, 2)\n")?;
            child.wait_with_output()
        })
        .expect("calc runs");
    let text = shown(&output);

    let second = text.split("r2").nth(1).unwrap_or_default();
    assert!(row(second, "value").ends_with("1715404623"), "{text}");
}

#[test]
fn an_unnamed_generator_is_refused_naming_the_one_to_write() {
    for expression in ["random()", "rand()", "random(1)"] {
        let output = calc(&[expression, "--locale", "en"]);

        assert_eq!(output.status.code(), Some(1), "{expression}");
        assert!(
            shown(&output).contains("write philox4x32_10(seed, stream, index)"),
            "{expression}: {}",
            shown(&output)
        );
    }
}

#[test]
fn a_machine_number_argument_is_refused_as_one() {
    let output = calc(&["philox4x32_10(to_f64(1), 0, 0)", "--locale", "en"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        shown(&output).contains("to_f64(1) is a machine number"),
        "{}",
        shown(&output)
    );
}
