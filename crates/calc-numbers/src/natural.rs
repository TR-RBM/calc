use std::cmp::Ordering;

use crate::word_conversion::{high_half, low_half, usize_from_u32};

const LIMB_BITS: usize = 32;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Natural {
    limbs: Vec<u32>,
}

impl Natural {
    pub(crate) fn zero() -> Self {
        Self { limbs: Vec::new() }
    }

    pub(crate) fn one() -> Self {
        Self::from_u64(1)
    }

    pub(crate) fn from_u64(value: u64) -> Self {
        Self::from_limbs(vec![low_half(value), high_half(value)])
    }

    pub(crate) fn from_u128(value: u128) -> Self {
        let bytes = value.to_le_bytes();
        let limbs = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| u32::from_le_bytes(*chunk))
            .collect();
        Self::from_limbs(limbs)
    }

    fn from_limbs(mut limbs: Vec<u32>) -> Self {
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        Self { limbs }
    }

    pub(crate) fn to_u64(&self) -> Option<u64> {
        match self.limbs.as_slice() {
            [] => Some(0),
            [low] => Some(u64::from(*low)),
            [low, high] => Some((u64::from(*high) << 32) | u64::from(*low)),
            _ => None,
        }
    }

    pub(crate) fn to_u128(&self) -> Option<u128> {
        if self.limbs.len() > 4 {
            return None;
        }
        Some(self.limbs.iter().rev().fold(0u128, |accumulated, limb| {
            (accumulated << 32) | u128::from(*limb)
        }))
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    pub(crate) fn is_one(&self) -> bool {
        self.limbs.as_slice() == [1]
    }

    pub(crate) fn is_odd(&self) -> bool {
        self.limbs.first().is_some_and(|low| low & 1 == 1)
    }

    pub(crate) fn bit_length(&self) -> usize {
        match self.limbs.last() {
            None => 0,
            Some(top) => self.limbs.len() * LIMB_BITS - usize_from_u32(top.leading_zeros()),
        }
    }

    pub(crate) fn bit(&self, index: usize) -> bool {
        let limb_index = index / LIMB_BITS;
        let bit_index = index % LIMB_BITS;
        self.limbs
            .get(limb_index)
            .is_some_and(|limb| (limb >> bit_index) & 1 == 1)
    }

    pub(crate) fn has_set_bit_below(&self, index: usize) -> bool {
        let full_limbs = index / LIMB_BITS;
        let partial_bits = index % LIMB_BITS;
        let full_limbs_set = self.limbs.iter().take(full_limbs).any(|limb| *limb != 0);
        let partial_set = partial_bits > 0
            && self
                .limbs
                .get(full_limbs)
                .is_some_and(|limb| limb & ((1u32 << partial_bits) - 1) != 0);
        full_limbs_set || partial_set
    }

    pub(crate) fn shifted_left(&self, bits: usize) -> Self {
        if self.is_zero() {
            return Self::zero();
        }
        let limb_shift = bits / LIMB_BITS;
        let bit_shift = bits % LIMB_BITS;
        let mut limbs = vec![0u32; limb_shift];
        limbs.reserve(self.limbs.len() + 1);
        if bit_shift == 0 {
            limbs.extend_from_slice(&self.limbs);
        } else {
            let mut carry = 0u32;
            for limb in &self.limbs {
                limbs.push((limb << bit_shift) | carry);
                carry = limb >> (LIMB_BITS - bit_shift);
            }
            limbs.push(carry);
        }
        Self::from_limbs(limbs)
    }

    pub(crate) fn shifted_right(&self, bits: usize) -> Self {
        let limb_shift = bits / LIMB_BITS;
        let bit_shift = bits % LIMB_BITS;
        let Some(remaining) = self.limbs.get(limb_shift..) else {
            return Self::zero();
        };
        if bit_shift == 0 {
            return Self::from_limbs(remaining.to_vec());
        }
        let limbs = remaining
            .iter()
            .enumerate()
            .map(|(index, limb)| {
                let incoming = remaining
                    .get(index + 1)
                    .map_or(0, |higher| higher << (LIMB_BITS - bit_shift));
                (limb >> bit_shift) | incoming
            })
            .collect();
        Self::from_limbs(limbs)
    }

    pub(crate) fn bitwise(&self, other: &Self, combine: fn(u32, u32) -> u32) -> Self {
        let length = self.limbs.len().max(other.limbs.len());
        let limbs = (0..length)
            .map(|index| {
                combine(
                    self.limbs.get(index).copied().unwrap_or(0),
                    other.limbs.get(index).copied().unwrap_or(0),
                )
            })
            .collect();
        Self::from_limbs(limbs)
    }

    pub(crate) fn add(&self, other: &Self) -> Self {
        let (longer, shorter) = if self.limbs.len() >= other.limbs.len() {
            (&self.limbs, &other.limbs)
        } else {
            (&other.limbs, &self.limbs)
        };
        let mut limbs = Vec::with_capacity(longer.len() + 1);
        let mut carry = 0u64;
        for (index, limb) in longer.iter().enumerate() {
            let addend = shorter.get(index).map_or(0, |value| u64::from(*value));
            let sum = u64::from(*limb) + addend + carry;
            limbs.push(low_half(sum));
            carry = u64::from(high_half(sum));
        }
        limbs.push(low_half(carry));
        Self::from_limbs(limbs)
    }

    pub(crate) fn checked_sub(&self, other: &Self) -> Option<Self> {
        if *self < *other {
            return None;
        }
        let mut limbs = Vec::with_capacity(self.limbs.len());
        let mut borrow = false;
        for (index, limb) in self.limbs.iter().enumerate() {
            let subtrahend = other.limbs.get(index).copied().unwrap_or(0);
            let (partial, first_borrow) = limb.overflowing_sub(subtrahend);
            let (difference, second_borrow) = partial.overflowing_sub(u32::from(borrow));
            limbs.push(difference);
            borrow = first_borrow || second_borrow;
        }
        Some(Self::from_limbs(limbs))
    }

    pub(crate) fn mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self::zero();
        }
        let mut limbs = vec![0u32; self.limbs.len() + other.limbs.len()];
        for (left_index, left) in self.limbs.iter().enumerate() {
            let mut carry = 0u64;
            for (right_index, right) in other.limbs.iter().enumerate() {
                let position = left_index + right_index;
                let product =
                    u64::from(*left) * u64::from(*right) + u64::from(limbs[position]) + carry;
                limbs[position] = low_half(product);
                carry = u64::from(high_half(product));
            }
            limbs[left_index + other.limbs.len()] = low_half(carry);
        }
        Self::from_limbs(limbs)
    }

    pub(crate) fn pow(&self, exponent: u32) -> Self {
        let mut result = Self::one();
        let mut base = self.clone();
        let mut remaining = exponent;
        while remaining > 0 {
            if remaining & 1 == 1 {
                result = result.mul(&base);
            }
            remaining >>= 1;
            if remaining > 0 {
                base = base.mul(&base);
            }
        }
        result
    }

    pub(crate) fn div_rem(&self, divisor: &Self) -> Option<(Self, Self)> {
        match divisor.limbs.as_slice() {
            [] => None,
            [single] => {
                let (quotient, remainder) = self.div_rem_by_limb(*single);
                Some((quotient, Self::from_u64(u64::from(remainder))))
            }
            _ if *self < *divisor => Some((Self::zero(), self.clone())),
            _ => Some(self.div_rem_by_long_divisor(divisor)),
        }
    }

    fn div_rem_by_limb(&self, divisor: u32) -> (Self, u32) {
        let divisor = u64::from(divisor);
        let mut quotient = vec![0u32; self.limbs.len()];
        let mut remainder = 0u64;
        for (index, limb) in self.limbs.iter().enumerate().rev() {
            let current = (remainder << 32) | u64::from(*limb);
            quotient[index] = low_half(current / divisor);
            remainder = current % divisor;
        }
        (Self::from_limbs(quotient), low_half(remainder))
    }

    fn div_rem_by_long_divisor(&self, divisor: &Self) -> (Self, Self) {
        let normalization = divisor
            .limbs
            .last()
            .map_or(0, |top| usize_from_u32(top.leading_zeros()));
        let normalized_divisor = divisor.shifted_left(normalization).limbs;
        let mut work = self.shifted_left(normalization).limbs;
        work.resize(self.limbs.len() + 1, 0);

        let divisor_length = normalized_divisor.len();
        let quotient_length = work.len() - divisor_length;
        let divisor_top = u64::from(normalized_divisor[divisor_length - 1]);
        let divisor_next = u64::from(normalized_divisor[divisor_length - 2]);
        let limb_max = u64::from(u32::MAX);
        let mut quotient = vec![0u32; quotient_length];

        for position in (0..quotient_length).rev() {
            let leading = (u64::from(work[position + divisor_length]) << 32)
                | u64::from(work[position + divisor_length - 1]);
            let mut estimate = leading / divisor_top;
            let mut estimate_remainder = leading % divisor_top;
            while estimate > limb_max
                || estimate * divisor_next
                    > ((estimate_remainder << 32) | u64::from(work[position + divisor_length - 2]))
            {
                estimate -= 1;
                estimate_remainder += divisor_top;
                if estimate_remainder > limb_max {
                    break;
                }
            }

            let mut carry = 0u64;
            let mut borrow = false;
            for (offset, divisor_limb) in normalized_divisor.iter().enumerate() {
                let product = estimate * u64::from(*divisor_limb) + carry;
                carry = u64::from(high_half(product));
                let (partial, first_borrow) =
                    work[position + offset].overflowing_sub(low_half(product));
                let (difference, second_borrow) = partial.overflowing_sub(u32::from(borrow));
                work[position + offset] = difference;
                borrow = first_borrow || second_borrow;
            }
            let (partial, first_borrow) =
                work[position + divisor_length].overflowing_sub(low_half(carry));
            let (difference, second_borrow) = partial.overflowing_sub(u32::from(borrow));
            work[position + divisor_length] = difference;

            if first_borrow || second_borrow {
                estimate -= 1;
                let mut add_carry = 0u64;
                for (offset, divisor_limb) in normalized_divisor.iter().enumerate() {
                    let sum =
                        u64::from(work[position + offset]) + u64::from(*divisor_limb) + add_carry;
                    work[position + offset] = low_half(sum);
                    add_carry = u64::from(high_half(sum));
                }
                work[position + divisor_length] =
                    work[position + divisor_length].wrapping_add(low_half(add_carry));
            }
            quotient[position] = low_half(estimate);
        }

        work.truncate(divisor_length);
        let remainder = Self::from_limbs(work).shifted_right(normalization);
        (Self::from_limbs(quotient), remainder)
    }

    pub(crate) fn floor_sqrt(&self) -> Self {
        if self.is_zero() {
            return Self::zero();
        }
        let mut estimate = Self::one().shifted_left(self.bit_length().div_ceil(2));
        loop {
            let Some((quotient, _)) = self.div_rem(&estimate) else {
                return estimate;
            };
            let next = estimate.add(&quotient).shifted_right(1);
            if next >= estimate {
                return estimate;
            }
            estimate = next;
        }
    }

    pub(crate) fn gcd(&self, other: &Self) -> Self {
        let mut larger = self.clone();
        let mut smaller = other.clone();
        while let Some((_, remainder)) = larger.div_rem(&smaller) {
            larger = smaller;
            smaller = remainder;
        }
        larger
    }
}

