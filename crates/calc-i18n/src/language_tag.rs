#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageTag {
    text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LanguageTagError {
    NotWellFormed,
}

const SOURCE_LANGUAGE: &str = "en";
const POSIX_DEFAULT_LOCALES: [&str; 2] = ["C", "POSIX"];
const POSIX_DEFAULT_PREFIX: &str = "C.";
const POSIX_CODESET_SEPARATOR: char = '.';
const POSIX_MODIFIER_SEPARATOR: char = '@';
const POSIX_TERRITORY_SEPARATOR: char = '_';
const SUBTAG_SEPARATOR: char = '-';
const SUBTAG_SEPARATOR_TEXT: &str = "-";
const PRIVATE_USE_SINGLETON: &str = "x";
const LANGUAGE_LENGTHS: std::ops::RangeInclusive<usize> = 2..=8;
const SHORT_LANGUAGE_LENGTHS: std::ops::RangeInclusive<usize> = 2..=3;
const EXTLANG_LENGTH: usize = 3;
const MAXIMUM_EXTLANGS: usize = 3;
const SCRIPT_LENGTH: usize = 4;
const ALPHABETIC_REGION_LENGTH: usize = 2;
const NUMERIC_REGION_LENGTH: usize = 3;
const LONG_VARIANT_LENGTHS: std::ops::RangeInclusive<usize> = 5..=8;
const DIGIT_VARIANT_LENGTH: usize = 4;
const EXTENSION_SUBTAG_LENGTHS: std::ops::RangeInclusive<usize> = 2..=8;
const PRIVATE_USE_SUBTAG_LENGTHS: std::ops::RangeInclusive<usize> = 1..=8;
const GRANDFATHERED_TAGS: [&str; 26] = [
    "en-GB-oed",
    "i-ami",
    "i-bnn",
    "i-default",
    "i-enochian",
    "i-hak",
    "i-klingon",
    "i-lux",
    "i-mingo",
    "i-navajo",
    "i-pwn",
    "i-tao",
    "i-tay",
    "i-tsu",
    "sgn-BE-FR",
    "sgn-BE-NL",
    "sgn-CH-DE",
    "art-lojban",
    "cel-gaulish",
    "no-bok",
    "no-nyn",
    "zh-guoyu",
    "zh-hakka",
    "zh-min",
    "zh-min-nan",
    "zh-xiang",
];

impl LanguageTag {
    pub fn parse(text: &str) -> Result<Self, LanguageTagError> {
        if is_well_formed(text) {
            Ok(Self {
                text: text.to_owned(),
            })
        } else {
            Err(LanguageTagError::NotWellFormed)
        }
    }

    pub fn from_posix(value: &str) -> Self {
        if POSIX_DEFAULT_LOCALES.contains(&value) || value.starts_with(POSIX_DEFAULT_PREFIX) {
            return Self::source();
        }
        let without_codeset_and_modifier = value
            .split([POSIX_CODESET_SEPARATOR, POSIX_MODIFIER_SEPARATOR])
            .next()
            .unwrap_or_default();
        let candidate =
            without_codeset_and_modifier.replace(POSIX_TERRITORY_SEPARATOR, SUBTAG_SEPARATOR_TEXT);
        Self::parse(&candidate).unwrap_or_else(|_| Self::source())
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn language(&self) -> Option<&str> {
        if is_grandfathered(&self.text) {
            return None;
        }
        self.text
            .split(SUBTAG_SEPARATOR)
            .next()
            .filter(|language| !language.eq_ignore_ascii_case(PRIVATE_USE_SINGLETON))
    }

    fn source() -> Self {
        Self {
            text: SOURCE_LANGUAGE.to_owned(),
        }
    }
}

fn is_grandfathered(text: &str) -> bool {
    GRANDFATHERED_TAGS
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case(text))
}

fn is_well_formed(text: &str) -> bool {
    if is_grandfathered(text) {
        return true;
    }
    let subtags: Vec<&str> = text.split(SUBTAG_SEPARATOR).collect();
    if subtags
        .iter()
        .any(|subtag| subtag.is_empty() || !subtag.chars().all(|c| c.is_ascii_alphanumeric()))
    {
        return false;
    }
    let mut rest = subtags.as_slice();
    if let Some((first, tail)) = rest.split_first()
        && first.eq_ignore_ascii_case(PRIVATE_USE_SINGLETON)
    {
        return is_private_use_tail(tail);
    }
    rest = match rest.split_first() {
        Some((language, tail))
            if is_alphabetic(language) && LANGUAGE_LENGTHS.contains(&language.len()) =>
        {
            if SHORT_LANGUAGE_LENGTHS.contains(&language.len()) {
                skip_extlangs(tail)
            } else {
                tail
            }
        }
        _ => return false,
    };
    rest = skip_first_if(rest, |subtag| {
        subtag.len() == SCRIPT_LENGTH && is_alphabetic(subtag)
    });
    rest = skip_first_if(rest, is_region);
    while let Some((subtag, tail)) = rest.split_first()
        && is_variant(subtag)
    {
        rest = tail;
    }
    while let Some((singleton, tail)) = rest.split_first()
        && singleton.len() == 1
        && !singleton.eq_ignore_ascii_case(PRIVATE_USE_SINGLETON)
    {
        let extension_length = tail
            .iter()
            .take_while(|subtag| EXTENSION_SUBTAG_LENGTHS.contains(&subtag.len()))
            .count();
        if extension_length == 0 {
            return false;
        }
        rest = tail.get(extension_length..).unwrap_or_default();
    }
    match rest.split_first() {
        None => true,
        Some((singleton, tail)) if singleton.eq_ignore_ascii_case(PRIVATE_USE_SINGLETON) => {
            is_private_use_tail(tail)
        }
        Some(_) => false,
    }
}

