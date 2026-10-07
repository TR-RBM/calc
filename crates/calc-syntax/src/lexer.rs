use std::ops::Range;

use crate::error::{ParseError, ParseErrorKind};
use crate::names::{DEGREE_SIGN, DEGREE_SIGN_CHARACTER};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TypedLiteralKind {
    Rational,
    F32,
    F64,
    Chemistry,
    Nuclear,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Number(String),
    TypedLiteral(TypedLiteralKind, String),
    Identifier(String),
    Superscript(i64),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    PlusMinus,
    Bang,
    Percent,
    LeftParenthesis,
    RightParenthesis,
    LeftBracket,
    RightBracket,
    Comma,
    Semicolon,
    Equals,
    DoubleEquals,
    NotEquals,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Arrow,
    MapsTo,
    DotDot,
    SquareRoot,
    Integral,
    Summation,
    ProductSign,
    Infinity,
    CelsiusSign,
    FahrenheitSign,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) span: Range<usize>,
    pub(crate) spaced_before: bool,
}

const SUPERSCRIPT_DIGITS: [char; 10] = [
    '\u{2070}', '\u{00B9}', '\u{00B2}', '\u{00B3}', '\u{2074}', '\u{2075}', '\u{2076}', '\u{2077}',
    '\u{2078}', '\u{2079}',
];
const SUPERSCRIPT_MINUS: char = '\u{207B}';
const COMBINING_MARK_RANGES: [(u32, u32); 5] = [
    (0x0300, 0x036F),
    (0x1AB0, 0x1AFF),
    (0x1DC0, 0x1DFF),
    (0x20D0, 0x20FF),
    (0xFE20, 0xFE2F),
];
const TYPED_LITERAL_PREFIXES: [(&str, TypedLiteralKind); 5] = [
    ("q", TypedLiteralKind::Rational),
    ("f32", TypedLiteralKind::F32),
    ("f64", TypedLiteralKind::F64),
    ("chem", TypedLiteralKind::Chemistry),
    ("nuc", TypedLiteralKind::Nuclear),
];

pub(crate) fn superscript_digit_value(character: char) -> Option<i64> {
    SUPERSCRIPT_DIGITS
        .iter()
        .position(|digit| *digit == character)
        .and_then(|position| i64::try_from(position).ok())
}

pub(crate) fn superscript_digit(value: u32) -> Option<char> {
    usize::try_from(value)
        .ok()
        .and_then(|index| SUPERSCRIPT_DIGITS.get(index).copied())
}

pub(crate) fn superscript_minus() -> char {
    SUPERSCRIPT_MINUS
}

fn is_combining_mark(character: char) -> bool {
    let code = u32::from(character);
    COMBINING_MARK_RANGES
        .iter()
        .any(|(first, last)| (*first..=*last).contains(&code))
}

pub(crate) fn is_identifier_start(character: char) -> bool {
    character.is_alphabetic()
}

pub(crate) fn is_identifier_continue(character: char) -> bool {
    character.is_alphanumeric() && superscript_digit_value(character).is_none()
        || character == '_'
        || is_combining_mark(character)
}

struct Scanner<'text> {
    text: &'text str,
    position: usize,
}

impl Scanner<'_> {
    fn peek(&self) -> Option<char> {
        self.text[self.position..].chars().next()
    }

    fn peek_second(&self) -> Option<char> {
        self.text[self.position..].chars().nth(1)
    }

    fn advance(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.position += character.len_utf8();
        Some(character)
    }

    fn take_while(&mut self, predicate: impl Fn(char) -> bool) -> &str {
        let start = self.position;
        while self.peek().is_some_and(&predicate) {
            self.advance();
        }
        &self.text[start..self.position]
    }
}

pub fn written_radix(text: &str) -> Option<u32> {
    let tokens = tokenize(text).ok()?;
    let mut radixes = tokens.iter().filter_map(|token| match &token.kind {
        TokenKind::Number(written) => Some(crate::decimal::radix_prefix(written).unwrap_or(10)),
        _ => None,
    });
    let first = radixes.next()?;
    (first != 10 && radixes.all(|radix| radix == first)).then_some(first)
}

pub(crate) fn tokenize(text: &str) -> Result<Vec<Token>, ParseError> {
    let mut scanner = Scanner { text, position: 0 };
    let mut tokens = Vec::new();
    let mut spaced_before = false;
    while let Some(character) = scanner.peek() {
        let start = scanner.position;
        if character.is_whitespace() {
            scanner.advance();
            spaced_before = true;
            continue;
        }
        if character == '#' {
            break;
        }
        let kind = scan_token(&mut scanner, character, start)?;
        tokens.push(Token {
            kind,
            span: start..scanner.position,
            spaced_before,
        });
        spaced_before = false;
    }
    Ok(tokens)
}

