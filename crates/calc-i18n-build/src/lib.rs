#[allow(dead_code)]
#[path = "../../calc-i18n/src/catalog.rs"]
mod catalog;
#[allow(dead_code)]
#[path = "../../calc-i18n/src/language_tag.rs"]
mod language_tag;
#[allow(dead_code)]
#[path = "../../calc-i18n/src/plural.rs"]
mod plural;

use std::fmt::{self, Write as _};
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use catalog::{
    AreaFile, Catalog, CatalogError, Element, Field, FieldKind, InlineElement, LocaleFiles,
    SelectExpression, SelectKey,
};

const FLUENT_EXTENSION: &str = "ftl";

#[derive(Debug)]
pub enum BuildScriptError {
    Read { path: PathBuf, kind: io::ErrorKind },
    NonUnicodeName(PathBuf),
    Catalog(String),
    UnresolvedPlaceholder { key: String, variable: String },
    Format,
    Write { path: PathBuf, kind: io::ErrorKind },
}

impl From<fmt::Error> for BuildScriptError {
    fn from(_: fmt::Error) -> Self {
        Self::Format
    }
}

pub fn write_catalog(locales_directory: &Path, output_file: &Path) -> Result<(), BuildScriptError> {
    writeln!(
        io::stdout(),
        "cargo::rerun-if-changed={}",
        locales_directory.display()
    )
    .map_err(|error| BuildScriptError::Write {
        path: PathBuf::new(),
        kind: error.kind(),
    })?;
    let locales = read_locales(locales_directory)?;
    let catalog = catalog::build_catalog(locales)
        .map_err(|error| BuildScriptError::Catalog(format!("{error:?}")))?;
    let generated = generate(&catalog)?;
    fs::write(output_file, generated).map_err(|error| BuildScriptError::Write {
        path: output_file.to_path_buf(),
        kind: error.kind(),
    })
}

fn read_locales(directory: &Path) -> Result<Vec<LocaleFiles>, BuildScriptError> {
    let mut locales = Vec::new();
    for locale_path in sorted_entries(directory)? {
        if !locale_path.is_dir() {
            continue;
        }
        let mut areas = Vec::new();
        for file_path in sorted_entries(&locale_path)? {
            if file_path
                .extension()
                .and_then(|extension| extension.to_str())
                != Some(FLUENT_EXTENSION)
            {
                continue;
            }
            let area = file_name(&file_path, Path::file_stem)?;
            let text = fs::read_to_string(&file_path).map_err(|error| BuildScriptError::Read {
                path: file_path.clone(),
                kind: error.kind(),
            })?;
            areas.push(AreaFile { area, text });
        }
        locales.push(LocaleFiles {
            tag: file_name(&locale_path, Path::file_name)?,
            areas,
        });
    }
    Ok(locales)
}

fn sorted_entries(directory: &Path) -> Result<Vec<PathBuf>, BuildScriptError> {
    let read_error = |error: io::Error| BuildScriptError::Read {
        path: directory.to_path_buf(),
        kind: error.kind(),
    };
    let mut paths = fs::read_dir(directory)
        .map_err(read_error)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(read_error)?;
    paths.sort();
    Ok(paths)
}

fn file_name(
    path: &Path,
    part: impl Fn(&Path) -> Option<&std::ffi::OsStr>,
) -> Result<String, BuildScriptError> {
    part(path)
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| BuildScriptError::NonUnicodeName(path.to_path_buf()))
}

