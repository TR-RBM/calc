use std::ops::Range;

use crate::error::{ParseError, ParseErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecognisedAttempt {
    DoubleStarPower,
    CommaBetweenDigits,
    GroupingComma,
    AbsoluteValueBars,
    TimesSign,
    DivisionSign,
    ColonDivision,
    ColonOrTime,
    DegreeAfterName,
}

const DOUBLE_STAR: &str = "**";
const COMMA: char = ',';
const COLON: char = ':';
const VERTICAL_BAR: char = '|';
const MAPS_TO_TAIL: &str = "->";
const TIMES_SIGN: char = '\u{00D7}';
const DIVISION_SIGN: char = '\u{00F7}';
const OPENING: [char; 2] = ['(', '['];
const CLOSING: [char; 2] = [')', ']'];
const GROUP_DIGITS: usize = 3;
const TIME_MINUTE_DIGITS: usize = 2;
const TIME_HOUR_DIGITS_LARGEST: usize = 2;
const ABSOLUTE_OPENING: &str = "abs(";
const ABSOLUTE_CLOSING: &str = ")";
const DEGREE_SIGN: char = '\u{00B0}';
const DEGREE_REPLACEMENT: &str = "* 1\u{00B0}";
const DEGREE_CORRECTION: &str = " * 1\u{00B0}";

impl RecognisedAttempt {
    pub const ALL: [RecognisedAttempt; 9] = [
        RecognisedAttempt::DoubleStarPower,
        RecognisedAttempt::CommaBetweenDigits,
        RecognisedAttempt::GroupingComma,
        RecognisedAttempt::AbsoluteValueBars,
        RecognisedAttempt::TimesSign,
        RecognisedAttempt::DivisionSign,
        RecognisedAttempt::ColonDivision,
        RecognisedAttempt::ColonOrTime,
        RecognisedAttempt::DegreeAfterName,
    ];

    pub fn name(self) -> &'static str {
        match self {
            RecognisedAttempt::DoubleStarPower => "double_star_power",
            RecognisedAttempt::CommaBetweenDigits => "comma_between_digits",
            RecognisedAttempt::GroupingComma => "grouping_comma",
            RecognisedAttempt::AbsoluteValueBars => "absolute_value_bars",
            RecognisedAttempt::TimesSign => "times_sign",
            RecognisedAttempt::DivisionSign => "division_sign",
            RecognisedAttempt::ColonDivision => "colon_division",
            RecognisedAttempt::ColonOrTime => "colon_or_time",
            RecognisedAttempt::DegreeAfterName => "degree_after_name",
        }
    }

    pub fn replacement(self) -> Option<&'static str> {
        match self {
            RecognisedAttempt::DoubleStarPower => Some("^"),
            RecognisedAttempt::CommaBetweenDigits => Some("."),
            RecognisedAttempt::GroupingComma | RecognisedAttempt::ColonOrTime => None,
            RecognisedAttempt::AbsoluteValueBars => Some("abs"),
            RecognisedAttempt::TimesSign => Some("*"),
            RecognisedAttempt::DivisionSign | RecognisedAttempt::ColonDivision => Some("/"),
            RecognisedAttempt::DegreeAfterName => Some(DEGREE_REPLACEMENT),
        }
    }
}

fn digits_before(characters: &[(usize, char)], position: usize) -> usize {
    characters[..position]
        .iter()
        .rev()
        .take_while(|(_, c)| c.is_ascii_digit())
        .count()
}

fn digits_after(characters: &[(usize, char)], position: usize) -> usize {
    characters[position + 1..]
        .iter()
        .take_while(|(_, c)| c.is_ascii_digit())
        .count()
}

fn is_operand_end(character: char) -> bool {
    character.is_alphanumeric() || CLOSING.contains(&character)
}

fn is_operand_start(character: char) -> bool {
    character.is_alphanumeric() || OPENING.contains(&character)
}

