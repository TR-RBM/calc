use calc_i18n::{Locale, Message, render};

use crate::application_place::{Pointer, search_answers};
use crate::language_reference::localized_language_reference;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchEntry {
    Precision,
    Backend,
    Language,
    Theme,
    TextSize,
    Units,
    GoToInputArea,
    GoToStack,
    GoToInstrumentPanel,
    GoToSessionBar,
    CancelRunningLine,
    AxisUnit,
    LanguageReference,
}

impl SearchEntry {
    pub const ALL: [Self; 13] = [
        Self::Precision,
        Self::Backend,
        Self::Language,
        Self::Theme,
        Self::TextSize,
        Self::Units,
        Self::GoToInputArea,
        Self::GoToStack,
        Self::GoToInstrumentPanel,
        Self::GoToSessionBar,
        Self::CancelRunningLine,
        Self::AxisUnit,
        Self::LanguageReference,
    ];

    pub const fn name(self) -> Message {
        match self {
            Self::Precision => Message::UiSearchPrecision,
            Self::Backend => Message::UiSearchBackend,
            Self::Language => Message::UiSearchLanguage,
            Self::Theme => Message::UiSearchTheme,
            Self::TextSize => Message::UiSearchTextSize,
            Self::Units => Message::UiSearchUnits,
            Self::GoToInputArea => Message::UiSearchGoToInputArea,
            Self::GoToStack => Message::UiSearchGoToStack,
            Self::GoToInstrumentPanel => Message::UiSearchGoToInstrumentPanel,
            Self::GoToSessionBar => Message::UiSearchGoToSessionBar,
            Self::CancelRunningLine => Message::UiSearchCancelRunningLine,
            Self::AxisUnit => Message::UiSearchAxisUnit,
            Self::LanguageReference => Message::UiSearchLanguageReference,
        }
    }

    pub const fn words(self) -> Message {
        match self {
            Self::Precision => Message::UiSearchPrecisionWords,
            Self::Backend => Message::UiSearchBackendWords,
            Self::Language => Message::UiSearchLanguageWords,
            Self::Theme => Message::UiSearchThemeWords,
            Self::TextSize => Message::UiSearchTextSizeWords,
            Self::Units => Message::UiSearchUnitsWords,
            Self::GoToInputArea => Message::UiSearchGoToInputAreaWords,
            Self::GoToStack => Message::UiSearchGoToStackWords,
            Self::GoToInstrumentPanel => Message::UiSearchGoToInstrumentPanelWords,
            Self::GoToSessionBar => Message::UiSearchGoToSessionBarWords,
            Self::CancelRunningLine => Message::UiSearchCancelRunningLineWords,
            Self::AxisUnit => Message::UiSearchAxisUnitWords,
            Self::LanguageReference => Message::UiSearchLanguageReferenceWords,
        }
    }

