use std::process::{Command, Output};

const CALC: &str = env!("CARGO_BIN_EXE_calc");

fn calc(arguments: &[&str]) -> Output {
    Command::new(CALC)
        .args(arguments)
        .env_clear()
        .output()
        .expect("calc runs")
}

fn terse(expression: &str) -> String {
    let output = calc(&[expression, "--terse", "--locale", "en"]);
    let mut shown = String::from_utf8(output.stdout).expect("output is UTF-8");
    shown.push_str(&String::from_utf8(output.stderr).expect("output is UTF-8"));
    shown
}

#[test]
fn a_question_in_hexadecimal_is_answered_in_hexadecimal() {
    assert_eq!(terse("0xFF000000 + 0x00AF0100"), "0xFFAF0100 exact\n");
}

#[test]
fn a_hexadecimal_answer_notes_its_decimal_value() {
    let output = calc(&["0xFF000000 + 0x00AF0100", "--locale", "en"]);
    let shown = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(shown.contains("in decimal 4289659136"), "{shown}");
}

#[test]
fn binary_is_grouped_in_fours() {
    assert_eq!(
        terse("0xFFAF0100 -> bin"),
        "0b1111_1111_1010_1111_0000_0001_0000_0000 exact\n"
    );
}

#[test]
fn a_width_pads_with_zeros() {
    assert_eq!(terse("0xAF0100 -> hex 32"), "0x00AF0100 exact\n");
}

#[test]
fn a_signed_type_writes_twos_complement() {
    assert_eq!(terse("-1 -> hex i16"), "0xFFFF exact\n");
}

#[test]
fn a_negative_number_without_a_type_keeps_its_sign() {
    assert_eq!(terse("-1 -> hex"), "-0x1 exact\n");
}

#[test]
fn bytes_are_written_in_the_order_named() {
    assert_eq!(terse("0xFFAF0100 -> bytes be"), "FF AF 01 00 exact\n");
    assert_eq!(terse("0xFFAF0100 -> bytes le"), "00 01 AF FF exact\n");
}

#[test]
fn bytes_without_an_order_are_refused_naming_both() {
    let shown = terse("0xFFAF0100 -> bytes");
    assert!(
        shown.contains("bytes be") && shown.contains("bytes le"),
        "{shown}"
    );
}

#[test]
fn a_value_outside_its_type_is_refused_with_the_range() {
    assert!(terse("200 -> i8").contains("-128 to 127"));
    assert!(terse("0xFF -> i8").contains("-128 to 127"));
}

#[test]
fn wrap_reads_a_bit_pattern_in_a_type() {
    assert_eq!(terse("wrap(0xFF, i8)"), "-1 exact\n");
    assert_eq!(terse("wrap(200, i8)"), "-56 exact\n");
}

#[test]
fn bit_operations_are_exact() {
    assert_eq!(terse("bitand(0b1100, 0b1010)"), "0b1000 exact\n");
    assert_eq!(terse("bitxor(5, 3)"), "6 exact\n");
    assert_eq!(terse("shl(1, 4)"), "16 exact\n");
    assert_eq!(terse("bitnot(0, u8) -> hex"), "0xFF exact\n");
}

#[test]
fn the_caret_stays_the_power() {
    assert_eq!(terse("2^3"), "8 exact\n");
}

#[test]
fn a_bitwise_operation_on_a_negative_number_advises_an_unsigned_wrap() {
    let shown = terse("bitand(-1, 3)");
    assert!(shown.contains("wrap(x, u32)"), "{shown}");
    assert!(!shown.contains("i32"), "{shown}");
}

#[test]
fn the_advised_unsigned_wrap_gives_the_bits() {
    assert_eq!(terse("bitand(wrap(-1, u32), 1)"), "1 exact\n");
}

#[test]
fn a_signed_wrap_of_a_negative_number_is_still_refused() {
    let shown = terse("bitand(wrap(-1, i32), 1)");
    assert!(shown.contains("needs a number from 0"), "{shown}");
    assert!(shown.contains("wrap(x, u32)"), "{shown}");
}

#[test]
fn a_left_shift_of_a_negative_number_is_refused_as_a_right_shift_is() {
    assert!(terse("shl(-1, 3)").contains("needs a number from 0"));
    assert!(terse("shr(-8, 1)").contains("needs a number from 0"));
}

#[test]
fn a_refusal_repeats_an_unsigned_type_as_it_was_written() {
    let shown = terse("-1 -> hex u8");
    assert!(shown.contains("-1 -> hex u8 lies outside u8"), "{shown}");
}

#[test]
fn a_refusal_repeats_a_width_as_it_was_written() {
    let shown = terse("-1 -> hex 8");
    assert!(shown.contains("-1 -> hex 8 lies outside u8"), "{shown}");
}

#[test]
fn bytes_of_a_signed_type_that_is_not_whole_bytes_name_the_signed_type() {
    let shown = terse("-2 -> bytes le i12");
    assert!(
        shown.contains("i12 is not a whole number of bytes"),
        "{shown}"
    );
}

#[test]
fn a_fraction_asked_in_hexadecimal_says_it_is_written_in_decimal() {
    let output = calc(&["0x1A / 0x3", "--locale", "en"]);
    let shown = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(shown.contains("26/3"), "{shown}");
    assert!(shown.contains("written in decimal"), "{shown}");
}

#[test]
fn a_value_in_a_type_asked_in_hexadecimal_says_it_is_written_in_decimal() {
    let output = calc(&["wrap(0xFF, u8)", "--locale", "en"]);
    let shown = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(shown.contains("written in decimal"), "{shown}");
}

#[test]
fn a_hexadecimal_answer_does_not_say_it_is_written_in_decimal() {
    let output = calc(&["0xFF000000 + 0x00AF0100", "--locale", "en"]);
    let shown = String::from_utf8(output.stdout).expect("UTF-8");
    assert!(!shown.contains("written in decimal"), "{shown}");
}

#[test]
fn an_underscore_stands_only_between_digits() {
    assert_eq!(terse("0xFF_00"), "0xFF00 exact\n");
    assert!(terse("0xFF_").contains("error"));
    assert!(terse("0xF__F").contains("error"));
    assert!(terse("0x_FF").contains("error"));
}

#[test]
fn the_widest_type_is_accepted_and_one_bit_wider_is_not() {
    assert!(terse("1 -> hex u65536").contains("exact"));
    assert!(terse("1 -> hex u65537").contains("error"));
}

#[test]
fn the_largest_shift_is_accepted_and_one_more_is_refused_naming_the_limit() {
    assert!(terse("shl(1, 65536)").contains("exact"));
    let shown = terse("shl(1, 65537)");
    assert!(
        shown.contains("shifts by more than 65536 places"),
        "{shown}"
    );
}

#[test]
fn a_hexadecimal_amount_of_bytes_converts() {
    assert_eq!(terse("0x1000 B -> KiB"), "4 KiB exact\n");
}