fn generate(catalog: &Catalog) -> Result<String, BuildScriptError> {
    let mut out = String::new();
    let message_count = catalog.messages.len();
    let locale_count = catalog.locales.len();
    let source_locale = catalog.source_locale;
    writeln!(out, "#[derive(Clone, Debug, PartialEq, Eq)]")?;
    writeln!(out, "pub enum Message {{")?;
    for message in &catalog.messages {
        let variant = &message.variant_name;
        if message.fields.is_empty() {
            writeln!(out, "    {variant},")?;
        } else {
            let fields: Vec<String> = message.fields.iter().map(field_declaration).collect();
            let fields = fields.join(", ");
            writeln!(out, "    {variant} {{ {fields} }},")?;
        }
    }
    writeln!(out, "}}")?;
    writeln!(
        out,
        "pub(crate) const MESSAGE_COUNT: usize = {message_count};"
    )?;
    writeln!(
        out,
        "pub(crate) const SOURCE_LOCALE_INDEX: usize = {source_locale};"
    )?;
    writeln!(
        out,
        "pub(crate) static MESSAGE_KEYS: [&str; MESSAGE_COUNT] = ["
    )?;
    for message in &catalog.messages {
        writeln!(out, "    {:?},", message.key)?;
    }
    writeln!(out, "];")?;
    writeln!(out, "impl Message {{")?;
    writeln!(out, "    pub fn key(&self) -> &'static str {{")?;
    writeln!(out, "        MESSAGE_KEYS[self.index()]")?;
    writeln!(out, "    }}")?;
    writeln!(out, "    pub(crate) fn index(&self) -> usize {{")?;
    writeln!(out, "        match self {{")?;
    for (index, message) in catalog.messages.iter().enumerate() {
        let variant = &message.variant_name;
        let rest = if message.fields.is_empty() {
            ""
        } else {
            " { .. }"
        };
        writeln!(out, "            Self::{variant}{rest} => {index},")?;
    }
    writeln!(out, "        }}")?;
    writeln!(out, "    }}")?;
    writeln!(
        out,
        "    pub(crate) fn argument(&self, slot: usize) -> Option<crate::pattern::Argument<'_>> {{"
    )?;
    writeln!(out, "        match (self, slot) {{")?;
    for message in &catalog.messages {
        let variant = &message.variant_name;
        for (slot, field) in message.fields.iter().enumerate() {
            let name = &field.name;
            let argument = match field.kind {
                FieldKind::Text => format!("crate::pattern::Argument::Text({name}.as_str())"),
                FieldKind::Count => format!("crate::pattern::Argument::Count(*{name})"),
            };
            writeln!(
                out,
                "            (Self::{variant} {{ {name}, .. }}, {slot}) => Some({argument}),"
            )?;
        }
    }
    writeln!(out, "            _ => None,")?;
    writeln!(out, "        }}")?;
    writeln!(out, "    }}")?;
    writeln!(out, "}}")?;
    writeln!(out, "impl crate::CatalogMessage for Message {{")?;
    writeln!(out, "    fn index(&self) -> usize {{")?;
    writeln!(out, "        Message::index(self)")?;
    writeln!(out, "    }}")?;
    writeln!(
        out,
        "    fn argument(&self, slot: usize) -> Option<crate::pattern::Argument<'_>> {{"
    )?;
    writeln!(out, "        Message::argument(self, slot)")?;
    writeln!(out, "    }}")?;
    writeln!(
        out,
        "    fn source_patterns() -> &'static [crate::pattern::Pattern] {{"
    )?;
    writeln!(out, "        &SOURCE_PATTERNS")?;
    writeln!(out, "    }}")?;
    writeln!(
        out,
        "    fn shipped_locales() -> &'static [crate::pattern::LocaleEntry] {{"
    )?;
    writeln!(out, "        &SHIPPED_LOCALES")?;
    writeln!(out, "    }}")?;
    writeln!(out, "    fn source_locale_index() -> usize {{")?;
    writeln!(out, "        SOURCE_LOCALE_INDEX")?;
    writeln!(out, "    }}")?;
    writeln!(out, "}}")?;
    let source = catalog.locales.get(source_locale).ok_or_else(|| {
        BuildScriptError::Catalog(format!("{:?}", CatalogError::MissingSourceLocale))
    })?;
    writeln!(
        out,
        "pub(crate) static SOURCE_PATTERNS: [crate::pattern::Pattern; MESSAGE_COUNT] = ["
    )?;
    for (message, pattern) in catalog.messages.iter().zip(&source.patterns) {
        let elements = pattern.as_deref().unwrap_or_default();
        writeln!(out, "    {},", pattern_code(message, elements)?)?;
    }
    writeln!(out, "];")?;
    writeln!(
        out,
        "pub(crate) static SHIPPED_LOCALES: [crate::pattern::LocaleEntry; {locale_count}] = ["
    )?;
    for locale in &catalog.locales {
        let tag = &locale.tag;
        let plural_rule = locale.plural_rule;
        writeln!(out, "    crate::pattern::LocaleEntry {{")?;
        writeln!(out, "        tag: {tag:?},")?;
        writeln!(
            out,
            "        plural_rule: crate::plural::PluralRule::{plural_rule:?},"
        )?;
        writeln!(out, "        patterns: &[")?;
        for (message, pattern) in catalog.messages.iter().zip(&locale.patterns) {
            match pattern {
                Some(elements) => writeln!(
                    out,
                    "            Some({}),",
                    pattern_code(message, elements)?
                )?,
                None => writeln!(out, "            None,")?,
            }
        }
        writeln!(out, "        ],")?;
        writeln!(out, "    }},")?;
    }
    writeln!(out, "];")?;
    Ok(out)
}

