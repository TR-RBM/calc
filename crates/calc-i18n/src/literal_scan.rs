use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const ALLOWLIST: &str = include_str!("literal_scan_allowlist.txt");
const SCANNED_CRATES: [&str; 1] = ["calc-cli"];
const SOURCE_DIRECTORY: &str = "src";
const RUST_EXTENSION: &str = "rs";
const MODULE_ROOT_STEMS: [&str; 3] = ["lib", "main", "mod"];
const MODULE_FILE: &str = "mod.rs";
const CONFIGURATION_ATTRIBUTE: &str = "cfg";
const TEST_CONFIGURATION: &str = "test";
const MODULE_KEYWORD: &str = "mod";
const RAW_STRING_PREFIXES: [&str; 3] = ["r", "br", "cr"];
const NON_TEXT_STRING_PREFIXES: [&str; 5] = ["b", "br", "c", "cr", "rb"];

#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
    Identifier(String),
    Punctuation(char),
    StringLiteral(StringLiteral),
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StringLiteral {
    spelling: String,
    value: String,
    is_text: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct FileScan {
    literals: Vec<StringLiteral>,
    test_modules_without_body: Vec<String>,
    is_test_file: bool,
}

struct Lexer {
    chars: Vec<char>,
    position: usize,
}

impl Lexer {
    fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            position: 0,
        }
    }

    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.position + offset).copied()
    }

    fn is_at(&self, text: &str) -> bool {
        text.chars()
            .enumerate()
            .all(|(offset, c)| self.peek(offset) == Some(c))
    }

    fn tokens(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while let Some(c) = self.peek(0) {
            if c.is_whitespace() {
                self.position += 1;
            } else if self.is_at("//") {
                self.skip_line_comment();
            } else if self.is_at("/*") {
                self.skip_block_comment();
            } else if c == '"' {
                self.position += 1;
                tokens.push(Token::StringLiteral(self.quoted_string(true)));
            } else if c == '\'' {
                tokens.push(self.character_or_lifetime());
            } else if c.is_ascii_digit() {
                self.skip_number();
                tokens.push(Token::Other);
            } else if c.is_alphabetic() || c == '_' {
                tokens.push(self.identifier_or_prefixed_string());
            } else {
                self.position += 1;
                tokens.push(Token::Punctuation(c));
            }
        }
        tokens
    }

    fn skip_line_comment(&mut self) {
        while let Some(c) = self.peek(0) {
            if c == '\n' {
                break;
            }
            self.position += 1;
        }
    }

    fn skip_block_comment(&mut self) {
        let mut depth = 0usize;
        while self.peek(0).is_some() {
            if self.is_at("/*") {
                depth += 1;
                self.position += 2;
            } else if self.is_at("*/") {
                depth -= 1;
                self.position += 2;
                if depth == 0 {
                    return;
                }
            } else {
                self.position += 1;
            }
        }
    }

    fn skip_number(&mut self) {
        while let Some(c) = self.peek(0) {
            let is_fraction_point =
                c == '.' && self.peek(1).is_some_and(|next| next.is_ascii_digit());
            if c.is_alphanumeric() || c == '_' || is_fraction_point {
                self.position += 1;
            } else {
                break;
            }
        }
    }

    fn identifier_or_prefixed_string(&mut self) -> Token {
        let start = self.position;
        while self
            .peek(0)
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            self.position += 1;
        }
        let identifier: String = self.chars[start..self.position].iter().collect();
        let is_text = !NON_TEXT_STRING_PREFIXES.contains(&identifier.as_str());
        let is_raw = RAW_STRING_PREFIXES.contains(&identifier.as_str());
        match self.peek(0) {
            Some('"') if is_raw || !is_text => {
                self.position += 1;
                if is_raw {
                    Token::StringLiteral(self.raw_string(0, is_text))
                } else {
                    Token::StringLiteral(self.quoted_string(is_text))
                }
            }
            Some('#') if is_raw && self.raw_string_hashes().is_some() => {
                let hashes = self.raw_string_hashes().unwrap_or_default();
                self.position += hashes + 1;
                Token::StringLiteral(self.raw_string(hashes, is_text))
            }
            _ => Token::Identifier(identifier),
        }
    }

    fn raw_string_hashes(&self) -> Option<usize> {
        let hashes = (0..)
            .take_while(|offset| self.peek(*offset) == Some('#'))
            .count();
        (self.peek(hashes) == Some('"')).then_some(hashes)
    }

    fn raw_string(&mut self, hashes: usize, is_text: bool) -> StringLiteral {
        let start = self.position;
        while let Some(c) = self.peek(0) {
            if c == '"' && (1..=hashes).all(|offset| self.peek(offset) == Some('#')) {
                let value: String = self.chars[start..self.position].iter().collect();
                self.position += hashes + 1;
                return StringLiteral {
                    spelling: value.clone(),
                    value,
                    is_text,
                };
            }
            self.position += 1;
        }
        let value: String = self.chars[start..].iter().collect();
        StringLiteral {
            spelling: value.clone(),
            value,
            is_text,
        }
    }

    fn quoted_string(&mut self, is_text: bool) -> StringLiteral {
        let start = self.position;
        let mut value = String::new();
        while let Some(c) = self.peek(0) {
            self.position += 1;
            match c {
                '"' => {
                    let spelling = self.chars[start..self.position - 1].iter().collect();
                    return StringLiteral {
                        spelling,
                        value,
                        is_text,
                    };
                }
                '\\' => self.push_escape(&mut value),
                other => value.push(other),
            }
        }
        StringLiteral {
            spelling: self.chars[start..].iter().collect(),
            value,
            is_text,
        }
    }

    fn push_escape(&mut self, value: &mut String) {
        let Some(escaped) = self.peek(0) else {
            return;
        };
        self.position += 1;
        match escaped {
            'n' => value.push('\n'),
            'r' => value.push('\r'),
            't' => value.push('\t'),
            '0' => value.push('\0'),
            'x' => {
                let digits: String = (0..2).filter_map(|offset| self.peek(offset)).collect();
                self.position += digits.len();
                if let Some(c) = u32::from_str_radix(&digits, 16)
                    .ok()
                    .and_then(char::from_u32)
                {
                    value.push(c);
                }
            }
            'u' => {
                let digits: String = (1..)
                    .map_while(|offset| self.peek(offset).filter(|c| *c != '}'))
                    .collect();
                self.position += digits.chars().count() + 2;
                if let Some(c) = u32::from_str_radix(&digits, 16)
                    .ok()
                    .and_then(char::from_u32)
                {
                    value.push(c);
                }
            }
            '\n' => {
                while self.peek(0).is_some_and(char::is_whitespace) {
                    self.position += 1;
                }
            }
            other => value.push(other),
        }
    }

    fn character_or_lifetime(&mut self) -> Token {
        self.position += 1;
        if self.peek(0) == Some('\\') {
            while let Some(c) = self.peek(0) {
                self.position += 1;
                if c == '\'' {
                    break;
                }
                if c == '\\' {
                    self.position += 1;
                }
            }
            return Token::Other;
        }
        if self.peek(1) == Some('\'') {
            self.position += 2;
            return Token::Other;
        }
        while self
            .peek(0)
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            self.position += 1;
        }
        Token::Other
    }
}

