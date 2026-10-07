use calc_syntax::NotationMode;

use crate::error::{LoadError, LoadErrorKind};

const TITLE_PREFIX: &str = "# ";
const SECTION_PREFIX: &str = "## ";
const FENCE_OPEN: &str = "```calc";
const FENCE_CLOSE: &str = "```";
const KEY_SEPARATOR: &str = ": ";
const KEY_TERMINATOR: char = ':';

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Field {
    pub(crate) key: String,
    pub(crate) value: String,
    pub(crate) line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Block {
    pub(crate) text: String,
    pub(crate) line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Section {
    pub(crate) kind: String,
    pub(crate) name: String,
    pub(crate) line: usize,
    pub(crate) fields: Vec<Field>,
    pub(crate) blocks: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Document {
    pub(crate) path: String,
    pub(crate) title: String,
    pub(crate) fields: Vec<Field>,
    pub(crate) sections: Vec<Section>,
}

impl Document {
    pub(crate) fn error(&self, line: usize, kind: LoadErrorKind) -> LoadError {
        LoadError {
            file: self.path.clone(),
            line,
            kind,
        }
    }
}

fn split_field(line: &str) -> Option<(&str, &str)> {
    if let Some((key, value)) = line.split_once(KEY_SEPARATOR) {
        return Some((key, value));
    }
    line.strip_suffix(KEY_TERMINATOR).map(|key| (key, ""))
}

const NOTATION_SECTION: &str = "Notation";
const MODE_SOURCE_PREFIX: &str = "Source ";

fn is_key(key: &str) -> bool {
    key.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && key.chars().all(|c| c.is_ascii_alphabetic() || c == ' ')
}

fn is_notation_key(key: &str) -> bool {
    let mode = key.strip_prefix(MODE_SOURCE_PREFIX).unwrap_or(key);
    NotationMode::from_name(mode).is_some()
}

pub(crate) fn parse_document(path: &str, bytes: &[u8]) -> Result<Document, LoadError> {
    let error = |line: usize, kind: LoadErrorKind| LoadError {
        file: path.to_string(),
        line,
        kind,
    };
    let text = std::str::from_utf8(bytes).map_err(|_| error(0, LoadErrorKind::NotUtf8))?;
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line));
    let title = match lines.next() {
        Some((_, line)) => line
            .strip_prefix(TITLE_PREFIX)
            .filter(|title| !title.trim().is_empty())
            .ok_or_else(|| error(1, LoadErrorKind::MissingTitle))?,
        None => return Err(error(1, LoadErrorKind::MissingTitle)),
    };
    let mut document = Document {
        path: path.to_string(),
        title: title.to_string(),
        fields: Vec::new(),
        sections: Vec::new(),
    };
    while let Some((number, line)) = lines.next() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(heading) = line.strip_prefix(SECTION_PREFIX) {
            let (kind, name) = heading
                .split_once(' ')
                .filter(|(kind, name)| !kind.is_empty() && !name.trim().is_empty())
                .ok_or_else(|| error(number, LoadErrorKind::MalformedSection))?;
            if document
                .sections
                .iter()
                .any(|section| section.kind == kind && section.name == name)
            {
                return Err(error(number, LoadErrorKind::DuplicateSection));
            }
            document.sections.push(Section {
                kind: kind.to_string(),
                name: name.to_string(),
                line: number,
                fields: Vec::new(),
                blocks: Vec::new(),
            });
            continue;
        }
        if line == FENCE_OPEN {
            let mut block = Vec::new();
            let mut closed = false;
            for (_, inner) in lines.by_ref() {
                if inner == FENCE_CLOSE {
                    closed = true;
                    break;
                }
                block.push(inner);
            }
            if !closed {
                return Err(error(number, LoadErrorKind::UnclosedBlock));
            }
            let section = document
                .sections
                .last_mut()
                .ok_or_else(|| error(number, LoadErrorKind::BlockOutsideSection))?;
            section.blocks.push(Block {
                text: block.join("\n"),
                line: number,
            });
            continue;
        }
        match split_field(line) {
            Some((key, value))
                if is_key(key)
                    || (document
                        .sections
                        .last()
                        .is_some_and(|section| section.kind == NOTATION_SECTION)
                        && is_notation_key(key)) =>
            {
                let fields = match document.sections.last_mut() {
                    Some(section) => &mut section.fields,
                    None => &mut document.fields,
                };
                if fields.iter().any(|field| field.key == key) {
                    return Err(error(number, LoadErrorKind::DuplicateKey(key.to_string())));
                }
                fields.push(Field {
                    key: key.to_string(),
                    value: value.trim().to_string(),
                    line: number,
                });
            }
            _ => return Err(error(number, LoadErrorKind::UnexpectedText)),
        }
    }
    Ok(document)
}

pub(crate) struct FieldReader<'document> {
    document: &'document Document,
    fields: &'document [Field],
    consumed: Vec<bool>,
    line: usize,
}

impl<'document> FieldReader<'document> {
    pub(crate) fn new(
        document: &'document Document,
        fields: &'document [Field],
        line: usize,
    ) -> Self {
        Self {
            document,
            fields,
            consumed: vec![false; fields.len()],
            line,
        }
    }

