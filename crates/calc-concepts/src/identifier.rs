const WAY_SEPARATOR: char = '/';
const ROLE_SEPARATOR: char = '.';
const PERMUTATION_SUFFIX: char = '~';

pub fn is_identifier(text: &str) -> bool {
    !text.is_empty()
        && text.split('-').all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
        && text.chars().next().is_some_and(|c| c.is_ascii_lowercase())
}

pub fn is_role_identifier(text: &str) -> bool {
    text.split_once(ROLE_SEPARATOR)
        .is_some_and(|(object, role)| is_identifier(object) && is_identifier(role))
}

pub(crate) fn role_object(role: &str) -> Option<&str> {
    role.split_once(ROLE_SEPARATOR).map(|(object, _)| object)
}

pub(crate) fn role_name(role: &str) -> Option<&str> {
    role.split_once(ROLE_SEPARATOR).map(|(_, name)| name)
}

pub(crate) fn role_identifier(object: &str, role: &str) -> String {
    format!("{object}{ROLE_SEPARATOR}{role}")
}

pub(crate) fn way_identifier(concept: &str, name: &str) -> String {
    format!("{concept}{WAY_SEPARATOR}{name}")
}

pub(crate) fn permuted_identifier(way: &str, index: usize) -> String {
    format!("{way}{PERMUTATION_SUFFIX}{index}")
}

pub fn is_corpus_id(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    match parts.as_slice() {
        [area, domain, kind, number] => {
            area.len() == 1
                && area.chars().all(|c| c.is_ascii_uppercase())
                && domain.len() == 3
                && domain.chars().all(|c| c.is_ascii_uppercase())
                && kind.len() == 1
                && kind.chars().all(|c| c.is_ascii_uppercase())
                && number.len() == 3
                && number.chars().all(|c| c.is_ascii_digit())
        }
        _ => false,
    }
}

pub(crate) fn is_locale(text: &str) -> bool {
    let (language, region) = match text.split_once('-') {
        Some((language, region)) => (language, Some(region)),
        None => (text, None),
    };
    (2..=3).contains(&language.len())
        && language.chars().all(|c| c.is_ascii_lowercase())
        && region.is_none_or(|region| {
            region.len() == 2 && region.chars().all(|c| c.is_ascii_uppercase())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyphenated_lowercase_words_are_identifiers() {
        assert!(is_identifier("circle-circumference"));
        assert!(is_identifier("isced-2"));
    }

    #[test]
    fn capitals_underscores_and_empty_segments_are_not_identifiers() {
        assert!(!is_identifier("Circle"));
        assert!(!is_identifier("triangle_area"));
        assert!(!is_identifier("a--b"));
        assert!(!is_identifier("2d"));
    }

    #[test]
    fn role_identifier_is_object_dot_role() {
        assert!(is_role_identifier("triangle.side-a"));
        assert!(!is_role_identifier("side-a"));
    }

    #[test]
    fn corpus_id_has_the_block_form() {
        assert!(is_corpus_id("M-GEO-S-030"));
        assert!(!is_corpus_id("M-GEO-S-30"));
    }

    #[test]
    fn locale_is_a_language_with_an_optional_region() {
        assert!(is_locale("en"));
        assert!(is_locale("de-CH"));
        assert!(!is_locale("english"));
    }
}