impl Ord for Natural {
    fn cmp(&self, other: &Self) -> Ordering {
        self.limbs
            .len()
            .cmp(&other.limbs.len())
            .then_with(|| self.limbs.iter().rev().cmp(other.limbs.iter().rev()))
    }
}

impl PartialOrd for Natural {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SplitMix64 {
        state: u64,
    }

    impl SplitMix64 {
        fn next(&mut self) -> u64 {
            self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut mixed = self.state;
            mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            mixed ^ (mixed >> 31)
        }

        fn next_u128_with_varied_width(&mut self) -> u128 {
            let value = (u128::from(self.next()) << 64) | u128::from(self.next());
            let width_selector = self.next() % 129;
            match u32::try_from(width_selector) {
                Ok(0) | Err(_) => 0,
                Ok(128) => value,
                Ok(width) => value & ((1u128 << width) - 1),
            }
        }

        fn next_natural(&mut self, limb_count: usize) -> Natural {
            let limbs = (0..limb_count).map(|_| low_half(self.next())).collect();
            Natural::from_limbs(limbs)
        }
    }

    const PAIR_COUNT: usize = 2000;

    #[test]
    fn leading_zero_limbs_are_removed() {
        assert_eq!(Natural::from_limbs(vec![5, 0, 0]), Natural::from_u64(5));
    }