pub(crate) const MAPS_TO_START: char = '|';

pub(crate) const CONSTANT_LETTERS: [(&str, &str); 2] = [("\u{212F}", "e"), ("\u{2148}", "i")];

pub(crate) const TWO_CHARACTER_TOKENS: [(&str, TokenKind); 7] = [
    ("+-", TokenKind::PlusMinus),
    ("->", TokenKind::Arrow),
    ("==", TokenKind::DoubleEquals),
    ("!=", TokenKind::NotEquals),
    ("<=", TokenKind::LessOrEqual),
    (">=", TokenKind::GreaterOrEqual),
    ("..", TokenKind::DotDot),
];

pub(crate) const SINGLE_CHARACTER_TOKENS: [(char, TokenKind); 31] = [
    ('+', TokenKind::Plus),
    ('-', TokenKind::Minus),
    ('\u{2212}', TokenKind::Minus),
    ('*', TokenKind::Star),
    ('\u{00B7}', TokenKind::Star),
    ('/', TokenKind::Slash),
    ('^', TokenKind::Caret),
    ('\u{00B1}', TokenKind::PlusMinus),
    ('!', TokenKind::Bang),
    ('%', TokenKind::Percent),
    ('(', TokenKind::LeftParenthesis),
    (')', TokenKind::RightParenthesis),
    ('[', TokenKind::LeftBracket),
    (']', TokenKind::RightBracket),
    (',', TokenKind::Comma),
    (';', TokenKind::Semicolon),
    ('=', TokenKind::Equals),
    ('<', TokenKind::Less),
    ('>', TokenKind::Greater),
    ('\u{2260}', TokenKind::NotEquals),
    ('\u{2264}', TokenKind::LessOrEqual),
    ('\u{2265}', TokenKind::GreaterOrEqual),
    ('\u{2192}', TokenKind::Arrow),
    ('\u{21A6}', TokenKind::MapsTo),
    ('\u{221A}', TokenKind::SquareRoot),
    ('\u{222B}', TokenKind::Integral),
    ('\u{2211}', TokenKind::Summation),
    ('\u{03A3}', TokenKind::Summation),
    ('\u{220F}', TokenKind::ProductSign),
    ('\u{03A0}', TokenKind::ProductSign),
    ('\u{221E}', TokenKind::Infinity),
];

fn scan_token(
    scanner: &mut Scanner<'_>,
    character: char,
    start: usize,
) -> Result<TokenKind, ParseError> {
    if character.is_ascii_digit() {
        return Ok(scan_number(scanner));
    }
    if let Some((_, kind)) = SINGLE_CHARACTER_TOKENS
        .iter()
        .find(|(candidate, _)| *candidate == character && candidate.is_alphabetic())
    {
        scanner.advance();
        return Ok(kind.clone());
    }
    if is_identifier_start(character) {
        let name = scanner.take_while(is_identifier_continue).to_string();
        let name = CONSTANT_LETTERS
            .iter()
            .find(|(letter, _)| *letter == name)
            .map_or(name, |(_, ascii)| (*ascii).to_string());
        if scanner.peek() == Some('\'')
            && let Some((_, kind)) = TYPED_LITERAL_PREFIXES
                .iter()
                .find(|(prefix, _)| *prefix == name)
        {
            return scan_typed_literal(scanner, kind.clone(), start);
        }
        return Ok(TokenKind::Identifier(name));
    }
    if superscript_digit_value(character).is_some() || character == SUPERSCRIPT_MINUS {
        return scan_superscript(scanner, start);
    }
    scanner.advance();
    let next = scanner.peek();
    let two_character = |scanner: &mut Scanner<'_>, kind: TokenKind| {
        scanner.advance();
        Ok(kind)
    };
    if character == DEGREE_SIGN_CHARACTER {
        return match next {
            Some('C') => two_character(scanner, TokenKind::CelsiusSign),
            Some('F') => two_character(scanner, TokenKind::FahrenheitSign),
            _ => Ok(TokenKind::Identifier(DEGREE_SIGN.to_string())),
        };
    }
    if character == MAPS_TO_START && next == Some('-') && scanner.peek_second() == Some('>') {
        scanner.advance();
        scanner.advance();
        return Ok(TokenKind::MapsTo);
    }
    if let Some(next) = next
        && let Some((_, kind)) = TWO_CHARACTER_TOKENS.iter().find(|(text, _)| {
            let mut characters = text.chars();
            characters.next() == Some(character) && characters.next() == Some(next)
        })
    {
        return two_character(scanner, kind.clone());
    }
    if let Some((_, kind)) = SINGLE_CHARACTER_TOKENS
        .iter()
        .find(|(candidate, _)| *candidate == character)
    {
        return Ok(kind.clone());
    }
    Err(ParseError::new(
        ParseErrorKind::UnexpectedCharacter,
        start..scanner.position,
    ))
}

