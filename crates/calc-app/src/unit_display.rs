use calc_expr::{ExprPool, NodeView};
use calc_syntax::parse_expression;
use calc_units::UnitId;

use crate::session_file::UNIT_VALUE_PREFIX;

const PRODUCT_SIGN: char = '\u{00B7}';
const QUOTIENT_SIGN: char = '/';
const INPUT_PRODUCT_SIGN: char = '*';
const INPUT_POWER_SIGN: char = '^';
const GROUP_OPEN: char = '(';
const GROUP_CLOSE: char = ')';
const SUPERSCRIPT_MINUS: char = '\u{207B}';
const SUPERSCRIPT_DIGITS: [char; 10] = [
    '\u{2070}', '\u{00B9}', '\u{00B2}', '\u{00B3}', '\u{2074}', '\u{2075}', '\u{2076}', '\u{2077}',
    '\u{2078}', '\u{2079}',
];

fn superscript(exponent: i8) -> String {
    if exponent == 1 {
        return String::new();
    }
    let mut text = String::new();
    if exponent < 0 {
        text.push(SUPERSCRIPT_MINUS);
    }
    text.extend(
        exponent
            .unsigned_abs()
            .to_string()
            .bytes()
            .map(|digit| SUPERSCRIPT_DIGITS[usize::from(digit - b'0')]),
    );
    text
}

pub(crate) fn input_text(pool: &ExprPool, unit: UnitId) -> Option<String> {
    let units = pool.units();
    let factors = units.factors(unit).ok()?;
    if factors.is_empty() {
        return None;
    }
    let product = |exponent_of: &dyn Fn(i8) -> Option<i8>, positive: bool| -> Option<String> {
        let mut text = String::new();
        for factor in factors
            .iter()
            .filter(|factor| (factor.exponent() > 0) == positive)
        {
            if !text.is_empty() {
                text.push(INPUT_PRODUCT_SIGN);
            }
            text.push_str(units.symbol(factor.named_unit()).ok()?);
            let exponent = exponent_of(factor.exponent())?;
            if exponent != 1 {
                text.push(INPUT_POWER_SIGN);
                text.push_str(&exponent.to_string());
            }
        }
        Some(text)
    };
    let numerator = product(&Some, true)?;
    if numerator.is_empty() {
        return product(&Some, false);
    }
    let negative_count = factors
        .iter()
        .filter(|factor| factor.exponent() < 0)
        .count();
    let denominator = product(&i8::checked_neg, false)?;
    Some(match negative_count {
        0 => numerator,
        1 => format!("{numerator}{QUOTIENT_SIGN}{denominator}"),
        _ => format!("{numerator}{QUOTIENT_SIGN}{GROUP_OPEN}{denominator}{GROUP_CLOSE}"),
    })
}

pub(crate) fn display_text(pool: &ExprPool, unit: UnitId) -> Option<String> {
    let units = pool.units();
    let factors = units.factors(unit).ok()?;
    if factors.is_empty() {
        return None;
    }
    let product = |exponent_of: &dyn Fn(i8) -> Option<i8>, positive: bool| -> Option<String> {
        let mut text = String::new();
        for factor in factors
            .iter()
            .filter(|factor| (factor.exponent() > 0) == positive)
        {
            if !text.is_empty() {
                text.push(PRODUCT_SIGN);
            }
            text.push_str(units.display_symbol(factor.named_unit()).ok()?);
            text.push_str(&superscript(exponent_of(factor.exponent())?));
        }
        Some(text)
    };
    let numerator = product(&Some, true)?;
    if numerator.is_empty() {
        return product(&Some, false);
    }
    let negative_count = factors
        .iter()
        .filter(|factor| factor.exponent() < 0)
        .count();
    let denominator = product(&i8::checked_neg, false)?;
    Some(match negative_count {
        0 => numerator,
        1 => format!("{numerator}{QUOTIENT_SIGN}{denominator}"),
        _ => format!("{numerator}{QUOTIENT_SIGN}{GROUP_OPEN}{denominator}{GROUP_CLOSE}"),
    })
}

pub fn display_unit_text(unit_text: &str) -> String {
    let mut pool = ExprPool::new();
    parse_expression(&mut pool, &format!("{UNIT_VALUE_PREFIX}{unit_text}"))
        .ok()
        .and_then(|expression| match pool.node(expression).ok()? {
            NodeView::Quantity { unit, .. } => display_text(&pool, unit),
            _ => None,
        })
        .unwrap_or_else(|| unit_text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_unit_takes_a_superscript_exponent() {
        assert_eq!(display_unit_text("m^2"), "m\u{00B2}");
    }

    #[test]
    fn negative_exponent_after_a_positive_one_becomes_a_quotient() {
        assert_eq!(display_unit_text("m*s^-2"), "m/s\u{00B2}");
    }

    #[test]
    fn several_negative_exponents_share_one_solidus_in_parentheses() {
        assert_eq!(display_unit_text("kg*m^-1*s^-2"), "kg/(m\u{00B7}s\u{00B2})");
    }

    #[test]
    fn product_of_units_uses_the_middle_dot() {
        assert_eq!(display_unit_text("kg*m"), "kg\u{00B7}m");
    }

    #[test]
    fn unit_with_only_negative_exponents_keeps_a_superscript_minus() {
        assert_eq!(display_unit_text("s^-1"), "s\u{207B}\u{00B9}");
    }

    #[test]
    fn degree_celsius_shows_its_display_symbol() {
        assert_eq!(display_unit_text("degC"), "\u{00B0}C");
    }

    #[test]
    fn text_that_is_not_a_unit_is_kept() {
        assert_eq!(display_unit_text("not a unit +"), "not a unit +");
    }
}
