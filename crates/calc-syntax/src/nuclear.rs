use std::ops::Range;

use calc_expr::NuclearParticle;

use crate::chemistry::{atomic_number, element_symbol, find_any};
use crate::error::{ParseError, ParseErrorKind};

pub(crate) const NUCLEAR_PREFIX_LENGTH: usize = "nuc'".len();

const REACTION_ARROWS: [&str; 2] = ["->", "\u{2192}"];
const RESONANCE_ARROWS: [&str; 2] = ["<->", "\u{2194}"];
const EQUILIBRIUM_ARROWS: [&str; 2] = ["<=>", "\u{21CC}"];
const WRONG_ARROWS: [&str; 2] = ["=>", "="];
const SPECIES_SEPARATOR: &str = " + ";
const MASS_MARK: char = '^';
const NUMBER_MARK: char = '_';
const HYPHEN: char = '-';
const ISOMER_MARK: char = 'm';
const COMPACT_OPENING: char = '(';
const SUBSCRIPT_ZERO: u32 = 0x2080;
const SUBSCRIPT_PLUS: char = '\u{208A}';
const SUBSCRIPT_MINUS: char = '\u{208B}';
const SUPERSCRIPT_DIGITS: [char; 10] = [
    '\u{2070}', '\u{00B9}', '\u{00B2}', '\u{00B3}', '\u{2074}', '\u{2075}', '\u{2076}', '\u{2077}',
    '\u{2078}', '\u{2079}',
];
const SUPERSCRIPT_ISOMER: char = '\u{1D50}';
const CHARGE_MARKS: [char; 5] = ['^', '+', '-', '\u{207A}', '\u{207B}'];
const NUMBER_LIMIT: u64 = i64::MAX.unsigned_abs();

const NEUTRON: NuclearParticle = NuclearParticle::Nuclide {
    mass_number: 1,
    atomic_number: 0,
};
const PROTON: NuclearParticle = NuclearParticle::Nuclide {
    mass_number: 1,
    atomic_number: 1,
};
const DEUTERON: NuclearParticle = NuclearParticle::Nuclide {
    mass_number: 2,
    atomic_number: 1,
};
const TRITON: NuclearParticle = NuclearParticle::Nuclide {
    mass_number: 3,
    atomic_number: 1,
};
const ALPHA: NuclearParticle = NuclearParticle::Nuclide {
    mass_number: 4,
    atomic_number: 2,
};

