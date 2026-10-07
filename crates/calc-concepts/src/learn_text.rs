use calc_core::{DecimalEnclosureError, digits_of_expression, evaluate_exact};
use calc_expr::{ExprId, ExprPool, NodeView, SymbolKind};
use calc_numbers::{EnclosureError, Integer, Number};

use crate::error::LoadErrorKind;

const VALUE_OPEN: char = '{';
const VALUE_CLOSE: char = '}';
const PLACES_MARK: char = '|';
const APPROXIMATELY: char = '≈';
const MINUS: char = '−';
const TYPED_SEPARATORS: [char; 2] = [',', '.'];
const TEN: i64 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecimalMark {
    Comma,
    Point,
}

impl DecimalMark {
    pub(crate) fn from_name(name: &str) -> Option<DecimalMark> {
        match name {
            "comma" => Some(DecimalMark::Comma),
            "point" => Some(DecimalMark::Point),
            _ => None,
        }
    }

    fn character(self) -> char {
        match self {
            DecimalMark::Comma => ',',
            DecimalMark::Point => '.',
        }
    }
}

pub(crate) fn render_learn_text(
    text: &str,
    mark: Option<DecimalMark>,
    locale: &str,
    pool: &mut ExprPool,
) -> Result<String, LoadErrorKind> {
    let mut rendered = String::new();
    let mut rest = text;
    while let Some(open) = rest.find(VALUE_OPEN) {
        check_typed(&rest[..open])?;
        rendered.push_str(&rest[..open]);
        let after = &rest[open + VALUE_OPEN.len_utf8()..];
        let close = after
            .find(VALUE_CLOSE)
            .ok_or(LoadErrorKind::UnclosedValue)?;
        let mark = mark.ok_or_else(|| LoadErrorKind::MissingNotation(locale.to_string()))?;
        rendered.push_str(&rounded_value(&after[..close], mark, pool)?);
        rest = &after[close + VALUE_CLOSE.len_utf8()..];
    }
    check_typed(rest)?;
    rendered.push_str(rest);
    Ok(rendered)
}

fn check_typed(text: &str) -> Result<(), LoadErrorKind> {
    if text.contains(VALUE_CLOSE) {
        return Err(LoadErrorKind::UnclosedValue);
    }
    if text.contains(APPROXIMATELY) {
        return Err(LoadErrorKind::TypedApproximation);
    }
    let characters: Vec<char> = text.chars().collect();
    for window in characters.windows(3) {
        if window[0].is_ascii_digit()
            && TYPED_SEPARATORS.contains(&window[1])
            && window[2].is_ascii_digit()
        {
            return Err(LoadErrorKind::TypedDecimal(window.iter().collect()));
        }
    }
    Ok(())
}

fn rounded_value(
    written: &str,
    mark: DecimalMark,
    pool: &mut ExprPool,
) -> Result<String, LoadErrorKind> {
    let (formula, places) = written
        .rsplit_once(PLACES_MARK)
        .ok_or_else(|| LoadErrorKind::ValueWithoutPlaces(written.trim().to_string()))?;
    let places: u32 = places
        .trim()
        .parse()
        .map_err(|_| LoadErrorKind::InvalidPlaces(places.trim().to_string()))?;
    let formula = formula.trim();
    let not_a_number = || LoadErrorKind::ValueNotANumber(formula.to_string());
    let expression = calc_syntax::parse_expression(pool, formula)
        .map_err(|error| LoadErrorKind::Formula(error.kind))?;
    if !is_a_plain_number(pool, expression) {
        return Err(not_a_number());
    }
    let evaluation = evaluate_exact(pool, expression).map_err(|_| not_a_number())?;
    let scaled = match evaluation.rational_value() {
        Some(value) => rational_rounded(value, places)
            .ok_or_else(|| LoadErrorKind::ValueIsExact(formula.to_string()))?,
        None => irrational_rounded(pool, expression, places).map_err(|refusal| match refusal {
            Refusal::NotANumber => not_a_number(),
            Refusal::Undecided => LoadErrorKind::RoundingUndecided(formula.to_string()),
        })?,
    };
    scaled_text(&scaled, places, mark).ok_or_else(not_a_number)
}