    #[test]
    fn u128_round_trips() {
        let value = 0xfedc_ba98_7654_3210_0123_4567_89ab_cdef_u128;

        assert_eq!(Natural::from_u128(value).to_u128(), Some(value));
    }

    #[test]
    fn bit_length_counts_from_highest_set_bit() {
        assert_eq!(Natural::from_u128(1u128 << 100).bit_length(), 101);
    }

    #[test]
    fn has_set_bit_below_ignores_bits_at_and_above_index() {
        let value = Natural::from_u128(1u128 << 40);

        assert!(!value.has_set_bit_below(40));
        assert!(value.has_set_bit_below(41));
    }

    #[test]
    fn shifts_match_u128() {
        let value = 0xdead_beef_cafe_babe_1234_u128;

        assert_eq!(
            Natural::from_u128(value).shifted_left(37).to_u128(),
            Some(value << 37)
        );
        assert_eq!(
            Natural::from_u128(value).shifted_right(45).to_u128(),
            Some(value >> 45)
        );
    }

    #[test]
    fn shift_right_beyond_length_is_zero() {
        assert!(Natural::from_u64(u64::MAX).shifted_right(64).is_zero());
    }

    #[test]
    fn addition_matches_u128() {
        let mut generator = SplitMix64 { state: 1 };
        for _ in 0..PAIR_COUNT {
            let left = generator.next_u128_with_varied_width() >> 1;
            let right = generator.next_u128_with_varied_width() >> 1;

            let sum = Natural::from_u128(left).add(&Natural::from_u128(right));

            assert_eq!(sum.to_u128(), Some(left + right));
        }
    }

