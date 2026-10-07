pub(crate) fn low_half(value: u64) -> u32 {
    value as u32
}

pub(crate) fn high_half(value: u64) -> u32 {
    (value >> 32) as u32
}

pub(crate) fn usize_from_u32(value: u32) -> usize {
    value as usize
}

pub(crate) fn usize_from_u64(value: u64) -> usize {
    value as usize
}

pub(crate) fn i32_from_small_integral_f32(value: f32) -> i32 {
    value as i32
}

pub(crate) fn f32_from_small_i32(value: i32) -> f32 {
    value as f32
}

pub(crate) fn f64_from_small_u64(value: u64) -> f64 {
    value as f64
}

pub(crate) fn i64_from_small_integral_f64(value: f64) -> i64 {
    value as i64
}

pub(crate) fn i64_from_usize_saturating(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_of_u64_recombine_to_the_original() {
        let value = 0x0123_4567_89ab_cdef_u64;

        let low = low_half(value);
        let high = high_half(value);

        assert_eq!((u64::from(high) << 32) | u64::from(low), value);
    }

    #[test]
    fn usize_saturates_to_i64_max() {
        assert_eq!(i64_from_usize_saturating(usize::MAX), i64::MAX);
    }
}