fn is_a_plain_number(pool: &ExprPool, expression: ExprId) -> bool {
    match pool.node(expression) {
        Ok(NodeView::Number(_) | NodeView::Bound(_)) => true,
        Ok(NodeView::Symbol(symbol)) => {
            matches!(pool.symbol_kind(symbol), Ok(SymbolKind::Constant))
        }
        Ok(NodeView::Apply { arguments, .. }) => arguments
            .iter()
            .all(|argument| is_a_plain_number(pool, *argument)),
        Ok(NodeView::Bind {
            arguments, body, ..
        }) => {
            is_a_plain_number(pool, body)
                && arguments
                    .iter()
                    .all(|argument| is_a_plain_number(pool, *argument))
        }
        Ok(NodeView::Quantity { .. } | NodeView::Array { .. }) | Err(_) => false,
    }
}

fn power_of_ten(exponent: u32) -> Integer {
    Integer::from(TEN).pow(exponent)
}

fn fraction_parts(value: &Number) -> Option<(Integer, Integer)> {
    match value.to_exact().ok()? {
        Number::Integer(integer) => Some((integer, Integer::one())),
        Number::Rational(rational) => {
            Some((rational.numerator().clone(), rational.denominator().clone()))
        }
        Number::F32(_) | Number::F64(_) => None,
    }
}

fn half_away_quotient(twice_numerator: &Integer, twice_denominator: &Integer) -> Option<Integer> {
    twice_numerator
        .div_rem_euclid(twice_denominator)
        .ok()
        .map(|(quotient, _)| quotient)
}

fn rational_rounded(value: &Number, places: u32) -> Option<Integer> {
    let (numerator, denominator) = fraction_parts(value)?;
    let scaled = &numerator.absolute() * &power_of_ten(places);
    let (_, remainder) = scaled.div_rem_euclid(&denominator).ok()?;
    if remainder.is_zero() {
        return None;
    }
    let two = Integer::from(2_i64);
    let magnitude =
        half_away_quotient(&(&(&scaled * &two) + &denominator), &(&denominator * &two))?;
    Some(with_sign(magnitude, numerator.is_negative()))
}

enum Refusal {
    NotANumber,
    Undecided,
}

fn irrational_rounded(
    pool: &mut ExprPool,
    expression: ExprId,
    places: u32,
) -> Result<Integer, Refusal> {
    let precise = places + 1;
    let digits =
        digits_of_expression(pool, expression, precise, &|| false).map_err(
            |error| match error {
                DecimalEnclosureError::Enclosure(
                    EnclosureError::NoEnclosure { .. } | EnclosureError::OverBudget { .. },
                ) => Refusal::Undecided,
                _ => Refusal::NotANumber,
            },
        )?;
    let (numerator, denominator) = fraction_parts(&digits.digits).ok_or(Refusal::NotANumber)?;
    let truncated = (&numerator.absolute() * &power_of_ten(precise))
        .div_rem_euclid(&denominator)
        .map_err(|_| Refusal::NotANumber)?
        .0;
    let two = Integer::from(2_i64);
    let magnitude = half_away_quotient(
        &(&(&truncated * &two) + &Integer::from(TEN)),
        &Integer::from(2 * TEN),
    )
    .ok_or(Refusal::NotANumber)?;
    Ok(with_sign(magnitude, numerator.is_negative()))
}

fn with_sign(magnitude: Integer, is_negative: bool) -> Integer {
    if is_negative {
        magnitude.negated()
    } else {
        magnitude
    }
}