fn neighbour(characters: &[(usize, char)], position: usize, forward: bool) -> Option<char> {
    let mut index = position;
    loop {
        index = if forward {
            index.checked_add(1)?
        } else {
            index.checked_sub(1)?
        };
        let (_, character) = characters.get(index)?;
        if !character.is_whitespace() {
            return Some(*character);
        }
    }
}

fn found(text: &str) -> Vec<(Range<usize>, RecognisedAttempt)> {
    let mut attempts = Vec::new();
    let mut depth = 0usize;
    let mut open_bar: Option<(usize, usize, usize)> = None;
    let characters: Vec<(usize, char)> = text.char_indices().collect();
    for (position, &(offset, character)) in characters.iter().enumerate() {
        let end = offset + character.len_utf8();
        if OPENING.contains(&character) {
            depth += 1;
        } else if CLOSING.contains(&character) {
            depth = depth.saturating_sub(1);
        }
        match character {
            '*' if text[offset..].starts_with(DOUBLE_STAR) => {
                let already = attempts.last().is_some_and(
                    |(range, _): &(Range<usize>, RecognisedAttempt)| range.end > offset,
                );
                if !already {
                    attempts.push((
                        offset..offset + DOUBLE_STAR.len(),
                        RecognisedAttempt::DoubleStarPower,
                    ));
                }
            }
            COMMA if depth == 0 => {
                let before = digits_before(&characters, position);
                let after = digits_after(&characters, position);
                if before > 0 && after == GROUP_DIGITS {
                    attempts.push((offset..end, RecognisedAttempt::GroupingComma));
                } else if before > 0 && after > 0 {
                    attempts.push((offset..end, RecognisedAttempt::CommaBetweenDigits));
                }
            }
            COLON => {
                let before = digits_before(&characters, position);
                let after = digits_after(&characters, position);
                let is_time =
                    (1..=TIME_HOUR_DIGITS_LARGEST).contains(&before) && after == TIME_MINUTE_DIGITS;
                let between_operands = neighbour(&characters, position, false)
                    .is_some_and(is_operand_end)
                    && neighbour(&characters, position, true).is_some_and(is_operand_start);
                if between_operands && is_time {
                    attempts.push((offset..end, RecognisedAttempt::ColonOrTime));
                } else if between_operands {
                    attempts.push((offset..end, RecognisedAttempt::ColonDivision));
                }
            }
            VERTICAL_BAR if !text[end..].starts_with(MAPS_TO_TAIL) => match open_bar.take() {
                Some((open_offset, open_end, open_depth)) if open_depth == depth => {
                    if !text[open_end..offset].trim().is_empty() {
                        attempts
                            .push((open_offset..open_end, RecognisedAttempt::AbsoluteValueBars));
                        attempts.push((offset..end, RecognisedAttempt::AbsoluteValueBars));
                    }
                }
                _ => {
                    let after_digit = position
                        .checked_sub(1)
                        .and_then(|index| characters.get(index))
                        .is_some_and(|(_, c)| c.is_ascii_digit());
                    if !after_digit {
                        open_bar = Some((offset, end, depth));
                    }
                }
            },
            DEGREE_SIGN
                if position
                    .checked_sub(1)
                    .and_then(|index| characters.get(index))
                    .is_some_and(|(_, c)| c.is_alphabetic()) =>
            {
                attempts.push((offset..end, RecognisedAttempt::DegreeAfterName));
            }
            TIMES_SIGN => attempts.push((offset..end, RecognisedAttempt::TimesSign)),
            DIVISION_SIGN => attempts.push((offset..end, RecognisedAttempt::DivisionSign)),
            _ => {}
        }
    }
    attempts.sort_by_key(|(range, _)| range.start);
    attempts
}

