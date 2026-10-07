use crate::integer::Integer;
use crate::natural::Natural;
use crate::number::Number;
use crate::rational::{Rational, ReducedFraction};
use crate::word_conversion::{i64_from_usize_saturating, usize_from_u64};

pub(crate) struct BinaryFormat {
    precision: usize,
    exponent_bits: usize,
    max_exponent: i64,
}

pub(crate) const BINARY32: BinaryFormat = BinaryFormat {
    precision: 24,
    exponent_bits: 8,
    max_exponent: 127,
};

pub(crate) const BINARY64: BinaryFormat = BinaryFormat {
    precision: 53,
    exponent_bits: 11,
    max_exponent: 1023,
};

pub(crate) struct NonFinite;

pub(crate) struct FiniteParts {
    pub(crate) is_negative: bool,
    pub(crate) significand: u64,
    pub(crate) exponent: i64,
}

impl BinaryFormat {
    fn fraction_bits(&self) -> usize {
        self.precision - 1
    }

    fn sign_mask(&self) -> u64 {
        1u64 << (self.exponent_bits + self.fraction_bits())
    }

    fn exponent_field_max(&self) -> u64 {
        (1u64 << self.exponent_bits) - 1
    }

    fn fraction_mask(&self) -> u64 {
        (1u64 << self.fraction_bits()) - 1
    }

    fn quiet_mask(&self) -> u64 {
        1u64 << (self.fraction_bits() - 1)
    }

    fn min_quantum_exponent(&self) -> i64 {
        1 - self.max_exponent - i64_from_usize_saturating(self.fraction_bits())
    }

    fn sign_bits(&self, is_negative: bool) -> u64 {
        if is_negative { self.sign_mask() } else { 0 }
    }

    pub(crate) fn infinity_bits(&self, is_negative: bool) -> u64 {
        self.sign_bits(is_negative) | (self.exponent_field_max() << self.fraction_bits())
    }

    pub(crate) fn round_fraction(
        &self,
        is_negative: bool,
        numerator: &Natural,
        denominator: &Natural,
    ) -> u64 {
        let sign = self.sign_bits(is_negative);
        if numerator.is_zero() {
            return sign;
        }
        let precision = i64_from_usize_saturating(self.precision);
        let magnitude_exponent = i64_from_usize_saturating(numerator.bit_length())
            - i64_from_usize_saturating(denominator.bit_length());
        if magnitude_exponent - 1 > self.max_exponent {
            return self.infinity_bits(is_negative);
        }
        if magnitude_exponent + 2 <= self.min_quantum_exponent() {
            return sign;
        }

        let scale_exponent = magnitude_exponent - precision - 2;
        let scale = usize_from_u64(scale_exponent.unsigned_abs());
        let (scaled_numerator, scaled_denominator) = if scale_exponent < 0 {
            (numerator.shifted_left(scale), denominator.clone())
        } else {
            (numerator.clone(), denominator.shifted_left(scale))
        };
        let Some((quotient, remainder)) = scaled_numerator.div_rem(&scaled_denominator) else {
            return self.infinity_bits(is_negative);
        };

        let quotient_exponent =
            i64_from_usize_saturating(quotient.bit_length()) + scale_exponent - precision;
        let mut result_exponent = quotient_exponent.max(self.min_quantum_exponent());
        let dropped_bits = usize_from_u64((result_exponent - scale_exponent).unsigned_abs());
        let mut mantissa = quotient.shifted_right(dropped_bits);
        let is_half_or_more = quotient.bit(dropped_bits - 1);
        let is_above_half = quotient.has_set_bit_below(dropped_bits - 1) || !remainder.is_zero();
        if is_half_or_more && (is_above_half || mantissa.is_odd()) {
            mantissa = mantissa.add(&Natural::one());
            if mantissa.bit_length() > self.precision {
                mantissa = mantissa.shifted_right(1);
                result_exponent += 1;
            }
        }
        if result_exponent + precision - 1 > self.max_exponent {
            return self.infinity_bits(is_negative);
        }

        let mantissa_bits = mantissa.to_u64().unwrap_or_default();
        let normal_threshold = 1u64 << self.fraction_bits();
        if mantissa_bits < normal_threshold {
            return sign | mantissa_bits;
        }
        let biased_exponent = result_exponent + precision - 1 + self.max_exponent;
        let exponent_field = u64::try_from(biased_exponent).unwrap_or_default();
        sign | (exponent_field << self.fraction_bits()) | (mantissa_bits - normal_threshold)
    }

