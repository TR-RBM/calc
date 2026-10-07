use crate::binary_format::{BINARY32, BINARY64};
use crate::word_conversion::low_half;

fn quiet_f64(value: f64) -> f64 {
    f64::from_bits(BINARY64.quieted(value.to_bits()))
}

fn quiet_f32(value: f32) -> f32 {
    f32::from_bits(low_half(BINARY32.quieted(u64::from(value.to_bits()))))
}

pub fn minimum_f64(left: f64, right: f64) -> f64 {
    if left.is_nan() {
        return quiet_f64(left);
    }
    if right.is_nan() {
        return quiet_f64(right);
    }
    if left < right || (left == right && left.is_sign_negative()) {
        left
    } else {
        right
    }
}

pub fn maximum_f64(left: f64, right: f64) -> f64 {
    if left.is_nan() {
        return quiet_f64(left);
    }
    if right.is_nan() {
        return quiet_f64(right);
    }
    if left > right || (left == right && left.is_sign_positive()) {
        left
    } else {
        right
    }
}

pub fn minimum_f32(left: f32, right: f32) -> f32 {
    if left.is_nan() {
        return quiet_f32(left);
    }
    if right.is_nan() {
        return quiet_f32(right);
    }
    if left < right || (left == right && left.is_sign_negative()) {
        left
    } else {
        right
    }
}