const PARTICLES: [(&str, NuclearParticle, &str); 22] = [
    ("n", NEUTRON, "n"),
    ("p", PROTON, "p"),
    ("d", DEUTERON, "d"),
    ("t", TRITON, "t"),
    ("\u{03B1}", ALPHA, "\u{03B1}"),
    ("alpha", ALPHA, "\u{03B1}"),
    ("\u{03B3}", NuclearParticle::Photon, "\u{03B3}"),
    ("gamma", NuclearParticle::Photon, "\u{03B3}"),
    ("e-", NuclearParticle::Electron, "e-"),
    ("e\u{207B}", NuclearParticle::Electron, "e-"),
    ("\u{03B2}-", NuclearParticle::Electron, "e-"),
    ("\u{03B2}\u{207B}", NuclearParticle::Electron, "e-"),
    ("e+", NuclearParticle::Positron, "e+"),
    ("e\u{207A}", NuclearParticle::Positron, "e+"),
    ("\u{03B2}+", NuclearParticle::Positron, "e+"),
    ("\u{03B2}\u{207A}", NuclearParticle::Positron, "e+"),
    (
        "\u{03BD}_e",
        NuclearParticle::ElectronNeutrino,
        "\u{03BD}_e",
    ),
    (
        "\u{03BD}\u{2091}",
        NuclearParticle::ElectronNeutrino,
        "\u{03BD}_e",
    ),
    ("nu_e", NuclearParticle::ElectronNeutrino, "\u{03BD}_e"),
    (
        "\u{03BD}\u{0304}_e",
        NuclearParticle::ElectronAntineutrino,
        "\u{03BD}\u{0304}_e",
    ),
    (
        "\u{03BD}\u{0304}\u{2091}",
        NuclearParticle::ElectronAntineutrino,
        "\u{03BD}\u{0304}_e",
    ),
    (
        "anti_nu_e",
        NuclearParticle::ElectronAntineutrino,
        "\u{03BD}\u{0304}_e",
    ),
];
const ATOM_SYMBOLS: [&str; 2] = ["D", "T"];
const SCHOOL_LETTERS: [&str; 3] = ["h", "g", "a"];
const UNSIGNED_ELECTRONS: [&str; 2] = ["e", "\u{03B2}"];
const UNNAMED_NEUTRINOS: [&str; 6] = [
    "\u{03BD}",
    "nu",
    "nubar",
    "\u{03BD}\u{0304}",
    "anti_nu",
    "antinu",
];
const OTHER_LEPTON_STARTS: [&str; 6] = ["\u{03BC}", "mu", "\u{03C4}", "tau", "anti_nu_", "nu_"];
const OTHER_NEUTRINO_STARTS: [&str; 2] = ["\u{03BD}_", "\u{03BD}\u{0304}_"];
const PARTICLES_OR_ELEMENTS: [&str; 2] = ["N", "P"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NuclearProblem {
    Empty,
    UnknownSpecies,
    AttachedNumber,
    AtomSymbol,
    BareElement,
    ParticleOrElement,
    SchoolLetter,
    UnsignedElectron,
    NeutrinoFlavour,
    Charge,
    MassBelowCharge,
    NumbersDisagree,
    AtomicNumberDisagrees { written: i64, symbol: u32 },
    Isomer,
    OtherLepton,
    CompactForm,
    ElementName,
    Resonance,
    Equilibrium,
    WrongArrow,
    NoArrow,
    TwoArrows,
    EmptySide,
    ZeroCoefficient,
    FractionalCoefficient,
    CoefficientWithoutReaction,
    NumberTooLarge,
    CoefficientOutsideLiteral,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParsedParticle {
    pub(crate) text: String,
    pub(crate) particle: NuclearParticle,
    pub(crate) coefficient: Option<u64>,
    pub(crate) is_product: bool,
    pub(crate) at: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParsedNuclear {
    Nuclide(ParsedParticle),
    Reaction(Vec<ParsedParticle>),
}

fn problem(kind: NuclearProblem, at: Range<usize>) -> ParseError {
    let end = at.end.max(at.start + 1);
    ParseError::new(ParseErrorKind::Nuclear(kind), at.start..end)
}

fn whole_number(digits: &str) -> Result<u64, NuclearProblem> {
    let mut value: u64 = 0;
    for digit in digits.chars() {
        let digit = digit
            .to_digit(10)
            .map(u64::from)
            .ok_or(NuclearProblem::UnknownSpecies)?;
        value = value
            .checked_mul(10)
            .and_then(|shifted| shifted.checked_add(digit))
            .filter(|next| *next <= NUMBER_LIMIT)
            .ok_or(NuclearProblem::NumberTooLarge)?;
    }
    Ok(value)
}

fn superscript_value(character: char) -> Option<char> {
    SUPERSCRIPT_DIGITS
        .iter()
        .position(|digit| *digit == character)
        .and_then(|position| char::from_digit(u32::try_from(position).ok()?, 10))
}

fn subscript_value(character: char) -> Option<char> {
    let code = u32::from(character);
    (SUBSCRIPT_ZERO..SUBSCRIPT_ZERO + 10)
        .contains(&code)
        .then(|| char::from_digit(code - SUBSCRIPT_ZERO, 10))
        .flatten()
}

struct Marked<'text> {
    mass_number: String,
    atomic_number: Option<(bool, String)>,
    is_isomer: bool,
    symbol: &'text str,
}

fn split_ascii_marks(body: &str) -> Marked<'_> {
    let after_mark = &body[MASS_MARK.len_utf8()..];
    let digits_end = after_mark
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after_mark.len());
    let mass_number = after_mark[..digits_end].to_string();
    let mut rest = &after_mark[digits_end..];
    let mut is_isomer = false;
    if let Some(after_isomer) = rest.strip_prefix(ISOMER_MARK)
        && after_isomer
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        is_isomer = true;
        rest = after_isomer.trim_start_matches(|c: char| c.is_ascii_digit());
    }
    let mut atomic_number = None;
    if let Some(after_number_mark) = rest.strip_prefix(NUMBER_MARK) {
        let (is_negative, unsigned) = match after_number_mark.chars().next() {
            Some('-') => (true, &after_number_mark[1..]),
            Some('+') => (false, &after_number_mark[1..]),
            _ => (false, after_number_mark),
        };
        let end = unsigned
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(unsigned.len());
        atomic_number = Some((is_negative, unsigned[..end].to_string()));
        rest = &unsigned[end..];
    }
    Marked {
        mass_number,
        atomic_number,
        is_isomer,
        symbol: rest,
    }
}

