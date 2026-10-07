use std::ops::Range;

use crate::error::{ParseError, ParseErrorKind};

pub(crate) const CHEMISTRY_PREFIX_LENGTH: usize = "chem'".len();

const ELEMENT_SYMBOLS: [&str; 118] = [
    "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S", "Cl",
    "Ar", "K", "Ca", "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn", "Ga", "Ge", "As",
    "Se", "Br", "Kr", "Rb", "Sr", "Y", "Zr", "Nb", "Mo", "Tc", "Ru", "Rh", "Pd", "Ag", "Cd", "In",
    "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce", "Pr", "Nd", "Pm", "Sm", "Eu", "Gd", "Tb",
    "Dy", "Ho", "Er", "Tm", "Yb", "Lu", "Hf", "Ta", "W", "Re", "Os", "Ir", "Pt", "Au", "Hg", "Tl",
    "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th", "Pa", "U", "Np", "Pu", "Am", "Cm", "Bk",
    "Cf", "Es", "Fm", "Md", "No", "Lr", "Rf", "Db", "Sg", "Bh", "Hs", "Mt", "Ds", "Rg", "Cn", "Nh",
    "Fl", "Mc", "Lv", "Ts", "Og",
];
const STATES: [&str; 4] = ["(s)", "(l)", "(g)", "(aq)"];
const REACTION_ARROWS: [&str; 2] = ["->", "\u{2192}"];
const RESONANCE_ARROWS: [&str; 2] = ["<->", "\u{2194}"];
const EQUILIBRIUM_ARROWS: [&str; 2] = ["<=>", "\u{21CC}"];
const SPECIES_SEPARATOR: &str = " + ";
const HYDRATE_DOTS: [char; 2] = ['\u{00B7}', '*'];
const CHARGE_MARK: char = '^';
const SUBSCRIPT_ZERO: u32 = 0x2080;
const COUNT_LIMIT: u64 = i64::MAX.unsigned_abs();
const SUPERSCRIPT_DIGITS: [char; 10] = [
    '\u{2070}', '\u{00B9}', '\u{00B2}', '\u{00B3}', '\u{2074}', '\u{2075}', '\u{2076}', '\u{2077}',
    '\u{2078}', '\u{2079}',
];
const SUPERSCRIPT_PLUS: char = '\u{207A}';
const SUPERSCRIPT_MINUS: char = '\u{207B}';
const NEUTRINO_NAMES: [&str; 2] = ["\u{03BD}", "nu"];
const ELECTRON: &str = "e";
const OXIDATION_LETTERS: [char; 2] = ['I', 'V'];
const ISOTOPE_SYMBOLS: [char; 2] = ['D', 'T'];
const WRONG_ARROWS: [&str; 2] = ["=>", "="];
const ELECTRON_WITH_CARET: &str = "e^-";
const POSITRON_SPELLINGS: [&str; 2] = ["e+", "e^+"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChemistryProblem {
    Empty,
    UnknownElement,
    WrongCase,
    AmbiguousCharge,
    SignBeforeNumber,
    AttachedNumber,
    OxidationState,
    SumWithoutSpaces,
    DotSeparator,
    Resonance,
    Nuclide,
    Equilibrium,
    FractionalCoefficient,
    DecimalCount,
    TwoArrows,
    EmptySide,
    UnclosedBracket,
    UnexpectedCharacter,
    CoefficientWithoutReaction,
    WrongArrow,
    NoArrow,
    RepeatedSpecies,
    ZeroCount,
    ZeroCoefficient,
    ZeroCharge,
    CountTooLarge,
    Isotope,
    DanglingHydrateDot,
    CoefficientOutsideLiteral,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParsedSpecies {
    pub(crate) text: String,
    pub(crate) elements: Vec<(u8, u64)>,
    pub(crate) charge: i64,
    pub(crate) coefficient: Option<u64>,
    pub(crate) is_product: bool,
    pub(crate) at: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParsedChemistry {
    Substance(ParsedSpecies),
    Reaction(Vec<ParsedSpecies>),
}

struct Reader<'text> {
    text: &'text str,
    position: usize,
    offset: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<char> {
        self.text[self.position..].chars().next()
    }

    fn rest(&self) -> &str {
        &self.text[self.position..]
    }

    fn advance(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.position += character.len_utf8();
        Some(character)
    }

    fn fail(&self, problem: ChemistryProblem, from: usize, to: usize) -> ParseError {
        ParseError::new(
            ParseErrorKind::Chemistry(problem),
            self.offset + from..self.offset + to.max(from + 1),
        )
    }

    fn fail_here(&self, problem: ChemistryProblem) -> ParseError {
        let width = self.peek().map_or(1, char::len_utf8);
        self.fail(problem, self.position, self.position + width)
    }

    fn count(&mut self) -> Result<Option<u64>, ParseError> {
        let start = self.position;
        let written = self.digits()?;
        let text = &self.text[start..self.position];
        let leading_zero = text.starts_with('0') || text.starts_with('\u{2080}');
        let fraction_follows = matches!(self.peek(), Some('.' | '/'));
        if (written == Some(0) || leading_zero) && !fraction_follows {
            return Err(self.fail(ChemistryProblem::ZeroCount, start, self.position));
        }
        Ok(written)
    }

    fn digits(&mut self) -> Result<Option<u64>, ParseError> {
        if self.peek().is_some_and(|c| c.is_ascii_digit()) {
            return self.number(ascii_digit_value);
        }
        self.number(subscript_value)
    }

    fn number(&mut self, value_of: fn(char) -> Option<u64>) -> Result<Option<u64>, ParseError> {
        let start = self.position;
        let mut value: Option<u64> = None;
        let mut too_large = false;
        while let Some(digit) = self.peek().and_then(value_of) {
            self.advance();
            let next = value
                .unwrap_or(0)
                .checked_mul(10)
                .and_then(|shifted| shifted.checked_add(digit))
                .filter(|next| *next <= COUNT_LIMIT);
            too_large |= next.is_none();
            value = Some(next.unwrap_or(COUNT_LIMIT));
        }
        if too_large {
            return Err(self.fail(ChemistryProblem::CountTooLarge, start, self.position));
        }
        Ok(value)
    }
}

fn ascii_digit_value(character: char) -> Option<u64> {
    character.to_digit(10).map(u64::from)
}

fn subscript_value(character: char) -> Option<u64> {
    let code = u32::from(character);
    (SUBSCRIPT_ZERO..SUBSCRIPT_ZERO + 10)
        .contains(&code)
        .then(|| u64::from(code - SUBSCRIPT_ZERO))
}

fn superscript_value(character: char) -> Option<u64> {
    SUPERSCRIPT_DIGITS
        .iter()
        .position(|digit| *digit == character)
        .and_then(|position| u64::try_from(position).ok())
}

pub fn element_symbol(atomic_number: u8) -> Option<&'static str> {
    ELEMENT_SYMBOLS
        .get(usize::from(atomic_number).checked_sub(1)?)
        .copied()
}

pub(crate) fn atomic_number(symbol: &str) -> Option<u8> {
    ELEMENT_SYMBOLS
        .iter()
        .position(|candidate| *candidate == symbol)
        .and_then(|position| u8::try_from(position + 1).ok())
}

fn is_any_case_element(symbol: &str) -> bool {
    ELEMENT_SYMBOLS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(symbol))
}

