const SPLITTER: f64 = 134_217_729.0;
const EXPONENT_MASK: u64 = 0x7ff;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DoubleWord {
    pub(crate) high: f64,
    pub(crate) low: f64,
}

impl DoubleWord {
    pub(crate) const fn exact(value: f64) -> Self {
        DoubleWord {
            high: value,
            low: 0.0,
        }
    }

    pub(crate) fn negated(self) -> Self {
        DoubleWord {
            high: -self.high,
            low: -self.low,
        }
    }
}

pub(crate) fn two_sum(first: f64, second: f64) -> DoubleWord {
    let sum = first + second;
    let first_part = sum - second;
    let second_part = sum - first_part;
    let error = (first - first_part) + (second - second_part);
    DoubleWord {
        high: sum,
        low: error,
    }
}

pub(crate) fn fast_two_sum(larger: f64, smaller: f64) -> DoubleWord {
    let sum = larger + smaller;
    let error = smaller - (sum - larger);
    DoubleWord {
        high: sum,
        low: error,
    }
}

fn split(value: f64) -> (f64, f64) {
    let scaled = SPLITTER * value;
    let high = scaled - (scaled - value);
    (high, value - high)
}

pub(crate) fn two_product(first: f64, second: f64) -> DoubleWord {
    let product = first * second;
    let (first_high, first_low) = split(first);
    let (second_high, second_low) = split(second);
    let error = first_low * second_low
        - (((product - first_high * second_high) - first_low * second_high)
            - first_high * second_low);
    DoubleWord {
        high: product,
        low: error,
    }
}

pub(crate) fn plus_double(word: DoubleWord, value: f64) -> DoubleWord {
    let sum = two_sum(word.high, value);
    let low = word.low + sum.low;
    fast_two_sum(sum.high, low)
}

pub(crate) fn plus(first: DoubleWord, second: DoubleWord) -> DoubleWord {
    let high = two_sum(first.high, second.high);
    let low = two_sum(first.low, second.low);
    let carried = high.low + low.high;
    let middle = fast_two_sum(high.high, carried);
    let rest = low.low + middle.low;
    fast_two_sum(middle.high, rest)
}

pub(crate) fn times(first: DoubleWord, second: DoubleWord) -> DoubleWord {
    let product = two_product(first.high, second.high);
    let cross = first.high * second.low + first.low * second.high;
    fast_two_sum(product.high, product.low + cross)
}

pub(crate) fn divided(numerator: DoubleWord, denominator: DoubleWord) -> DoubleWord {
    let first = numerator.high / denominator.high;
    let product = two_product(first, denominator.high);
    let remainder =
        (((numerator.high - product.high) - product.low) + numerator.low) - first * denominator.low;
    fast_two_sum(first, remainder / denominator.high)
}

pub(crate) fn square(value: DoubleWord) -> DoubleWord {
    let product = two_product(value.high, value.high);
    let cross = 2.0 * value.high * value.low;
    fast_two_sum(product.high, product.low + cross)
}

pub(crate) fn decided_within(value: DoubleWord, error_exponent: i64) -> Option<f64> {
    let magnitude = value.high.abs();
    let low = if value.high < 0.0 {
        -value.low
    } else {
        value.low
    };
    let biased = i64::try_from((magnitude.to_bits() >> 52) & EXPONENT_MASK).ok()?;
    let error_field = u64::try_from(biased - error_exponent).ok()?;
    if error_field == 0 || !magnitude.is_normal() {
        return None;
    }
    let error = f64::from_bits(error_field << 52);
    let half_up = (magnitude.next_up() - magnitude) / 2.0;
    let half_down = (magnitude - magnitude.next_down()) / 2.0;
    (low < half_up - error && low > -(half_down - error)).then_some(value.high)
}

pub(crate) fn decided_with_absolute(
    value: DoubleWord,
    error_exponent: i64,
    absolute_error: f64,
) -> Option<f64> {
    let magnitude = value.high.abs();
    let biased = i64::try_from((magnitude.to_bits() >> 52) & EXPONENT_MASK).ok()?;
    let relative_field = u64::try_from(biased - error_exponent).ok()?;
    let absolute_field =
        u64::try_from(i64::try_from((absolute_error.to_bits() >> 52) & EXPONENT_MASK).ok()? + 1)
            .ok()?;
    let floor_field = u64::try_from(biased - 104).ok()?;
    let field = relative_field.max(absolute_field).max(floor_field) + 1;
    if !magnitude.is_normal() || field >= u64::try_from(biased - 53).ok()? {
        return None;
    }
    let error = f64::from_bits(field << 52);
    let low = if value.high < 0.0 {
        -value.low
    } else {
        value.low
    };
    let half_up = (magnitude.next_up() - magnitude) / 2.0;
    let half_down = (magnitude - magnitude.next_down()) / 2.0;
    (low < half_up - error && low > -(half_down - error)).then_some(value.high)
}