fn split_raised_marks(body: &str) -> Marked<'_> {
    let mut mass_number = String::new();
    let mut position = 0;
    for character in body.chars() {
        match superscript_value(character) {
            Some(digit) => {
                mass_number.push(digit);
                position += character.len_utf8();
            }
            None => break,
        }
    }
    let mut rest = &body[position..];
    let mut is_isomer = false;
    if let Some(after_isomer) = rest.strip_prefix(SUPERSCRIPT_ISOMER) {
        is_isomer = true;
        rest = after_isomer;
    }
    let mut atomic_number = None;
    let (is_negative, signed_rest) = match rest.chars().next() {
        Some(SUBSCRIPT_MINUS) => (true, &rest[SUBSCRIPT_MINUS.len_utf8()..]),
        Some(SUBSCRIPT_PLUS) => (false, &rest[SUBSCRIPT_PLUS.len_utf8()..]),
        _ => (false, rest),
    };
    let mut digits = String::new();
    let mut consumed = 0;
    for character in signed_rest.chars() {
        match subscript_value(character) {
            Some(digit) => {
                digits.push(digit);
                consumed += character.len_utf8();
            }
            None => break,
        }
    }
    if !digits.is_empty() {
        atomic_number = Some((is_negative, digits));
        rest = &signed_rest[consumed..];
    }
    Marked {
        mass_number,
        atomic_number,
        is_isomer,
        symbol: rest,
    }
}

fn leading_symbol(text: &str) -> Option<(u32, &str)> {
    let mut characters = text.chars();
    let first = characters.next().filter(char::is_ascii_uppercase)?;
    if let Some(second) = characters.next().filter(char::is_ascii_lowercase) {
        let two = format!("{first}{second}");
        if let Some(found) = atomic_number(&two) {
            return Some((u32::from(found), &text[two.len()..]));
        }
    }
    let found = atomic_number(&first.to_string())?;
    Some((u32::from(found), &text[first.len_utf8()..]))
}

fn nuclide(
    mass_number: u64,
    atomic_number: u32,
) -> Result<(NuclearParticle, String), NuclearProblem> {
    let mass_number = u32::try_from(mass_number).map_err(|_| NuclearProblem::NumberTooLarge)?;
    if mass_number < atomic_number || mass_number == 0 {
        return Err(NuclearProblem::MassBelowCharge);
    }
    let symbol = u8::try_from(atomic_number)
        .ok()
        .and_then(element_symbol)
        .ok_or(NuclearProblem::UnknownSpecies)?;
    Ok((
        NuclearParticle::Nuclide {
            mass_number,
            atomic_number,
        },
        format!("{MASS_MARK}{mass_number}{symbol}"),
    ))
}