fn add_counts(into: &mut Vec<(u8, u64)>, from: &[(u8, u64)], multiplier: u64) -> Option<()> {
    for (element, count) in from {
        let count = count
            .checked_mul(multiplier)
            .filter(|count| *count <= COUNT_LIMIT)?;
        match into.iter_mut().find(|(present, _)| present == element) {
            Some((_, total)) => {
                *total = total
                    .checked_add(count)
                    .filter(|total| *total <= COUNT_LIMIT)?;
            }
            None => into.push((*element, count)),
        }
    }
    Some(())
}

fn closing(opening: char) -> Option<char> {
    match opening {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        _ => None,
    }
}

fn element(reader: &mut Reader<'_>) -> Result<u8, ParseError> {
    let start = reader.position;
    let Some(first) = reader.peek().filter(char::is_ascii_alphabetic) else {
        return Err(reader.fail_here(ChemistryProblem::UnexpectedCharacter));
    };
    reader.advance();
    if !first.is_ascii_uppercase() {
        while reader.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            reader.advance();
        }
        let written = &reader.text[start..reader.position];
        let problem = if is_any_case_element(written) {
            ChemistryProblem::WrongCase
        } else {
            ChemistryProblem::UnknownElement
        };
        return Err(reader.fail(problem, start, reader.position));
    }
    if let Some(second) = reader.peek().filter(char::is_ascii_lowercase) {
        let two = format!("{first}{second}");
        if let Some(found) = atomic_number(&two) {
            reader.advance();
            return Ok(found);
        }
    }
    if ISOTOPE_SYMBOLS.contains(&first) {
        return Err(reader.fail(ChemistryProblem::Isotope, start, reader.position));
    }
    if let Some(found) = atomic_number(&first.to_string()) {
        return Ok(found);
    }
    let previous = reader.text[..start].chars().next_back();
    if let Some(previous) = previous.filter(char::is_ascii_uppercase) {
        let joined = format!("{previous}{}", first.to_ascii_lowercase());
        if atomic_number(&joined).is_some() {
            return Err(reader.fail(
                ChemistryProblem::WrongCase,
                start - previous.len_utf8(),
                reader.position,
            ));
        }
    }
    Err(reader.fail(ChemistryProblem::UnknownElement, start, reader.position))
}

