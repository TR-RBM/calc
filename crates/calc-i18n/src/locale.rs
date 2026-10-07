use std::marker::PhantomData;

use crate::catalog_message::CatalogMessage;
use crate::language_tag::LanguageTag;
use crate::message::Message;
use crate::pattern::LocaleEntry;

#[derive(Debug)]
pub struct CatalogLocale<M> {
    index: usize,
    catalog: PhantomData<fn() -> M>,
}

impl<M> Clone for CatalogLocale<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M> Copy for CatalogLocale<M> {}

impl<M> PartialEq for CatalogLocale<M> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<M> Eq for CatalogLocale<M> {}

pub type Locale = CatalogLocale<Message>;

#[derive(Clone, Copy, Debug)]
pub enum LocaleSource<'a> {
    Tag(&'a LanguageTag),
    Posix(&'a str),
}

impl<M: CatalogMessage> CatalogLocale<M> {
    fn at(index: usize) -> Self {
        Self {
            index,
            catalog: PhantomData,
        }
    }

    pub fn source() -> Self {
        Self::at(M::source_locale_index())
    }

    pub fn shipped() -> impl Iterator<Item = Self> {
        (0..M::shipped_locales().len()).map(Self::at)
    }

    pub fn matching(tag: &LanguageTag) -> Self {
        Self::shipped_for(tag).unwrap_or_else(Self::source)
    }

    pub fn is_shipped(tag: &LanguageTag) -> bool {
        Self::shipped_for(tag).is_some()
    }

    fn shipped_for(tag: &LanguageTag) -> Option<Self> {
        Self::find(|shipped| shipped.eq_ignore_ascii_case(tag.as_str())).or_else(|| {
            tag.language()
                .and_then(|language| Self::find(|shipped| shipped.eq_ignore_ascii_case(language)))
        })
    }

    pub fn tag(self) -> &'static str {
        self.entry().tag
    }

    pub(crate) fn entry(self) -> &'static LocaleEntry {
        &M::shipped_locales()[self.index]
    }

    fn find(matches: impl Fn(&str) -> bool) -> Option<Self> {
        M::shipped_locales()
            .iter()
            .position(|entry| matches(entry.tag))
            .map(Self::at)
    }
}

pub fn resolve_locale(sources: &[LocaleSource<'_>]) -> Locale {
    resolve_locale_in::<Message>(sources)
}

pub fn resolve_locale_in<M: CatalogMessage>(sources: &[LocaleSource<'_>]) -> CatalogLocale<M> {
    for source in sources {
        match source {
            LocaleSource::Tag(tag) => return CatalogLocale::matching(tag),
            LocaleSource::Posix(value) if !value.is_empty() => {
                return CatalogLocale::matching(&LanguageTag::from_posix(value));
            }
            LocaleSource::Posix(_) => {}
        }
    }
    CatalogLocale::source()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(text: &str) -> LanguageTag {
        LanguageTag::parse(text).unwrap()
    }

    #[test]
    fn source_locale_is_english() {
        assert_eq!(Locale::source().tag(), "en");
    }

    #[test]
    fn shipped_locales_include_english_and_german() {
        let tags: Vec<&str> = Locale::shipped().map(Locale::tag).collect();
        assert_eq!(tags, vec!["de", "en"]);
    }

    #[test]
    fn exact_tag_matches_shipped_locale() {
        assert_eq!(Locale::matching(&tag("de")).tag(), "de");
    }

    #[test]
    fn tag_with_region_matches_its_language() {
        assert_eq!(Locale::matching(&tag("de-DE")).tag(), "de");
    }

    #[test]
    fn matching_ignores_letter_case() {
        assert_eq!(Locale::matching(&tag("DE")).tag(), "de");
    }

    #[test]
    fn unshipped_language_matches_english() {
        assert_eq!(Locale::matching(&tag("fr-FR")).tag(), "en");
    }

    #[test]
    fn no_source_resolves_to_english() {
        assert_eq!(resolve_locale(&[]).tag(), "en");
    }

    #[test]
    fn tag_source_comes_before_posix_sources() {
        let requested = tag("en");
        let sources = [
            LocaleSource::Tag(&requested),
            LocaleSource::Posix("de_DE.UTF-8"),
        ];
        assert_eq!(resolve_locale(&sources).tag(), "en");
    }

    #[test]
    fn empty_posix_value_is_skipped() {
        let sources = [LocaleSource::Posix(""), LocaleSource::Posix("de_DE.UTF-8")];
        assert_eq!(resolve_locale(&sources).tag(), "de");
    }

    #[test]
    fn first_non_empty_posix_value_decides_even_when_it_maps_to_english() {
        let sources = [LocaleSource::Posix("C"), LocaleSource::Posix("de_DE.UTF-8")];
        assert_eq!(resolve_locale(&sources).tag(), "en");
    }
}