fn marked(marks: &Marked<'_>) -> Result<(NuclearParticle, String), NuclearProblem> {
    if marks.mass_number.is_empty() {
        return Err(NuclearProblem::UnknownSpecies);
    }
    if marks.is_isomer {
        return Err(NuclearProblem::Isomer);
    }
    let mass_number = whole_number(&marks.mass_number)?;
    let written_number = match &marks.atomic_number {
        Some((is_negative, digits)) => {
            let magnitude =
                i64::try_from(whole_number(digits)?).map_err(|_| NuclearProblem::NumberTooLarge)?;
            Some(if *is_negative { -magnitude } else { magnitude })
        }
        None => None,
    };
    let particle = match marks.symbol {
        "e" | "\u{03B2}" => match written_number {
            Some(-1) if mass_number == 0 => Some((NuclearParticle::Electron, "e-")),
            Some(1) if mass_number == 0 => Some((NuclearParticle::Positron, "e+")),
            None if mass_number == 0 => return Err(NuclearProblem::UnsignedElectron),
            _ => return Err(NuclearProblem::NumbersDisagree),
        },
        "n" => Some((NEUTRON, "n")),
        "p" => Some((PROTON, "p")),
        _ => None,
    };
    if let Some((particle, text)) = particle {
        let agrees = mass_number == u64::try_from(particle.nucleons()).unwrap_or(u64::MAX)
            && written_number.is_none_or(|number| number == particle.charge());
        if !agrees {
            return Err(NuclearProblem::NumbersDisagree);
        }
        return Ok((particle, text.to_string()));
    }
    let Some((atomic_number, after)) = leading_symbol(marks.symbol) else {
        return Err(NuclearProblem::UnknownSpecies);
    };
    if !after.is_empty() {
        return Err(
            if after.starts_with(CHARGE_MARKS) || after.starts_with(|c: char| c.is_ascii_digit()) {
                NuclearProblem::Charge
            } else if after.starts_with(COMPACT_OPENING) {
                NuclearProblem::CompactForm
            } else {
                NuclearProblem::UnknownSpecies
            },
        );
    }
    if let Some(written) = written_number.filter(|number| *number != i64::from(atomic_number)) {
        return Err(NuclearProblem::AtomicNumberDisagrees {
            written,
            symbol: atomic_number,
        });
    }
    nuclide(mass_number, atomic_number)
}

fn hyphenated(body: &str) -> Option<Result<(NuclearParticle, String), NuclearProblem>> {
    let (name, number) = body.split_once(HYPHEN)?;
    if number.is_empty() || !number.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    if name.len() > 2 && name.chars().all(|c| c.is_ascii_lowercase()) {
        return Some(Err(NuclearProblem::ElementName));
    }
    let (atomic_number, after) = leading_symbol(name)?;
    if !after.is_empty() {
        return None;
    }
    let digits_end = number
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(number.len());
    let rest = &number[digits_end..];
    if rest.starts_with(ISOMER_MARK) {
        return Some(Err(NuclearProblem::Isomer));
    }
    if !rest.is_empty() {
        return Some(Err(NuclearProblem::Charge));
    }
    Some(whole_number(&number[..digits_end]).and_then(|mass| nuclide(mass, atomic_number)))
}