fn skip_extlangs<'a, 'b>(subtags: &'a [&'b str]) -> &'a [&'b str] {
    let extlang_count = subtags
        .iter()
        .take(MAXIMUM_EXTLANGS)
        .take_while(|subtag| subtag.len() == EXTLANG_LENGTH && is_alphabetic(subtag))
        .count();
    subtags.get(extlang_count..).unwrap_or_default()
}

fn skip_first_if<'a, 'b>(subtags: &'a [&'b str], matches: impl Fn(&str) -> bool) -> &'a [&'b str] {
    match subtags.split_first() {
        Some((first, tail)) if matches(first) => tail,
        _ => subtags,
    }
}

fn is_private_use_tail(subtags: &[&str]) -> bool {
    !subtags.is_empty()
        && subtags
            .iter()
            .all(|subtag| PRIVATE_USE_SUBTAG_LENGTHS.contains(&subtag.len()))
}

fn is_alphabetic(subtag: &str) -> bool {
    subtag.chars().all(|c| c.is_ascii_alphabetic())
}

fn is_region(subtag: &str) -> bool {
    (subtag.len() == ALPHABETIC_REGION_LENGTH && is_alphabetic(subtag))
        || (subtag.len() == NUMERIC_REGION_LENGTH && subtag.chars().all(|c| c.is_ascii_digit()))
}

fn is_variant(subtag: &str) -> bool {
    LONG_VARIANT_LENGTHS.contains(&subtag.len())
        || (subtag.len() == DIGIT_VARIANT_LENGTH
            && subtag.chars().next().is_some_and(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_accepted(text: &str) -> bool {
        LanguageTag::parse(text).is_ok()
    }

    #[test]
    fn language_alone_is_well_formed() {
        assert!(is_accepted("en"));
    }

    #[test]
    fn language_with_region_is_well_formed() {
        assert!(is_accepted("de-DE"));
    }

    #[test]
    fn language_with_script_and_region_is_well_formed() {
        assert!(is_accepted("zh-Hant-TW"));
    }

    #[test]
    fn language_with_extlang_is_well_formed() {
        assert!(is_accepted("zh-yue"));
    }

    #[test]
    fn language_with_numeric_region_is_well_formed() {
        assert!(is_accepted("es-419"));
    }

    #[test]
    fn language_with_variants_is_well_formed() {
        assert!(is_accepted("sl-rozaj-biske"));
    }

    #[test]
    fn language_with_digit_variant_is_well_formed() {
        assert!(is_accepted("de-1996"));
    }

    #[test]
    fn language_with_extension_and_private_use_is_well_formed() {
        assert!(is_accepted("en-a-bbb-x-a-ccc"));
    }

    #[test]
    fn private_use_tag_is_well_formed() {
        assert!(is_accepted("x-whatever"));
    }

    #[test]
    fn grandfathered_tag_is_well_formed() {
        assert!(is_accepted("i-klingon"));
    }

    #[test]
    fn empty_text_is_not_well_formed() {
        assert_eq!(LanguageTag::parse(""), Err(LanguageTagError::NotWellFormed));
    }

    #[test]
    fn single_letter_language_is_not_well_formed() {
        assert!(!is_accepted("e"));
    }

    #[test]
    fn underscore_separator_is_not_well_formed() {
        assert!(!is_accepted("de_DE"));
    }

    #[test]
    fn empty_subtag_is_not_well_formed() {
        assert!(!is_accepted("de--DE"));
    }

    #[test]
    fn singleton_without_extension_subtag_is_not_well_formed() {
        assert!(!is_accepted("en-a"));
    }

    #[test]
    fn private_use_without_subtag_is_not_well_formed() {
        assert!(!is_accepted("en-x"));
    }

    #[test]
    fn language_longer_than_eight_letters_is_not_well_formed() {
        assert!(!is_accepted("toolonglanguage"));
    }

    #[test]
    fn punctuation_in_subtag_is_not_well_formed() {
        assert!(!is_accepted("de-DE!"));
    }

    #[test]
    fn posix_c_maps_to_english() {
        assert_eq!(LanguageTag::from_posix("C").as_str(), "en");
    }

    #[test]
    fn posix_posix_maps_to_english() {
        assert_eq!(LanguageTag::from_posix("POSIX").as_str(), "en");
    }

    #[test]
    fn posix_c_with_codeset_maps_to_english() {
        assert_eq!(LanguageTag::from_posix("C.UTF-8").as_str(), "en");
    }

    #[test]
    fn posix_codeset_is_removed() {
        assert_eq!(LanguageTag::from_posix("de_DE.UTF-8").as_str(), "de-DE");
    }

    #[test]
    fn posix_modifier_is_removed() {
        assert_eq!(LanguageTag::from_posix("de_AT@euro").as_str(), "de-AT");
    }

    #[test]
    fn malformed_posix_value_maps_to_english() {
        assert_eq!(LanguageTag::from_posix("toolonglanguage").as_str(), "en");
    }

    #[test]
    fn language_is_the_first_subtag() {
        let tag = LanguageTag::parse("de-AT").unwrap();
        assert_eq!(tag.language(), Some("de"));
    }

    #[test]
    fn private_use_tag_has_no_language() {
        let tag = LanguageTag::parse("x-whatever").unwrap();
        assert_eq!(tag.language(), None);
    }

    #[test]
    fn grandfathered_tag_has_no_language() {
        let tag = LanguageTag::parse("i-klingon").unwrap();
        assert_eq!(tag.language(), None);
    }
}
