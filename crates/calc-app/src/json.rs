use std::fmt::Write as _;

const INDENT: &str = "  ";
const ONE_LINE_SEPARATOR: &str = ", ";
const NAME_SEPARATOR: &str = ": ";
pub(crate) const MAXIMUM_SAFE_COUNT: u64 = (1 << 53) - 1;
const MAXIMUM_NESTING: usize = 256;
const HEX_DIGITS_IN_ESCAPE: usize = 4;
const HIGH_SURROGATES: std::ops::RangeInclusive<u32> = 0xD800..=0xDBFF;
const LOW_SURROGATES: std::ops::RangeInclusive<u32> = 0xDC00..=0xDFFF;
const SURROGATE_OFFSET: u32 = 0x10000;
const SURROGATE_SHIFT: u32 = 10;
const BYTE_ORDER_MARK: &[u8] = &[0xEF, 0xBB, 0xBF];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Json {
    Null,
    Boolean(bool),
    Count(u64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonSyntaxError {
    ByteOrderMark,
    NotUtf8 { offset: usize },
    UnexpectedEnd,
    UnexpectedCharacter { offset: usize },
    InvalidEscape { offset: usize },
    CountOutOfRange { offset: usize },
    NestingTooDeep { offset: usize },
    TrailingContent { offset: usize },
}

impl Json {
    pub fn object(members: Vec<(&str, Json)>) -> Self {
        Json::Object(
            members
                .into_iter()
                .map(|(name, value)| (name.to_string(), value))
                .collect(),
        )
    }

    pub fn string(text: &str) -> Self {
        Json::String(text.to_string())
    }

    pub fn optional(value: Option<Json>) -> Self {
        value.unwrap_or(Json::Null)
    }
}

pub(crate) fn write_canonical(value: &Json) -> String {
    let mut text = String::new();
    write_value(&mut text, value, 0);
    text.push('\n');
    text
}

pub(crate) fn write_one_line(value: &Json) -> String {
    let mut text = String::new();
    write_one_line_value(&mut text, value);
    text
}

fn write_one_line_value(text: &mut String, value: &Json) {
    match value {
        Json::Array(elements) => {
            text.push('[');
            for (position, element) in elements.iter().enumerate() {
                if position > 0 {
                    text.push_str(ONE_LINE_SEPARATOR);
                }
                write_one_line_value(text, element);
            }
            text.push(']');
        }
        Json::Object(members) => {
            text.push('{');
            for (position, (name, member)) in members.iter().enumerate() {
                if position > 0 {
                    text.push_str(ONE_LINE_SEPARATOR);
                }
                write_string(text, name);
                text.push_str(NAME_SEPARATOR);
                write_one_line_value(text, member);
            }
            text.push('}');
        }
        scalar => write_value(text, scalar, 0),
    }
}

fn write_indent(text: &mut String, level: usize) {
    for _ in 0..level {
        text.push_str(INDENT);
    }
}

fn write_value(text: &mut String, value: &Json, level: usize) {
    match value {
        Json::Null => text.push_str("null"),
        Json::Boolean(true) => text.push_str("true"),
        Json::Boolean(false) => text.push_str("false"),
        Json::Count(count) => {
            let _ = write!(text, "{count}");
        }
        Json::String(string) => write_string(text, string),
        Json::Array(elements) if elements.is_empty() => text.push_str("[]"),
        Json::Object(members) if members.is_empty() => text.push_str("{}"),
        Json::Array(elements) => {
            text.push_str("[\n");
            for (position, element) in elements.iter().enumerate() {
                write_indent(text, level + 1);
                write_value(text, element, level + 1);
                if position + 1 < elements.len() {
                    text.push(',');
                }
                text.push('\n');
            }
            write_indent(text, level);
            text.push(']');
        }
        Json::Object(members) => {
            text.push_str("{\n");
            for (position, (name, member)) in members.iter().enumerate() {
                write_indent(text, level + 1);
                write_string(text, name);
                text.push_str(NAME_SEPARATOR);
                write_value(text, member, level + 1);
                if position + 1 < members.len() {
                    text.push(',');
                }
                text.push('\n');
            }
            write_indent(text, level);
            text.push('}');
        }
    }
}

fn write_string(text: &mut String, string: &str) {
    text.push('"');
    for character in string.chars() {
        match character {
            '"' => text.push_str("\\\""),
            '\\' => text.push_str("\\\\"),
            control if control.is_control() => {
                let _ = write!(text, "\\u{:04x}", u32::from(control));
            }
            other => text.push(other),
        }
    }
    text.push('"');
}

pub(crate) fn parse(bytes: &[u8]) -> Result<Json, JsonSyntaxError> {
    if bytes.starts_with(BYTE_ORDER_MARK) {
        return Err(JsonSyntaxError::ByteOrderMark);
    }
    let text = std::str::from_utf8(bytes).map_err(|error| JsonSyntaxError::NotUtf8 {
        offset: error.valid_up_to(),
    })?;
    let mut parser = Parser {
        text,
        position: 0,
        depth: 0,
    };
    parser.skip_whitespace();
    let value = parser.value()?;
    parser.skip_whitespace();
    if parser.position < text.len() {
        return Err(JsonSyntaxError::TrailingContent {
            offset: parser.position,
        });
    }
    Ok(value)
}

struct Parser<'text> {
    text: &'text str,
    position: usize,
    depth: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.text.get(self.position..)?.chars().next()
    }

    fn next_character(&mut self) -> Result<char, JsonSyntaxError> {
        let character = self.peek().ok_or(JsonSyntaxError::UnexpectedEnd)?;
        self.position += character.len_utf8();
        Ok(character)
    }

    fn unexpected(&self) -> JsonSyntaxError {
        if self.position >= self.text.len() {
            JsonSyntaxError::UnexpectedEnd
        } else {
            JsonSyntaxError::UnexpectedCharacter {
                offset: self.position,
            }
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.position += 1;
        }
    }

    fn expect_keyword(&mut self, keyword: &str, value: Json) -> Result<Json, JsonSyntaxError> {
        if self
            .text
            .get(self.position..)
            .is_some_and(|rest| rest.starts_with(keyword))
        {
            self.position += keyword.len();
            Ok(value)
        } else {
            Err(self.unexpected())
        }
    }

    fn value(&mut self) -> Result<Json, JsonSyntaxError> {
        match self.peek() {
            Some('n') => self.expect_keyword("null", Json::Null),
            Some('t') => self.expect_keyword("true", Json::Boolean(true)),
            Some('f') => self.expect_keyword("false", Json::Boolean(false)),
            Some('"') => self.string().map(Json::String),
            Some('[') => self.nested(Self::array),
            Some('{') => self.nested(Self::object),
            Some(digit) if digit.is_ascii_digit() => self.count(),
            _ => Err(self.unexpected()),
        }
    }

    fn nested(
        &mut self,
        parse: fn(&mut Self) -> Result<Json, JsonSyntaxError>,
    ) -> Result<Json, JsonSyntaxError> {
        if self.depth >= MAXIMUM_NESTING {
            return Err(JsonSyntaxError::NestingTooDeep {
                offset: self.position,
            });
        }
        self.depth += 1;
        let value = parse(self);
        self.depth -= 1;
        value
    }

    fn count(&mut self) -> Result<Json, JsonSyntaxError> {
        let start = self.position;
        while self
            .peek()
            .is_some_and(|character| character.is_ascii_digit())
        {
            self.position += 1;
        }
        if matches!(self.peek(), Some('.' | 'e' | 'E')) {
            return Err(self.unexpected());
        }
        let digits = &self.text[start..self.position];
        let is_canonical = digits == "0" || !digits.starts_with('0');
        digits
            .parse::<u64>()
            .ok()
            .filter(|count| is_canonical && *count <= MAXIMUM_SAFE_COUNT)
            .map(Json::Count)
            .ok_or(JsonSyntaxError::CountOutOfRange { offset: start })
    }

    fn hex_escape(&mut self) -> Result<u32, JsonSyntaxError> {
        let start = self.position;
        let digits = self
            .text
            .get(start..start + HEX_DIGITS_IN_ESCAPE)
            .ok_or(JsonSyntaxError::UnexpectedEnd)?;
        let value = u32::from_str_radix(digits, 16)
            .ok()
            .filter(|_| {
                digits
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
            })
            .ok_or(JsonSyntaxError::InvalidEscape { offset: start })?;
        self.position += HEX_DIGITS_IN_ESCAPE;
        Ok(value)
    }

    fn string(&mut self) -> Result<String, JsonSyntaxError> {
        self.position += 1;
        let mut string = String::new();
        loop {
            let offset = self.position;
            match self.next_character()? {
                '"' => return Ok(string),
                '\\' => {
                    let escaped = match self.next_character()? {
                        '"' => '"',
                        '\\' => '\\',
                        '/' => '/',
                        'b' => '\u{8}',
                        'f' => '\u{c}',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        'u' => self.unicode_escape(offset)?,
                        _ => return Err(JsonSyntaxError::InvalidEscape { offset }),
                    };
                    string.push(escaped);
                }
                control if u32::from(control) < 0x20 => {
                    return Err(JsonSyntaxError::UnexpectedCharacter { offset });
                }
                other => string.push(other),
            }
        }
    }

    fn unicode_escape(&mut self, offset: usize) -> Result<char, JsonSyntaxError> {
        let first = self.hex_escape()?;
        let code_point = if HIGH_SURROGATES.contains(&first) {
            if !self
                .text
                .get(self.position..)
                .is_some_and(|rest| rest.starts_with("\\u"))
            {
                return Err(JsonSyntaxError::InvalidEscape { offset });
            }
            self.position += 2;
            let second = self.hex_escape()?;
            if !LOW_SURROGATES.contains(&second) {
                return Err(JsonSyntaxError::InvalidEscape { offset });
            }
            SURROGATE_OFFSET
                + ((first - HIGH_SURROGATES.start()) << SURROGATE_SHIFT)
                + (second - LOW_SURROGATES.start())
        } else {
            first
        };
        char::from_u32(code_point).ok_or(JsonSyntaxError::InvalidEscape { offset })
    }

    fn array(&mut self) -> Result<Json, JsonSyntaxError> {
        self.position += 1;
        let mut elements = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(']') {
            self.position += 1;
            return Ok(Json::Array(elements));
        }
        loop {
            self.skip_whitespace();
            elements.push(self.value()?);
            self.skip_whitespace();
            match self.next_character()? {
                ',' => continue,
                ']' => return Ok(Json::Array(elements)),
                _ => {
                    self.position -= 1;
                    return Err(self.unexpected());
                }
            }
        }
    }

    fn object(&mut self) -> Result<Json, JsonSyntaxError> {
        self.position += 1;
        let mut members = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some('}') {
            self.position += 1;
            return Ok(Json::Object(members));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some('"') {
                return Err(self.unexpected());
            }
            let name = self.string()?;
            self.skip_whitespace();
            if self.peek() != Some(':') {
                return Err(self.unexpected());
            }
            self.position += 1;
            self.skip_whitespace();
            let value = self.value()?;
            members.push((name, value));
            self.skip_whitespace();
            match self.next_character()? {
                ',' => continue,
                '}' => return Ok(Json::Object(members)),
                _ => {
                    self.position -= 1;
                    return Err(self.unexpected());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_form_indents_two_spaces_and_ends_with_newline() {
        let value = Json::object(vec![
            ("format", Json::string("calc-session")),
            ("lines", Json::Array(vec![Json::Count(1), Json::Null])),
            ("empty", Json::Array(Vec::new())),
        ]);

        let text = write_canonical(&value);

        assert_eq!(
            text,
            "{\n  \"format\": \"calc-session\",\n  \"lines\": [\n    1,\n    null\n  ],\n  \"empty\": []\n}\n"
        );
    }

    #[test]
    fn strings_escape_only_quote_backslash_and_control_characters() {
        let text = write_canonical(&Json::string("a\"b\\c\n\u{1}π"));

        assert_eq!(text, "\"a\\\"b\\\\c\\u000a\\u0001π\"\n");
    }

    #[test]
    fn parsing_the_canonical_form_gives_the_value_back() {
        let value = Json::object(vec![
            ("text", Json::string("tab\there \u{1F600} \"quoted\"")),
            ("flag", Json::Boolean(false)),
            (
                "nested",
                Json::object(vec![("count", Json::Count(MAXIMUM_SAFE_COUNT))]),
            ),
        ]);

        let parsed = parse(write_canonical(&value).as_bytes());

        assert_eq!(parsed, Ok(value));
    }

    #[test]
    fn surrogate_pair_escape_is_one_character() {
        assert_eq!(parse(b"\"\\ud83d\\ude00\""), Ok(Json::string("\u{1F600}")));
    }

    #[test]
    fn lone_surrogate_is_an_invalid_escape() {
        assert_eq!(
            parse(b"\"\\ud83d\""),
            Err(JsonSyntaxError::InvalidEscape { offset: 1 })
        );
    }

    #[test]
    fn byte_order_mark_is_rejected() {
        assert_eq!(
            parse(b"\xEF\xBB\xBFnull"),
            Err(JsonSyntaxError::ByteOrderMark)
        );
    }

    #[test]
    fn invalid_utf8_is_rejected() {
        assert_eq!(
            parse(b"\"a\xFF\""),
            Err(JsonSyntaxError::NotUtf8 { offset: 2 })
        );
    }

    #[test]
    fn fractional_number_is_rejected() {
        assert_eq!(
            parse(b"1.5"),
            Err(JsonSyntaxError::UnexpectedCharacter { offset: 1 })
        );
    }

    #[test]
    fn count_above_two_to_the_fifty_three_is_out_of_range() {
        assert_eq!(
            parse(b"9007199254740992"),
            Err(JsonSyntaxError::CountOutOfRange { offset: 0 })
        );
    }

    #[test]
    fn count_with_leading_zero_is_out_of_range() {
        assert_eq!(
            parse(b"01"),
            Err(JsonSyntaxError::CountOutOfRange { offset: 0 })
        );
    }

    #[test]
    fn negative_number_is_rejected() {
        assert_eq!(
            parse(b"-1"),
            Err(JsonSyntaxError::UnexpectedCharacter { offset: 0 })
        );
    }

    #[test]
    fn raw_control_character_in_string_is_rejected() {
        assert_eq!(
            parse(b"\"a\nb\""),
            Err(JsonSyntaxError::UnexpectedCharacter { offset: 2 })
        );
    }

    #[test]
    fn trailing_content_is_rejected() {
        assert_eq!(
            parse(b"{} {}"),
            Err(JsonSyntaxError::TrailingContent { offset: 3 })
        );
    }

    #[test]
    fn unterminated_array_is_an_unexpected_end() {
        assert_eq!(parse(b"[1, 2"), Err(JsonSyntaxError::UnexpectedEnd));
    }

    #[test]
    fn deep_nesting_is_rejected_without_exhausting_the_stack() {
        let text = "[".repeat(100_000);

        assert_eq!(
            parse(text.as_bytes()),
            Err(JsonSyntaxError::NestingTooDeep { offset: 256 })
        );
    }

    #[test]
    fn duplicate_members_are_kept_for_the_reader_to_reject() {
        assert_eq!(
            parse(b"{\"a\": 1, \"a\": 2}"),
            Ok(Json::Object(vec![
                ("a".to_string(), Json::Count(1)),
                ("a".to_string(), Json::Count(2)),
            ]))
        );
    }
}