fn species_of(body: &str) -> Result<(NuclearParticle, String), NuclearProblem> {
    if let Some((_, particle, text)) = PARTICLES.iter().find(|(written, ..)| *written == body) {
        return Ok((*particle, (*text).to_string()));
    }
    if ATOM_SYMBOLS.contains(&body) {
        return Err(NuclearProblem::AtomSymbol);
    }
    if SCHOOL_LETTERS.contains(&body) {
        return Err(NuclearProblem::SchoolLetter);
    }
    if UNSIGNED_ELECTRONS.contains(&body) {
        return Err(NuclearProblem::UnsignedElectron);
    }
    if UNNAMED_NEUTRINOS.contains(&body) {
        return Err(NuclearProblem::NeutrinoFlavour);
    }
    if OTHER_LEPTON_STARTS
        .iter()
        .chain(OTHER_NEUTRINO_STARTS.iter())
        .any(|start| body.starts_with(start))
    {
        return Err(NuclearProblem::OtherLepton);
    }
    if PARTICLES_OR_ELEMENTS.contains(&body) {
        return Err(NuclearProblem::ParticleOrElement);
    }
    if body.starts_with(MASS_MARK) {
        return marked(&split_ascii_marks(body));
    }
    if body.starts_with(|c| superscript_value(c).is_some()) {
        return marked(&split_raised_marks(body));
    }
    if body.starts_with(|c: char| c.is_ascii_digit()) {
        return Err(NuclearProblem::AttachedNumber);
    }
    if let Some(found) = hyphenated(body) {
        return found;
    }
    if body.contains(COMPACT_OPENING) {
        return Err(NuclearProblem::CompactForm);
    }
    match leading_symbol(body) {
        Some((_, "")) => Err(NuclearProblem::BareElement),
        Some(_) => Err(NuclearProblem::Charge),
        None => Err(NuclearProblem::UnknownSpecies),
    }
}

fn species(
    text: &str,
    offset: usize,
    allow_coefficient: bool,
    is_product: bool,
) -> Result<ParsedParticle, ParseError> {
    let whole = offset..offset + text.len();
    if text.is_empty() {
        return Err(problem(NuclearProblem::EmptySide, whole));
    }
    let mut coefficient = None;
    let mut body_start = 0;
    if text.starts_with(|c: char| c.is_ascii_digit()) {
        let digits_end = text
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(text.len());
        let digits = &text[..digits_end];
        let digits_at = offset..offset + digits_end;
        match text[digits_end..].chars().next() {
            Some(' ') => {
                if !allow_coefficient {
                    return Err(problem(
                        NuclearProblem::CoefficientWithoutReaction,
                        digits_at,
                    ));
                }
                let value =
                    whole_number(digits).map_err(|kind| problem(kind, digits_at.clone()))?;
                if value == 0 || digits.starts_with('0') {
                    return Err(problem(NuclearProblem::ZeroCoefficient, digits_at));
                }
                coefficient = Some(value);
                body_start = digits_end + 1;
            }
            Some('/' | '.') => {
                return Err(problem(NuclearProblem::FractionalCoefficient, digits_at));
            }
            _ => return Err(problem(NuclearProblem::AttachedNumber, whole)),
        }
    }
    let body = &text[body_start..];
    let at = offset + body_start..offset + text.len();
    let (particle, canonical) = species_of(body).map_err(|kind| problem(kind, at.clone()))?;
    Ok(ParsedParticle {
        text: canonical,
        particle,
        coefficient,
        is_product,
        at,
    })
}

fn side(text: &str, offset: usize, is_product: bool) -> Result<Vec<ParsedParticle>, ParseError> {
    let mut found = Vec::new();
    let mut start = 0;
    for part in text.split(SPECIES_SEPARATOR) {
        let trimmed_start = part.len() - part.trim_start().len();
        found.push(species(
            part.trim(),
            offset + start + trimmed_start,
            true,
            is_product,
        )?);
        start += part.len() + SPECIES_SEPARATOR.len();
    }
    Ok(found)
}

