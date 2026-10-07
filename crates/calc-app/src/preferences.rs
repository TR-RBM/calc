use std::collections::{BTreeMap, VecDeque};
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use calc_concepts::{QuantityKind, is_identifier};
use calc_i18n::LanguageTag;

use crate::display_units::UnitsChoice;
use crate::json::{self, Json};
use crate::platform::{Storage, StorageError, StorageLocation};

const FORMAT_NAME: &str = "calc-preferences";
const FORMAT_VERSION: u64 = 1;
const FORMAT_MEMBER: &str = "format";
const VERSION_MEMBER: &str = "version";
const LANGUAGE_MEMBER: &str = "language";
const THEME_MEMBER: &str = "theme";
const TEXT_SIZE_MEMBER: &str = "text_size_percent";
const UNITS_MEMBER: &str = "units";
const SYSTEM_MEMBER: &str = "system";
const OVERRIDES_MEMBER: &str = "overrides";
const KIND_MEMBER: &str = "kind";
const UNIT_MEMBER: &str = "unit";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];

    pub fn name(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    pub fn from_name(name: &str) -> Option<Theme> {
        Theme::ALL.into_iter().find(|theme| theme.name() == name)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    pub language: Option<LanguageTag>,
    pub theme: Theme,
    pub text_size_percent: Option<NonZeroU32>,
    pub units: Option<UnitsChoice>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreState {
    Loaded,
    Missing,
    Unreadable,
    Newer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreferenceChange {
    Language(Option<LanguageTag>),
    Theme(Theme),
    TextSizePercent(Option<NonZeroU32>),
    Units(Option<UnitsChoice>),
}

impl PreferenceChange {
    fn apply(&self, preferences: &mut Preferences) {
        match self {
            PreferenceChange::Language(language) => preferences.language = language.clone(),
            PreferenceChange::Theme(theme) => preferences.theme = *theme,
            PreferenceChange::TextSizePercent(size) => preferences.text_size_percent = *size,
            PreferenceChange::Units(units) => preferences.units = units.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreferenceEvent {
    Opened {
        preferences: Preferences,
        state: StoreState,
    },
    Written(Result<(), StorageError>),
}

pub type PreferenceCompletion = Arc<dyn Fn(PreferenceEvent) + Send + Sync>;

struct Shared {
    preferences: Preferences,
    state: StoreState,
    is_open: bool,
    is_writing: bool,
    pending: VecDeque<PreferenceChange>,
}

fn locked(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Clone)]
struct Writer {
    storage: Arc<dyn Storage>,
    location: StorageLocation,
    shared: Arc<Mutex<Shared>>,
    done: PreferenceCompletion,
}

impl Writer {
    fn write_next(self) {
        let change = {
            let mut shared = locked(&self.shared);
            match shared.pending.pop_front() {
                Some(change) => change,
                None => {
                    shared.is_writing = false;
                    return;
                }
            }
        };
        let writer = self.clone();
        self.storage.read(
            &self.location,
            Box::new(move |result| {
                let mut updated = match read_preferences(result) {
                    (stored, StoreState::Loaded | StoreState::Newer) => stored,
                    (_, StoreState::Missing | StoreState::Unreadable) => {
                        locked(&writer.shared).preferences.clone()
                    }
                };
                change.apply(&mut updated);
                let bytes = json::write_canonical(&preferences_json(&updated)).into_bytes();
                let next = writer.clone();
                writer.storage.write_atomically(
                    &writer.location,
                    bytes,
                    Box::new(move |outcome| {
                        (next.done)(PreferenceEvent::Written(outcome));
                        next.write_next();
                    }),
                );
            }),
        );
    }
}

pub struct PreferenceStore {
    storage: Arc<dyn Storage>,
    location: Option<StorageLocation>,
    shared: Arc<Mutex<Shared>>,
    done: PreferenceCompletion,
}

impl PreferenceStore {
    pub fn open(
        storage: Arc<dyn Storage>,
        location: Option<StorageLocation>,
        done: PreferenceCompletion,
    ) -> PreferenceStore {
        let store = PreferenceStore {
            storage,
            location,
            shared: Arc::new(Mutex::new(Shared {
                preferences: Preferences::default(),
                state: StoreState::Missing,
                is_open: false,
                is_writing: false,
                pending: VecDeque::new(),
            })),
            done,
        };
        let Some(writer) = store.writer() else {
            locked(&store.shared).is_open = true;
            (store.done)(PreferenceEvent::Opened {
                preferences: Preferences::default(),
                state: StoreState::Missing,
            });
            return store;
        };
        let reader = writer.clone();
        writer.storage.read(
            &writer.location,
            Box::new(move |result| {
                let (stored, state) = read_preferences(result);
                let (preferences, starts_writing) = {
                    let mut shared = locked(&reader.shared);
                    let mut preferences = stored;
                    for change in &shared.pending {
                        change.apply(&mut preferences);
                    }
                    shared.preferences = preferences.clone();
                    shared.state = state;
                    shared.is_open = true;
                    let starts_writing = !shared.pending.is_empty() && !shared.is_writing;
                    shared.is_writing |= starts_writing;
                    (preferences, starts_writing)
                };
                (reader.done)(PreferenceEvent::Opened { preferences, state });
                if starts_writing {
                    reader.write_next();
                }
            }),
        );
        store
    }

    fn writer(&self) -> Option<Writer> {
        Some(Writer {
            storage: Arc::clone(&self.storage),
            location: self.location.clone()?,
            shared: Arc::clone(&self.shared),
            done: Arc::clone(&self.done),
        })
    }

    pub fn preferences(&self) -> Preferences {
        locked(&self.shared).preferences.clone()
    }

    pub fn state(&self) -> StoreState {
        locked(&self.shared).state
    }

    pub fn set(&mut self, change: PreferenceChange) {
        let has_location = self.location.is_some();
        let starts_writing = {
            let mut shared = locked(&self.shared);
            change.apply(&mut shared.preferences);
            if !has_location {
                return;
            }
            shared.pending.push_back(change);
            let starts_writing = shared.is_open && !shared.is_writing;
            shared.is_writing |= starts_writing;
            starts_writing
        };
        if let (true, Some(writer)) = (starts_writing, self.writer()) {
            writer.write_next();
        }
    }
}

fn read_preferences(result: Result<Vec<u8>, StorageError>) -> (Preferences, StoreState) {
    match result {
        Ok(bytes) => parse_preferences(&bytes),
        Err(StorageError::NotFound) => (Preferences::default(), StoreState::Missing),
        Err(StorageError::ReadFailed(_) | StorageError::WriteFailed(_)) => {
            (Preferences::default(), StoreState::Unreadable)
        }
    }
}

fn member<'json>(members: &'json [(String, Json)], name: &str) -> Option<&'json Json> {
    members
        .iter()
        .find(|(member_name, _)| member_name == name)
        .map(|(_, value)| value)
}

fn parse_preferences(bytes: &[u8]) -> (Preferences, StoreState) {
    let unreadable = (Preferences::default(), StoreState::Unreadable);
    let Ok(Json::Object(members)) = json::parse(bytes) else {
        return unreadable;
    };
    if !matches!(member(&members, FORMAT_MEMBER), Some(Json::String(name)) if name == FORMAT_NAME) {
        return unreadable;
    }
    let state = match member(&members, VERSION_MEMBER) {
        Some(Json::Count(FORMAT_VERSION)) => StoreState::Loaded,
        Some(Json::Count(version)) if *version > FORMAT_VERSION => StoreState::Newer,
        _ => return unreadable,
    };
    let preferences = Preferences {
        language: match member(&members, LANGUAGE_MEMBER) {
            Some(Json::String(tag)) => LanguageTag::parse(tag).ok(),
            _ => None,
        },
        theme: match member(&members, THEME_MEMBER) {
            Some(Json::String(name)) => Theme::from_name(name).unwrap_or_default(),
            _ => Theme::default(),
        },
        text_size_percent: match member(&members, TEXT_SIZE_MEMBER) {
            Some(Json::Count(percent)) => u32::try_from(*percent).ok().and_then(NonZeroU32::new),
            _ => None,
        },
        units: member(&members, UNITS_MEMBER).and_then(units_from_json),
    };
    (preferences, state)
}

fn units_from_json(value: &Json) -> Option<UnitsChoice> {
    let Json::Object(members) = value else {
        return None;
    };
    let system = match member(members, SYSTEM_MEMBER)? {
        Json::Null => None,
        Json::String(identifier) if is_identifier(identifier) => Some(identifier.clone()),
        _ => return None,
    };
    let Json::Array(entries) = member(members, OVERRIDES_MEMBER)? else {
        return None;
    };
    let mut overrides = BTreeMap::new();
    let mut previous_kind: Option<&str> = None;
    for entry in entries {
        let Json::Object(entry) = entry else {
            return None;
        };
        let (Some(Json::String(kind_name)), Some(Json::String(unit))) =
            (member(entry, KIND_MEMBER), member(entry, UNIT_MEMBER))
        else {
            return None;
        };
        if previous_kind.is_some_and(|previous| previous >= kind_name.as_str()) || unit.is_empty() {
            return None;
        }
        overrides.insert(QuantityKind::from_name(kind_name)?, unit.clone());
        previous_kind = Some(kind_name.as_str());
    }
    if system.is_none() && overrides.is_empty() {
        return None;
    }
    Some(UnitsChoice { system, overrides })
}

fn units_json(units: &UnitsChoice) -> Option<Json> {
    if units.system.is_none() && units.overrides.is_empty() {
        return None;
    }
    let mut overrides: Vec<(&str, &str)> = units
        .overrides
        .iter()
        .map(|(kind, unit)| (kind.name(), unit.as_str()))
        .collect();
    overrides.sort_unstable();
    Some(Json::object(vec![
        (
            SYSTEM_MEMBER,
            Json::optional(units.system.as_deref().map(Json::string)),
        ),
        (
            OVERRIDES_MEMBER,
            Json::Array(
                overrides
                    .into_iter()
                    .map(|(kind, unit)| {
                        Json::object(vec![
                            (KIND_MEMBER, Json::string(kind)),
                            (UNIT_MEMBER, Json::string(unit)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]))
}

fn preferences_json(preferences: &Preferences) -> Json {
    Json::object(vec![
        (FORMAT_MEMBER, Json::string(FORMAT_NAME)),
        (VERSION_MEMBER, Json::Count(FORMAT_VERSION)),
        (
            LANGUAGE_MEMBER,
            Json::optional(
                preferences
                    .language
                    .as_ref()
                    .map(|tag| Json::string(tag.as_str())),
            ),
        ),
        (THEME_MEMBER, Json::string(preferences.theme.name())),
        (
            TEXT_SIZE_MEMBER,
            Json::optional(
                preferences
                    .text_size_percent
                    .map(|percent| Json::Count(u64::from(percent.get()))),
            ),
        ),
        (
            UNITS_MEMBER,
            Json::optional(preferences.units.as_ref().and_then(units_json)),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::MemoryStorage;
    use std::path::PathBuf;

    struct Fixture {
        storage: Arc<MemoryStorage>,
        location: StorageLocation,
        events: Arc<Mutex<Vec<PreferenceEvent>>>,
    }

    impl Fixture {
        fn new() -> Fixture {
            Fixture::with_storage(MemoryStorage::new())
        }

        fn with_storage(storage: MemoryStorage) -> Fixture {
            Fixture {
                storage: Arc::new(storage),
                location: StorageLocation::from_path(PathBuf::from("preferences.json")),
                events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_file(text: &str) -> Fixture {
            let fixture = Fixture::new();
            fixture
                .storage
                .insert(&fixture.location, text.as_bytes().to_vec());
            fixture
        }

        fn open(&self) -> PreferenceStore {
            let events = Arc::clone(&self.events);
            let storage: Arc<dyn Storage> = self.storage.clone();
            PreferenceStore::open(
                storage,
                Some(self.location.clone()),
                Arc::new(move |event: PreferenceEvent| events.lock().unwrap().push(event)),
            )
        }

        fn file(&self) -> Option<String> {
            self.storage
                .contents(&self.location)
                .map(|bytes| String::from_utf8(bytes).unwrap())
        }

        fn events(&self) -> Vec<PreferenceEvent> {
            self.events.lock().unwrap().clone()
        }
    }

    fn tag(text: &str) -> LanguageTag {
        LanguageTag::parse(text).unwrap()
    }

    fn percent(value: u32) -> Option<NonZeroU32> {
        NonZeroU32::new(value)
    }

    fn reopened(fixture: &Fixture) -> Preferences {
        Fixture::with_file(&fixture.file().unwrap())
            .open()
            .preferences()
    }

    const FULL_FILE: &str = r#"{
  "format": "calc-preferences",
  "version": 1,
  "language": "de-DE",
  "theme": "dark",
  "text_size_percent": 125,
  "units": {
    "system": "us-customary",
    "overrides": [
      {
        "kind": "speed",
        "unit": "km/h"
      }
    ]
  }
}
"#;

    #[test]
    fn missing_file_opens_with_defaults() {
        let fixture = Fixture::new();

        let store = fixture.open();

        assert_eq!(
            (store.preferences(), store.state()),
            (Preferences::default(), StoreState::Missing)
        );
    }

    #[test]
    fn open_reports_the_stored_preferences_and_state() {
        let fixture = Fixture::with_file(FULL_FILE);

        let store = fixture.open();

        assert_eq!(
            fixture.events(),
            [PreferenceEvent::Opened {
                preferences: store.preferences(),
                state: StoreState::Loaded
            }]
        );
    }

    #[test]
    fn stored_file_is_read_as_stored() {
        let fixture = Fixture::with_file(FULL_FILE);

        let preferences = fixture.open().preferences();

        assert_eq!(
            preferences,
            Preferences {
                language: Some(tag("de-DE")),
                theme: Theme::Dark,
                text_size_percent: percent(125),
                units: Some(UnitsChoice {
                    system: Some("us-customary".to_string()),
                    overrides: BTreeMap::from([(QuantityKind::Speed, "km/h".to_string())]),
                }),
            }
        );
    }

    #[test]
    fn file_that_is_not_json_is_unreadable_with_defaults() {
        let fixture = Fixture::with_file("theme = dark");

        let store = fixture.open();

        assert_eq!(
            (store.preferences(), store.state()),
            (Preferences::default(), StoreState::Unreadable)
        );
    }

    #[test]
    fn file_with_another_format_is_unreadable() {
        let fixture = Fixture::with_file(r#"{"format": "calc-session", "version": 1}"#);

        assert_eq!(fixture.open().state(), StoreState::Unreadable);
    }

    #[test]
    fn file_without_a_version_is_unreadable() {
        let fixture = Fixture::with_file(r#"{"format": "calc-preferences", "theme": "dark"}"#);

        let store = fixture.open();

        assert_eq!(
            (store.preferences().theme, store.state()),
            (Theme::System, StoreState::Unreadable)
        );
    }

    #[test]
    fn member_with_an_unknown_value_takes_its_default_and_the_others_are_read() {
        let fixture = Fixture::with_file(
            r#"{"format": "calc-preferences", "version": 1, "theme": "sepia", "text_size_percent": 150}"#,
        );

        let store = fixture.open();

        assert_eq!(
            (
                store.preferences().theme,
                store.preferences().text_size_percent,
                store.state()
            ),
            (Theme::System, percent(150), StoreState::Loaded)
        );
    }

    #[test]
    fn language_that_is_not_a_language_tag_takes_its_default() {
        let fixture = Fixture::with_file(
            r#"{"format": "calc-preferences", "version": 1, "language": "not a tag!"}"#,
        );

        assert_eq!(fixture.open().preferences().language, None);
    }

    #[test]
    fn valid_but_unshipped_language_tag_is_kept() {
        let fixture = Fixture::with_file(
            r#"{"format": "calc-preferences", "version": 1, "language": "de-AT"}"#,
        );

        assert_eq!(fixture.open().preferences().language, Some(tag("de-AT")));
    }

    #[test]
    fn zero_text_size_takes_its_default() {
        let fixture = Fixture::with_file(
            r#"{"format": "calc-preferences", "version": 1, "text_size_percent": 0}"#,
        );

        assert_eq!(fixture.open().preferences().text_size_percent, None);
    }

    #[test]
    fn unsorted_unit_overrides_take_the_default() {
        let fixture = Fixture::with_file(
            r#"{"format": "calc-preferences", "version": 1, "units": {"system": null, "overrides": [{"kind": "time", "unit": "h"}, {"kind": "speed", "unit": "km/h"}]}}"#,
        );

        assert_eq!(fixture.open().preferences().units, None);
    }

    #[test]
    fn unknown_member_of_a_known_version_is_ignored() {
        let fixture = Fixture::with_file(
            r#"{"format": "calc-preferences", "version": 1, "theme": "light", "colour_blind": true}"#,
        );

        let store = fixture.open();

        assert_eq!(
            (store.preferences().theme, store.state()),
            (Theme::Light, StoreState::Loaded)
        );
    }

    #[test]
    fn newer_file_reads_the_known_members() {
        let fixture = Fixture::with_file(
            r#"{"format": "calc-preferences", "version": 2, "language": "de", "text_size_percent": 200, "read_aloud": true}"#,
        );

        let store = fixture.open();

        assert_eq!(
            (
                store.preferences().language,
                store.preferences().text_size_percent,
                store.state()
            ),
            (Some(tag("de")), percent(200), StoreState::Newer)
        );
    }

    #[test]
    fn newer_file_is_not_written_by_opening() {
        let newer = r#"{"format": "calc-preferences", "version": 2, "read_aloud": true}"#;
        let fixture = Fixture::with_file(newer);

        fixture.open();

        assert_eq!(fixture.file().as_deref(), Some(newer));
    }

    #[test]
    fn change_to_a_newer_file_converts_it_to_the_current_version() {
        let fixture = Fixture::with_file(
            r#"{"format": "calc-preferences", "version": 2, "language": "de", "read_aloud": true}"#,
        );
        let mut store = fixture.open();

        store.set(PreferenceChange::Theme(Theme::Dark));

        assert_eq!(
            fixture.file().as_deref(),
            Some(
                "{\n  \"format\": \"calc-preferences\",\n  \"version\": 1,\n  \"language\": \"de\",\n  \"theme\": \"dark\",\n  \"text_size_percent\": null,\n  \"units\": null\n}\n"
            )
        );
    }

    #[test]
    fn first_change_on_a_missing_file_writes_it() {
        let fixture = Fixture::new();
        let mut store = fixture.open();

        store.set(PreferenceChange::Theme(Theme::Light));

        assert_eq!(reopened(&fixture).theme, Theme::Light);
    }

    #[test]
    fn first_change_replaces_an_unreadable_file() {
        let fixture = Fixture::with_file("\u{FEFF}garbage");
        let mut store = fixture.open();

        store.set(PreferenceChange::TextSizePercent(percent(110)));

        assert_eq!(reopened(&fixture).text_size_percent, percent(110));
    }

    #[test]
    fn language_is_set_and_stored() {
        let fixture = Fixture::new();
        let mut store = fixture.open();

        store.set(PreferenceChange::Language(Some(tag("de-DE"))));

        assert_eq!(
            (store.preferences().language, reopened(&fixture).language),
            (Some(tag("de-DE")), Some(tag("de-DE")))
        );
    }

    #[test]
    fn platform_language_is_stored_as_null() {
        let fixture = Fixture::with_file(FULL_FILE);
        let mut store = fixture.open();

        store.set(PreferenceChange::Language(None));

        assert_eq!(reopened(&fixture).language, None);
    }

    #[test]
    fn theme_is_set_and_stored() {
        let fixture = Fixture::new();
        let mut store = fixture.open();

        store.set(PreferenceChange::Theme(Theme::Dark));

        assert_eq!(
            (store.preferences().theme, reopened(&fixture).theme),
            (Theme::Dark, Theme::Dark)
        );
    }

    #[test]
    fn text_size_is_set_and_stored() {
        let fixture = Fixture::new();
        let mut store = fixture.open();

        store.set(PreferenceChange::TextSizePercent(percent(150)));

        assert_eq!(
            (
                store.preferences().text_size_percent,
                reopened(&fixture).text_size_percent
            ),
            (percent(150), percent(150))
        );
    }

    #[test]
    fn units_are_set_and_stored_with_sorted_overrides() {
        let fixture = Fixture::new();
        let mut store = fixture.open();
        let units = UnitsChoice {
            system: None,
            overrides: BTreeMap::from([
                (QuantityKind::Time, "h min".to_string()),
                (QuantityKind::Speed, "km/h".to_string()),
            ]),
        };

        store.set(PreferenceChange::Units(Some(units.clone())));

        assert_eq!(reopened(&fixture).units, Some(units));
    }

    #[test]
    fn empty_units_choice_is_stored_as_null() {
        let fixture = Fixture::new();
        let mut store = fixture.open();

        store.set(PreferenceChange::Units(Some(UnitsChoice::default())));

        assert!(fixture.file().unwrap().contains("\"units\": null"));
    }

    #[test]
    fn change_keeps_what_another_instance_wrote_meanwhile() {
        let fixture = Fixture::new();
        let mut first = fixture.open();
        let mut second = fixture.open();
        first.set(PreferenceChange::Theme(Theme::Dark));

        second.set(PreferenceChange::TextSizePercent(percent(125)));

        let stored = reopened(&fixture);
        assert_eq!(
            (stored.theme, stored.text_size_percent),
            (Theme::Dark, percent(125))
        );
    }

    #[test]
    fn failed_write_keeps_the_change_in_memory_and_reports_the_error() {
        let fixture = Fixture::with_storage(MemoryStorage::refusing_writes());
        let mut store = fixture.open();

        store.set(PreferenceChange::Theme(Theme::Dark));

        assert_eq!(store.preferences().theme, Theme::Dark);
        assert!(fixture.events().contains(&PreferenceEvent::Written(Err(
            StorageError::WriteFailed(std::io::ErrorKind::PermissionDenied)
        ))));
    }

    #[test]
    fn store_without_a_location_keeps_changes_in_memory_only() {
        let fixture = Fixture::new();
        let storage: Arc<dyn Storage> = fixture.storage.clone();
        let mut store = PreferenceStore::open(storage, None, Arc::new(|_: PreferenceEvent| {}));

        store.set(PreferenceChange::Theme(Theme::Light));

        assert_eq!(
            (store.preferences().theme, fixture.file()),
            (Theme::Light, None)
        );
    }
}
