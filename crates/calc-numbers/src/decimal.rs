use std::cmp::Ordering;

use crate::integer::Integer;
use crate::number::Number;

pub const DECIMAL_PLACES_LIMIT: u32 = 78_913;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecimalError {
    NotFinite,
    PlacesAboveLimit { places: u32, limit: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecimalPeriod {
    pub start: u32,
    pub length: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecimalDigits {
    pub places: u32,
    pub digits: Number,
    pub terminates: bool,
    pub remainder: Option<Number>,
    pub expansion: Expansion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expansion {
    Ends,
    Recurs(DecimalPeriod),
    RecursBeyondLimit,
    NotKnownToRecur,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecimalRounding {
    TiesToEven,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoundedDecimal {
    pub rule: DecimalRounding,
    pub places: u32,
    pub rounded: Number,
    pub error: Number,
}

struct Scaled {
    is_negative: bool,
    quotient: Integer,
    remainder: Integer,
    denominator: Integer,
    scale: Integer,
}

fn exact_fraction(value: &Number) -> Result<(Integer, Integer), DecimalError> {
    match value.to_exact().map_err(|_| DecimalError::NotFinite)? {
        Number::Integer(integer) => Ok((integer, Integer::one())),
        Number::Rational(rational) => {
            Ok((rational.numerator().clone(), rational.denominator().clone()))
        }
        Number::F32(_) | Number::F64(_) => Err(DecimalError::NotFinite),
    }
}

fn checked_places(places: u32) -> Result<(), DecimalError> {
    if places > DECIMAL_PLACES_LIMIT {
        return Err(DecimalError::PlacesAboveLimit {
            places,
            limit: DECIMAL_PLACES_LIMIT,
        });
    }
    Ok(())
}

fn scaled(value: &Number, places: u32) -> Result<Scaled, DecimalError> {
    checked_places(places)?;
    let (numerator, denominator) = exact_fraction(value)?;
    let scale = Integer::from(10_i64).pow(places);
    let (quotient, remainder) = (&numerator.absolute() * &scale)
        .div_rem_euclid(&denominator)
        .map_err(|_| DecimalError::NotFinite)?;
    Ok(Scaled {
        is_negative: numerator.is_negative(),
        quotient,
        remainder,
        denominator,
        scale,
    })
}

fn signed(magnitude: Integer, is_negative: bool) -> Integer {
    if is_negative {
        magnitude.negated()
    } else {
        magnitude
    }
}

fn decimal(numerator: &Integer, scale: &Integer) -> Result<Number, DecimalError> {
    Number::fraction(numerator, scale).map_err(|_| DecimalError::NotFinite)
}

fn difference(value: &Number, other: &Number) -> Result<Number, DecimalError> {
    value
        .to_exact()
        .map_err(|_| DecimalError::NotFinite)?
        .sub_exact(other)
        .map_err(|_| DecimalError::NotFinite)
}

fn without_factor(mut value: Integer, factor: i64) -> (Integer, u64) {
    let divisor = Integer::from(factor);
    let mut count = 0_u64;
    while let Ok((quotient, remainder)) = value.div_rem_euclid(&divisor) {
        if !remainder.is_zero() || value.is_zero() {
            break;
        }
        value = quotient;
        count += 1;
    }
    (value, count)
}

fn times_ten_modulo(residue: &Integer, modulus: &Integer) -> Integer {
    let mut product = residue * &Integer::from(10_i64);
    while product >= *modulus {
        product = &product - modulus;
    }
    product
}

struct DenominatorParts {
    twos: u64,
    fives: u64,
    coprime: Integer,
}

fn denominator_parts(denominator: &Integer) -> DenominatorParts {
    let (without_twos, twos) = without_factor(denominator.clone(), 2);
    let (coprime, fives) = without_factor(without_twos, 5);
    DenominatorParts {
        twos,
        fives,
        coprime,
    }
}

fn expansion(parts: &DenominatorParts) -> Expansion {
    if parts.coprime.is_one() {
        return Expansion::Ends;
    }
    match period(parts) {
        Some(period) => Expansion::Recurs(period),
        None => Expansion::RecursBeyondLimit,
    }
}

fn period(parts: &DenominatorParts) -> Option<DecimalPeriod> {
    if parts.coprime.is_one() {
        return None;
    }
    let start = u32::try_from(parts.twos.max(parts.fives))
        .ok()?
        .checked_add(1)?;
    let longest = DECIMAL_PLACES_LIMIT.checked_sub(start - 1)?;
    let mut residue = times_ten_modulo(&Integer::one(), &parts.coprime);
    let mut length = 1_u32;
    while !residue.is_one() {
        if length >= longest {
            return None;
        }
        residue = times_ten_modulo(&residue, &parts.coprime);
        length += 1;
    }
    Some(DecimalPeriod { start, length })
}

pub const POWER_OF_TEN_MARK: char = 'e';

const MINUS: char = '-';

pub fn power_of_ten_text(digits: &str, exponent: i64, decimal_separator: char) -> String {
    let mut text = String::new();
    let magnitude = match digits.strip_prefix(MINUS) {
        Some(rest) => {
            text.push(MINUS);
            rest
        }
        None => digits,
    };
    let mut characters = magnitude.chars();
    if let Some(leading) = characters.next() {
        text.push(leading);
    }
    let rest = characters.as_str();
    if !rest.is_empty() {
        text.push(decimal_separator);
        text.push_str(rest);
    }
    text.push(POWER_OF_TEN_MARK);
    text.push_str(&exponent.to_string());
    text
}

pub fn truncated_to_places(value: &Number, places: u32) -> Result<Number, DecimalError> {
    let scaled = scaled(value, places)?;
    decimal(&signed(scaled.quotient, scaled.is_negative), &scaled.scale)
}

pub fn decimal_digits(value: &Number, places: u32) -> Result<DecimalDigits, DecimalError> {
    let scaled = scaled(value, places)?;
    let digits = decimal(&signed(scaled.quotient, scaled.is_negative), &scaled.scale)?;
    let remainder = Some(difference(value, &digits)?);
    let parts = denominator_parts(&scaled.denominator);
    Ok(DecimalDigits {
        places,
        digits,
        terminates: scaled.remainder.is_zero(),
        remainder,
        expansion: expansion(&parts),
    })
}

pub fn round_to_decimal_places(
    value: &Number,
    places: u32,
    rule: DecimalRounding,
) -> Result<RoundedDecimal, DecimalError> {
    let scaled = scaled(value, places)?;
    let twice_remainder = &scaled.remainder * &Integer::from(2_i64);
    let rounds_up = match (rule, twice_remainder.cmp(&scaled.denominator)) {
        (DecimalRounding::TiesToEven, Ordering::Greater) => true,
        (DecimalRounding::TiesToEven, Ordering::Less) => false,
        (DecimalRounding::TiesToEven, Ordering::Equal) => {
            let (_, parity) = scaled
                .quotient
                .div_rem_euclid(&Integer::from(2_i64))
                .map_err(|_| DecimalError::NotFinite)?;
            parity.is_one()
        }
    };
    let magnitude = if rounds_up {
        &scaled.quotient + &Integer::one()
    } else {
        scaled.quotient
    };
    let rounded = decimal(&signed(magnitude, scaled.is_negative), &scaled.scale)?;
    let error = difference(value, &rounded)?;
    Ok(RoundedDecimal {
        rule,
        places,
        rounded,
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_digit_needs_no_separator() {
        assert_eq!(power_of_ten_text("7", 9, '.'), "7e9");
    }

    #[test]
    fn the_separator_follows_the_first_digit() {
        assert_eq!(power_of_ten_text("3142", 9, '.'), "3.142e9");
    }

    #[test]
    fn the_caller_chooses_the_separator() {
        assert_eq!(power_of_ten_text("3142", 9, ','), "3,142e9");
    }

    #[test]
    fn a_minus_stays_in_front_of_the_digits() {
        assert_eq!(power_of_ten_text("-3142", -9, '.'), "-3.142e-9");
    }

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    fn digits_of(numerator: i64, denominator: i64, places: u32) -> DecimalDigits {
        decimal_digits(&fraction(numerator, denominator), places).unwrap()
    }

    fn rounded_of(numerator: i64, denominator: i64, places: u32) -> RoundedDecimal {
        round_to_decimal_places(
            &fraction(numerator, denominator),
            places,
            DecimalRounding::TiesToEven,
        )
        .unwrap()
    }

    #[test]
    fn one_third_to_five_places_is_truncated_with_its_remainder() {
        let digits = digits_of(1, 3, 5);

        assert_eq!(
            (digits.digits, digits.remainder),
            (fraction(33_333, 100_000), Some(fraction(1, 300_000)))
        );
    }

    #[test]
    fn negative_value_is_truncated_toward_zero() {
        let digits = digits_of(-2, 3, 5);

        assert_eq!(
            (digits.digits, digits.remainder),
            (fraction(-66_666, 100_000), Some(fraction(-1, 150_000)))
        );
    }

    #[test]
    fn negative_value_truncating_to_zero_keeps_its_sign_in_the_remainder() {
        let digits = digits_of(-1, 300, 2);

        assert_eq!(
            (digits.digits, digits.remainder),
            (Number::from(0_i64), Some(fraction(-1, 300)))
        );
    }

    #[test]
    fn negative_machine_zero_gives_zero_with_zero_remainder() {
        let digits = decimal_digits(&Number::F64(-0.0), 3).unwrap();

        assert_eq!(
            (digits.digits, digits.remainder, digits.terminates),
            (Number::from(0_i64), Some(Number::from(0_i64)), true)
        );
    }

    #[test]
    fn value_with_at_most_the_places_terminates() {
        let digits = digits_of(1, 8, 3);

        assert!(digits.terminates && digits.remainder == Some(Number::from(0_i64)));
    }

    #[test]
    fn finite_expansion_cut_short_has_no_period() {
        let digits = digits_of(1, 8, 2);

        assert_eq!(
            (digits.terminates, digits.expansion),
            (false, Expansion::Ends)
        );
    }

    #[test]
    fn one_third_repeats_from_the_first_place_with_length_one() {
        assert_eq!(
            digits_of(1, 3, 0).expansion,
            Expansion::Recurs(DecimalPeriod {
                start: 1,
                length: 1
            })
        );
    }

    #[test]
    fn one_sixth_repeats_from_the_second_place() {
        assert_eq!(
            digits_of(1, 6, 4).expansion,
            Expansion::Recurs(DecimalPeriod {
                start: 2,
                length: 1
            })
        );
    }

    #[test]
    fn one_seventh_has_period_length_six() {
        assert_eq!(
            digits_of(1, 7, 1).expansion,
            Expansion::Recurs(DecimalPeriod {
                start: 1,
                length: 6
            })
        );
    }

    #[test]
    fn period_ending_beyond_the_limit_says_so() {
        let digits = digits_of(1, 78_977, 1);

        assert_eq!(digits.expansion, Expansion::RecursBeyondLimit);
    }

    #[test]
    fn places_above_the_limit_are_an_error() {
        assert_eq!(
            decimal_digits(&fraction(1, 3), 78_914),
            Err(DecimalError::PlacesAboveLimit {
                places: 78_914,
                limit: 78_913
            })
        );
    }

    #[test]
    fn machine_value_gives_the_digits_of_the_machine_number() {
        let digits = decimal_digits(&Number::F64(0.1), 20).unwrap();

        assert_eq!(
            digits.digits,
            Number::fraction(
                &Integer::from(10_000_000_000_000_000_555_i128),
                &Integer::from(10_i64).pow(20)
            )
            .unwrap()
        );
    }

    #[test]
    fn not_a_number_has_no_digits() {
        assert_eq!(
            decimal_digits(&Number::F64(f64::NAN), 2),
            Err(DecimalError::NotFinite)
        );
    }

    #[test]
    fn remainder_below_half_rounds_toward_zero() {
        let rounded = rounded_of(123, 1000, 2);

        assert_eq!(
            (rounded.rounded, rounded.error),
            (fraction(12, 100), fraction(3, 1000))
        );
    }

    #[test]
    fn remainder_above_half_rounds_away_from_zero() {
        let rounded = rounded_of(127, 1000, 2);

        assert_eq!(
            (rounded.rounded, rounded.error),
            (fraction(13, 100), fraction(-3, 1000))
        );
    }

    #[test]
    fn tie_below_an_even_digit_rounds_down() {
        let rounded = rounded_of(125, 1000, 2);

        assert_eq!(
            (rounded.rounded, rounded.error),
            (fraction(12, 100), fraction(5, 1000))
        );
    }

    #[test]
    fn tie_above_an_odd_digit_rounds_up() {
        let rounded = rounded_of(135, 1000, 2);

        assert_eq!(
            (rounded.rounded, rounded.error),
            (fraction(14, 100), fraction(-5, 1000))
        );
    }

    #[test]
    fn negative_tie_rounds_its_magnitude_to_even() {
        let rounded = rounded_of(-125, 1000, 2);

        assert_eq!(
            (rounded.rounded, rounded.error),
            (fraction(-12, 100), fraction(-5, 1000))
        );
    }

    #[test]
    fn value_on_the_grid_rounds_to_itself_with_zero_error() {
        let rounded = rounded_of(1, 2, 2);

        assert_eq!(
            (rounded.rounded, rounded.error),
            (fraction(1, 2), Number::from(0_i64))
        );
    }

    #[test]
    fn rounding_names_ties_to_even() {
        assert_eq!(rounded_of(1, 3, 4).rule, DecimalRounding::TiesToEven);
    }

    #[test]
    fn rounding_places_above_the_limit_are_an_error() {
        assert_eq!(
            round_to_decimal_places(&fraction(1, 3), 78_914, DecimalRounding::TiesToEven),
            Err(DecimalError::PlacesAboveLimit {
                places: 78_914,
                limit: 78_913
            })
        );
    }
}
