use std::collections::{BTreeMap, BTreeSet};

use crate::language_tag::LanguageTag;
use crate::plural::{PluralCategory, PluralRule};

pub(crate) const SOURCE_LOCALE: &str = "en";
pub(crate) const AREAS: [&str; 4] = ["common", "error", "cli", "ui"];

const ONE_OTHER_LANGUAGES: [&str; 2] = ["de", "en"];
const KEY_SEPARATOR: char = '-';
const COMMENT_START: char = '#';
const INDENT: char = ' ';
const ASSIGNMENT: char = '=';
const PLACEABLE_OPEN: char = '{';
const PLACEABLE_CLOSE: char = '}';
const VARIABLE_SIGIL: char = '$';
const STRING_QUOTE: char = '"';
const STRING_ESCAPE: char = '\\';
const SELECT_ARROW: &str = "->";
const VARIANT_OPEN: char = '[';
const VARIANT_CLOSE: char = ']';
const DEFAULT_VARIANT_MARK: char = '*';
const ATTRIBUTE_START: char = '.';
const LINE_BREAK: &str = "\n";
const FIELD_NAME_SEPARATOR: char = '_';
const RUST_KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "union", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Element {
    Text(String),
    Variable(String),
    Select(SelectExpression),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InlineElement {
    Text(String),
    Variable(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SelectExpression {
    pub(crate) selector: String,
    pub(crate) variants: Vec<SelectVariant>,
    pub(crate) default_variant: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SelectVariant {
    pub(crate) key: SelectKey,
    pub(crate) elements: Vec<InlineElement>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectKey {
    Category(PluralCategory),
    Exact(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FluentMessage {
    pub(crate) key: String,
    pub(crate) elements: Vec<Element>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FluentParseError {
    pub(crate) kind: FluentParseErrorKind,
    pub(crate) line: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FluentParseErrorKind {
    ExpectedMessage,
    IndentedLineOutsideMessage,
    DuplicateKey,
    EmptyValue,
    UnsupportedAttribute,
    UnclosedPlaceable,
    UnsupportedExpression,
    ExpectedVariableName,
    UnexpectedClosingBrace,
    InvalidEscape,
    NestedSelect,
    ExpectedVariant,
    InvalidVariantKey,
    EmptyVariantValue,
    MissingDefaultVariant,
    MultipleDefaultVariants,
    UnclosedSelect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LocaleFiles {
    pub(crate) tag: String,
    pub(crate) areas: Vec<AreaFile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AreaFile {
    pub(crate) area: String,
    pub(crate) text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Catalog {
    pub(crate) messages: Vec<CatalogMessage>,
    pub(crate) locales: Vec<CatalogLocale>,
    pub(crate) source_locale: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CatalogMessage {
    pub(crate) key: String,
    pub(crate) variant_name: String,
    pub(crate) fields: Vec<Field>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Field {
    pub(crate) name: String,
    pub(crate) kind: FieldKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FieldKind {
    Text,
    Count,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CatalogLocale {
    pub(crate) tag: String,
    pub(crate) plural_rule: PluralRule,
    pub(crate) patterns: Vec<Option<Vec<Element>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CatalogError {
    MissingSourceLocale,
    InvalidLocaleTag {
        locale: String,
    },
    NoPluralRule {
        locale: String,
    },
    Parse {
        locale: String,
        area: String,
        error: FluentParseError,
    },
    InvalidKey {
        locale: String,
        key: String,
    },
    KeyInWrongArea {
        locale: String,
        area: String,
        key: String,
    },
    KeyNotInSource {
        locale: String,
        key: String,
    },
    UnknownPlaceholder {
        locale: String,
        key: String,
        variable: String,
    },
    SelectorNotCount {
        locale: String,
        key: String,
        variable: String,
    },
    InvalidPlaceholderName {
        key: String,
        variable: String,
    },
    VariantNameCollision {
        first_key: String,
        second_key: String,
        variant_name: String,
    },
}

struct PatternBuilder {
    elements: Vec<Element>,
    line_indents: Vec<(usize, usize)>,
    pending_blank_lines: usize,
    has_content: bool,
}

struct OpenSelect {
    selector: String,
    line: usize,
    variants: Vec<SelectVariant>,
    default_variant: Option<usize>,
}

enum Placeable<'a> {
    Variable { name: String, rest: &'a str },
    Text { text: String, rest: &'a str },
    SelectOpen { selector: String },
}

impl FluentParseError {
    fn new(kind: FluentParseErrorKind, line: usize) -> Self {
        Self { kind, line }
    }
}

pub(crate) fn parse_resource(source: &str) -> Result<Vec<FluentMessage>, FluentParseError> {
    let lines: Vec<&str> = source.lines().map(str::trim_end).collect();
    let mut messages = Vec::new();
    let mut keys = BTreeSet::new();
    let mut index = 0;
    while let Some(line) = lines.get(index) {
        let line_number = index + 1;
        if line.is_empty() || line.starts_with(COMMENT_START) {
            index += 1;
            continue;
        }
        if line.starts_with(INDENT) {
            return Err(FluentParseError::new(
                FluentParseErrorKind::IndentedLineOutsideMessage,
                line_number,
            ));
        }
        let (key, first_value) = message_start(line).ok_or(FluentParseError::new(
            FluentParseErrorKind::ExpectedMessage,
            line_number,
        ))?;
        let block_end = continuation_end(&lines, index + 1);
        let continuation = lines.get(index + 1..block_end).unwrap_or_default();
        let elements = parse_pattern(first_value, continuation, line_number)?;
        if !keys.insert(key.to_owned()) {
            return Err(FluentParseError::new(
                FluentParseErrorKind::DuplicateKey,
                line_number,
            ));
        }
        messages.push(FluentMessage {
            key: key.to_owned(),
            elements,
        });
        index = block_end.max(index + 1);
    }
    Ok(messages)
}

fn message_start(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(ASSIGNMENT)?;
    let key = key.trim_end();
    is_fluent_identifier(key).then(|| (key, value.trim_start()))
}

fn is_fluent_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == KEY_SEPARATOR)
}

fn continuation_end(lines: &[&str], start: usize) -> usize {
    let mut end = start;
    let mut position = start;
    while let Some(line) = lines.get(position) {
        if line.is_empty() {
            position += 1;
        } else if line.starts_with(INDENT) || line.starts_with(PLACEABLE_CLOSE) {
            position += 1;
            end = position;
        } else {
            break;
        }
    }
    end
}

fn parse_pattern(
    first_value: &str,
    continuation: &[&str],
    first_line: usize,
) -> Result<Vec<Element>, FluentParseError> {
    let mut builder = PatternBuilder::new();
    let mut open_select = builder
        .push_inline(first_value, first_line)?
        .map(|selector| OpenSelect::new(selector, first_line));
    for (offset, line) in continuation.iter().enumerate() {
        let line_number = first_line + offset + 1;
        let trimmed = line.trim_start();
        if let Some(mut select) = open_select.take() {
            if trimmed.is_empty() {
                open_select = Some(select);
            } else if let Some(rest) = trimmed.strip_prefix(PLACEABLE_CLOSE) {
                builder.push_select(select.finish()?);
                open_select = builder
                    .push_inline(rest, line_number)?
                    .map(|selector| OpenSelect::new(selector, line_number));
            } else {
                select.push_variant(trimmed, line_number)?;
                open_select = Some(select);
            }
            continue;
        }
        if trimmed.is_empty() {
            builder.push_blank_line();
            continue;
        }
        if trimmed.starts_with(ATTRIBUTE_START) {
            return Err(FluentParseError::new(
                FluentParseErrorKind::UnsupportedAttribute,
                line_number,
            ));
        }
        builder.start_line(line.len() - trimmed.len());
        open_select = builder
            .push_inline(trimmed, line_number)?
            .map(|selector| OpenSelect::new(selector, line_number));
    }
    if let Some(select) = open_select {
        return Err(FluentParseError::new(
            FluentParseErrorKind::UnclosedSelect,
            select.line,
        ));
    }
    builder.finish(first_line)
}

impl PatternBuilder {
    fn new() -> Self {
        Self {
            elements: Vec::new(),
            line_indents: Vec::new(),
            pending_blank_lines: 0,
            has_content: false,
        }
    }

    fn push_blank_line(&mut self) {
        if self.has_content {
            self.pending_blank_lines += 1;
        }
    }

    fn start_line(&mut self, indent: usize) {
        if self.has_content {
            for _ in 0..=self.pending_blank_lines {
                self.elements.push(Element::Text(LINE_BREAK.to_owned()));
            }
        }
        self.pending_blank_lines = 0;
        self.line_indents.push((self.elements.len(), indent));
        self.elements
            .push(Element::Text(INDENT.to_string().repeat(indent)));
    }

    fn push_inline(&mut self, text: &str, line: usize) -> Result<Option<String>, FluentParseError> {
        let (elements, selector) = parse_inline(text, line)?;
        for element in elements {
            self.has_content = true;
            self.elements.push(match element {
                InlineElement::Text(text) => Element::Text(text),
                InlineElement::Variable(name) => Element::Variable(name),
            });
        }
        if selector.is_some() {
            self.has_content = true;
        }
        Ok(selector)
    }

    fn push_select(&mut self, select: SelectExpression) {
        self.has_content = true;
        self.elements.push(Element::Select(select));
    }

    fn finish(mut self, first_line: usize) -> Result<Vec<Element>, FluentParseError> {
        let common_indent = self
            .line_indents
            .iter()
            .map(|(_, indent)| *indent)
            .min()
            .unwrap_or(0);
        for (element_index, indent) in &self.line_indents {
            if let Some(Element::Text(text)) = self.elements.get_mut(*element_index) {
                *text = INDENT.to_string().repeat(indent - common_indent);
            }
        }
        let mut merged: Vec<Element> = Vec::new();
        for element in self.elements {
            match (merged.last_mut(), element) {
                (_, Element::Text(text)) if text.is_empty() => {}
                (Some(Element::Text(previous)), Element::Text(text)) => previous.push_str(&text),
                (_, element) => merged.push(element),
            }
        }
        if merged.is_empty() {
            return Err(FluentParseError::new(
                FluentParseErrorKind::EmptyValue,
                first_line,
            ));
        }
        Ok(merged)
    }
}

impl OpenSelect {
    fn new(selector: String, line: usize) -> Self {
        Self {
            selector,
            line,
            variants: Vec::new(),
            default_variant: None,
        }
    }

    fn push_variant(&mut self, text: &str, line: usize) -> Result<(), FluentParseError> {
        let (is_default, rest) = match text.strip_prefix(DEFAULT_VARIANT_MARK) {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let rest = rest
            .strip_prefix(VARIANT_OPEN)
            .ok_or(FluentParseError::new(
                FluentParseErrorKind::ExpectedVariant,
                line,
            ))?;
        let (key_text, value) = rest.split_once(VARIANT_CLOSE).ok_or(FluentParseError::new(
            FluentParseErrorKind::InvalidVariantKey,
            line,
        ))?;
        let key = select_key(key_text.trim()).ok_or(FluentParseError::new(
            FluentParseErrorKind::InvalidVariantKey,
            line,
        ))?;
        let (elements, selector) = parse_inline(value.trim_start(), line)?;
        if selector.is_some() {
            return Err(FluentParseError::new(
                FluentParseErrorKind::NestedSelect,
                line,
            ));
        }
        if elements.is_empty() {
            return Err(FluentParseError::new(
                FluentParseErrorKind::EmptyVariantValue,
                line,
            ));
        }
        if is_default {
            if self.default_variant.is_some() {
                return Err(FluentParseError::new(
                    FluentParseErrorKind::MultipleDefaultVariants,
                    line,
                ));
            }
            self.default_variant = Some(self.variants.len());
        }
        self.variants.push(SelectVariant { key, elements });
        Ok(())
    }

    fn finish(self) -> Result<SelectExpression, FluentParseError> {
        let default_variant = self.default_variant.ok_or(FluentParseError::new(
            FluentParseErrorKind::MissingDefaultVariant,
            self.line,
        ))?;
        Ok(SelectExpression {
            selector: self.selector,
            variants: self.variants,
            default_variant,
        })
    }
}

fn select_key(text: &str) -> Option<SelectKey> {
    if !text.is_empty() && text.chars().all(|c| c.is_ascii_digit()) {
        return text.parse().ok().map(SelectKey::Exact);
    }
    plural_category_from_identifier(text).map(SelectKey::Category)
}

pub(crate) fn plural_category_from_identifier(identifier: &str) -> Option<PluralCategory> {
    match identifier {
        "one" => Some(PluralCategory::One),
        "other" => Some(PluralCategory::Other),
        _ => None,
    }
}

pub(crate) fn plural_rule_for_language(language: &str) -> Option<PluralRule> {
    ONE_OTHER_LANGUAGES
        .iter()
        .any(|known| known.eq_ignore_ascii_case(language))
        .then_some(PluralRule::OneOther)
}

fn parse_inline(
    text: &str,
    line: usize,
) -> Result<(Vec<InlineElement>, Option<String>), FluentParseError> {
    let mut elements = Vec::new();
    let mut rest = text;
    loop {
        let Some(position) = rest.find([PLACEABLE_OPEN, PLACEABLE_CLOSE]) else {
            push_inline_text(&mut elements, rest);
            return Ok((elements, None));
        };
        let (before, from_brace) = rest.split_at(position);
        push_inline_text(&mut elements, before);
        let Some(inside) = from_brace.strip_prefix(PLACEABLE_OPEN) else {
            return Err(FluentParseError::new(
                FluentParseErrorKind::UnexpectedClosingBrace,
                line,
            ));
        };
        match parse_placeable(inside.trim_start(), line)? {
            Placeable::Variable { name, rest: after } => {
                elements.push(InlineElement::Variable(name));
                rest = after;
            }
            Placeable::Text { text, rest: after } => {
                push_inline_text(&mut elements, &text);
                rest = after;
            }
            Placeable::SelectOpen { selector } => return Ok((elements, Some(selector))),
        }
    }
}

fn push_inline_text(elements: &mut Vec<InlineElement>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(InlineElement::Text(previous)) = elements.last_mut() {
        previous.push_str(text);
    } else {
        elements.push(InlineElement::Text(text.to_owned()));
    }
}

fn parse_placeable(inside: &str, line: usize) -> Result<Placeable<'_>, FluentParseError> {
    if let Some(after_sigil) = inside.strip_prefix(VARIABLE_SIGIL) {
        let name_length = after_sigil
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == KEY_SEPARATOR))
            .unwrap_or(after_sigil.len());
        let (name, after_name) = after_sigil.split_at(name_length);
        if !is_fluent_identifier(name) {
            return Err(FluentParseError::new(
                FluentParseErrorKind::ExpectedVariableName,
                line,
            ));
        }
        let after_name = after_name.trim_start();
        if let Some(rest) = after_name.strip_prefix(PLACEABLE_CLOSE) {
            return Ok(Placeable::Variable {
                name: name.to_owned(),
                rest,
            });
        }
        if after_name
            .strip_prefix(SELECT_ARROW)
            .is_some_and(|rest| rest.trim().is_empty())
        {
            return Ok(Placeable::SelectOpen {
                selector: name.to_owned(),
            });
        }
        return Err(placeable_end_error(after_name, line));
    }
    if let Some(after_quote) = inside.strip_prefix(STRING_QUOTE) {
        let (text, after_literal) = parse_string_literal(after_quote, line)?;
        let after_literal = after_literal.trim_start();
        return match after_literal.strip_prefix(PLACEABLE_CLOSE) {
            Some(rest) => Ok(Placeable::Text { text, rest }),
            None => Err(placeable_end_error(after_literal, line)),
        };
    }
    Err(placeable_end_error(inside, line))
}

fn placeable_end_error(rest: &str, line: usize) -> FluentParseError {
    let kind = if rest.is_empty() {
        FluentParseErrorKind::UnclosedPlaceable
    } else {
        FluentParseErrorKind::UnsupportedExpression
    };
    FluentParseError::new(kind, line)
}

fn parse_string_literal(text: &str, line: usize) -> Result<(String, &str), FluentParseError> {
    let mut value = String::new();
    let mut chars = text.char_indices();
    while let Some((position, c)) = chars.next() {
        match c {
            STRING_QUOTE => {
                let rest = text.get(position + c.len_utf8()..).unwrap_or_default();
                return Ok((value, rest));
            }
            STRING_ESCAPE => match chars.next() {
                Some((_, escaped @ (STRING_QUOTE | STRING_ESCAPE))) => value.push(escaped),
                Some(_) => {
                    return Err(FluentParseError::new(
                        FluentParseErrorKind::InvalidEscape,
                        line,
                    ));
                }
                None => break,
            },
            other => value.push(other),
        }
    }
    Err(FluentParseError::new(
        FluentParseErrorKind::UnclosedPlaceable,
        line,
    ))
}

pub(crate) fn build_catalog(mut locales: Vec<LocaleFiles>) -> Result<Catalog, CatalogError> {
    locales.sort_by(|first, second| first.tag.cmp(&second.tag));
    let mut parsed = Vec::new();
    for locale in &mut locales {
        let tag = LanguageTag::parse(&locale.tag).map_err(|_| CatalogError::InvalidLocaleTag {
            locale: locale.tag.clone(),
        })?;
        let plural_rule = tag
            .language()
            .and_then(plural_rule_for_language)
            .ok_or_else(|| CatalogError::NoPluralRule {
                locale: locale.tag.clone(),
            })?;
        locale
            .areas
            .sort_by(|first, second| first.area.cmp(&second.area));
        parsed.push((locale.tag.clone(), plural_rule, locale_messages(locale)?));
    }
    let source_locale = parsed
        .iter()
        .position(|(tag, _, _)| tag == SOURCE_LOCALE)
        .ok_or(CatalogError::MissingSourceLocale)?;
    let messages = match parsed.get(source_locale) {
        Some((_, _, source_messages)) => catalog_messages(source_messages)?,
        None => return Err(CatalogError::MissingSourceLocale),
    };
    for (tag, _, translations) in &parsed {
        check_translation(tag, translations, &messages)?;
    }
    let locales = parsed
        .into_iter()
        .map(|(tag, plural_rule, mut translations)| CatalogLocale {
            tag,
            plural_rule,
            patterns: messages
                .iter()
                .map(|message| translations.remove(&message.key))
                .collect(),
        })
        .collect();
    Ok(Catalog {
        messages,
        locales,
        source_locale,
    })
}

fn locale_messages(locale: &LocaleFiles) -> Result<BTreeMap<String, Vec<Element>>, CatalogError> {
    let mut messages = BTreeMap::new();
    for area_file in &locale.areas {
        let parsed = parse_resource(&area_file.text).map_err(|error| CatalogError::Parse {
            locale: locale.tag.clone(),
            area: area_file.area.clone(),
            error,
        })?;
        for message in parsed {
            if !is_valid_key(&message.key) {
                return Err(CatalogError::InvalidKey {
                    locale: locale.tag.clone(),
                    key: message.key,
                });
            }
            if key_area(&message.key) != Some(area_file.area.as_str()) {
                return Err(CatalogError::KeyInWrongArea {
                    locale: locale.tag.clone(),
                    area: area_file.area.clone(),
                    key: message.key,
                });
            }
            messages.insert(message.key, message.elements);
        }
    }
    Ok(messages)
}

fn is_valid_key(key: &str) -> bool {
    let mut segments = key.split(KEY_SEPARATOR);
    let first_is_valid = segments.next().is_some_and(|first| {
        first.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && is_lowercase_alphanumeric(first)
            && AREAS.contains(&first)
    });
    let rest: Vec<&str> = segments.collect();
    first_is_valid
        && !rest.is_empty()
        && rest
            .iter()
            .all(|segment| !segment.is_empty() && is_lowercase_alphanumeric(segment))
}

fn is_lowercase_alphanumeric(text: &str) -> bool {
    text.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

fn key_area(key: &str) -> Option<&str> {
    key.split(KEY_SEPARATOR).next()
}

fn catalog_messages(
    source: &BTreeMap<String, Vec<Element>>,
) -> Result<Vec<CatalogMessage>, CatalogError> {
    let mut variant_owners: BTreeMap<String, String> = BTreeMap::new();
    let mut messages = Vec::new();
    for (key, elements) in source {
        let fields = fields_of(elements);
        if let Some(field) = fields
            .iter()
            .find(|field| !is_valid_field_name(&field.name))
        {
            return Err(CatalogError::InvalidPlaceholderName {
                key: key.clone(),
                variable: field.name.clone(),
            });
        }
        let variant_name = variant_name(key);
        if let Some(first_key) = variant_owners.insert(variant_name.clone(), key.clone()) {
            return Err(CatalogError::VariantNameCollision {
                first_key,
                second_key: key.clone(),
                variant_name,
            });
        }
        messages.push(CatalogMessage {
            key: key.clone(),
            variant_name,
            fields,
        });
    }
    Ok(messages)
}

fn variable_uses(elements: &[Element]) -> Vec<(&str, FieldKind)> {
    let mut uses = Vec::new();
    for element in elements {
        match element {
            Element::Text(_) => {}
            Element::Variable(name) => uses.push((name.as_str(), FieldKind::Text)),
            Element::Select(select) => {
                uses.push((select.selector.as_str(), FieldKind::Count));
                for variant in &select.variants {
                    for inline in &variant.elements {
                        if let InlineElement::Variable(name) = inline {
                            uses.push((name.as_str(), FieldKind::Text));
                        }
                    }
                }
            }
        }
    }
    uses
}

fn fields_of(elements: &[Element]) -> Vec<Field> {
    let mut fields: Vec<Field> = Vec::new();
    for (name, kind) in variable_uses(elements) {
        match fields.iter_mut().find(|field| field.name == name) {
            Some(field) if kind == FieldKind::Count => field.kind = FieldKind::Count,
            Some(_) => {}
            None => fields.push(Field {
                name: name.to_owned(),
                kind,
            }),
        }
    }
    fields
}

fn is_valid_field_name(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == FIELD_NAME_SEPARATOR)
        && !RUST_KEYWORDS.contains(&name)
}

fn variant_name(key: &str) -> String {
    key.split(KEY_SEPARATOR)
        .map(|segment| {
            let mut chars = segment.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

fn check_translation(
    tag: &str,
    translations: &BTreeMap<String, Vec<Element>>,
    messages: &[CatalogMessage],
) -> Result<(), CatalogError> {
    for (key, elements) in translations {
        let message = messages
            .binary_search_by(|message| message.key.as_str().cmp(key))
            .ok()
            .and_then(|index| messages.get(index))
            .ok_or_else(|| CatalogError::KeyNotInSource {
                locale: tag.to_owned(),
                key: key.clone(),
            })?;
        for (name, kind) in variable_uses(elements) {
            let field = message
                .fields
                .iter()
                .find(|field| field.name == name)
                .ok_or_else(|| CatalogError::UnknownPlaceholder {
                    locale: tag.to_owned(),
                    key: key.clone(),
                    variable: name.to_owned(),
                })?;
            if kind == FieldKind::Count && field.kind != FieldKind::Count {
                return Err(CatalogError::SelectorNotCount {
                    locale: tag.to_owned(),
                    key: key.clone(),
                    variable: name.to_owned(),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_error(source: &str) -> FluentParseErrorKind {
        parse_resource(source).unwrap_err().kind
    }

    fn single_message(source: &str) -> Vec<Element> {
        let mut messages = parse_resource(source).unwrap();
        assert_eq!(messages.len(), 1);
        messages.remove(0).elements
    }

    fn text(value: &str) -> Element {
        Element::Text(value.to_owned())
    }

    fn locale(tag: &str, areas: &[(&str, &str)]) -> LocaleFiles {
        LocaleFiles {
            tag: tag.to_owned(),
            areas: areas
                .iter()
                .map(|(area, text)| AreaFile {
                    area: (*area).to_owned(),
                    text: (*text).to_owned(),
                })
                .collect(),
        }
    }

    fn catalog_error(locales: Vec<LocaleFiles>) -> CatalogError {
        build_catalog(locales).unwrap_err()
    }

    const COUNT_SOURCE: &str =
        "common-lines = { $count ->\n    [one] one line\n   *[other] { $count } lines\n}\n";

    #[test]
    fn simple_message_is_text() {
        let elements = single_message("common-kind-exact = exact\n");
        assert_eq!(elements, vec![text("exact")]);
    }

    #[test]
    fn variable_placeable_is_a_variable() {
        let elements = single_message("error-undefined = { $name } is not defined\n");
        assert_eq!(
            elements,
            vec![
                Element::Variable("name".to_owned()),
                text(" is not defined")
            ]
        );
    }

    #[test]
    fn string_literal_placeable_is_text() {
        let elements = single_message("ui-brace = a { \"{\" } b\n");
        assert_eq!(elements, vec![text("a { b")]);
    }

    #[test]
    fn string_literal_escapes_are_decoded() {
        let elements = single_message("ui-quote = { \"\\\"\\\\\" }\n");
        assert_eq!(elements, vec![text("\"\\")]);
    }

    #[test]
    fn select_has_variants_and_default() {
        let elements = single_message(COUNT_SOURCE);
        let expected = Element::Select(SelectExpression {
            selector: "count".to_owned(),
            variants: vec![
                SelectVariant {
                    key: SelectKey::Category(PluralCategory::One),
                    elements: vec![InlineElement::Text("one line".to_owned())],
                },
                SelectVariant {
                    key: SelectKey::Category(PluralCategory::Other),
                    elements: vec![
                        InlineElement::Variable("count".to_owned()),
                        InlineElement::Text(" lines".to_owned()),
                    ],
                },
            ],
            default_variant: 1,
        });
        assert_eq!(elements, vec![expected]);
    }

    #[test]
    fn select_closed_at_line_start_ends_the_message() {
        let messages = parse_resource(&format!("{COUNT_SOURCE}common-next = next\n")).unwrap();
        let keys: Vec<&str> = messages.iter().map(|m| m.key.as_str()).collect();
        assert_eq!(keys, vec!["common-lines", "common-next"]);
    }

    #[test]
    fn numeric_variant_key_is_exact() {
        let elements = single_message("ui-n = { $n ->\n    [0] none\n   *[other] some\n}\n");
        let Element::Select(select) = &elements[0] else {
            panic!("expected a select");
        };
        assert_eq!(select.variants[0].key, SelectKey::Exact(0));
    }

    #[test]
    fn text_continues_after_closing_select() {
        let elements = single_message("ui-n = a { $n ->\n   *[other] b\n  } c\n");
        assert_eq!(elements.len(), 3);
        assert_eq!(elements[2], text(" c"));
    }

    #[test]
    fn multiline_value_removes_common_indentation() {
        let elements = single_message("cli-help =\n    Usage:\n      calc\n\n    Done\n");
        assert_eq!(elements, vec![text("Usage:\n  calc\n\nDone")]);
    }

    #[test]
    fn inline_first_line_joins_continuation_with_a_line_break() {
        let elements = single_message("cli-help = first\n    second\n");
        assert_eq!(elements, vec![text("first\nsecond")]);
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let messages =
            parse_resource("# comment\n\ncommon-a = a\n\n## section\ncommon-b = b\n").unwrap();
        let keys: Vec<&str> = messages.iter().map(|m| m.key.as_str()).collect();
        assert_eq!(keys, vec!["common-a", "common-b"]);
    }

    #[test]
    fn line_without_assignment_is_expected_message_error() {
        assert_eq!(
            parse_error("common-a\n"),
            FluentParseErrorKind::ExpectedMessage
        );
    }

    #[test]
    fn term_is_expected_message_error() {
        assert_eq!(
            parse_error("-brand = Calc\n"),
            FluentParseErrorKind::ExpectedMessage
        );
    }

    #[test]
    fn indented_first_line_is_outside_message_error() {
        assert_eq!(
            parse_error("  common-a = a\n"),
            FluentParseErrorKind::IndentedLineOutsideMessage
        );
    }

    #[test]
    fn repeated_key_is_duplicate_key_error() {
        assert_eq!(
            parse_error("common-a = a\ncommon-a = b\n"),
            FluentParseErrorKind::DuplicateKey
        );
    }

    #[test]
    fn message_without_value_is_empty_value_error() {
        assert_eq!(
            parse_error("common-a =\n"),
            FluentParseErrorKind::EmptyValue
        );
    }

    #[test]
    fn attribute_is_unsupported_attribute_error() {
        assert_eq!(
            parse_error("common-a = a\n    .title = b\n"),
            FluentParseErrorKind::UnsupportedAttribute
        );
    }

    #[test]
    fn placeable_without_closing_brace_is_unclosed_placeable_error() {
        assert_eq!(
            parse_error("common-a = { $name\n"),
            FluentParseErrorKind::UnclosedPlaceable
        );
    }

    #[test]
    fn message_reference_is_unsupported_expression_error() {
        assert_eq!(
            parse_error("common-a = { common-b }\n"),
            FluentParseErrorKind::UnsupportedExpression
        );
    }

    #[test]
    fn sigil_without_name_is_expected_variable_name_error() {
        assert_eq!(
            parse_error("common-a = { $ }\n"),
            FluentParseErrorKind::ExpectedVariableName
        );
    }

    #[test]
    fn closing_brace_in_text_is_unexpected_closing_brace_error() {
        assert_eq!(
            parse_error("common-a = a } b\n"),
            FluentParseErrorKind::UnexpectedClosingBrace
        );
    }

    #[test]
    fn unknown_string_escape_is_invalid_escape_error() {
        assert_eq!(
            parse_error("common-a = { \"\\n\" }\n"),
            FluentParseErrorKind::InvalidEscape
        );
    }

    #[test]
    fn select_inside_variant_is_nested_select_error() {
        assert_eq!(
            parse_error("common-a = { $n ->\n   *[other] { $m ->\n}\n"),
            FluentParseErrorKind::NestedSelect
        );
    }

    #[test]
    fn text_line_inside_select_is_expected_variant_error() {
        assert_eq!(
            parse_error("common-a = { $n ->\n    text\n}\n"),
            FluentParseErrorKind::ExpectedVariant
        );
    }

    #[test]
    fn unknown_category_is_invalid_variant_key_error() {
        assert_eq!(
            parse_error("common-a = { $n ->\n   *[few] a\n}\n"),
            FluentParseErrorKind::InvalidVariantKey
        );
    }

    #[test]
    fn variant_without_value_is_empty_variant_value_error() {
        assert_eq!(
            parse_error("common-a = { $n ->\n   *[other]\n}\n"),
            FluentParseErrorKind::EmptyVariantValue
        );
    }

    #[test]
    fn select_without_default_is_missing_default_variant_error() {
        assert_eq!(
            parse_error("common-a = { $n ->\n    [one] a\n}\n"),
            FluentParseErrorKind::MissingDefaultVariant
        );
    }

    #[test]
    fn two_defaults_is_multiple_default_variants_error() {
        assert_eq!(
            parse_error("common-a = { $n ->\n   *[one] a\n   *[other] b\n}\n"),
            FluentParseErrorKind::MultipleDefaultVariants
        );
    }

    #[test]
    fn select_without_closing_line_is_unclosed_select_error() {
        assert_eq!(
            parse_error("common-a = { $n ->\n   *[other] a\n"),
            FluentParseErrorKind::UnclosedSelect
        );
    }

    #[test]
    fn parse_error_names_its_line() {
        let error = parse_resource("common-a = a\n\ncommon-b\n").unwrap_err();
        assert_eq!(error.line, 3);
    }

    #[test]
    fn valid_catalog_orders_messages_by_key() {
        let catalog = build_catalog(vec![locale(
            "en",
            &[("common", "common-b = b\ncommon-a = a\n")],
        )])
        .unwrap();
        let keys: Vec<&str> = catalog.messages.iter().map(|m| m.key.as_str()).collect();
        assert_eq!(keys, vec!["common-a", "common-b"]);
    }

    #[test]
    fn variant_name_capitalizes_each_key_segment() {
        let catalog = build_catalog(vec![locale(
            "en",
            &[("error", "error-division-by-zero = division by zero\n")],
        )])
        .unwrap();
        assert_eq!(catalog.messages[0].variant_name, "ErrorDivisionByZero");
    }

    #[test]
    fn selector_variable_is_a_count_field() {
        let catalog = build_catalog(vec![locale("en", &[("common", COUNT_SOURCE)])]).unwrap();
        assert_eq!(
            catalog.messages[0].fields,
            vec![Field {
                name: "count".to_owned(),
                kind: FieldKind::Count,
            }]
        );
    }

    #[test]
    fn placeable_variable_is_a_text_field() {
        let catalog = build_catalog(vec![locale(
            "en",
            &[("error", "error-undefined = { $name } is not defined\n")],
        )])
        .unwrap();
        assert_eq!(catalog.messages[0].fields[0].kind, FieldKind::Text);
    }

    #[test]
    fn missing_translation_is_an_absent_pattern() {
        let catalog = build_catalog(vec![
            locale("en", &[("common", "common-a = a\ncommon-b = b\n")]),
            locale("de", &[("common", "common-a = A\n")]),
        ])
        .unwrap();
        let german = &catalog.locales[0];
        assert_eq!(german.tag, "de");
        assert_eq!(german.patterns[1], None);
    }

    #[test]
    fn catalog_without_english_is_missing_source_locale_error() {
        let error = catalog_error(vec![locale("de", &[("common", "common-a = A\n")])]);
        assert_eq!(error, CatalogError::MissingSourceLocale);
    }

    #[test]
    fn malformed_locale_directory_is_invalid_locale_tag_error() {
        let error = catalog_error(vec![locale("de_DE", &[])]);
        assert_eq!(
            error,
            CatalogError::InvalidLocaleTag {
                locale: "de_DE".to_owned()
            }
        );
    }

    #[test]
    fn language_without_rule_is_no_plural_rule_error() {
        let error = catalog_error(vec![locale("ja", &[])]);
        assert_eq!(
            error,
            CatalogError::NoPluralRule {
                locale: "ja".to_owned()
            }
        );
    }

    #[test]
    fn unparsable_file_is_parse_error() {
        let error = catalog_error(vec![locale("en", &[("common", "common-a\n")])]);
        assert_eq!(
            error,
            CatalogError::Parse {
                locale: "en".to_owned(),
                area: "common".to_owned(),
                error: FluentParseError::new(FluentParseErrorKind::ExpectedMessage, 1),
            }
        );
    }

    #[test]
    fn uppercase_key_is_invalid_key_error() {
        let error = catalog_error(vec![locale("en", &[("common", "common-Kind = a\n")])]);
        assert_eq!(
            error,
            CatalogError::InvalidKey {
                locale: "en".to_owned(),
                key: "common-Kind".to_owned()
            }
        );
    }

    #[test]
    fn key_with_one_segment_is_invalid_key_error() {
        let error = catalog_error(vec![locale("en", &[("common", "common = a\n")])]);
        assert!(matches!(error, CatalogError::InvalidKey { .. }));
    }

    #[test]
    fn key_with_unknown_area_is_invalid_key_error() {
        let error = catalog_error(vec![locale("en", &[("common", "menu-open = a\n")])]);
        assert!(matches!(error, CatalogError::InvalidKey { .. }));
    }

    #[test]
    fn key_with_underscore_is_invalid_key_error() {
        let error = catalog_error(vec![locale("en", &[("common", "common-kind_a = a\n")])]);
        assert!(matches!(error, CatalogError::InvalidKey { .. }));
    }

    #[test]
    fn key_in_other_area_file_is_key_in_wrong_area_error() {
        let error = catalog_error(vec![locale("en", &[("common", "ui-menu = a\n")])]);
        assert_eq!(
            error,
            CatalogError::KeyInWrongArea {
                locale: "en".to_owned(),
                area: "common".to_owned(),
                key: "ui-menu".to_owned()
            }
        );
    }

    #[test]
    fn translation_key_missing_in_english_is_key_not_in_source_error() {
        let error = catalog_error(vec![
            locale("en", &[("common", "common-a = a\n")]),
            locale("de", &[("common", "common-b = B\n")]),
        ]);
        assert_eq!(
            error,
            CatalogError::KeyNotInSource {
                locale: "de".to_owned(),
                key: "common-b".to_owned()
            }
        );
    }

    #[test]
    fn translation_with_new_variable_is_unknown_placeholder_error() {
        let error = catalog_error(vec![
            locale("en", &[("common", "common-a = a\n")]),
            locale("de", &[("common", "common-a = { $x }\n")]),
        ]);
        assert_eq!(
            error,
            CatalogError::UnknownPlaceholder {
                locale: "de".to_owned(),
                key: "common-a".to_owned(),
                variable: "x".to_owned()
            }
        );
    }

    #[test]
    fn translation_selecting_on_text_variable_is_selector_not_count_error() {
        let error = catalog_error(vec![
            locale("en", &[("common", "common-a = { $x }\n")]),
            locale(
                "de",
                &[("common", "common-a = { $x ->\n   *[other] b\n}\n")],
            ),
        ]);
        assert_eq!(
            error,
            CatalogError::SelectorNotCount {
                locale: "de".to_owned(),
                key: "common-a".to_owned(),
                variable: "x".to_owned()
            }
        );
    }

    #[test]
    fn hyphenated_variable_is_invalid_placeholder_name_error() {
        let error = catalog_error(vec![locale(
            "en",
            &[("common", "common-a = { $line-count }\n")],
        )]);
        assert_eq!(
            error,
            CatalogError::InvalidPlaceholderName {
                key: "common-a".to_owned(),
                variable: "line-count".to_owned()
            }
        );
    }

    #[test]
    fn keyword_variable_is_invalid_placeholder_name_error() {
        let error = catalog_error(vec![locale("en", &[("common", "common-a = { $type }\n")])]);
        assert!(matches!(error, CatalogError::InvalidPlaceholderName { .. }));
    }

    #[test]
    fn keys_with_same_variant_name_are_variant_name_collision_error() {
        let error = catalog_error(vec![locale("en", &[("ui", "ui-a1b = a\nui-a-1b = b\n")])]);
        assert_eq!(
            error,
            CatalogError::VariantNameCollision {
                first_key: "ui-a-1b".to_owned(),
                second_key: "ui-a1b".to_owned(),
                variant_name: "UiA1b".to_owned()
            }
        );
    }
}