pub(crate) fn parse_nuclear(
    content: &str,
    span: &Range<usize>,
) -> Result<ParsedNuclear, ParseError> {
    let offset = span.start + NUCLEAR_PREFIX_LENGTH;
    let at = |from: usize, width: usize| offset + from..offset + from + width;
    if content.trim().is_empty() {
        return Err(problem(NuclearProblem::Empty, at(0, 0)));
    }
    if let Some((found, width)) = find_any(content, &RESONANCE_ARROWS) {
        return Err(problem(NuclearProblem::Resonance, at(found, width)));
    }
    if let Some((found, width)) = find_any(content, &EQUILIBRIUM_ARROWS) {
        return Err(problem(NuclearProblem::Equilibrium, at(found, width)));
    }
    let Some((arrow, width)) = find_any(content, &REACTION_ARROWS) else {
        if let Some((found, wrong_width)) = find_any(content, &WRONG_ARROWS) {
            return Err(problem(NuclearProblem::WrongArrow, at(found, wrong_width)));
        }
        if let Some(found) = content.find(SPECIES_SEPARATOR) {
            return Err(problem(
                NuclearProblem::NoArrow,
                at(found, SPECIES_SEPARATOR.len()),
            ));
        }
        return species(content, offset, false, false).map(ParsedNuclear::Nuclide);
    };
    let (left, right) = (&content[..arrow], &content[arrow + width..]);
    if let Some((second, second_width)) = find_any(right, &REACTION_ARROWS) {
        return Err(problem(
            NuclearProblem::TwoArrows,
            at(arrow + width + second, second_width),
        ));
    }
    let right_start = right.len() - right.trim_start().len();
    if left.trim().is_empty() || right.trim().is_empty() {
        return Err(problem(NuclearProblem::EmptySide, at(arrow, width)));
    }
    let mut all = side(left.trim_end(), offset, false)?;
    all.extend(side(
        right.trim(),
        offset + arrow + width + right_start,
        true,
    )?);
    Ok(ParsedNuclear::Reaction(all))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(content: &str) -> Result<ParsedNuclear, NuclearProblem> {
        parse_nuclear(content, &(0..content.len() + 5)).map_err(|error| match error.kind {
            ParseErrorKind::Nuclear(problem) => problem,
            other => panic!("not a nuclear problem: {other:?}"),
        })
    }

    fn one(content: &str) -> Result<(NuclearParticle, String), NuclearProblem> {
        match parsed(content)? {
            ParsedNuclear::Nuclide(found) => Ok((found.particle, found.text)),
            other => panic!("not one species: {other:?}"),
        }
    }

    fn nuclide_of(mass_number: u32, atomic_number: u32) -> NuclearParticle {
        NuclearParticle::Nuclide {
            mass_number,
            atomic_number,
        }
    }

    #[test]
    fn carbon_fourteen_reads_the_same_in_every_notation() {
        for written in ["^14C", "¹⁴C", "C-14", "^14_6C", "¹⁴₆C"] {
            assert_eq!(one(written), Ok((nuclide_of(14, 6), "^14C".to_string())));
        }
    }

    #[test]
    fn named_particles_are_nuclei_or_leptons() {
        assert_eq!(one("p").map(|found| found.0), Ok(nuclide_of(1, 1)));
        assert_eq!(one("α").map(|found| found.0), Ok(nuclide_of(4, 2)));
        assert_eq!(one("n").map(|found| found.0), Ok(nuclide_of(1, 0)));
        assert_eq!(
            one("β+").map(|found| found.0),
            Ok(NuclearParticle::Positron)
        );
        assert_eq!(
            one("anti_nu_e").map(|found| found.0),
            Ok(NuclearParticle::ElectronAntineutrino)
        );
        assert_eq!(
            one("gamma").map(|found| found.0),
            Ok(NuclearParticle::Photon)
        );
    }

    #[test]
    fn school_forms_of_particles_are_read_and_checked() {
        assert_eq!(
            one("^0_-1e").map(|found| found.0),
            Ok(NuclearParticle::Electron)
        );
        assert_eq!(
            one("⁰₊₁e").map(|found| found.0),
            Ok(NuclearParticle::Positron)
        );
        assert_eq!(
            one("^0_+1β").map(|found| found.0),
            Ok(NuclearParticle::Positron)
        );
        assert_eq!(one("^1_0n").map(|found| found.0), Ok(nuclide_of(1, 0)));
        assert_eq!(one("¹₁p").map(|found| found.0), Ok(nuclide_of(1, 1)));
        assert_eq!(one("^1_1n"), Err(NuclearProblem::NumbersDisagree));
        assert_eq!(one("^0e"), Err(NuclearProblem::UnsignedElectron));
    }

    #[test]
    fn an_atomic_number_that_disagrees_with_the_symbol_is_refused() {
        assert_eq!(
            one("^14_7C"),
            Err(NuclearProblem::AtomicNumberDisagrees {
                written: 7,
                symbol: 6
            })
        );
    }

    #[test]
    fn forms_with_two_readings_are_refused_by_name() {
        assert_eq!(one("14C"), Err(NuclearProblem::AttachedNumber));
        assert_eq!(one("D"), Err(NuclearProblem::AtomSymbol));
        assert_eq!(one("C"), Err(NuclearProblem::BareElement));
        assert_eq!(one("N"), Err(NuclearProblem::ParticleOrElement));
        assert_eq!(one("h"), Err(NuclearProblem::SchoolLetter));
        assert_eq!(one("nu"), Err(NuclearProblem::NeutrinoFlavour));
        assert_eq!(one("nubar"), Err(NuclearProblem::NeutrinoFlavour));
        assert_eq!(one("^4He^2+"), Err(NuclearProblem::Charge));
        assert_eq!(one("He2+"), Err(NuclearProblem::Charge));
        assert_eq!(one("^2Li"), Err(NuclearProblem::MassBelowCharge));
    }

    #[test]
    fn forms_not_read_yet_are_refused_each_in_its_own_kind() {
        assert_eq!(one("^99mTc"), Err(NuclearProblem::Isomer));
        assert_eq!(one("Tc-99m"), Err(NuclearProblem::Isomer));
        assert_eq!(one("μ-"), Err(NuclearProblem::OtherLepton));
        assert_eq!(one("nu_mu"), Err(NuclearProblem::OtherLepton));
        assert_eq!(one("^14N(α,p)^17O"), Err(NuclearProblem::CompactForm));
        assert_eq!(one("carbon-14"), Err(NuclearProblem::ElementName));
    }

    #[test]
    fn a_reaction_reads_coefficients_and_sides() {
        let Ok(ParsedNuclear::Reaction(all)) = parsed("^235U + n -> ^141Ba + ^92Kr + 3 n") else {
            panic!("not a reaction");
        };
        let products: Vec<bool> = all.iter().map(|one| one.is_product).collect();
        assert_eq!(products, vec![false, false, true, true, true]);
        assert_eq!(all[4].coefficient, Some(3));
        assert_eq!(all[4].particle, nuclide_of(1, 0));
    }

    #[test]
    fn a_species_may_stand_twice_on_a_side_and_on_both_sides() {
        assert!(parsed("^235U + n -> ^141Ba + ^92Kr + n").is_ok());
        assert!(parsed("p + p -> d + e+ + ν_e").is_ok());
    }

    #[test]
    fn line_forms_are_refused_as_in_chemistry() {
        assert_eq!(parsed("p + p").err(), Some(NuclearProblem::NoArrow));
        assert_eq!(parsed("p => d").err(), Some(NuclearProblem::WrongArrow));
        assert_eq!(parsed("p -> d -> t").err(), Some(NuclearProblem::TwoArrows));
        assert_eq!(
            parsed("2 p").err(),
            Some(NuclearProblem::CoefficientWithoutReaction)
        );
        assert_eq!(
            parsed("0 p -> d").err(),
            Some(NuclearProblem::ZeroCoefficient)
        );
        assert_eq!(parsed(" ").err(), Some(NuclearProblem::Empty));
    }
}