    fn take(&mut self, key: &str) -> Option<&'document Field> {
        let position = self.fields.iter().position(|field| field.key == key)?;
        if let Some(mark) = self.consumed.get_mut(position) {
            *mark = true;
        }
        self.fields.get(position)
    }

    pub(crate) fn optional(&mut self, key: &str) -> Option<&'document Field> {
        self.take(key)
    }

    pub(crate) fn required(&mut self, key: &str) -> Result<&'document Field, LoadError> {
        let line = self.line;
        self.take(key).ok_or_else(|| {
            self.document
                .error(line, LoadErrorKind::MissingKey(key.to_string()))
        })
    }

    pub(crate) fn required_value(&mut self, key: &str) -> Result<&'document Field, LoadError> {
        let field = self.required(key)?;
        if field.value.is_empty() {
            return Err(self
                .document
                .error(field.line, LoadErrorKind::EmptyValue(key.to_string())));
        }
        Ok(field)
    }

    pub(crate) fn finish(self) -> Result<(), LoadError> {
        match self
            .fields
            .iter()
            .zip(&self.consumed)
            .find(|(_, consumed)| !**consumed)
        {
            Some((field, _)) => Err(self
                .document
                .error(field.line, LoadErrorKind::UnknownKey(field.key.clone()))),
            None => Ok(()),
        }
    }
}

pub(crate) fn list(value: &str) -> Vec<String> {
    if value.is_empty() {
        return Vec::new();
    }
    value.split(", ").map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# circle\n\nLevel: isced-1\nPrerequisites:\n\n## Way from-radius\nOutput: circle.circumference\n```calc\nr |-> 2 * pi * r\n```\n";

    #[test]
    fn document_keeps_title_fields_sections_and_blocks() {
        let document = parse_document("concepts/circle/node.md", SAMPLE.as_bytes()).unwrap();
        assert_eq!(document.title, "circle");
        assert_eq!(document.fields.len(), 2);
        assert_eq!(document.sections[0].kind, "Way");
        assert_eq!(document.sections[0].name, "from-radius");
        assert_eq!(document.sections[0].blocks[0].text, "r |-> 2 * pi * r");
    }

    #[test]
    fn key_without_value_has_an_empty_value() {
        let document = parse_document("a.md", SAMPLE.as_bytes()).unwrap();
        assert_eq!(document.fields[1].value, "");
    }

    #[test]
    fn missing_title_is_an_error() {
        let error = parse_document("a.md", b"Level: x\n").unwrap_err();
        assert_eq!(error.kind, LoadErrorKind::MissingTitle);
    }

    #[test]
    fn text_outside_the_form_is_an_error_with_its_line() {
        let error = parse_document("a.md", b"# a\n\nsome prose\n").unwrap_err();
        assert_eq!((error.kind, error.line), (LoadErrorKind::UnexpectedText, 3));
    }

    #[test]
    fn duplicate_key_is_an_error() {
        let error = parse_document("a.md", b"# a\nLevel: x\nLevel: y\n").unwrap_err();
        assert_eq!(error.kind, LoadErrorKind::DuplicateKey("Level".to_string()));
    }

    #[test]
    fn duplicate_section_is_an_error() {
        let error = parse_document("a.md", b"# a\n## Way w\n## Way w\n").unwrap_err();
        assert_eq!(error.kind, LoadErrorKind::DuplicateSection);
    }

    #[test]
    fn unclosed_block_is_an_error() {
        let error = parse_document("a.md", b"# a\n## Way w\n```calc\nx\n").unwrap_err();
        assert_eq!(error.kind, LoadErrorKind::UnclosedBlock);
    }

    #[test]
    fn block_before_any_section_is_an_error() {
        let error = parse_document("a.md", b"# a\n```calc\nx\n```\n").unwrap_err();
        assert_eq!(error.kind, LoadErrorKind::BlockOutsideSection);
    }

    #[test]
    fn unconsumed_key_is_reported_as_unknown() {
        let document = parse_document("a.md", b"# a\nColour: red\n").unwrap();
        let reader = FieldReader::new(&document, &document.fields, 1);
        assert_eq!(
            reader.finish().unwrap_err().kind,
            LoadErrorKind::UnknownKey("Colour".to_string())
        );
    }

    #[test]
    fn mode_keys_are_read_inside_a_notation_section() {
        let document = parse_document(
            "a.md",
            b"# a\n## Notation n 1\nrecurring_mark: bar\nSource recurring_mark: book\n",
        )
        .unwrap();
        assert_eq!(document.sections[0].fields.len(), 2);
    }

    #[test]
    fn mode_keys_outside_a_notation_section_are_text_outside_the_form() {
        let error = parse_document("a.md", b"# a\n## Group g\nrecurring_mark: bar\n").unwrap_err();
        assert_eq!(error.kind, LoadErrorKind::UnexpectedText);
    }

    #[test]
    fn list_splits_on_comma_and_space() {
        assert_eq!(list("a, b"), vec!["a".to_string(), "b".to_string()]);
        assert!(list("").is_empty());
    }
}