pub fn maximum_f32(left: f32, right: f32) -> f32 {
    if left.is_nan() {
        return quiet_f32(left);
    }
    if right.is_nan() {
        return quiet_f32(right);
    }
    if left > right || (left == right && left.is_sign_positive()) {
        left
    } else {
        right
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALLEST_SUBNORMAL_F64: f64 = 5e-324;
    const SIGNALING_NAN_F64_BITS: u64 = 0x7ff0_0000_0000_0001;
    const NEGATIVE_NAN_F64_BITS: u64 = 0xfff8_0000_0000_0000;
    const SMALLEST_SUBNORMAL_F32: f32 = 1e-45;
    const SIGNALING_NAN_F32_BITS: u32 = 0x7f80_0001;
    const NEGATIVE_NAN_F32_BITS: u32 = 0xffc0_0000;

    fn assert_same_f64(actual: f64, expected: f64) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }

    fn assert_same_f32(actual: f32, expected: f32) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }

    #[test]
    fn minimum_f64_returns_nan_on_the_left() {
        let nan = f64::from_bits(NEGATIVE_NAN_F64_BITS);

        assert_same_f64(minimum_f64(nan, 1.0), nan);
    }

    #[test]
    fn minimum_f64_returns_nan_on_the_right() {
        let nan = f64::from_bits(NEGATIVE_NAN_F64_BITS);

        assert_same_f64(minimum_f64(f64::NEG_INFINITY, nan), nan);
    }

    #[test]
    fn minimum_f64_quiets_signaling_nan() {
        let result = minimum_f64(f64::from_bits(SIGNALING_NAN_F64_BITS), 0.0);

        assert_eq!(result.to_bits(), 0x7ff8_0000_0000_0001);
    }

    #[test]
    fn minimum_f64_orders_negative_zero_below_positive_zero() {
        assert_same_f64(minimum_f64(0.0, -0.0), -0.0);
        assert_same_f64(minimum_f64(-0.0, 0.0), -0.0);
    }

    #[test]
    fn minimum_f64_of_infinities_is_negative_infinity() {
        assert_same_f64(
            minimum_f64(f64::INFINITY, f64::NEG_INFINITY),
            f64::NEG_INFINITY,
        );
    }

    #[test]
    fn minimum_f64_orders_subnormal_above_zero() {
        assert_same_f64(minimum_f64(SMALLEST_SUBNORMAL_F64, 0.0), 0.0);
    }

    #[test]
    fn minimum_f64_orders_subnormal_below_smallest_normal() {
        assert_same_f64(
            minimum_f64(f64::MIN_POSITIVE, SMALLEST_SUBNORMAL_F64),
            SMALLEST_SUBNORMAL_F64,
        );
    }

    #[test]
    fn minimum_f64_of_finite_extremes_is_min() {
        assert_same_f64(minimum_f64(f64::MAX, f64::MIN), f64::MIN);
    }

    #[test]
    fn minimum_f64_distinguishes_neighbours_of_one() {
        let below_one = f64::from_bits(1.0f64.to_bits() - 1);
        let above_one = f64::from_bits(1.0f64.to_bits() + 1);

        assert_same_f64(minimum_f64(above_one, below_one), below_one);
    }

    #[test]
    fn maximum_f64_returns_nan_on_the_right() {
        let nan = f64::from_bits(NEGATIVE_NAN_F64_BITS);

        assert_same_f64(maximum_f64(f64::INFINITY, nan), nan);
    }

    #[test]
    fn maximum_f64_orders_positive_zero_above_negative_zero() {
        assert_same_f64(maximum_f64(-0.0, 0.0), 0.0);
        assert_same_f64(maximum_f64(0.0, -0.0), 0.0);
    }

    #[test]
    fn maximum_f64_of_finite_extremes_is_max() {
        assert_same_f64(maximum_f64(f64::MIN, f64::MAX), f64::MAX);
    }

    #[test]
    fn maximum_f64_distinguishes_neighbours_of_one() {
        let below_one = f64::from_bits(1.0f64.to_bits() - 1);
        let above_one = f64::from_bits(1.0f64.to_bits() + 1);

        assert_same_f64(maximum_f64(below_one, above_one), above_one);
    }

    #[test]
    fn minimum_f32_returns_nan_on_the_right() {
        let nan = f32::from_bits(NEGATIVE_NAN_F32_BITS);

        assert_same_f32(minimum_f32(1.0, nan), nan);
    }

    #[test]
    fn minimum_f32_quiets_signaling_nan() {
        let result = minimum_f32(f32::from_bits(SIGNALING_NAN_F32_BITS), 0.0);

        assert_eq!(result.to_bits(), 0x7fc0_0001);
    }

    #[test]
    fn minimum_f32_orders_negative_zero_below_positive_zero() {
        assert_same_f32(minimum_f32(0.0, -0.0), -0.0);
        assert_same_f32(minimum_f32(-0.0, 0.0), -0.0);
    }

    #[test]
    fn minimum_f32_orders_subnormal_below_smallest_normal() {
        assert_same_f32(
            minimum_f32(f32::MIN_POSITIVE, SMALLEST_SUBNORMAL_F32),
            SMALLEST_SUBNORMAL_F32,
        );
    }

    #[test]
    fn minimum_f32_of_finite_extremes_is_min() {
        assert_same_f32(minimum_f32(f32::MAX, f32::MIN), f32::MIN);
    }

    #[test]
    fn maximum_f32_returns_nan_on_the_left() {
        let nan = f32::from_bits(NEGATIVE_NAN_F32_BITS);

        assert_same_f32(maximum_f32(nan, f32::INFINITY), nan);
    }

    #[test]
    fn maximum_f32_orders_positive_zero_above_negative_zero() {
        assert_same_f32(maximum_f32(-0.0, 0.0), 0.0);
        assert_same_f32(maximum_f32(0.0, -0.0), 0.0);
    }

    #[test]
    fn maximum_f32_of_infinities_is_positive_infinity() {
        assert_same_f32(maximum_f32(f32::NEG_INFINITY, f32::INFINITY), f32::INFINITY);
    }

    #[test]
    fn maximum_f32_distinguishes_neighbours_of_one() {
        let below_one = f32::from_bits(1.0f32.to_bits() - 1);
        let above_one = f32::from_bits(1.0f32.to_bits() + 1);

        assert_same_f32(maximum_f32(below_one, above_one), above_one);
    }
}
