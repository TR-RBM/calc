use crate::pattern::{Argument, LocaleEntry, Pattern};

pub trait CatalogMessage {
    fn index(&self) -> usize;
    fn argument(&self, slot: usize) -> Option<Argument<'_>>;
    fn source_patterns() -> &'static [Pattern];
    fn shipped_locales() -> &'static [LocaleEntry];
    fn source_locale_index() -> usize;
}