fn replaced_text(text: &str) -> String {
    let mut corrected = String::with_capacity(text.len() + 8);
    let mut copied = 0;
    let mut inside_bars = false;
    for (range, attempt) in found(text) {
        corrected.push_str(&text[copied..range.start]);
        let written = match attempt {
            RecognisedAttempt::DegreeAfterName => DEGREE_CORRECTION,
            RecognisedAttempt::AbsoluteValueBars if inside_bars => ABSOLUTE_CLOSING,
            RecognisedAttempt::AbsoluteValueBars => ABSOLUTE_OPENING,
            other => other.replacement().unwrap_or(&text[range.clone()]),
        };
        if attempt == RecognisedAttempt::AbsoluteValueBars {
            inside_bars = !inside_bars;
        }
        corrected.push_str(written);
        copied = range.end;
    }
    corrected.push_str(&text[copied..]);
    corrected
}

pub fn corrected_text(text: &str) -> Option<String> {
    let corrected = replaced_text(text);
    let parses = corrected != text
        && crate::parse_statement(&mut calc_expr::ExprPool::new(), &corrected).is_ok();
    parses.then_some(corrected)
}

pub(crate) fn recognise(text: &str, error: ParseError) -> ParseError {
    let position = error.span.start;
    found(text)
        .into_iter()
        .find(|(range, _)| range.start <= position && position < range.end.max(range.start + 1))
        .map_or(error, |(span, attempt)| {
            ParseError::new(ParseErrorKind::RecognisedAttempt(attempt), span)
        })
}

#[cfg(test)]
mod tests {
    use calc_expr::ExprPool;

    use super::*;
    use crate::parse_statement;

    fn kind(text: &str) -> ParseErrorKind {
        parse_statement(&mut ExprPool::new(), text)
            .unwrap_err()
            .kind
    }

    fn error(text: &str) -> ParseError {
        parse_statement(&mut ExprPool::new(), text).unwrap_err()
    }

    #[test]
    fn double_star_is_recognised_as_a_power() {
        assert_eq!(
            error("2**3"),
            ParseError::new(
                ParseErrorKind::RecognisedAttempt(RecognisedAttempt::DoubleStarPower),
                1..3
            )
        );
    }

    #[test]
    fn comma_between_digits_at_the_top_is_recognised_as_a_decimal_point() {
        assert_eq!(
            error("2,5 + 1"),
            ParseError::new(
                ParseErrorKind::RecognisedAttempt(RecognisedAttempt::CommaBetweenDigits),
                1..2
            )
        );
    }

    #[test]
    fn vertical_bars_are_recognised_as_absolute_value() {
        assert_eq!(
            kind("|x - 3|"),
            ParseErrorKind::RecognisedAttempt(RecognisedAttempt::AbsoluteValueBars)
        );
    }

    #[test]
    fn times_sign_is_recognised_as_multiplication() {
        assert_eq!(
            kind("2 \u{00D7} 3"),
            ParseErrorKind::RecognisedAttempt(RecognisedAttempt::TimesSign)
        );
    }

    #[test]
    fn division_sign_is_recognised_as_division() {
        assert_eq!(
            kind("6 \u{00F7} 3"),
            ParseErrorKind::RecognisedAttempt(RecognisedAttempt::DivisionSign)
        );
    }

    #[test]
    fn comma_inside_call_parentheses_is_an_argument_separator() {
        assert!(parse_statement(&mut ExprPool::new(), "max(2,5)").is_ok());
    }

    #[test]
    fn comma_inside_an_array_is_an_element_separator() {
        assert!(parse_statement(&mut ExprPool::new(), "[2,5]").is_ok());
    }

    #[test]
    fn comma_with_a_space_is_not_recognised() {
        assert_eq!(kind("2, 5"), ParseErrorKind::UnexpectedToken);
    }

    #[test]
    fn number_letter_number_is_not_recognised() {
        assert!(!matches!(kind("2x3"), ParseErrorKind::RecognisedAttempt(_)));
    }

    #[test]
    fn colon_between_operands_is_recognised_as_division() {
        assert_eq!(
            kind("6 : 3"),
            ParseErrorKind::RecognisedAttempt(RecognisedAttempt::ColonDivision)
        );
    }