fn is_punctuation(token: Option<&Token>, expected: char) -> bool {
    token == Some(&Token::Punctuation(expected))
}

fn is_identifier(token: Option<&Token>, expected: &str) -> bool {
    matches!(token, Some(Token::Identifier(name)) if name == expected)
}

fn test_attribute_length(tokens: &[Token], index: usize) -> Option<(usize, bool)> {
    let at = |offset: usize| tokens.get(index + offset);
    let is_inner = is_punctuation(at(1), '!');
    let bracket = if is_inner { 2 } else { 1 };
    let matches = is_punctuation(at(0), '#')
        && is_punctuation(at(bracket), '[')
        && is_identifier(at(bracket + 1), CONFIGURATION_ATTRIBUTE)
        && is_punctuation(at(bracket + 2), '(')
        && is_identifier(at(bracket + 3), TEST_CONFIGURATION)
        && is_punctuation(at(bracket + 4), ')')
        && is_punctuation(at(bracket + 5), ']');
    matches.then_some((bracket + 6, is_inner))
}

fn item_end(tokens: &[Token], start: usize) -> (usize, Option<String>) {
    let mut index = start;
    while is_punctuation(tokens.get(index), '#') && is_punctuation(tokens.get(index + 1), '[') {
        index = group_end(tokens, index + 1);
    }
    let mut depth = 0usize;
    let mut module_name = None;
    while let Some(token) = tokens.get(index) {
        index += 1;
        match token {
            Token::Identifier(name) if depth == 0 && name == MODULE_KEYWORD => {
                if let Some(Token::Identifier(module)) = tokens.get(index) {
                    module_name = Some(module.clone());
                }
            }
            Token::Punctuation('(' | '[' | '{') => depth += 1,
            Token::Punctuation('}') if depth == 1 => return (index, None),
            Token::Punctuation(')' | ']' | '}') => depth = depth.saturating_sub(1),
            Token::Punctuation(';') if depth == 0 => return (index, module_name),
            _ => {}
        }
    }
    (index, None)
}

