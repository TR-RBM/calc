use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotationMode {
    DecimalSeparator,
    DivisionSigns,
    MultiplicationSigns,
    Juxtaposition,
    Coordinates,
    MixedNumbers,
    RecurringMark,
}

impl NotationMode {
    pub const ALL: [NotationMode; 7] = [
        NotationMode::DecimalSeparator,
        NotationMode::DivisionSigns,
        NotationMode::MultiplicationSigns,
        NotationMode::Juxtaposition,
        NotationMode::Coordinates,
        NotationMode::MixedNumbers,
        NotationMode::RecurringMark,
    ];

    pub fn name(self) -> &'static str {
        match self {
            NotationMode::DecimalSeparator => "decimal_separator",
            NotationMode::DivisionSigns => "division_signs",
            NotationMode::MultiplicationSigns => "multiplication_signs",
            NotationMode::Juxtaposition => "juxtaposition",
            NotationMode::Coordinates => "coordinates",
            NotationMode::MixedNumbers => "mixed_numbers",
            NotationMode::RecurringMark => "recurring_mark",
        }
    }

    pub fn from_name(name: &str) -> Option<NotationMode> {
        NotationMode::ALL
            .iter()
            .copied()
            .find(|mode| mode.name() == name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecimalSeparator {
    Point,
    Comma,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DivisionSign {
    Slash,
    Colon,
    Obelus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MultiplicationSign {
    Asterisk,
    MiddleDot,
    Cross,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Juxtaposition {
    None,
    NumberLetter,
    NumberLetterAndLetters,
    LettersAndParentheses,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coordinates {
    None,
    ParenthesesComma,
    ParenthesesSemicolon,
    ParenthesesBar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MixedNumbers {
    None,
    Space,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecurringMark {
    None,
    Bar,
    Dots,
    Parentheses,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotationConflict {
    CommaDecimalWithCommaCoordinates,
    JuxtapositionWithCross,
    MixedNumbersWithoutSlash,
    EmptyDivisionSigns,
    EmptyMultiplicationSigns,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnswerNotation {
    pub decimal_separator: DecimalSeparator,
    pub division_signs: BTreeSet<DivisionSign>,
    pub multiplication_signs: BTreeSet<MultiplicationSign>,
    pub juxtaposition: Juxtaposition,
    pub coordinates: Coordinates,
    pub mixed_numbers: MixedNumbers,
    pub recurring_mark: RecurringMark,
}

const DECIMAL_SEPARATORS: [(DecimalSeparator, &str); 2] = [
    (DecimalSeparator::Point, "point"),
    (DecimalSeparator::Comma, "comma"),
];
const DIVISION_SIGNS: [(DivisionSign, &str); 3] = [
    (DivisionSign::Slash, "slash"),
    (DivisionSign::Colon, "colon"),
    (DivisionSign::Obelus, "obelus"),
];
const MULTIPLICATION_SIGNS: [(MultiplicationSign, &str); 3] = [
    (MultiplicationSign::Asterisk, "asterisk"),
    (MultiplicationSign::MiddleDot, "middle_dot"),
    (MultiplicationSign::Cross, "cross"),
];
const JUXTAPOSITIONS: [(Juxtaposition, &str); 4] = [
    (Juxtaposition::None, "none"),
    (Juxtaposition::NumberLetter, "number_letter"),
    (
        Juxtaposition::NumberLetterAndLetters,
        "number_letter_and_letters",
    ),
    (
        Juxtaposition::LettersAndParentheses,
        "letters_and_parentheses",
    ),
];
const COORDINATES: [(Coordinates, &str); 4] = [
    (Coordinates::None, "none"),
    (Coordinates::ParenthesesComma, "parentheses_comma"),
    (Coordinates::ParenthesesSemicolon, "parentheses_semicolon"),
    (Coordinates::ParenthesesBar, "parentheses_bar"),
];
const MIXED_NUMBERS: [(MixedNumbers, &str); 2] =
    [(MixedNumbers::None, "none"), (MixedNumbers::Space, "space")];
const RECURRING_MARKS: [(RecurringMark, &str); 4] = [
    (RecurringMark::None, "none"),
    (RecurringMark::Bar, "bar"),
    (RecurringMark::Dots, "dots"),
    (RecurringMark::Parentheses, "parentheses"),
];
const SET_SEPARATOR: &str = ", ";

fn value_named<T: Copy>(table: &[(T, &str)], name: &str) -> Option<T> {
    table
        .iter()
        .find(|(_, candidate)| *candidate == name)
        .map(|(value, _)| *value)
}

fn set_named<T: Copy + Ord>(table: &[(T, &str)], text: &str) -> Option<BTreeSet<T>> {
    if text.is_empty() {
        return Some(BTreeSet::new());
    }
    let names: Vec<&str> = text.split(SET_SEPARATOR).collect();
    let set: BTreeSet<T> = names
        .iter()
        .map(|name| value_named(table, name))
        .collect::<Option<_>>()?;
    (set.len() == names.len()).then_some(set)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NotationValues {
    values: Vec<(NotationMode, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotationValueError {
    MissingMode(NotationMode),
    InvalidValue(NotationMode, String),
}

impl NotationValues {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, mode: NotationMode, value: &str) {
        self.values.retain(|(existing, _)| *existing != mode);
        self.values.push((mode, value.to_string()));
    }

    fn text(&self, mode: NotationMode) -> Result<&str, NotationValueError> {
        self.values
            .iter()
            .find(|(existing, _)| *existing == mode)
            .map(|(_, value)| value.as_str())
            .ok_or(NotationValueError::MissingMode(mode))
    }

    fn one<T: Copy>(
        &self,
        mode: NotationMode,
        table: &[(T, &str)],
    ) -> Result<T, NotationValueError> {
        let text = self.text(mode)?;
        value_named(table, text)
            .ok_or_else(|| NotationValueError::InvalidValue(mode, text.to_string()))
    }

    fn many<T: Copy + Ord>(
        &self,
        mode: NotationMode,
        table: &[(T, &str)],
    ) -> Result<BTreeSet<T>, NotationValueError> {
        let text = self.text(mode)?;
        set_named(table, text)
            .ok_or_else(|| NotationValueError::InvalidValue(mode, text.to_string()))
    }

    pub fn notation(&self) -> Result<AnswerNotation, NotationValueError> {
        Ok(AnswerNotation {
            decimal_separator: self.one(NotationMode::DecimalSeparator, &DECIMAL_SEPARATORS)?,
            division_signs: self.many(NotationMode::DivisionSigns, &DIVISION_SIGNS)?,
            multiplication_signs: self
                .many(NotationMode::MultiplicationSigns, &MULTIPLICATION_SIGNS)?,
            juxtaposition: self.one(NotationMode::Juxtaposition, &JUXTAPOSITIONS)?,
            coordinates: self.one(NotationMode::Coordinates, &COORDINATES)?,
            mixed_numbers: self.one(NotationMode::MixedNumbers, &MIXED_NUMBERS)?,
            recurring_mark: self.one(NotationMode::RecurringMark, &RECURRING_MARKS)?,
        })
    }
}

fn coordinates_read_at_least(later: Coordinates, earlier: Coordinates) -> bool {
    earlier == Coordinates::None || later == earlier
}

fn recurring_mark_reads_at_least(later: RecurringMark, earlier: RecurringMark) -> bool {
    earlier == RecurringMark::None || later == earlier
}

impl AnswerNotation {
    pub fn check(&self) -> Vec<NotationConflict> {
        let mut conflicts = Vec::new();
        if self.decimal_separator == DecimalSeparator::Comma
            && self.coordinates == Coordinates::ParenthesesComma
        {
            conflicts.push(NotationConflict::CommaDecimalWithCommaCoordinates);
        }
        if self.juxtaposition != Juxtaposition::None
            && self
                .multiplication_signs
                .contains(&MultiplicationSign::Cross)
        {
            conflicts.push(NotationConflict::JuxtapositionWithCross);
        }
        if self.mixed_numbers == MixedNumbers::Space
            && !self.division_signs.contains(&DivisionSign::Slash)
        {
            conflicts.push(NotationConflict::MixedNumbersWithoutSlash);
        }
        if self.division_signs.is_empty() {
            conflicts.push(NotationConflict::EmptyDivisionSigns);
        }
        if self.multiplication_signs.is_empty() {
            conflicts.push(NotationConflict::EmptyMultiplicationSigns);
        }
        conflicts
    }

    pub fn modes_reading_less_than(&self, earlier: &AnswerNotation) -> Vec<NotationMode> {
        let keeps = [
            (
                NotationMode::DecimalSeparator,
                self.decimal_separator == earlier.decimal_separator,
            ),
            (
                NotationMode::DivisionSigns,
                self.division_signs.is_superset(&earlier.division_signs),
            ),
            (
                NotationMode::MultiplicationSigns,
                self.multiplication_signs
                    .is_superset(&earlier.multiplication_signs),
            ),
            (
                NotationMode::Juxtaposition,
                self.juxtaposition >= earlier.juxtaposition,
            ),
            (
                NotationMode::Coordinates,
                coordinates_read_at_least(self.coordinates, earlier.coordinates),
            ),
            (
                NotationMode::MixedNumbers,
                self.mixed_numbers >= earlier.mixed_numbers,
            ),
            (
                NotationMode::RecurringMark,
                recurring_mark_reads_at_least(self.recurring_mark, earlier.recurring_mark),
            ),
        ];
        keeps
            .iter()
            .filter(|(_, kept)| !kept)
            .map(|(mode, _)| *mode)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(changes: &[(NotationMode, &str)]) -> NotationValues {
        let mut values = NotationValues::new();
        for (mode, value) in [
            (NotationMode::DecimalSeparator, "comma"),
            (NotationMode::DivisionSigns, "colon"),
            (NotationMode::MultiplicationSigns, "middle_dot"),
            (NotationMode::Juxtaposition, "none"),
            (NotationMode::Coordinates, "parentheses_bar"),
            (NotationMode::MixedNumbers, "none"),
            (NotationMode::RecurringMark, "none"),
        ] {
            values.set(mode, value);
        }
        for (mode, value) in changes {
            values.set(*mode, value);
        }
        values
    }

    fn notation(changes: &[(NotationMode, &str)]) -> AnswerNotation {
        values(changes).notation().unwrap()
    }

    #[test]
    fn every_mode_name_reads_back() {
        for mode in NotationMode::ALL {
            assert_eq!(NotationMode::from_name(mode.name()), Some(mode));
        }
    }

    #[test]
    fn values_read_into_a_notation() {
        let read = notation(&[(NotationMode::DivisionSigns, "slash, colon")]);
        assert_eq!(
            read.division_signs,
            BTreeSet::from([DivisionSign::Slash, DivisionSign::Colon])
        );
    }

    #[test]
    fn missing_mode_is_reported() {
        let mut incomplete = NotationValues::new();
        incomplete.set(NotationMode::DecimalSeparator, "point");
        assert_eq!(
            incomplete.notation(),
            Err(NotationValueError::MissingMode(NotationMode::DivisionSigns))
        );
    }

    #[test]
    fn unknown_value_name_is_reported() {
        assert_eq!(
            values(&[(NotationMode::RecurringMark, "tilde")]).notation(),
            Err(NotationValueError::InvalidValue(
                NotationMode::RecurringMark,
                "tilde".to_string()
            ))
        );
    }

    #[test]
    fn repeated_sign_in_a_set_is_invalid() {
        assert!(
            values(&[(NotationMode::DivisionSigns, "colon, colon")])
                .notation()
                .is_err()
        );
    }

    #[test]
    fn valid_notation_has_no_conflict() {
        assert!(notation(&[]).check().is_empty());
    }

    #[test]
    fn comma_decimal_with_comma_coordinates_conflicts() {
        assert_eq!(
            notation(&[(NotationMode::Coordinates, "parentheses_comma")]).check(),
            vec![NotationConflict::CommaDecimalWithCommaCoordinates]
        );
    }

    #[test]
    fn juxtaposition_with_cross_conflicts() {
        assert_eq!(
            notation(&[
                (NotationMode::Juxtaposition, "number_letter"),
                (NotationMode::MultiplicationSigns, "cross")
            ])
            .check(),
            vec![NotationConflict::JuxtapositionWithCross]
        );
    }

    #[test]
    fn mixed_numbers_without_slash_conflict() {
        assert_eq!(
            notation(&[(NotationMode::MixedNumbers, "space")]).check(),
            vec![NotationConflict::MixedNumbersWithoutSlash]
        );
    }

    #[test]
    fn empty_division_signs_conflict() {
        assert_eq!(
            notation(&[(NotationMode::DivisionSigns, "")]).check(),
            vec![NotationConflict::EmptyDivisionSigns]
        );
    }

    #[test]
    fn parentheses_mark_with_parenthesised_products_does_not_conflict() {
        assert!(
            notation(&[
                (NotationMode::RecurringMark, "parentheses"),
                (NotationMode::Juxtaposition, "letters_and_parentheses")
            ])
            .check()
            .is_empty()
        );
    }

    #[test]
    fn wider_notation_reads_no_less() {
        let later = notation(&[
            (NotationMode::DivisionSigns, "slash, colon"),
            (NotationMode::Juxtaposition, "letters_and_parentheses"),
            (NotationMode::RecurringMark, "bar"),
        ]);
        assert!(later.modes_reading_less_than(&notation(&[])).is_empty());
    }

    #[test]
    fn dropped_sign_reads_less() {
        let earlier = notation(&[(NotationMode::DivisionSigns, "slash, colon")]);
        assert_eq!(
            notation(&[]).modes_reading_less_than(&earlier),
            vec![NotationMode::DivisionSigns]
        );
    }

    #[test]
    fn changed_decimal_separator_reads_less() {
        let later = notation(&[(NotationMode::DecimalSeparator, "point")]);
        assert_eq!(
            later.modes_reading_less_than(&notation(&[])),
            vec![NotationMode::DecimalSeparator]
        );
    }

    #[test]
    fn narrower_juxtaposition_reads_less() {
        let earlier = notation(&[(NotationMode::Juxtaposition, "number_letter")]);
        assert_eq!(
            notation(&[]).modes_reading_less_than(&earlier),
            vec![NotationMode::Juxtaposition]
        );
    }

    #[test]
    fn swapped_coordinates_read_less() {
        let later = notation(&[(NotationMode::Coordinates, "parentheses_semicolon")]);
        assert_eq!(
            later.modes_reading_less_than(&notation(&[])),
            vec![NotationMode::Coordinates]
        );
    }

    #[test]
    fn swapped_recurring_mark_reads_less() {
        let earlier = notation(&[(NotationMode::RecurringMark, "bar")]);
        let later = notation(&[(NotationMode::RecurringMark, "dots")]);
        assert_eq!(
            later.modes_reading_less_than(&earlier),
            vec![NotationMode::RecurringMark]
        );
    }
}