    #[test]
    fn colon_that_may_be_a_time_names_both_readings() {
        assert_eq!(
            kind("80:20"),
            ParseErrorKind::RecognisedAttempt(RecognisedAttempt::ColonOrTime)
        );
    }

    #[test]
    fn nested_bars_give_no_correction_that_fails() {
        assert_eq!(corrected_text("||x|-1|"), None);
    }

    #[test]
    fn comma_before_three_digits_is_recognised_as_grouping() {
        assert_eq!(
            kind("1,000 + 1"),
            ParseErrorKind::RecognisedAttempt(RecognisedAttempt::GroupingComma)
        );
    }

    #[test]
    fn bar_in_a_point_is_not_recognised() {
        assert!(!matches!(
            kind("(2|5)"),
            ParseErrorKind::RecognisedAttempt(_)
        ));
    }

    #[test]
    fn bar_of_divisibility_is_not_recognised() {
        assert!(!matches!(
            kind("3|12"),
            ParseErrorKind::RecognisedAttempt(_)
        ));
    }

    #[test]
    fn letter_x_between_spaced_numbers_is_not_recognised() {
        assert!(!matches!(
            kind("2 x 3"),
            ParseErrorKind::RecognisedAttempt(_)
        ));
    }

    #[test]
    fn middle_dot_is_accepted_and_never_an_attempt() {
        assert!(parse_statement(&mut ExprPool::new(), "2 \u{00B7} 3").is_ok());
    }

    #[test]
    fn minus_sign_is_accepted_and_never_an_attempt() {
        assert!(parse_statement(&mut ExprPool::new(), "5 \u{2212} 3").is_ok());
    }

    #[test]
    fn maps_to_arrow_is_not_a_vertical_bar_attempt() {
        assert!(parse_statement(&mut ExprPool::new(), "x |-> x^2").is_ok());
    }

    #[test]
    fn corrected_text_replaces_every_attempt_of_the_input() {
        assert_eq!(corrected_text("2,5 + 3,7").as_deref(), Some("2.5 + 3.7"));
    }

    #[test]
    fn corrected_text_writes_a_pair_of_bars_as_abs() {
        assert_eq!(
            corrected_text("|x - 3| * 2").as_deref(),
            Some("abs(x - 3) * 2")
        );
    }

    #[test]
    fn corrected_text_leaves_a_grouping_comma_as_written() {
        assert_eq!(corrected_text("1,000"), None);
    }

    #[test]
    fn corrected_text_parses_for_every_listed_attempt() {
        for text in [
            "10**6",
            "2,5 + 1",
            "|x - 3|",
            "2 \u{00D7} 3",
            "6 \u{00F7} 3",
        ] {
            assert!(corrected_text(text).is_some(), "{text}");
        }
    }

    #[test]
    fn degree_sign_after_a_name_is_recognised_with_its_correction() {
        assert_eq!(
            kind("x\u{00B0}"),
            ParseErrorKind::RecognisedAttempt(RecognisedAttempt::DegreeAfterName)
        );
        assert_eq!(
            corrected_text("x\u{00B0}"),
            Some("x * 1\u{00B0}".to_string())
        );
    }

    #[test]
    fn degree_sign_after_a_number_is_no_attempt() {
        assert!(parse_statement(&mut ExprPool::new(), "30\u{00B0}").is_ok());
    }

    #[test]
    fn every_replacement_is_accepted_by_the_lexer() {
        for attempt in RecognisedAttempt::ALL {
            let text = match attempt {
                RecognisedAttempt::AbsoluteValueBars => "abs(2)".to_string(),
                RecognisedAttempt::CommaBetweenDigits
                | RecognisedAttempt::GroupingComma
                | RecognisedAttempt::ColonOrTime => "2.5".to_string(),
                RecognisedAttempt::DegreeAfterName => "x * 1\u{00B0}".to_string(),
                other => format!("2 {} 3", other.replacement().unwrap_or_default()),
            };
            assert!(
                parse_statement(&mut ExprPool::new(), &text).is_ok(),
                "{text}"
            );
        }
    }
}