fn scaled_text(scaled: &Integer, places: u32, mark: DecimalMark) -> Option<String> {
    let magnitude = scaled.absolute().to_i64()?;
    let places = usize::try_from(places).ok()?;
    let digits = format!("{magnitude:0width$}", width = places + 1);
    let (whole, fraction) = digits.split_at(digits.len() - places);
    let mut text = String::from(APPROXIMATELY);
    text.push(' ');
    if scaled.is_negative() {
        text.push(MINUS);
    }
    text.push_str(whole);
    if !fraction.is_empty() {
        text.push(mark.character());
        text.push_str(fraction);
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(text: &str, mark: DecimalMark) -> Result<String, LoadErrorKind> {
        render_learn_text(text, Some(mark), "de", &mut ExprPool::new())
    }

    #[test]
    fn a_value_in_a_locale_without_notation_is_refused() {
        assert_eq!(
            render_learn_text("{pi | 2}", None, "fr", &mut ExprPool::new()),
            Err(LoadErrorKind::MissingNotation("fr".to_string()))
        );
    }

    #[test]
    fn a_text_without_values_needs_no_notation() {
        assert_eq!(
            render_learn_text("Ein Kreis.", None, "fr", &mut ExprPool::new()),
            Ok("Ein Kreis.".to_string())
        );
    }

    #[test]
    fn a_value_is_rounded_with_the_comma() {
        assert_eq!(
            rendered("9π cm² {9*pi | 2} cm²", DecimalMark::Comma),
            Ok("9π cm² ≈ 28,27 cm²".to_string())
        );
    }

    #[test]
    fn a_value_is_rounded_with_the_point() {
        assert_eq!(
            rendered("{10*pi | 2}", DecimalMark::Point),
            Ok("≈ 31.42".to_string())
        );
    }

    #[test]
    fn a_value_rounds_up_across_a_whole_number() {
        assert_eq!(
            rendered("{sqrt(99.9999) | 2}", DecimalMark::Point),
            Ok("≈ 10.00".to_string())
        );
    }

    #[test]
    fn a_rational_tie_rounds_away_from_zero() {
        assert_eq!(
            rendered("{1/8 | 2}", DecimalMark::Point),
            Ok("≈ 0.13".to_string())
        );
    }

    #[test]
    fn a_negative_value_takes_the_minus_sign() {
        assert_eq!(
            rendered("{-1/8 | 2}", DecimalMark::Comma),
            Ok("≈ −0,13".to_string())
        );
    }

    #[test]
    fn no_places_leave_no_mark() {
        assert_eq!(
            rendered("{pi | 0}", DecimalMark::Comma),
            Ok("≈ 3".to_string())
        );
    }

    #[test]
    fn a_value_just_below_a_tie_rounds_down() {
        assert_eq!(
            rendered("{1/8 - pi/10^30 | 2}", DecimalMark::Point),
            Ok("≈ 0.12".to_string())
        );
    }

    #[test]
    fn a_value_just_above_a_tie_rounds_up() {
        assert_eq!(
            rendered("{1/8 + pi/10^30 | 2}", DecimalMark::Point),
            Ok("≈ 0.13".to_string())
        );
    }

    #[test]
    fn a_value_closer_to_a_tie_than_the_enclosure_budget_is_refused() {
        assert_eq!(
            rendered("{1/8 - pi/10^6000 | 2}", DecimalMark::Point),
            Err(LoadErrorKind::RoundingUndecided(
                "1/8 - pi/10^6000".to_string()
            ))
        );
    }

    #[test]
    fn a_value_that_the_places_hold_exactly_is_refused() {
        assert_eq!(
            rendered("{7/2 | 2}", DecimalMark::Comma),
            Err(LoadErrorKind::ValueIsExact("7/2".to_string()))
        );
    }

    #[test]
    fn a_typed_decimal_is_refused() {
        assert_eq!(
            rendered("den Radius 3,5 m", DecimalMark::Comma),
            Err(LoadErrorKind::TypedDecimal("3,5".to_string()))
        );
    }

    #[test]
    fn a_typed_approximation_is_refused() {
        assert_eq!(
            rendered("9π ≈ 28 und {pi | 2}", DecimalMark::Comma),
            Err(LoadErrorKind::TypedApproximation)
        );
    }

    #[test]
    fn a_list_of_whole_numbers_is_not_a_decimal() {
        assert_eq!(
            rendered("die Seiten 3, 4 und 5", DecimalMark::Comma),
            Ok("die Seiten 3, 4 und 5".to_string())
        );
    }

    #[test]
    fn a_value_without_places_is_refused() {
        assert_eq!(
            rendered("{9*pi}", DecimalMark::Comma),
            Err(LoadErrorKind::ValueWithoutPlaces("9*pi".to_string()))
        );
    }

    #[test]
    fn an_unclosed_value_is_refused() {
        assert_eq!(
            rendered("{9*pi | 2", DecimalMark::Comma),
            Err(LoadErrorKind::UnclosedValue)
        );
    }

    #[test]
    fn a_value_with_a_unit_is_refused() {
        assert_eq!(
            rendered("{2 m * pi | 2}", DecimalMark::Comma),
            Err(LoadErrorKind::ValueNotANumber("2 m * pi".to_string()))
        );
    }

    #[test]
    fn a_value_with_a_free_name_is_refused() {
        assert_eq!(
            rendered("{9*r | 2}", DecimalMark::Comma),
            Err(LoadErrorKind::ValueNotANumber("9*r".to_string()))
        );
    }
}
