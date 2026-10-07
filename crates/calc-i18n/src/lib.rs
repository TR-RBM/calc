mod catalog_message;
mod language_tag;
mod locale;
mod localized;
mod message;
pub mod pattern;
pub mod plural;
mod render;

#[cfg(test)]
#[allow(dead_code)]
mod catalog;
#[cfg(test)]
mod literal_scan;

pub use catalog_message::CatalogMessage;
pub use language_tag::{LanguageTag, LanguageTagError};
pub use locale::{CatalogLocale, Locale, LocaleSource, resolve_locale, resolve_locale_in};
pub use localized::Localized;
pub use message::Message;
pub use render::render;