fn field_declaration(field: &Field) -> String {
    let name = &field.name;
    match field.kind {
        FieldKind::Text => format!("{name}: String"),
        FieldKind::Count => format!("{name}: u64"),
    }
}

fn slot(message: &catalog::CatalogMessage, variable: &str) -> Result<usize, BuildScriptError> {
    message
        .fields
        .iter()
        .position(|field| field.name == variable)
        .ok_or_else(|| BuildScriptError::UnresolvedPlaceholder {
            key: message.key.clone(),
            variable: variable.to_owned(),
        })
}

fn pattern_code(
    message: &catalog::CatalogMessage,
    elements: &[Element],
) -> Result<String, BuildScriptError> {
    let segments = elements
        .iter()
        .map(|element| segment_code(message, element))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("&[{}]", segments.join(", ")))
}

fn segment_code(
    message: &catalog::CatalogMessage,
    element: &Element,
) -> Result<String, BuildScriptError> {
    Ok(match element {
        Element::Text(text) => format!("crate::pattern::Segment::Text({text:?})"),
        Element::Variable(name) => {
            let slot = slot(message, name)?;
            format!("crate::pattern::Segment::Argument({slot})")
        }
        Element::Select(select) => select_code(message, select)?,
    })
}

fn select_code(
    message: &catalog::CatalogMessage,
    select: &SelectExpression,
) -> Result<String, BuildScriptError> {
    let selector = slot(message, &select.selector)?;
    let variants = select
        .variants
        .iter()
        .map(|variant| {
            let key = match variant.key {
                SelectKey::Category(category) => format!(
                    "crate::pattern::VariantKey::Category(crate::plural::PluralCategory::{category:?})"
                ),
                SelectKey::Exact(value) => format!("crate::pattern::VariantKey::Exact({value})"),
            };
            let elements = inline_code(message, &variant.elements)?;
            Ok(format!(
                "crate::pattern::Variant {{ key: {key}, elements: {elements} }}"
            ))
        })
        .collect::<Result<Vec<_>, BuildScriptError>>()?;
    let default = select
        .variants
        .get(select.default_variant)
        .map(|variant| inline_code(message, &variant.elements))
        .transpose()?
        .unwrap_or_else(|| "&[]".to_owned());
    Ok(format!(
        "crate::pattern::Segment::Select(crate::pattern::Select {{ selector: {selector}, variants: &[{}], default: {default} }})",
        variants.join(", ")
    ))
}

fn inline_code(
    message: &catalog::CatalogMessage,
    elements: &[InlineElement],
) -> Result<String, BuildScriptError> {
    let parts = elements
        .iter()
        .map(|element| {
            Ok(match element {
                InlineElement::Text(text) => format!("crate::pattern::Inline::Text({text:?})"),
                InlineElement::Variable(name) => {
                    let slot = slot(message, name)?;
                    format!("crate::pattern::Inline::Argument({slot})")
                }
            })
        })
        .collect::<Result<Vec<_>, BuildScriptError>>()?;
    Ok(format!("&[{}]", parts.join(", ")))
}