fn group(reader: &mut Reader<'_>, close: Option<char>) -> Result<Vec<(u8, u64)>, ParseError> {
    let mut counts: Vec<(u8, u64)> = Vec::new();
    loop {
        let Some(character) = reader.peek() else {
            return match close {
                Some(_) => Err(reader.fail_here(ChemistryProblem::UnclosedBracket)),
                None => Ok(counts),
            };
        };
        if Some(character) == close {
            reader.advance();
            return Ok(counts);
        }
        if let Some(inner_close) = closing(character) {
            let start = reader.position;
            if close.is_none() && STATES.iter().any(|state| reader.rest() == *state) {
                return Ok(counts);
            }
            reader.advance();
            let inner_text_start = reader.position;
            let inner = group(reader, Some(inner_close))?;
            let inner_text =
                &reader.text[inner_text_start..reader.position - inner_close.len_utf8()];
            if !inner_text.is_empty() && inner_text.chars().all(|c| OXIDATION_LETTERS.contains(&c))
            {
                return Err(reader.fail(ChemistryProblem::OxidationState, start, reader.position));
            }
            let multiplier = reader.count()?.unwrap_or(1);
            add_counts(&mut counts, &inner, multiplier).ok_or_else(|| {
                reader.fail(ChemistryProblem::CountTooLarge, start, reader.position)
            })?;
            continue;
        }
        if character.is_ascii_alphabetic() {
            let start = reader.position;
            let found = element(reader)?;
            let written = reader.count()?;
            if written.is_some() && reader.peek() == Some('.') {
                return Err(reader.fail_here(ChemistryProblem::DecimalCount));
            }
            add_counts(&mut counts, &[(found, 1)], written.unwrap_or(1)).ok_or_else(|| {
                reader.fail(ChemistryProblem::CountTooLarge, start, reader.position)
            })?;
            continue;
        }
        return Ok(counts);
    }
}

