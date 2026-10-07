const GOLDEN_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;
const FIRST_MIX: u64 = 0xbf58_476d_1ce4_e5b9;
const SECOND_MIX: u64 = 0x94d0_49bb_1331_11eb;
const FIRST_SHIFT: u32 = 30;
const SECOND_SHIFT: u32 = 27;
const THIRD_SHIFT: u32 = 31;
const LOW_HALF_MASK: u64 = 0xffff_ffff;

pub struct SplitMix {
    state: u64,
}

impl SplitMix {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_bits(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_GAMMA);
        let mut mixed = self.state;
        mixed = (mixed ^ (mixed >> FIRST_SHIFT)).wrapping_mul(FIRST_MIX);
        mixed = (mixed ^ (mixed >> SECOND_SHIFT)).wrapping_mul(SECOND_MIX);
        mixed ^ (mixed >> THIRD_SHIFT)
    }

    pub fn next_f64(&mut self) -> f64 {
        f64::from_bits(self.next_bits())
    }

    pub fn next_f32(&mut self) -> f32 {
        f32::from_bits(u32::try_from(self.next_bits() & LOW_HALF_MASK).unwrap_or_default())
    }
}

pub fn special_f64_values() -> Vec<f64> {
    vec![
        0.0,
        -0.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        -f64::NAN,
        f64::from_bits(1),
        f64::from_bits(0x000f_ffff_ffff_ffff),
        f64::MIN_POSITIVE,
        f64::MAX,
        f64::MIN,
        1.0f64.next_down(),
        1.0f64.next_up(),
        1.0,
        -1.0,
        0.5,
        2.5,
        -1.0f64.next_up(),
    ]
}

pub fn special_f32_values() -> Vec<f32> {
    vec![
        0.0,
        -0.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        -f32::NAN,
        f32::from_bits(1),
        f32::from_bits(0x007f_ffff),
        f32::MIN_POSITIVE,
        f32::MAX,
        f32::MIN,
        1.0f32.next_down(),
        1.0f32.next_up(),
        1.0,
        -1.0,
        0.5,
        2.5,
        -1.0f32.next_up(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_gives_same_sequence() {
        let mut first = SplitMix::new(7);
        let mut second = SplitMix::new(7);

        let pairs: Vec<(u64, u64)> = (0..4)
            .map(|_| (first.next_bits(), second.next_bits()))
            .collect();

        assert!(pairs.iter().all(|(left, right)| left == right));
    }

    #[test]
    fn seed_zero_matches_published_first_output() {
        let mut generator = SplitMix::new(0);

        let first = generator.next_bits();

        assert_eq!(first, 0xe220_a839_7b1d_cdaf);
    }
}