    const fn reachable(self, context: &SearchContext) -> bool {
        match self {
            Self::Precision | Self::Backend => context.session_current,
            Self::CancelRunningLine => context.line_running,
            Self::AxisUnit => context.picture_axis_units,
            Self::GoToInstrumentPanel => context.instrument_panel_shown,
            Self::Language
            | Self::Theme
            | Self::TextSize
            | Self::Units
            | Self::GoToInputArea
            | Self::GoToStack
            | Self::GoToSessionBar
            | Self::LanguageReference => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchContext {
    pub session_current: bool,
    pub line_running: bool,
    pub picture_axis_units: bool,
    pub instrument_panel_shown: bool,
    pub exam: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchResult {
    pub hits: Vec<SearchEntry>,
    pub answers: Vec<Pointer>,
    pub constructs: Vec<ConstructHit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstructHit {
    pub construct: String,
    pub name: String,
}

pub fn search(query: &str, context: &SearchContext, locale: &Locale) -> SearchResult {
    if context.exam {
        return SearchResult::default();
    }
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return SearchResult::default();
    }
    let hits = SearchEntry::ALL
        .into_iter()
        .filter(|entry| entry.reachable(context))
        .filter(|entry| matches(&needle, entry.name(), entry.words(), locale))
        .collect();
    let answers = search_answers()
        .into_iter()
        .filter(|pointer| matches(&needle, pointer.name(), pointer.words(), locale))
        .collect();
    let constructs = matching_constructs(&needle, locale);
    SearchResult {
        hits,
        answers,
        constructs,
    }
}

fn matching_constructs(needle: &str, locale: &Locale) -> Vec<ConstructHit> {
    localized_language_reference()
        .sections
        .into_iter()
        .flat_map(|section| section.entries)
        .filter_map(|(entry, words)| {
            let words = words?;
            let name = render(&words.name, locale).to_string();
            let spelled = entry
                .spellings
                .iter()
                .any(|spelling| spelling.symbol.to_lowercase().contains(needle));
            (spelled || matches(needle, words.name, words.meaning, locale)).then(|| ConstructHit {
                construct: entry.construct.name.to_owned(),
                name,
            })
        })
        .collect()
}

fn matches(needle: &str, name: Message, words: Message, locale: &Locale) -> bool {
    let name = render(&name, locale).to_string().to_lowercase();
    let words = render(&words, locale).to_string().to_lowercase();
    name.contains(needle) || words.contains(needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locale() -> Locale {
        Locale::source()
    }

    fn everything() -> SearchContext {
        SearchContext {
            session_current: true,
            line_running: true,
            picture_axis_units: true,
            instrument_panel_shown: true,
            exam: false,
        }
    }

    #[test]
    fn a_spelling_finds_its_construct() {
        let result = search("\u{3a3}", &everything(), &locale());

        assert!(result.constructs.iter().any(|hit| hit.construct == "sum"));
    }

    #[test]
    fn an_everyday_word_finds_its_construct() {
        let result = search("sum", &everything(), &locale());

        assert!(result.constructs.iter().any(|hit| hit.construct == "sum"));
    }

    #[test]
    fn a_construct_hit_carries_the_name_in_the_locale() {
        let german = Locale::matching(&calc_i18n::LanguageTag::parse("de").expect("a tag"));

        let result = search("\u{3a3}", &everything(), &german);

        let hit = result
            .constructs
            .iter()
            .find(|hit| hit.construct == "sum")
            .expect("the sum binder");
        assert_eq!(hit.name, "Summe");
    }

    #[test]
    fn no_construct_is_found_during_an_exam() {
        let context = SearchContext {
            exam: true,
            ..everything()
        };

        assert!(search("sum", &context, &locale()).constructs.is_empty());
    }

    #[test]
    fn a_word_of_an_entry_finds_it() {
        let result = search("theme", &everything(), &locale());

        assert_eq!(result.hits, vec![SearchEntry::Theme]);
    }

    #[test]
    fn an_everyday_word_finds_the_entry_it_describes() {
        let result = search("dark", &everything(), &locale());

        assert_eq!(result.hits, vec![SearchEntry::Theme]);
    }

    #[test]
    fn a_search_for_a_thing_that_does_not_exist_answers() {
        let result = search("angle mode", &everything(), &locale());

        assert!(result.hits.is_empty());
        assert_eq!(result.answers, vec![Pointer::DegreesOrRadians]);
    }

    #[test]
    fn a_session_setting_is_listed_only_while_a_session_is_current() {
        let context = SearchContext {
            session_current: false,
            ..everything()
        };

        let result = search("precision", &context, &locale());

        assert!(result.hits.is_empty());
    }

    #[test]
    fn cancelling_is_listed_only_while_a_line_runs() {
        let context = SearchContext {
            line_running: false,
            ..everything()
        };

        let result = search("cancel", &context, &locale());

        assert!(result.hits.is_empty());
    }

    #[test]
    fn the_axis_unit_is_listed_only_where_a_picture_offers_one() {
        let context = SearchContext {
            picture_axis_units: false,
            ..everything()
        };

        let result = search("axis", &context, &locale());

        assert!(result.hits.is_empty());
    }

    #[test]
    fn the_panel_is_listed_only_where_it_is_shown() {
        let context = SearchContext {
            instrument_panel_shown: false,
            ..everything()
        };

        let result = search("panel", &context, &locale());

        assert!(result.hits.is_empty());
    }

    #[test]
    fn nothing_is_searched_during_an_exam() {
        let context = SearchContext {
            exam: true,
            ..everything()
        };

        let result = search("theme", &context, &locale());

        assert_eq!(result, SearchResult::default());
    }

    #[test]
    fn an_empty_query_finds_nothing() {
        let result = search("   ", &everything(), &locale());

        assert_eq!(result, SearchResult::default());
    }

    #[test]
    fn the_zones_of_a41_are_reachable_by_their_own_entries() {
        let result = search("go to", &everything(), &locale());

        assert_eq!(
            result.hits,
            vec![
                SearchEntry::GoToInputArea,
                SearchEntry::GoToStack,
                SearchEntry::GoToInstrumentPanel,
                SearchEntry::GoToSessionBar,
            ]
        );
    }
}