    #[test]
    fn subtraction_matches_u128() {
        let mut generator = SplitMix64 { state: 2 };
        for _ in 0..PAIR_COUNT {
            let first = generator.next_u128_with_varied_width();
            let second = generator.next_u128_with_varied_width();
            let (larger, smaller) = (first.max(second), first.min(second));

            let difference = Natural::from_u128(larger).checked_sub(&Natural::from_u128(smaller));

            assert_eq!(
                difference.and_then(|value| value.to_u128()),
                Some(larger - smaller)
            );
        }
    }

    #[test]
    fn subtraction_below_zero_is_none() {
        assert_eq!(
            Natural::from_u64(3).checked_sub(&Natural::from_u64(4)),
            None
        );
    }

    #[test]
    fn multiplication_matches_u128() {
        let mut generator = SplitMix64 { state: 3 };
        for _ in 0..PAIR_COUNT {
            let left = generator.next() >> (generator.next() % 64);
            let right = generator.next() >> (generator.next() % 64);

            let product = Natural::from_u64(left).mul(&Natural::from_u64(right));

            assert_eq!(
                product.to_u128(),
                Some(u128::from(left) * u128::from(right))
            );
        }
    }

    #[test]
    fn power_matches_repeated_multiplication() {
        let expected = Natural::from_u64(10)
            .mul(&Natural::from_u64(10))
            .mul(&Natural::from_u64(10))
            .mul(&Natural::from_u64(10))
            .mul(&Natural::from_u64(10));

        assert_eq!(Natural::from_u64(10).pow(5), expected);
    }

    #[test]
    fn division_matches_u128() {
        let mut generator = SplitMix64 { state: 4 };
        for _ in 0..PAIR_COUNT {
            let dividend = generator.next_u128_with_varied_width();
            let divisor = generator.next_u128_with_varied_width().max(1);

            let (quotient, remainder) = Natural::from_u128(dividend)
                .div_rem(&Natural::from_u128(divisor))
                .unwrap();

            assert_eq!(quotient.to_u128(), Some(dividend / divisor));
            assert_eq!(remainder.to_u128(), Some(dividend % divisor));
        }
    }

    #[test]
    fn division_of_long_values_reconstructs_dividend() {
        let mut generator = SplitMix64 { state: 5 };
        for round in 0..PAIR_COUNT {
            let dividend = generator.next_natural(4 + round % 9);
            let divisor = generator.next_natural(2 + round % 5);
            if divisor.is_zero() {
                continue;
            }

            let (quotient, remainder) = dividend.div_rem(&divisor).unwrap();

            assert!(remainder < divisor);
            assert_eq!(quotient.mul(&divisor).add(&remainder), dividend);
        }
    }

    #[test]
    fn division_needing_add_back_step_is_exact() {
        let dividend = Natural::from_limbs(vec![0, 0, 0x8000_0000, 0x7fff_ffff]);
        let divisor = Natural::from_limbs(vec![1, 0, 0x8000_0000]);

        let (quotient, remainder) = dividend.div_rem(&divisor).unwrap();

        assert_eq!(quotient.mul(&divisor).add(&remainder), dividend);
        assert!(remainder < divisor);
    }

    #[test]
    fn division_by_zero_is_none() {
        assert_eq!(Natural::from_u64(7).div_rem(&Natural::zero()), None);
    }

    #[test]
    fn floor_sqrt_matches_integer_square_root() {
        let mut generator = SplitMix64 { state: 6 };
        for _ in 0..PAIR_COUNT {
            let value = generator.next_u128_with_varied_width();

            let root = Natural::from_u128(value).floor_sqrt().to_u128().unwrap();

            assert!(root * root <= value);
            assert!(
                (root + 1)
                    .checked_mul(root + 1)
                    .is_none_or(|square| square > value)
            );
        }
    }

    #[test]
    fn floor_sqrt_of_perfect_square_is_exact() {
        let root = Natural::from_u128(0xffff_ffff_ffff_ffff_u128);

        assert_eq!(root.mul(&root).floor_sqrt(), root);
    }

    #[test]
    fn gcd_matches_euclid_on_small_values() {
        assert_eq!(
            Natural::from_u64(1071).gcd(&Natural::from_u64(462)),
            Natural::from_u64(21)
        );
    }

    #[test]
    fn gcd_with_zero_is_other_value() {
        assert_eq!(
            Natural::zero().gcd(&Natural::from_u64(9)),
            Natural::from_u64(9)
        );
    }

    #[test]
    fn ordering_compares_by_magnitude() {
        assert!(Natural::from_u128(1u128 << 64) > Natural::from_u64(u64::MAX));
    }
}