fn group_end(tokens: &[Token], open: usize) -> usize {
    let mut depth = 0usize;
    let mut index = open;
    while let Some(token) = tokens.get(index) {
        index += 1;
        match token {
            Token::Punctuation('(' | '[' | '{') => depth += 1,
            Token::Punctuation(')' | ']' | '}') => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return index;
                }
            }
            _ => {}
        }
    }
    index
}

fn scan_source(source: &str) -> FileScan {
    let tokens = Lexer::new(source).tokens();
    let mut scan = FileScan::default();
    let mut index = 0;
    while let Some(token) = tokens.get(index) {
        if let Some((length, is_inner)) = test_attribute_length(&tokens, index) {
            if is_inner {
                return FileScan {
                    is_test_file: true,
                    ..FileScan::default()
                };
            }
            let (end, module) = item_end(&tokens, index + length);
            scan.test_modules_without_body.extend(module);
            index = end;
            continue;
        }
        if let Token::StringLiteral(literal) = token
            && literal.is_text
        {
            scan.literals.push(literal.clone());
        }
        index += 1;
    }
    scan
}

fn looks_user_facing(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let has_words_with_space = chars
        .windows(3)
        .any(|window| window[0].is_alphabetic() && window[1] == ' ' && window[2].is_alphabetic());
    let starts_capitalized = matches!(
        chars.as_slice(),
        [first, second, ..] if first.is_uppercase() && second.is_lowercase()
    );
    has_words_with_space || starts_capitalized
}

