const DECIMAL_POINT: char = '.';
const EXPONENT_MARKERS: [char; 2] = ['e', 'E'];
const MINUS_SIGN: char = '-';
const PLUS_SIGN: char = '+';
const RADIX: u128 = 10;
const LARGEST_MANTISSA_DIGITS: u32 = 36;
const SMALLEST_PLAIN_EXPONENT: i32 = -5;
const LARGEST_PLAIN_EXPONENT: i32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    Up,
    NearestHalfEven,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decimal {
    is_negative: bool,
    mantissa: u128,
    exponent: i32,
}

fn digit_count(mantissa: u128) -> u32 {
    mantissa.checked_ilog10().map_or(1, |log| log + 1)
}

impl Decimal {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let (body, exponent_text) = match text.split_once(EXPONENT_MARKERS) {
            Some((body, exponent)) => (body, Some(exponent)),
            None => (text, None),
        };
        let written_exponent: i32 = match exponent_text {
            Some(exponent) => exponent
                .strip_prefix(PLUS_SIGN)
                .unwrap_or(exponent)
                .parse()
                .ok()?,
            None => 0,
        };
        let (is_negative, unsigned) = match body.strip_prefix(MINUS_SIGN) {
            Some(rest) => (true, rest),
            None => (false, body),
        };
        let (integer, fraction) = unsigned.split_once(DECIMAL_POINT).unwrap_or((unsigned, ""));
        if integer.is_empty() && fraction.is_empty() {
            return None;
        }
        let digits: String = integer.chars().chain(fraction.chars()).collect();
        if !digits.chars().all(|character| character.is_ascii_digit()) {
            return None;
        }
        let significant = digits.trim_start_matches('0');
        if u32::try_from(significant.len()).ok()? > LARGEST_MANTISSA_DIGITS {
            return None;
        }
        let mantissa = if significant.is_empty() {
            0
        } else {
            significant.parse().ok()?
        };
        let fraction_length = i32::try_from(fraction.len()).ok()?;
        Some(Self {
            is_negative,
            mantissa,
            exponent: written_exponent.checked_sub(fraction_length)?,
        })
    }

    pub const fn is_zero(self) -> bool {
        self.mantissa == 0
    }

    pub const fn last_digit_exponent(self) -> i32 {
        self.exponent
    }

    fn leading_digit_exponent(self) -> Option<i32> {
        let digits = i32::try_from(digit_count(self.mantissa)).ok()?;
        self.exponent.checked_add(digits - 1)
    }

    pub fn to_significant_digits(self, digits: u32, rounding: Rounding) -> Option<Self> {
        if self.is_zero() {
            return Some(self);
        }
        let leading = self.leading_digit_exponent()?;
        let last_kept = leading.checked_sub(i32::try_from(digits).ok()? - 1)?;
        self.round_to_exponent(last_kept, rounding)
    }

    pub fn round_to_exponent(self, last_kept: i32, rounding: Rounding) -> Option<Self> {
        if self.exponent >= last_kept {
            let added = u32::try_from(self.exponent.checked_sub(last_kept)?).ok()?;
            return Some(Self {
                is_negative: self.is_negative,
                mantissa: self.mantissa.checked_mul(RADIX.checked_pow(added)?)?,
                exponent: last_kept,
            });
        }
        let dropped = u32::try_from(last_kept.checked_sub(self.exponent)?).ok()?;
        if dropped > LARGEST_MANTISSA_DIGITS + 1 {
            return Some(Self {
                is_negative: self.is_negative,
                mantissa: u128::from(rounding == Rounding::Up && !self.is_zero()),
                exponent: last_kept,
            });
        }
        let divisor = RADIX.checked_pow(dropped)?;
        let kept = self.mantissa / divisor;
        let remainder = self.mantissa % divisor;
        let round_away = match rounding {
            Rounding::Up => remainder > 0,
            Rounding::NearestHalfEven => {
                let twice = remainder.checked_mul(2)?;
                twice > divisor || (twice == divisor && kept % 2 == 1)
            }
        };
        Some(Self {
            is_negative: self.is_negative,
            mantissa: kept.checked_add(u128::from(round_away))?,
            exponent: last_kept,
        })
    }

    pub fn to_text(self) -> String {
        let sign = if self.is_negative && !self.is_zero() {
            "-"
        } else {
            ""
        };
        let digits = self.mantissa.to_string();
        let leading = self.leading_digit_exponent().unwrap_or(self.exponent);
        let is_plain =
            self.is_zero() || (SMALLEST_PLAIN_EXPONENT..LARGEST_PLAIN_EXPONENT).contains(&leading);
        if !is_plain {
            let (first, rest) = digits.split_at(1);
            let point = if rest.is_empty() { "" } else { "." };
            return format!("{sign}{first}{point}{rest}e{leading}");
        }
        if self.exponent >= 0 {
            let zeros = "0".repeat(usize::try_from(self.exponent).unwrap_or(0));
            return format!("{sign}{digits}{zeros}");
        }
        let fraction_length = usize::try_from(-self.exponent).unwrap_or(0);
        let padded = format!("{digits:0>width$}", width = fraction_length + 1);
        let (integer, fraction) = padded.split_at(padded.len() - fraction_length);
        format!("{sign}{integer}.{fraction}")
    }
}

