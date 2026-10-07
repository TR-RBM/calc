use std::borrow::Borrow;

use crate::catalog_message::CatalogMessage;
use crate::locale::CatalogLocale;
use crate::localized::Localized;
use crate::pattern::{Argument, Inline, LocaleEntry, Pattern, Segment, Select, VariantKey};
use crate::plural::PluralRule;

pub fn render<M, R>(message: &R, locale: &CatalogLocale<M>) -> Localized
where
    M: CatalogMessage,
    R: Borrow<M> + ?Sized,
{
    let message = message.borrow();
    let (pattern, plural_rule) = pattern_with_fallback(locale.entry(), message);
    Localized::new(render_pattern(pattern, message, plural_rule))
}

fn pattern_with_fallback<M: CatalogMessage>(
    entry: &LocaleEntry,
    message: &M,
) -> (Pattern, PluralRule) {
    let index = message.index();
    match entry.patterns.get(index).copied().flatten() {
        Some(pattern) => (pattern, entry.plural_rule),
        None => (
            M::source_patterns()[index],
            CatalogLocale::<M>::source().entry().plural_rule,
        ),
    }
}

fn render_pattern<M: CatalogMessage>(
    pattern: Pattern,
    message: &M,
    plural_rule: PluralRule,
) -> String {
    let mut text = String::new();
    for segment in pattern {
        match segment {
            Segment::Text(literal) => text.push_str(literal),
            Segment::Argument(slot) => push_argument(&mut text, message.argument(*slot)),
            Segment::Select(select) => {
                let chosen = chosen_variant(select, message.argument(select.selector), plural_rule);
                for element in chosen {
                    match element {
                        Inline::Text(literal) => text.push_str(literal),
                        Inline::Argument(slot) => push_argument(&mut text, message.argument(*slot)),
                    }
                }
            }
        }
    }
    text
}

fn push_argument(text: &mut String, argument: Option<Argument<'_>>) {
    match argument {
        Some(Argument::Text(value)) => text.push_str(value),
        Some(Argument::Count(count)) => text.push_str(&count.to_string()),
        None => {}
    }
}

fn chosen_variant(
    select: &Select,
    argument: Option<Argument<'_>>,
    plural_rule: PluralRule,
) -> &'static [Inline] {
    let Some(Argument::Count(count)) = argument else {
        return select.default;
    };
    let category = plural_rule.category(count);
    select
        .variants
        .iter()
        .find(|variant| variant.key == VariantKey::Exact(count))
        .or_else(|| {
            select
                .variants
                .iter()
                .find(|variant| variant.key == VariantKey::Category(category))
        })
        .map_or(select.default, |variant| variant.elements)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language_tag::LanguageTag;
    use crate::locale::Locale;
    use crate::message::Message;
    use crate::pattern::Variant;
    use crate::plural::PluralCategory;

    fn german() -> Locale {
        Locale::matching(&LanguageTag::parse("de").unwrap())
    }

    #[test]
    fn plain_message_renders_in_english() {
        let text = render(&Message::CommonKindExact, &Locale::source());
        assert_eq!(text.as_str(), "exact");
    }

    #[test]
    fn plain_message_renders_in_german() {
        let text = render(&Message::CommonKindExact, &german());
        assert_eq!(text.as_str(), "exakt");
    }

    #[test]
    fn text_placeholder_is_filled() {
        let message = Message::ErrorUndefinedName {
            name: "x".to_owned(),
        };
        let text = render(&message, &Locale::source());
        assert_eq!(text.as_str(), "x is not defined");
    }

    #[test]
    fn count_of_one_selects_the_one_variant() {
        let message = Message::CommonStatusResults { count: 1 };
        let text = render(&message, &Locale::source());
        assert_eq!(text.as_str(), "1 result");
    }

    #[test]
    fn count_of_zero_selects_the_other_variant() {
        let message = Message::CommonStatusResults { count: 0 };
        let text = render(&message, &Locale::source());
        assert_eq!(text.as_str(), "0 results");
    }

    #[test]
    fn count_selects_german_plural_variant() {
        let message = Message::CommonStatusResults { count: 2 };
        let text = render(&message, &german());
        assert_eq!(text.as_str(), "2 Ergebnisse");
    }

    #[test]
    fn exact_variant_key_wins_over_plural_category() {
        const PATTERN: Pattern = &[Segment::Select(Select {
            selector: 0,
            variants: &[
                Variant {
                    key: VariantKey::Category(PluralCategory::One),
                    elements: &[Inline::Text("category")],
                },
                Variant {
                    key: VariantKey::Exact(1),
                    elements: &[Inline::Text("exact")],
                },
            ],
            default: &[Inline::Text("default")],
        })];
        let message = Message::CommonStatusResults { count: 1 };
        let text = render_pattern(PATTERN, &message, PluralRule::OneOther);
        assert_eq!(text, "exact");
    }

    #[test]
    fn unmatched_count_selects_the_default_variant() {
        const PATTERN: Pattern = &[Segment::Select(Select {
            selector: 0,
            variants: &[Variant {
                key: VariantKey::Exact(5),
                elements: &[Inline::Text("five")],
            }],
            default: &[Inline::Text("default")],
        })];
        let message = Message::CommonStatusResults { count: 3 };
        let text = render_pattern(PATTERN, &message, PluralRule::OneOther);
        assert_eq!(text, "default");
    }

    #[test]
    fn text_argument_as_selector_selects_the_default_variant() {
        const PATTERN: Pattern = &[Segment::Select(Select {
            selector: 0,
            variants: &[Variant {
                key: VariantKey::Category(PluralCategory::Other),
                elements: &[Inline::Text("other")],
            }],
            default: &[Inline::Text("default")],
        })];
        let message = Message::ErrorUndefinedName {
            name: "x".to_owned(),
        };
        let text = render_pattern(PATTERN, &message, PluralRule::OneOther);
        assert_eq!(text, "default");
    }

    #[test]
    fn missing_translation_falls_back_to_the_source_pattern() {
        let entry = LocaleEntry {
            tag: "xx",
            plural_rule: PluralRule::OneOther,
            patterns: &[],
        };
        let message = Message::CommonKindExact;
        let (pattern, plural_rule) = pattern_with_fallback(&entry, &message);
        assert_eq!(render_pattern(pattern, &message, plural_rule), "exact");
    }

    #[test]
    fn missing_argument_slot_renders_nothing() {
        const PATTERN: Pattern = &[Segment::Text("a"), Segment::Argument(7)];
        let text = render_pattern(PATTERN, &Message::CommonKindExact, PluralRule::OneOther);
        assert_eq!(text, "a");
    }

    #[test]
    fn multiline_message_keeps_its_relative_indentation() {
        let text = render(&Message::CliHelpUsage, &Locale::source());
        assert!(
            text.as_str()
                .starts_with("Usage:\n  calc <expression> | <file.calc>")
        );
    }

    #[test]
    fn localized_displays_its_text() {
        let text = render(&Message::CommonKindExact, &Locale::source());
        assert_eq!(text.to_string(), "exact");
    }
}