fn rust_files(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in fs::read_dir(&current).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some(RUST_EXTENSION) {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn module_path(file: &Path, module: &str) -> PathBuf {
    let directory = file.parent().unwrap_or_else(|| Path::new(""));
    let stem = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if MODULE_ROOT_STEMS.contains(&stem) {
        directory.join(module)
    } else {
        directory.join(stem).join(module)
    }
}

fn is_excluded(file: &Path, excluded_modules: &[PathBuf]) -> bool {
    excluded_modules.iter().any(|module| {
        file == module.with_extension(RUST_EXTENSION)
            || file == module.join(MODULE_FILE)
            || file.starts_with(module)
    })
}

fn read_sources(directory: &Path) -> Vec<(PathBuf, String)> {
    rust_files(directory)
        .into_iter()
        .map(|file| {
            let source = fs::read_to_string(&file).unwrap();
            (file, source)
        })
        .collect()
}

fn user_facing_literals(
    sources: &[(PathBuf, String)],
    allowlist: &BTreeSet<&str>,
) -> Vec<(PathBuf, StringLiteral)> {
    let scans: Vec<(&PathBuf, FileScan)> = sources
        .iter()
        .map(|(file, source)| (file, scan_source(source)))
        .collect();
    let excluded_modules: Vec<PathBuf> = scans
        .iter()
        .flat_map(|(file, scan)| {
            scan.test_modules_without_body
                .iter()
                .map(|module| module_path(file, module))
        })
        .collect();
    scans
        .into_iter()
        .filter(|(file, scan)| !scan.is_test_file && !is_excluded(file, &excluded_modules))
        .flat_map(|(file, scan)| {
            scan.literals
                .into_iter()
                .map(move |literal| (file.clone(), literal))
        })
        .filter(|(_, literal)| {
            looks_user_facing(&literal.value) && !allowlist.contains(literal.spelling.as_str())
        })
        .collect()
}

fn allowlist(text: &str) -> BTreeSet<&str> {
    text.lines().filter(|line| !line.is_empty()).collect()
}

mod tests {
    use super::*;

    #[test]
    fn cli_and_cockpit_sources_have_no_user_facing_literals() {
        let crates_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let allowlist = allowlist(ALLOWLIST);
        let violations: Vec<String> = SCANNED_CRATES
            .iter()
            .flat_map(|name| {
                user_facing_literals(
                    &read_sources(&crates_directory.join(name).join(SOURCE_DIRECTORY)),
                    &allowlist,
                )
            })
            .map(|(file, literal)| format!("{}: {}", file.display(), literal.spelling))
            .collect();
        assert_eq!(violations, Vec::<String>::new());
    }

    fn reported(source: &str) -> Vec<String> {
        scan_source(source)
            .literals
            .into_iter()
            .filter(|literal| looks_user_facing(&literal.value))
            .map(|literal| literal.value)
            .collect()
    }

    #[test]
    fn sentence_literal_is_reported() {
        assert_eq!(reported("fn f() { g(\"no result\"); }"), vec!["no result"]);
    }

    #[test]
    fn capitalized_word_literal_is_reported() {
        assert_eq!(reported("const A: &str = \"Inspect\";"), vec!["Inspect"]);
    }

    #[test]
    fn technical_literal_is_not_reported() {
        assert_eq!(
            reported("fn f() { g(\"x=0\"); g(\"non-zero\"); g(\"GPU\"); }"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn literal_in_test_module_is_not_reported() {
        let source = "#[cfg(test)]\nmod tests {\n    fn f() { g(\"no result\"); }\n}\n";
        assert_eq!(reported(source), Vec::<String>::new());
    }

    #[test]
    fn literal_after_test_function_is_reported() {
        let source =
            "#[cfg(test)]\nfn helper() -> [u8; 2] { [0; 2] }\nconst A: &str = \"Late text\";\n";
        assert_eq!(reported(source), vec!["Late text"]);
    }

    #[test]
    fn literal_after_test_constant_is_reported() {
        let source =
            "#[cfg(test)]\nconst B: [&str; 1] = [\"x\"];\nconst A: &str = \"Late text\";\n";
        assert_eq!(reported(source), vec!["Late text"]);
    }

    #[test]
    fn inner_test_attribute_marks_the_file_as_test_code() {
        let scan = scan_source("#![cfg(test)]\nconst A: &str = \"Some text\";\n");
        assert!(scan.is_test_file);
    }

    #[test]
    fn test_module_without_body_is_recorded() {
        let scan = scan_source("#[cfg(test)]\nmod tests;\n");
        assert_eq!(scan.test_modules_without_body, vec!["tests".to_owned()]);
    }

    #[test]
    fn literal_in_comment_is_not_reported() {
        let source = "// \"no result\"\n/* \"no /* nested */ result\" */ fn f() {}";
        assert_eq!(reported(source), Vec::<String>::new());
    }

    #[test]
    fn raw_string_literal_is_reported() {
        assert_eq!(
            reported("const A: &str = r#\"say \"hi\" to me\"#;"),
            vec!["say \"hi\" to me"]
        );
    }

    #[test]
    fn byte_string_literal_is_not_reported() {
        assert_eq!(
            reported("const A: &[u8] = b\"no result\";"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn escaped_quote_does_not_end_the_literal() {
        assert_eq!(reported("const A: &str = \"a\\\" b c\";"), vec!["a\" b c"]);
    }

    #[test]
    fn unicode_escape_is_decoded() {
        assert_eq!(reported("const A: &str = \"\\u{41}bc\";"), vec!["Abc"]);
    }

    #[test]
    fn lifetime_is_not_taken_for_a_character_literal() {
        let source = "fn f<'a>(x: &'a str) -> char { g(\"no result\"); 'q' }";
        assert_eq!(reported(source), vec!["no result"]);
    }

    #[test]
    fn quote_character_literal_does_not_start_a_string() {
        let source = "fn f() { g('\"'); g(\"no result\"); }";
        assert_eq!(reported(source), vec!["no result"]);
    }

    #[test]
    fn module_file_of_test_module_is_excluded() {
        let module = module_path(Path::new("src/text.rs"), "tests");
        assert!(is_excluded(Path::new("src/text/tests.rs"), &[module]));
    }

    #[test]
    fn module_file_beside_crate_root_is_excluded() {
        let module = module_path(Path::new("src/lib.rs"), "tests");
        assert!(is_excluded(Path::new("src/tests/mod.rs"), &[module]));
    }

    #[test]
    fn allowlisted_literal_is_not_a_violation() {
        let sources = vec![(
            PathBuf::from("src/main.rs"),
            "fn f() { g(\"Allowed text\"); g(\"Other text\"); }".to_owned(),
        )];
        let found = user_facing_literals(&sources, &allowlist("Allowed text\n"));
        let spellings: Vec<String> = found
            .into_iter()
            .map(|(_, literal)| literal.spelling)
            .collect();
        assert_eq!(spellings, vec!["Other text"]);
    }

    #[test]
    fn literal_in_module_file_of_test_module_is_not_a_violation() {
        let sources = vec![
            (
                PathBuf::from("src/main.rs"),
                "#[cfg(test)]\nmod tests;\n".to_owned(),
            ),
            (
                PathBuf::from("src/tests.rs"),
                "fn f() { g(\"Some text\"); }".to_owned(),
            ),
        ];
        assert_eq!(user_facing_literals(&sources, &BTreeSet::new()), Vec::new());
    }
}