pub fn relative_error(bound: Decimal, value: Decimal) -> Option<Decimal> {
    if value.is_zero() {
        return None;
    }
    const RELATIVE_DIGITS: u32 = 2;
    let scale = LARGEST_MANTISSA_DIGITS - digit_count(bound.mantissa);
    let numerator = bound.mantissa.checked_mul(RADIX.checked_pow(scale)?)?;
    let quotient = numerator / value.mantissa;
    let remainder = numerator % value.mantissa;
    let raw = Decimal {
        is_negative: false,
        mantissa: quotient.checked_add(u128::from(remainder > 0))?,
        exponent: bound
            .exponent
            .checked_sub(value.exponent)?
            .checked_sub(i32::try_from(scale).ok()?)?,
    };
    raw.to_significant_digits(RELATIVE_DIGITS, Rounding::Up)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decimal(text: &str) -> Decimal {
        Decimal::parse(text).expect("decimal text")
    }

    #[test]
    fn plain_decimal_keeps_its_last_digit_position() {
        assert_eq!(decimal("24.525").last_digit_exponent(), -3);
    }

    #[test]
    fn scientific_text_moves_the_exponent() {
        assert_eq!(decimal("7.1e-15").last_digit_exponent(), -16);
    }

    #[test]
    fn fraction_text_is_not_a_decimal() {
        assert_eq!(Decimal::parse("1/3"), None);
    }

    #[test]
    fn infinity_is_not_a_decimal() {
        assert_eq!(Decimal::parse("inf"), None);
    }

    #[test]
    fn bound_rounds_up_to_one_significant_digit() {
        let rounded = decimal("7.105427357601003e-15").to_significant_digits(1, Rounding::Up);

        assert_eq!(rounded.map(Decimal::to_text), Some("8e-15".to_owned()));
    }

    #[test]
    fn exact_one_digit_bound_is_not_rounded_up() {
        let rounded = decimal("2e-10").to_significant_digits(1, Rounding::Up);

        assert_eq!(rounded.map(Decimal::to_text), Some("2e-10".to_owned()));
    }

    #[test]
    fn uncertainty_keeps_two_significant_digits() {
        let rounded = decimal("0.1149").to_significant_digits(2, Rounding::NearestHalfEven);

        assert_eq!(rounded.map(Decimal::to_text), Some("0.11".to_owned()));
    }

    #[test]
    fn value_rounds_to_the_uncertainty_position() {
        let rounded = decimal("24.525").round_to_exponent(-2, Rounding::NearestHalfEven);

        assert_eq!(rounded.map(Decimal::to_text), Some("24.52".to_owned()));
    }

    #[test]
    fn value_rounding_ties_go_to_the_even_digit() {
        let rounded = decimal("24.535").round_to_exponent(-2, Rounding::NearestHalfEven);

        assert_eq!(rounded.map(Decimal::to_text), Some("24.54".to_owned()));
    }

    #[test]
    fn value_with_trailing_zero_keeps_the_zero() {
        let rounded = decimal("2.5").round_to_exponent(-2, Rounding::NearestHalfEven);

        assert_eq!(rounded.map(Decimal::to_text), Some("2.50".to_owned()));
    }

    #[test]
    fn value_below_one_keeps_its_leading_zero() {
        let rounded = decimal("0.74682413306").round_to_exponent(-10, Rounding::NearestHalfEven);

        assert_eq!(
            rounded.map(Decimal::to_text),
            Some("0.7468241331".to_owned())
        );
    }

    #[test]
    fn negative_value_keeps_its_sign() {
        assert_eq!(decimal("-3.25").to_text(), "-3.25");
    }

    #[test]
    fn relative_error_divides_bound_by_value_and_rounds_up() {
        let relative = relative_error(decimal("7.1e-15"), decimal("24.525"));

        assert_eq!(relative.map(Decimal::to_text), Some("2.9e-16".to_owned()));
    }

    #[test]
    fn relative_error_of_zero_value_does_not_exist() {
        assert_eq!(relative_error(decimal("1e-300"), decimal("0")), None);
    }
}
