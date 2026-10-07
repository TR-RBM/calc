use calc_numbers::{Integer, Number, Rational};

const CHUNK_DIGITS: usize = 18;
const CHUNK_BASE: u64 = 1_000_000_000_000_000_000;
const DECIMAL_BASE: i64 = 10;
const TWO: i64 = 2;
const FIVE: i64 = 5;

pub(crate) fn radix_prefix(text: &str) -> Option<u32> {
    let mut characters = text.chars();
    if characters.next() != Some('0') {
        return None;
    }
    match characters.next()? {
        'x' | 'X' => Some(16),
        'b' | 'B' => Some(2),
        'o' | 'O' => Some(8),
        _ => None,
    }
}

pub(crate) fn integer_from_radix(digits: &str, radix: u32) -> Option<Integer> {
    let base = Integer::from(u64::from(radix));
    let mut value = Integer::zero();
    let mut any = false;
    for character in digits.chars() {
        if character == '_' {
            continue;
        }
        let digit = character.to_digit(radix)?;
        value = &(&value * &base) + &Integer::from(u64::from(digit));
        any = true;
    }
    any.then_some(value)
}

pub(crate) fn integer_from_digits(digits: &str) -> Integer {
    let chunk_base = Integer::from(CHUNK_BASE);
    let leading = digits.len() % CHUNK_DIGITS;
    let mut chunks = Vec::new();
    if leading > 0 {
        chunks.push(&digits[..leading]);
    }
    chunks.extend(
        digits.as_bytes()[leading..]
            .chunks(CHUNK_DIGITS)
            .filter_map(|chunk| std::str::from_utf8(chunk).ok()),
    );
    chunks
        .into_iter()
        .fold(Integer::zero(), |accumulated, chunk| {
            let value = chunk.parse::<u64>().unwrap_or(0);
            &(&accumulated * &chunk_base) + &Integer::from(value)
        })
}

pub(crate) fn integer_to_digits(value: &Integer) -> String {
    let chunk_base = Integer::from(CHUNK_BASE);
    let mut magnitude = value.absolute();
    let mut chunks = Vec::new();
    while !magnitude.is_zero() {
        let Ok((quotient, remainder)) = magnitude.div_rem_euclid(&chunk_base) else {
            break;
        };
        chunks.push(remainder.to_i64().unwrap_or(0));
        magnitude = quotient;
    }
    let mut text = String::new();
    if value.is_negative() {
        text.push('-');
    }
    match chunks.split_last() {
        None => text.push('0'),
        Some((most_significant, rest)) => {
            text.push_str(&most_significant.to_string());
            for chunk in rest.iter().rev() {
                text.push_str(&format!("{chunk:0width$}", width = CHUNK_DIGITS));
            }
        }
    }
    text
}

pub(crate) fn power_of_ten(exponent: u32) -> Integer {
    Integer::from(DECIMAL_BASE).pow(exponent)
}

fn remove_factor(value: &Integer, factor: i64) -> (Integer, u32) {
    let factor = Integer::from(factor);
    let mut remaining = value.clone();
    let mut count = 0_u32;
    while let Ok((quotient, remainder)) = remaining.div_rem_euclid(&factor) {
        if !remainder.is_zero() || remaining.is_zero() {
            break;
        }
        remaining = quotient;
        count = count.saturating_add(1);
    }
    (remaining, count)
}

pub(crate) fn rational_to_finite_decimal(rational: &Rational) -> Option<String> {
    let denominator = rational.denominator();
    let (without_twos, twos) = remove_factor(denominator, TWO);
    let (rest, fives) = remove_factor(&without_twos, FIVE);
    if !rest.is_one() {
        return None;
    }
    let scale = twos.max(fives);
    let multiplier =
        &Integer::from(TWO).pow(scale - twos) * &Integer::from(FIVE).pow(scale - fives);
    let scaled = &rational.numerator().absolute() * &multiplier;
    let digits = integer_to_digits(&scaled);
    let scale = usize::try_from(scale).ok()?;
    let padded = if digits.len() <= scale {
        format!("{}{digits}", "0".repeat(scale - digits.len() + 1))
    } else {
        digits
    };
    let (whole, fraction) = padded.split_at(padded.len() - scale);
    let sign = if rational.numerator().is_negative() {
        "-"
    } else {
        ""
    };
    Some(format!("{sign}{whole}.{fraction}"))
}

pub(crate) fn number_to_exact_text(number: &Number) -> Option<String> {
    match number {
        Number::Integer(integer) => Some(integer_to_digits(integer)),
        Number::Rational(rational) => {
            Some(rational_to_finite_decimal(rational).unwrap_or_else(|| {
                format!(
                    "q'{}/{}'",
                    integer_to_digits(rational.numerator()),
                    integer_to_digits(rational.denominator())
                )
            }))
        }
        Number::F32(_) | Number::F64(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_digit_strings_convert_both_ways() {
        let digits = "123456789012345678901234567890123456789";
        assert_eq!(integer_to_digits(&integer_from_digits(digits)), digits);
    }

    #[test]
    fn inner_chunks_keep_their_leading_zeros() {
        let digits = "1000000000000000000000000000000000007";
        assert_eq!(integer_to_digits(&integer_from_digits(digits)), digits);
    }

    #[test]
    fn zero_prints_as_one_digit() {
        assert_eq!(integer_to_digits(&Integer::zero()), "0");
    }

    #[test]
    fn negative_integer_prints_with_its_sign() {
        assert_eq!(integer_to_digits(&Integer::from(-42_i64)), "-42");
    }

    #[test]
    fn quarter_prints_as_finite_decimal() {
        let Number::Rational(quarter) =
            Number::fraction(&Integer::from(1_i64), &Integer::from(4_i64)).unwrap()
        else {
            panic!("expected a rational");
        };
        assert_eq!(rational_to_finite_decimal(&quarter).unwrap(), "0.25");
    }

    #[test]
    fn negative_small_rational_keeps_leading_zeros() {
        let Number::Rational(value) =
            Number::fraction(&Integer::from(-3_i64), &Integer::from(2000_i64)).unwrap()
        else {
            panic!("expected a rational");
        };
        assert_eq!(rational_to_finite_decimal(&value).unwrap(), "-0.0015");
    }

    #[test]
    fn third_has_no_finite_decimal() {
        let Number::Rational(third) =
            Number::fraction(&Integer::from(1_i64), &Integer::from(3_i64)).unwrap()
        else {
            panic!("expected a rational");
        };
        assert_eq!(rational_to_finite_decimal(&third), None);
    }
}
