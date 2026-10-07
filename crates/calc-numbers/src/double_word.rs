const SPLITTER: f32 = 4097.0;

pub(crate) fn two_sum(left: f32, right: f32) -> (f32, f32) {
    let sum = left + right;
    let right_part = sum - left;
    let left_part = sum - right_part;
    let error = (left - left_part) + (right - right_part);
    (sum, error)
}

pub(crate) fn fast_two_sum(larger: f32, smaller: f32) -> (f32, f32) {
    let sum = larger + smaller;
    let error = smaller - (sum - larger);
    (sum, error)
}

fn split(value: f32) -> (f32, f32) {
    let scaled = SPLITTER * value;
    let high = scaled - (scaled - value);
    let low = value - high;
    (high, low)
}

pub(crate) fn two_product(left: f32, right: f32) -> (f32, f32) {
    let product = left * right;
    let (left_high, left_low) = split(left);
    let (right_high, right_low) = split(right);
    let error =
        ((left_high * right_high - product) + left_high * right_low + left_low * right_high)
            + left_low * right_low;
    (product, error)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DoubleWord {
    pub(crate) high: f32,
    pub(crate) low: f32,
}

impl DoubleWord {
    pub(crate) const fn from_bits(high: u32, low: u32) -> Self {
        Self {
            high: f32::from_bits(high),
            low: f32::from_bits(low),
        }
    }

    pub(crate) fn from_f32(value: f32) -> Self {
        Self {
            high: value,
            low: 0.0,
        }
    }

    pub(crate) fn from_sum(left: f32, right: f32) -> Self {
        let (high, low) = two_sum(left, right);
        Self { high, low }
    }

    pub(crate) fn from_product(left: f32, right: f32) -> Self {
        let (high, low) = two_product(left, right);
        Self { high, low }
    }

    pub(crate) fn from_quotient(numerator: f32, denominator: f32) -> Self {
        Self::from_f32(numerator).div_f32(denominator)
    }

    pub(crate) fn negated(self) -> Self {
        Self {
            high: -self.high,
            low: -self.low,
        }
    }

    pub(crate) fn add_f32(self, value: f32) -> Self {
        let (sum_high, sum_low) = two_sum(self.high, value);
        let combined_low = self.low + sum_low;
        let (high, low) = fast_two_sum(sum_high, combined_low);
        Self { high, low }
    }

    pub(crate) fn add(self, other: Self) -> Self {
        let (sum_high, sum_low) = two_sum(self.high, other.high);
        let (tail_high, tail_low) = two_sum(self.low, other.low);
        let carry = sum_low + tail_high;
        let (middle_high, middle_low) = fast_two_sum(sum_high, carry);
        let rest = tail_low + middle_low;
        let (high, low) = fast_two_sum(middle_high, rest);
        Self { high, low }
    }

    pub(crate) fn sub(self, other: Self) -> Self {
        self.add(other.negated())
    }

    pub(crate) fn mul_f32(self, value: f32) -> Self {
        let (product_high, product_low) = two_product(self.high, value);
        let cross = self.low * value;
        let (sum_high, sum_low) = fast_two_sum(product_high, cross);
        let rest = sum_low + product_low;
        let (high, low) = fast_two_sum(sum_high, rest);
        Self { high, low }
    }

    pub(crate) fn mul(self, other: Self) -> Self {
        let (product_high, product_low) = two_product(self.high, other.high);
        let cross = self.high * other.low + self.low * other.high;
        let rest = product_low + cross;
        let (high, low) = fast_two_sum(product_high, rest);
        Self { high, low }
    }

    pub(crate) fn square(self) -> Self {
        self.mul(self)
    }

    pub(crate) fn div_f32(self, value: f32) -> Self {
        let quotient = self.high / value;
        let (product_high, product_low) = two_product(quotient, value);
        let remainder = ((self.high - product_high) - product_low) + self.low;
        let correction = remainder / value;
        let (high, low) = fast_two_sum(quotient, correction);
        Self { high, low }
    }

    pub(crate) fn div(self, other: Self) -> Self {
        let quotient = self.high / other.high;
        let product = other.mul_f32(quotient);
        let difference_high = self.high - product.high;
        let difference_low = self.low - product.low;
        let correction = (difference_high + difference_low) / other.high;
        let (high, low) = fast_two_sum(quotient, correction);
        Self { high, low }
    }

    pub(crate) fn sqrt(self) -> Self {
        if self.high <= 0.0 {
            return Self::from_f32(0.0);
        }
        let root = self.high.sqrt();
        let (square_high, square_low) = two_product(root, root);
        let remainder = ((self.high - square_high) - square_low) + self.low;
        let correction = remainder / (2.0 * root);
        let (high, low) = fast_two_sum(root, correction);
        Self { high, low }
    }

    pub(crate) fn rounded(self) -> f32 {
        if self.low == 0.0 {
            return self.high;
        }
        self.high + self.low
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integer::Integer;
    use crate::number::Number;

    fn exact(value: f32) -> Number {
        Number::F32(value).to_exact().unwrap()
    }

    fn exact_sum(high: f32, low: f32) -> Number {
        exact(high).add_exact(&exact(low)).unwrap()
    }

    fn relative_error_is_below(computed: DoubleWord, expected: &Number, exponent: u32) -> bool {
        let difference = exact_sum(computed.high, computed.low)
            .sub_exact(expected)
            .unwrap();
        let magnitude = |value: &Number| {
            if value.is_negative_exact() {
                value.negate_exact().unwrap()
            } else {
                value.clone()
            }
        };
        let allowed = magnitude(expected)
            .div_exact(&Number::Integer(Integer::from(2i64).pow(exponent)))
            .unwrap();
        let excess = magnitude(&difference).sub_exact(&allowed).unwrap();
        excess.is_negative_exact() || excess == Number::Integer(Integer::zero())
    }

    const SAMPLES: [f32; 8] = [
        1.0,
        -3.25,
        0.1,
        16_777_215.0,
        1.0e-10,
        -7.0e15,
        std::f32::consts::PI,
        0.333_333_34,
    ];

    #[test]
    fn two_sum_is_exact() {
        for left in SAMPLES {
            for right in SAMPLES {
                let (sum, error) = two_sum(left, right);

                assert_eq!(
                    exact_sum(sum, error),
                    exact(left).add_exact(&exact(right)).unwrap()
                );
            }
        }
    }

    #[test]
    fn two_product_is_exact() {
        for left in SAMPLES {
            for right in SAMPLES {
                let (product, error) = two_product(left, right);

                assert_eq!(
                    exact_sum(product, error),
                    exact(left).mul_exact(&exact(right)).unwrap()
                );
            }
        }
    }

    #[test]
    fn double_word_sum_has_small_relative_error() {
        let left = DoubleWord::from_sum(1.0, 1.0e-9);
        let right = DoubleWord::from_sum(-0.5, 3.0e-10);
        let expected = exact_sum(left.high, left.low)
            .add_exact(&exact_sum(right.high, right.low))
            .unwrap();

        assert!(relative_error_is_below(left.add(right), &expected, 44));
    }

    #[test]
    fn double_word_product_has_small_relative_error() {
        let left = DoubleWord::from_sum(1.1, 1.0e-9);
        let right = DoubleWord::from_sum(-3.3, 2.0e-8);
        let expected = exact_sum(left.high, left.low)
            .mul_exact(&exact_sum(right.high, right.low))
            .unwrap();

        assert!(relative_error_is_below(left.mul(right), &expected, 44));
    }

    #[test]
    fn double_word_quotient_has_small_relative_error() {
        let numerator = DoubleWord::from_sum(1.0, 1.0e-9);
        let denominator = DoubleWord::from_sum(3.0, -2.0e-8);
        let quotient = numerator.div(denominator);
        let reconstructed = exact_sum(quotient.high, quotient.low)
            .mul_exact(&exact_sum(denominator.high, denominator.low))
            .unwrap();

        assert!(relative_error_is_below(
            DoubleWord {
                high: numerator.high,
                low: numerator.low
            },
            &reconstructed,
            43
        ));
    }

    #[test]
    fn double_word_square_root_has_small_relative_error() {
        let value = DoubleWord::from_sum(2.0, 1.0e-8);
        let root = value.sqrt();
        let squared = exact_sum(root.high, root.low)
            .mul_exact(&exact_sum(root.high, root.low))
            .unwrap();

        assert!(relative_error_is_below(value, &squared, 43));
    }

    #[test]
    fn rounding_a_double_word_rounds_to_nearest() {
        assert_eq!(DoubleWord::from_sum(1.0, 1.0e-8).rounded(), 1.0);
    }
}