fn charge(reader: &mut Reader<'_>, after_digit: bool) -> Result<i64, ParseError> {
    let start = reader.position;
    let Some(character) = reader.peek() else {
        return Ok(0);
    };
    if character == CHARGE_MARK {
        reader.advance();
        let magnitude = reader.digits()?.unwrap_or(1);
        let sign = match reader.advance() {
            Some('+') => 1,
            Some('-') => -1,
            _ => {
                return Err(reader.fail(
                    ChemistryProblem::UnexpectedCharacter,
                    start,
                    reader.position,
                ));
            }
        };
        if reader.peek().is_some_and(|c| c.is_ascii_digit()) {
            return Err(reader.fail(
                ChemistryProblem::SignBeforeNumber,
                start,
                reader.position + 1,
            ));
        }
        if magnitude == 0 {
            return Err(reader.fail(ChemistryProblem::ZeroCharge, start, reader.position));
        }
        return i64::try_from(magnitude)
            .map(|magnitude| sign * magnitude)
            .map_err(|_| {
                reader.fail(
                    ChemistryProblem::UnexpectedCharacter,
                    start,
                    reader.position,
                )
            });
    }
    if superscript_value(character).is_some()
        || character == SUPERSCRIPT_PLUS
        || character == SUPERSCRIPT_MINUS
    {
        let magnitude = reader.number(superscript_value)?;
        let sign = match reader.advance() {
            Some(SUPERSCRIPT_PLUS) => 1,
            Some(SUPERSCRIPT_MINUS) => -1,
            _ => {
                return Err(reader.fail(
                    ChemistryProblem::UnexpectedCharacter,
                    start,
                    reader.position,
                ));
            }
        };
        if magnitude == Some(0) {
            return Err(reader.fail(ChemistryProblem::ZeroCharge, start, reader.position));
        }
        return i64::try_from(magnitude.unwrap_or(1))
            .map(|magnitude| sign * magnitude)
            .map_err(|_| {
                reader.fail(
                    ChemistryProblem::UnexpectedCharacter,
                    start,
                    reader.position,
                )
            });
    }
    if character == '+' || character == '-' {
        reader.advance();
        if reader.peek().is_some_and(|c| c.is_ascii_digit()) {
            return Err(reader.fail(
                ChemistryProblem::SignBeforeNumber,
                start,
                reader.position + 1,
            ));
        }
        let state_follows = STATES.contains(&reader.rest());
        if !state_follows
            && reader
                .peek()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '(' || c == '[')
        {
            return Err(reader.fail(ChemistryProblem::SumWithoutSpaces, start, reader.position));
        }
        if after_digit {
            return Err(reader.fail(ChemistryProblem::AmbiguousCharge, start, reader.position));
        }
        return Ok(if character == '+' { 1 } else { -1 });
    }
    Ok(0)
}