    pub(crate) fn to_exact(&self, bits: u64) -> Result<Number, NonFinite> {
        let is_negative = bits & self.sign_mask() != 0;
        let exponent_field = (bits >> self.fraction_bits()) & self.exponent_field_max();
        let fraction = bits & self.fraction_mask();
        if exponent_field == self.exponent_field_max() {
            return Err(NonFinite);
        }
        let (significand, field_for_exponent) = if exponent_field == 0 {
            (fraction, 1)
        } else {
            (fraction | (1u64 << self.fraction_bits()), exponent_field)
        };
        let exponent = i64::try_from(field_for_exponent).unwrap_or_default()
            - self.max_exponent
            - i64_from_usize_saturating(self.fraction_bits());
        let magnitude = Natural::from_u64(significand);
        if exponent >= 0 {
            let shifted = magnitude.shifted_left(usize_from_u64(exponent.unsigned_abs()));
            return Ok(Number::Integer(Integer::from_sign_and_magnitude(
                is_negative,
                shifted,
            )));
        }
        let numerator = Integer::from_sign_and_magnitude(is_negative, magnitude);
        let denominator = Integer::from_sign_and_magnitude(
            false,
            Natural::one().shifted_left(usize_from_u64(exponent.unsigned_abs())),
        );
        Ok(match Rational::reduce(&numerator, &denominator) {
            Some(ReducedFraction::Rational(rational)) => Number::Rational(rational),
            Some(ReducedFraction::Integer(integer)) => Number::Integer(integer),
            None => Number::Integer(Integer::zero()),
        })
    }

    pub(crate) fn convert_nan_payload(&self, target: &BinaryFormat, bits: u64) -> u64 {
        let payload_bits = self.fraction_bits() - 1;
        let target_payload_bits = target.fraction_bits() - 1;
        let payload = bits & (self.quiet_mask() - 1);
        let target_payload = if target_payload_bits >= payload_bits {
            payload << (target_payload_bits - payload_bits)
        } else {
            payload >> (payload_bits - target_payload_bits)
        };
        let is_negative = bits & self.sign_mask() != 0;
        target.infinity_bits(is_negative) | target.quiet_mask() | target_payload
    }

    pub(crate) fn finite_parts(&self, bits: u64) -> Option<FiniteParts> {
        let exponent_field = (bits >> self.fraction_bits()) & self.exponent_field_max();
        if exponent_field == self.exponent_field_max() {
            return None;
        }
        let fraction = bits & self.fraction_mask();
        let (significand, field_for_exponent) = if exponent_field == 0 {
            (fraction, 1)
        } else {
            (fraction | (1u64 << self.fraction_bits()), exponent_field)
        };
        let exponent = i64::try_from(field_for_exponent).unwrap_or_default()
            - self.max_exponent
            - i64_from_usize_saturating(self.fraction_bits());
        Some(FiniteParts {
            is_negative: self.is_negative(bits),
            significand,
            exponent,
        })
    }

    pub(crate) fn is_nan(&self, bits: u64) -> bool {
        let exponent_field = (bits >> self.fraction_bits()) & self.exponent_field_max();
        exponent_field == self.exponent_field_max() && bits & self.fraction_mask() != 0
    }

    pub(crate) fn is_negative(&self, bits: u64) -> bool {
        bits & self.sign_mask() != 0
    }

    pub(crate) fn quieted(&self, bits: u64) -> u64 {
        bits | self.quiet_mask()
    }
}