fn scan_number(scanner: &mut Scanner<'_>) -> TokenKind {
    let start = scanner.position;
    if let Some(radix) = crate::decimal::radix_prefix(&scanner.text[start..]) {
        let after = &scanner.text[start + 2..];
        if after.chars().next().is_some_and(|c| c.is_digit(radix)) {
            scanner.advance();
            scanner.advance();
            scanner.take_while(|character| character.is_digit(radix));
            while scanner.peek() == Some('_')
                && scanner.peek_second().is_some_and(|c| c.is_digit(radix))
            {
                scanner.advance();
                scanner.take_while(|character| character.is_digit(radix));
            }
            return TokenKind::Number(scanner.text[start..scanner.position].to_string());
        }
    }
    scanner.take_while(|character| character.is_ascii_digit());
    if scanner.peek() == Some('.') && scanner.peek_second().is_some_and(|c| c.is_ascii_digit()) {
        scanner.advance();
        scanner.take_while(|character| character.is_ascii_digit());
    }
    if matches!(scanner.peek(), Some('e' | 'E')) {
        let rest = &scanner.text[scanner.position..];
        let mut characters = rest.chars().skip(1);
        let has_exponent = match characters.next() {
            Some('+' | '-') => characters.next().is_some_and(|c| c.is_ascii_digit()),
            Some(character) => character.is_ascii_digit(),
            None => false,
        };
        if has_exponent {
            scanner.advance();
            if matches!(scanner.peek(), Some('+' | '-')) {
                scanner.advance();
            }
            scanner.take_while(|character| character.is_ascii_digit());
        }
    }
    TokenKind::Number(scanner.text[start..scanner.position].to_string())
}

fn scan_typed_literal(
    scanner: &mut Scanner<'_>,
    kind: TypedLiteralKind,
    start: usize,
) -> Result<TokenKind, ParseError> {
    scanner.advance();
    let content = scanner
        .take_while(|character| character != '\'')
        .to_string();
    if scanner.advance() != Some('\'') {
        return Err(ParseError::new(
            ParseErrorKind::InvalidTypedLiteral,
            start..scanner.position,
        ));
    }
    Ok(TokenKind::TypedLiteral(kind, content))
}

fn scan_superscript(scanner: &mut Scanner<'_>, start: usize) -> Result<TokenKind, ParseError> {
    let is_negative = scanner.peek() == Some(SUPERSCRIPT_MINUS);
    if is_negative {
        scanner.advance();
    }
    let digits = scanner.take_while(|character| superscript_digit_value(character).is_some());
    let value =
        digits
            .chars()
            .filter_map(superscript_digit_value)
            .try_fold(0_i64, |value, digit| {
                value
                    .checked_mul(10)
                    .and_then(|value| value.checked_add(digit))
            });
    match value {
        Some(value) if !digits.is_empty() => Ok(TokenKind::Superscript(if is_negative {
            -value
        } else {
            value
        })),
        _ => Err(ParseError::new(
            ParseErrorKind::UnexpectedCharacter,
            start..scanner.position,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<TokenKind> {
        tokenize(text)
            .unwrap()
            .into_iter()
            .map(|token| token.kind)
            .collect()
    }

    #[test]
    fn comment_ends_the_line() {
        assert_eq!(kinds("x # note"), vec![TokenKind::Identifier("x".into())]);
    }

    #[test]
    fn space_before_a_token_is_recorded() {
        let tokens = tokenize("2 m/s").unwrap();
        assert!(tokens[1].spaced_before);
        assert!(!tokens[2].spaced_before);
    }

    #[test]
    fn identifier_ends_before_a_superscript() {
        assert_eq!(
            kinds("x\u{00B2}"),
            vec![TokenKind::Identifier("x".into()), TokenKind::Superscript(2)]
        );
    }

    #[test]
    fn identifier_keeps_a_combining_mark() {
        assert_eq!(
            kinds("\u{03C0}\u{0302}"),
            vec![TokenKind::Identifier("\u{03C0}\u{0302}".into())]
        );
    }

    #[test]
    fn unknown_character_is_an_error() {
        assert_eq!(
            tokenize("2 $").unwrap_err().kind,
            ParseErrorKind::UnexpectedCharacter
        );
    }
}