fn species(
    text: &str,
    offset: usize,
    allow_coefficient: bool,
) -> Result<ParsedSpecies, ParseError> {
    let mut reader = Reader {
        text,
        position: 0,
        offset,
    };
    if text.is_empty() {
        return Err(reader.fail(ChemistryProblem::EmptySide, 0, 0));
    }
    let mut coefficient = None;
    if reader.peek().is_some_and(|c| c.is_ascii_digit()) {
        let start = reader.position;
        let value = match reader.count() {
            Err(error)
                if reader.peek() == Some(' ')
                    && error.kind == ParseErrorKind::Chemistry(ChemistryProblem::ZeroCount) =>
            {
                return Err(reader.fail(ChemistryProblem::ZeroCoefficient, start, reader.position));
            }
            read => read?,
        };
        match reader.peek() {
            Some(' ') if allow_coefficient => {
                reader.advance();
                coefficient = value;
            }
            Some(' ') => {
                return Err(reader.fail(
                    ChemistryProblem::CoefficientWithoutReaction,
                    start,
                    reader.position,
                ));
            }
            Some('/' | '.') => {
                return Err(reader.fail(
                    ChemistryProblem::FractionalCoefficient,
                    start,
                    reader.position + 1,
                ));
            }
            _ => return Err(reader.fail(ChemistryProblem::AttachedNumber, start, text.len())),
        }
    }
    let body_start = reader.position;
    let body = reader.rest();
    if body.starts_with(CHARGE_MARK)
        || body.starts_with(|c| superscript_value(c).is_some())
        || NEUTRINO_NAMES.iter().any(|name| body.starts_with(name))
        || POSITRON_SPELLINGS.contains(&body)
    {
        return Err(reader.fail(ChemistryProblem::Nuclide, body_start, text.len()));
    }
    let at = offset + body_start..offset + text.len();
    if body == "e-" || body == ELECTRON_WITH_CARET {
        return Ok(ParsedSpecies {
            text: body.to_string(),
            elements: Vec::new(),
            charge: -1,
            coefficient,
            is_product: false,
            at,
        });
    }
    if body.starts_with(ELECTRON) {
        return Err(reader.fail(ChemistryProblem::UnknownElement, body_start, body_start + 1));
    }
    let mut elements = group(&mut reader, None)?;
    loop {
        match reader.peek() {
            Some(dot) if HYDRATE_DOTS.contains(&dot) => {
                let dot_at = reader.position;
                if elements.is_empty() {
                    return Err(reader.fail_here(ChemistryProblem::DanglingHydrateDot));
                }
                reader.advance();
                let multiplier = reader.count()?.unwrap_or(1);
                let part = group(&mut reader, None)?;
                if part.is_empty() {
                    return Err(reader.fail(
                        ChemistryProblem::DanglingHydrateDot,
                        dot_at,
                        dot_at + dot.len_utf8(),
                    ));
                }
                add_counts(&mut elements, &part, multiplier).ok_or_else(|| {
                    reader.fail(ChemistryProblem::CountTooLarge, dot_at, reader.position)
                })?;
            }
            Some('.') => return Err(reader.fail_here(ChemistryProblem::DotSeparator)),
            _ => break,
        }
    }
    if elements.is_empty() {
        return Err(reader.fail_here(ChemistryProblem::UnexpectedCharacter));
    }
    let after_digit = reader.position > 0
        && reader.text[..reader.position]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_digit() || subscript_value(c).is_some());
    let charge = charge(&mut reader, after_digit).map_err(|error| match error.kind {
        ParseErrorKind::Chemistry(problem) => reader.fail(problem, body_start, text.len()),
        _ => error,
    })?;
    if STATES.contains(&reader.rest()) {
        reader.position = text.len();
    }
    if reader.position != text.len() {
        let rest = reader.rest();
        let skipped = rest.len() - rest.trim_start().len();
        return Err(reader.fail(
            ChemistryProblem::UnexpectedCharacter,
            reader.position + skipped,
            text.len(),
        ));
    }
    Ok(ParsedSpecies {
        text: text[body_start..].to_string(),
        elements,
        charge,
        coefficient,
        is_product: false,
        at,
    })
}

fn side(text: &str, offset: usize, is_product: bool) -> Result<Vec<ParsedSpecies>, ParseError> {
    let mut found = Vec::new();
    let mut start = 0;
    let parts: Vec<&str> = text.split(SPECIES_SEPARATOR).collect();
    for part in parts {
        let trimmed_start = part.len() - part.trim_start().len();
        let species_text = part.trim();
        let mut parsed = species(species_text, offset + start + trimmed_start, true)?;
        parsed.is_product = is_product;
        found.push(parsed);
        start += part.len() + SPECIES_SEPARATOR.len();
    }
    Ok(found)
}

pub(crate) fn find_any(text: &str, needles: &[&str]) -> Option<(usize, usize)> {
    needles
        .iter()
        .filter_map(|needle| text.find(needle).map(|at| (at, needle.len())))
        .min_by_key(|(at, length)| (*at, std::cmp::Reverse(*length)))
}

