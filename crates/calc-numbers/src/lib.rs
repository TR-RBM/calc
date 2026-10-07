mod ball;
mod binary64_words;
mod binary_format;
mod decimal;
mod double_word;
mod elementary;
mod elementary32;
mod fast_arctangent;
mod fast_sine_cosine;
mod fixed_trig;
mod ieee;
mod integer;
mod interval;
mod natural;
mod number;
mod rational;
mod real_ball;
mod series;
#[cfg(test)]
mod test_support;
mod word_conversion;

pub use decimal::{
    DECIMAL_PLACES_LIMIT, DecimalDigits, DecimalError, DecimalPeriod, DecimalRounding, Expansion,
    POWER_OF_TEN_MARK, RoundedDecimal, decimal_digits, power_of_ten_text, round_to_decimal_places,
    truncated_to_places,
};
pub use elementary::{
    ElementaryError, acos_f64, asin_f64, atan_f64, atan2_f64, checked_acos_f64, checked_asin_f64,
    checked_atan_f64, checked_atan2_f64, checked_cos_f64, checked_exp_f64, checked_ln_f64,
    checked_pow_f64, checked_sin_f64, checked_tan_f64, cos_f64,
    elementary_rounding_error_bound_f64, exp_f64, ln_f64, pow_f64, sin_f64, sqrt_f64, tan_f64,
};
pub use elementary32::{
    acos_f32, asin_f32, atan_f32, atan2_f32, cos_f32, elementary_rounding_error_bound_f32, exp_f32,
    ln_f32, pow_f32, sin_f32, sqrt_f32, tan_f32,
};
pub use ieee::{maximum_f32, maximum_f64, minimum_f32, minimum_f64};
pub use integer::{DivisionError, Integer};
pub use interval::{Interval, Truth};
pub use number::{ExactArithmeticError, Number, ToExactError};
pub use rational::Rational;
pub use real_ball::{
    BallError, DecimalEnclosure, ENCLOSURE_BUDGET_BITS, EnclosureError, EnclosureStep, RealBall,
    SIGNIFICANT_DIGITS_LIMIT, at_least_one, correctly_rounded_to_significant_digits,
    enclose_between, enclose_exact, enclose_to_significant_digits,
    round_half_away_to_significant_digits, round_to_significant_digits, smallest_f64_at_least,
    truncated_at_places,
};
