use std::ops::RangeInclusive;

const SINGLE_CELL: u16 = 1;
const DOUBLE_CELL: u16 = 2;

const WIDE_OR_FULLWIDTH: [RangeInclusive<u32>; 14] = [
    0x1100..=0x115F,
    0x2E80..=0x303E,
    0x3041..=0x33FF,
    0x3400..=0x4DBF,
    0x4E00..=0x9FFF,
    0xA000..=0xA4CF,
    0xAC00..=0xD7A3,
    0xF900..=0xFAFF,
    0xFE30..=0xFE4F,
    0xFF00..=0xFF60,
    0xFFE0..=0xFFE6,
    0x1F300..=0x1F64F,
    0x1F900..=0x1F9FF,
    0x20000..=0x3FFFD,
];

const COMBINING_MARKS: [RangeInclusive<u32>; 5] = [
    0x0300..=0x036F,
    0x1AB0..=0x1AFF,
    0x1DC0..=0x1DFF,
    0x20D0..=0x20FF,
    0xFE20..=0xFE2F,
];

pub fn is_combining_mark(character: char) -> bool {
    let code_point = u32::from(character);
    COMBINING_MARKS
        .iter()
        .any(|range| range.contains(&code_point))
}

pub fn cells_of(text: &str) -> u16 {
    text.chars()
        .filter(|&character| !is_combining_mark(character))
        .map(cell_span)
        .fold(0, u16::saturating_add)
}

pub fn cell_span(base: char) -> u16 {
    let code_point = u32::from(base);
    if WIDE_OR_FULLWIDTH
        .iter()
        .any(|range| range.contains(&code_point))
    {
        DOUBLE_CELL
    } else {
        SINGLE_CELL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combining_mark_takes_no_cell_of_its_own() {
        assert_eq!(cells_of("\u{03C0}\u{0302}"), 1);
    }

    #[test]
    fn text_cells_add_wide_characters_twice() {
        assert_eq!(cells_of("x\u{4E00}"), 3);
    }

    #[test]
    fn combining_circumflex_is_a_combining_mark() {
        assert!(is_combining_mark('\u{0302}'));
    }

    #[test]
    fn latin_letter_takes_one_cell() {
        assert_eq!(cell_span('x'), 1);
    }

    #[test]
    fn greek_letter_takes_one_cell() {
        assert_eq!(cell_span('\u{03C0}'), 1);
    }

    #[test]
    fn integral_sign_takes_one_cell() {
        assert_eq!(cell_span('\u{222B}'), 1);
    }

    #[test]
    fn cjk_ideograph_takes_two_cells() {
        assert_eq!(cell_span('\u{4E00}'), 2);
    }

    #[test]
    fn fullwidth_latin_letter_takes_two_cells() {
        assert_eq!(cell_span('\u{FF21}'), 2);
    }

    #[test]
    fn halfwidth_katakana_takes_one_cell() {
        assert_eq!(cell_span('\u{FF71}'), 1);
    }
}