pub(crate) fn parse_chemistry(
    content: &str,
    span: &Range<usize>,
) -> Result<ParsedChemistry, ParseError> {
    let offset = span.start + CHEMISTRY_PREFIX_LENGTH;
    let fail = |problem: ChemistryProblem, from: usize, width: usize| {
        ParseError::new(
            ParseErrorKind::Chemistry(problem),
            offset + from..offset + from + width.max(1),
        )
    };
    if content.trim().is_empty() {
        return Err(fail(ChemistryProblem::Empty, 0, 0));
    }
    if let Some((at, width)) = find_any(content, &RESONANCE_ARROWS) {
        return Err(fail(ChemistryProblem::Resonance, at, width));
    }
    if let Some((at, width)) = find_any(content, &EQUILIBRIUM_ARROWS) {
        return Err(fail(ChemistryProblem::Equilibrium, at, width));
    }
    let Some((at, width)) = find_any(content, &REACTION_ARROWS) else {
        if let Some((at, width)) = find_any(content, &WRONG_ARROWS) {
            return Err(fail(ChemistryProblem::WrongArrow, at, width));
        }
        if let Some(at) = content.find(SPECIES_SEPARATOR) {
            return Err(fail(ChemistryProblem::NoArrow, at, SPECIES_SEPARATOR.len()));
        }
        return species(content, offset, false).map(ParsedChemistry::Substance);
    };
    let (left, right) = (&content[..at], &content[at + width..]);
    if let Some((second, second_width)) = find_any(right, &REACTION_ARROWS) {
        return Err(fail(
            ChemistryProblem::TwoArrows,
            at + width + second,
            second_width,
        ));
    }
    let left_trimmed = left.trim_end();
    let right_start = right.len() - right.trim_start().len();
    if left_trimmed.trim().is_empty() || right.trim().is_empty() {
        return Err(fail(ChemistryProblem::EmptySide, at, width));
    }
    let mut species = side(left_trimmed, offset, false)?;
    species.extend(side(right.trim(), offset + at + width + right_start, true)?);
    for (index, one) in species.iter().enumerate() {
        if species[..index]
            .iter()
            .any(|earlier| earlier.text == one.text)
        {
            return Err(ParseError::new(
                ParseErrorKind::Chemistry(ChemistryProblem::RepeatedSpecies),
                one.at.clone(),
            ));
        }
    }
    Ok(ParsedChemistry::Reaction(species))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(content: &str) -> Result<ParsedChemistry, ChemistryProblem> {
        parse_chemistry(content, &(0..content.len() + 6)).map_err(|error| match error.kind {
            ParseErrorKind::Chemistry(problem) => problem,
            other => panic!("not a chemistry problem: {other:?}"),
        })
    }

    fn substance(content: &str) -> (Vec<(u8, u64)>, i64) {
        match parsed(content) {
            Ok(ParsedChemistry::Substance(species)) => {
                let mut elements = species.elements;
                elements.sort_unstable();
                (elements, species.charge)
            }
            other => panic!("not a substance: {other:?}"),
        }
    }

    #[test]
    fn water_is_two_hydrogen_and_one_oxygen() {
        assert_eq!(substance("H2O"), (vec![(1, 2), (8, 1)], 0));
    }

    #[test]
    fn nested_brackets_and_a_hydrate_multiply_their_counts() {
        assert_eq!(substance("Ca(OH)2"), (vec![(1, 2), (8, 2), (20, 1)], 0));
        assert_eq!(
            substance("CuSO4·5H2O"),
            (vec![(1, 10), (8, 9), (16, 1), (29, 1)], 0)
        );
        assert_eq!(
            substance("K4[Fe(CN)6]"),
            (vec![(6, 6), (7, 6), (19, 4), (26, 1)], 0)
        );
    }

    #[test]
    fn a_charge_after_a_caret_or_a_bare_sign_is_read() {
        assert_eq!(substance("SO4^2-").1, -2);
        assert_eq!(substance("NH4^+").1, 1);
        assert_eq!(substance("Na+").1, 1);
        assert_eq!(substance("OH-").1, -1);
        assert_eq!(substance("SO₄²⁻"), (vec![(8, 4), (16, 1)], -2));
    }

    #[test]
    fn a_state_after_a_species_changes_nothing() {
        assert_eq!(substance("H2O(l)"), substance("H2O"));
    }

    #[test]
    fn a_sign_right_after_a_count_is_refused_as_ambiguous() {
        assert_eq!(parsed("NH4+"), Err(ChemistryProblem::AmbiguousCharge));
        assert_eq!(parsed("Fe3+"), Err(ChemistryProblem::AmbiguousCharge));
    }

    #[test]
    fn each_other_ambiguous_spelling_is_refused_by_name() {
        assert_eq!(parsed("Fe+3"), Err(ChemistryProblem::SignBeforeNumber));
        assert_eq!(parsed("14C"), Err(ChemistryProblem::AttachedNumber));
        assert_eq!(parsed("2H2O"), Err(ChemistryProblem::AttachedNumber));
        assert_eq!(parsed("Fe(III)"), Err(ChemistryProblem::OxidationState));
        assert_eq!(parsed("H2+O2"), Err(ChemistryProblem::SumWithoutSpaces));
        assert_eq!(parsed("CuSO4.5H2O"), Err(ChemistryProblem::DecimalCount));
        assert_eq!(parsed("H2O.CuSO4"), Err(ChemistryProblem::DotSeparator));
        assert_eq!(parsed("co"), Err(ChemistryProblem::WrongCase));
        assert_eq!(parsed("Xx"), Err(ChemistryProblem::UnknownElement));
        assert_eq!(parsed("A <-> B"), Err(ChemistryProblem::Resonance));
    }

    #[test]
    fn nuclides_and_other_forms_not_read_yet_are_refused() {
        assert_eq!(parsed("^4He"), Err(ChemistryProblem::Nuclide));
        assert_eq!(parsed("⁴He"), Err(ChemistryProblem::Nuclide));
        assert_eq!(parsed("e+"), Err(ChemistryProblem::Nuclide));
        assert_eq!(parsed("He-4"), Err(ChemistryProblem::SignBeforeNumber));
        assert_eq!(parsed("N2 <=> 2 N"), Err(ChemistryProblem::Equilibrium));
        assert_eq!(
            parsed("H2 + 1/2 O2 -> H2O"),
            Err(ChemistryProblem::FractionalCoefficient)
        );
        assert_eq!(parsed("Fe0.95O"), Err(ChemistryProblem::DecimalCount));
    }

    #[test]
    fn a_reaction_reads_both_sides_with_coefficients_and_the_electron() {
        let Ok(ParsedChemistry::Reaction(species)) = parsed("2 H2 + O2 -> 2 H2O") else {
            panic!("not a reaction");
        };
        let coefficients: Vec<Option<u64>> = species.iter().map(|one| one.coefficient).collect();
        let sides: Vec<bool> = species.iter().map(|one| one.is_product).collect();
        assert_eq!(coefficients, vec![Some(2), None, Some(2)]);
        assert_eq!(sides, vec![false, false, true]);
        let Ok(ParsedChemistry::Reaction(half)) = parsed("Fe^3+ + e- -> Fe^2+") else {
            panic!("not a reaction");
        };
        assert_eq!(half[1].charge, -1);
        assert!(half[1].elements.is_empty());
    }

    #[test]
    fn a_coefficient_without_a_reaction_and_two_arrows_are_refused() {
        assert_eq!(
            parsed("2 H2O"),
            Err(ChemistryProblem::CoefficientWithoutReaction)
        );
        assert_eq!(parsed("A -> B -> C"), Err(ChemistryProblem::TwoArrows));
        assert_eq!(parsed("-> H2O"), Err(ChemistryProblem::EmptySide));
    }

    #[test]
    fn a_state_after_a_bare_charge_is_read() {
        assert_eq!(substance("Na+(aq)"), (vec![(11, 1)], 1));
    }

    #[test]
    fn an_arrow_that_is_not_one_and_a_sum_without_an_arrow_are_refused() {
        assert_eq!(parsed("H2 + O2 => H2O"), Err(ChemistryProblem::WrongArrow));
        assert_eq!(parsed("H2 + O2 = H2O"), Err(ChemistryProblem::WrongArrow));
        assert_eq!(parsed("H2 + O2"), Err(ChemistryProblem::NoArrow));
    }

    #[test]
    fn a_species_written_twice_is_refused() {
        assert_eq!(
            parsed("H2 + H2 -> H4"),
            Err(ChemistryProblem::RepeatedSpecies)
        );
    }

    #[test]
    fn a_count_or_coefficient_of_zero_is_refused() {
        assert_eq!(parsed("H02"), Err(ChemistryProblem::ZeroCount));
        assert_eq!(parsed("H0"), Err(ChemistryProblem::ZeroCount));
    }

    #[test]
    fn a_coefficient_of_zero_is_refused_as_a_coefficient() {
        assert_eq!(
            parsed("0 H2 + O2 -> O2"),
            Err(ChemistryProblem::ZeroCoefficient)
        );
        assert_eq!(
            parsed("02 H2 + O2 -> 2 H2O"),
            Err(ChemistryProblem::ZeroCoefficient)
        );
    }

    #[test]
    fn a_count_above_the_limit_is_refused_and_not_read_as_unwritten() {
        let too_large = Err(ChemistryProblem::CountTooLarge);
        assert_eq!(parsed("H99999999999999999999999"), too_large);
        assert_eq!(parsed("99999999999999999999999 H2 + O2 -> H2O"), too_large);
        assert_eq!(
            parsed(
                "H\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}\u{2089}"
            ),
            too_large
        );
        assert_eq!(parsed("Fe^99999999999999999999+"), too_large);
    }

    #[test]
    fn a_count_at_the_limit_is_read() {
        assert!(parsed("H9223372036854775807").is_ok());
        assert_eq!(
            parsed("H9223372036854775808"),
            Err(ChemistryProblem::CountTooLarge)
        );
    }

    #[test]
    fn a_product_or_sum_of_counts_above_the_limit_is_refused() {
        let too_large = Err(ChemistryProblem::CountTooLarge);
        assert_eq!(parsed("(H4294967296)4294967296"), too_large);
        assert_eq!(
            parsed("H4611686018427387904H4611686018427387904"),
            too_large
        );
        assert_eq!(parsed("Cu*4294967296H4294967296"), too_large);
    }

    #[test]
    fn a_charge_of_zero_written_with_a_sign_is_refused() {
        assert_eq!(parsed("Fe^0+"), Err(ChemistryProblem::ZeroCharge));
        assert_eq!(
            parsed("Fe\u{2070}\u{207a}"),
            Err(ChemistryProblem::ZeroCharge)
        );
    }

    #[test]
    fn deuterium_and_tritium_are_refused_as_isotopes() {
        assert_eq!(parsed("D2O"), Err(ChemistryProblem::Isotope));
        assert_eq!(parsed("T2"), Err(ChemistryProblem::Isotope));
    }

    #[test]
    fn capitals_that_spell_an_element_are_refused_as_wrong_case() {
        assert_eq!(parsed("NACL"), Err(ChemistryProblem::WrongCase));
    }

    #[test]
    fn a_hydrate_dot_without_a_constituent_is_refused() {
        assert_eq!(parsed("NaCl·"), Err(ChemistryProblem::DanglingHydrateDot));
        assert_eq!(parsed("·H2O"), Err(ChemistryProblem::DanglingHydrateDot));
    }

    #[test]
    fn a_caret_sign_before_a_number_and_the_caret_electron_are_read_rightly() {
        assert_eq!(parsed("Fe^+3"), Err(ChemistryProblem::SignBeforeNumber));
        assert_eq!(parsed("CO3^-2"), Err(ChemistryProblem::SignBeforeNumber));
        let Ok(ParsedChemistry::Reaction(half)) = parsed("Fe^3+ + e^- -> Fe^2+") else {
            panic!("not a reaction");
        };
        assert_eq!(half[1].charge, -1);
    }
}
