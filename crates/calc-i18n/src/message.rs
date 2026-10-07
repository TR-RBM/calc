include!(concat!(env!("OUT_DIR"), "/message.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::LocaleEntry;
    use crate::plural::PluralRule;

    fn missing_translations(locales: &[LocaleEntry]) -> Vec<String> {
        locales
            .iter()
            .flat_map(|locale| {
                MESSAGE_KEYS
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| locale.patterns.get(*index).copied().flatten().is_none())
                    .map(move |(_, key)| format!("{} {key}", locale.tag))
            })
            .collect()
    }

    #[test]
    fn every_shipped_locale_has_every_source_key() {
        assert_eq!(missing_translations(&SHIPPED_LOCALES), Vec::<String>::new());
    }

    #[test]
    fn completeness_check_reports_a_locale_without_translations() {
        let empty = LocaleEntry {
            tag: "xx",
            plural_rule: PluralRule::OneOther,
            patterns: &[],
        };
        assert_eq!(missing_translations(&[empty]).len(), MESSAGE_COUNT);
    }

    #[test]
    fn a_message_names_its_key() {
        assert_eq!(Message::CommonKindExact.key(), "common-kind-exact");
    }

    #[test]
    fn a_message_with_fields_names_its_key() {
        let message = Message::CommonUnitSourceCurriculum {
            name: "school".to_owned(),
        };

        assert_eq!(message.key(), "common-unit-source-curriculum");
    }

    #[test]
    fn message_index_matches_its_key_position() {
        assert_eq!(
            MESSAGE_KEYS[Message::CommonKindExact.index()],
            "common-kind-exact"
        );
    }
}
