use std::ops::Range;

use crate::error::{ParseError, ParseErrorKind};
use crate::lexer::{Token, TokenKind};

const CELSIUS_UNIT: &str = "degC";
const FAHRENHEIT_UNIT: &str = "degF";
const CELSIUS_NAME: &str = "celsius";
const FAHRENHEIT_NAME: &str = "fahrenheit";
const FROM_CELSIUS: &str = "from_celsius";
const FROM_FAHRENHEIT: &str = "from_fahrenheit";
const TO_CELSIUS: &str = "to_celsius";
const TO_FAHRENHEIT: &str = "to_fahrenheit";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmbiguousTemperatureSign {
    is_celsius: bool,
    is_conversion_target: bool,
    value_start: usize,
    value_end: usize,
    sign_start: usize,
    sign_end: usize,
}

impl AmbiguousTemperatureSign {
    pub fn sign(&self) -> &'static str {
        if self.is_celsius {
            CELSIUS_NAME
        } else {
            FAHRENHEIT_NAME
        }
    }

    pub fn written(&self, text: &str) -> String {
        text.get(self.value_start..self.sign_end)
            .unwrap_or_default()
            .to_string()
    }

    pub fn difference(&self, text: &str) -> String {
        let unit = if self.is_celsius {
            CELSIUS_UNIT
        } else {
            FAHRENHEIT_UNIT
        };
        let before = text.get(..self.sign_start).unwrap_or_default();
        let separator = if before.ends_with(char::is_whitespace) {
            ""
        } else {
            " "
        };
        [
            before,
            separator,
            unit,
            text.get(self.sign_end..).unwrap_or_default(),
        ]
        .concat()
    }

    pub fn reading(&self, text: &str) -> String {
        let operator = match (self.is_celsius, self.is_conversion_target) {
            (true, false) => FROM_CELSIUS,
            (false, false) => FROM_FAHRENHEIT,
            (true, true) => TO_CELSIUS,
            (false, true) => TO_FAHRENHEIT,
        };
        [
            text.get(..self.value_start).unwrap_or_default(),
            operator,
            "(",
            text.get(self.value_start..self.value_end)
                .unwrap_or_default(),
            ")",
            text.get(self.sign_end..).unwrap_or_default(),
        ]
        .concat()
    }
}

fn group_start(tokens: &[Token], closing: usize) -> Option<usize> {
    let mut depth = 0usize;
    for index in (0..=closing).rev() {
        match tokens[index].kind {
            TokenKind::RightParenthesis | TokenKind::RightBracket => depth += 1,
            TokenKind::LeftParenthesis | TokenKind::LeftBracket => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn value_before(tokens: &[Token], sign: usize) -> Option<Range<usize>> {
    let previous = sign.checked_sub(1)?;
    match tokens[previous].kind {
        TokenKind::Arrow => {
            let start = tokens[..previous]
                .iter()
                .rposition(|token| token.kind == TokenKind::Equals)
                .map_or(0, |equals| equals + 1);
            let value = tokens.get(start..previous)?;
            Some(value.first()?.span.start..value.last()?.span.end)
        }
        TokenKind::RightParenthesis | TokenKind::RightBracket => {
            let start = group_start(tokens, previous)?;
            Some(tokens[start].span.start..tokens[previous].span.end)
        }
        TokenKind::Number(_)
        | TokenKind::TypedLiteral(..)
        | TokenKind::Identifier(_)
        | TokenKind::Infinity
        | TokenKind::Superscript(_) => Some(tokens[previous].span.clone()),
        _ => None,
    }
}

pub(crate) fn check(tokens: &[Token]) -> Result<(), ParseError> {
    for (index, token) in tokens.iter().enumerate() {
        let is_celsius = match token.kind {
            TokenKind::CelsiusSign => true,
            TokenKind::FahrenheitSign => false,
            _ => continue,
        };
        let Some(value) = value_before(tokens, index) else {
            continue;
        };
        let is_conversion_target = tokens[index - 1].kind == TokenKind::Arrow;
        return Err(ParseError::new(
            ParseErrorKind::AmbiguousTemperatureSign(AmbiguousTemperatureSign {
                is_celsius,
                is_conversion_target,
                value_start: value.start,
                value_end: value.end,
                sign_start: token.span.start,
                sign_end: token.span.end,
            }),
            token.span.clone(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use calc_expr::ExprPool;

    use crate::{ParseErrorKind, parse_statement};

    fn refusal(text: &str) -> (String, String, String, String) {
        let error = parse_statement(&mut ExprPool::new(), text).unwrap_err();
        let ParseErrorKind::AmbiguousTemperatureSign(sign) = error.kind else {
            panic!("expected a temperature sign, found {:?}", error.kind);
        };
        (
            sign.sign().to_string(),
            sign.written(text),
            sign.difference(text),
            sign.reading(text),
        )
    }

    #[test]
    fn celsius_sign_after_a_number_names_both_readings() {
        assert_eq!(
            refusal("20 \u{00B0}C"),
            (
                "celsius".to_string(),
                "20 \u{00B0}C".to_string(),
                "20 degC".to_string(),
                "from_celsius(20)".to_string()
            )
        );
    }

    #[test]
    fn celsius_sign_without_a_space_keeps_the_space_in_the_difference() {
        assert_eq!(refusal("20\u{00B0}C").2, "20 degC".to_string());
    }

    #[test]
    fn fahrenheit_sign_names_its_own_operator() {
        assert_eq!(
            refusal("68 \u{00B0}F"),
            (
                "fahrenheit".to_string(),
                "68 \u{00B0}F".to_string(),
                "68 degF".to_string(),
                "from_fahrenheit(68)".to_string()
            )
        );
    }

    #[test]
    fn celsius_sign_after_a_group_takes_the_group_as_the_value() {
        assert_eq!(
            refusal("(a + b) \u{00B0}C").3,
            "from_celsius((a + b))".to_string()
        );
    }

    #[test]
    fn celsius_sign_as_a_conversion_target_reads_the_other_way() {
        assert_eq!(
            refusal("x -> \u{00B0}C"),
            (
                "celsius".to_string(),
                "x -> \u{00B0}C".to_string(),
                "x -> degC".to_string(),
                "to_celsius(x)".to_string()
            )
        );
    }

    #[test]
    fn celsius_sign_in_a_naming_keeps_the_name_outside_the_operator() {
        assert_eq!(
            refusal("t = 20 \u{00B0}C").3,
            "t = from_celsius(20)".to_string()
        );
    }

    #[test]
    fn celsius_sign_as_a_conversion_target_in_a_naming_keeps_the_name() {
        assert_eq!(
            refusal("t = x -> \u{00B0}C").3,
            "t = to_celsius(x)".to_string()
        );
    }

    #[test]
    fn celsius_sign_with_nothing_before_it_stays_an_ordinary_parse_error() {
        let error = parse_statement(&mut ExprPool::new(), "\u{00B0}C").unwrap_err();
        assert!(!matches!(
            error.kind,
            ParseErrorKind::AmbiguousTemperatureSign(_)
        ));
    }
}
