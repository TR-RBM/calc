use calc_kernels::{
    RADIX_BUCKETS, RadixPasses, RadixRefusal, philox4x32_10_fill, radix_sort_f32, radix_sort_u32,
    total_order_bits,
};

fn sorted_u32(keys: &[u32]) -> (Vec<u32>, Vec<u32>, RadixPasses) {
    let mut keys = keys.to_vec();
    let mut payload: Vec<u32> = (0..u32::try_from(keys.len()).unwrap()).collect();
    let mut scratch_keys = vec![0; keys.len()];
    let mut scratch_payload = vec![0; keys.len()];
    let mut histogram = [0; RADIX_BUCKETS];
    let passes = radix_sort_u32(
        &mut keys,
        &mut payload,
        &mut scratch_keys,
        &mut scratch_payload,
        &mut histogram,
    )
    .unwrap();
    (keys, payload, passes)
}

fn stable_reference(keys: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let mut pairs: Vec<(u32, u32)> = keys
        .iter()
        .enumerate()
        .map(|(index, key)| (*key, u32::try_from(index).unwrap()))
        .collect();
    pairs.sort_by_key(|(key, _)| *key);
    pairs.into_iter().unzip()
}

fn random_words(seed: u64, length: usize) -> Vec<u32> {
    let mut words = vec![0; length];
    philox4x32_10_fill(seed, 0, 0, &mut words);
    words
}

#[test]
fn random_keys_sort_as_a_stable_sort_does() {
    for (seed, length) in [(1, 0), (2, 1), (3, 7), (4, 256), (5, 1000), (6, 65_537)] {
        let keys = random_words(seed, length);

        let (sorted, payload, _) = sorted_u32(&keys);

        assert_eq!((sorted, payload), stable_reference(&keys), "seed {seed}");
    }
}

#[test]
fn equal_keys_keep_their_input_order() {
    let keys: Vec<u32> = random_words(7, 5000)
        .into_iter()
        .map(|word| word % 5)
        .collect();

    let (sorted, payload, _) = sorted_u32(&keys);

    assert_eq!((sorted, payload), stable_reference(&keys));
}

#[test]
fn sorted_reversed_and_extreme_keys_sort() {
    let ascending: Vec<u32> = (0..3000).map(|key| key * 1_431_655).collect();
    let descending: Vec<u32> = ascending.iter().rev().copied().collect();
    let extremes = vec![u32::MAX, 0, u32::MAX, 1, 0x8000_0000, 0x7FFF_FFFF];
    for keys in [ascending, descending, extremes] {
        let (sorted, payload, _) = sorted_u32(&keys);

        assert_eq!((sorted, payload), stable_reference(&keys));
    }
}

#[test]
fn a_pass_whose_digit_is_the_same_everywhere_is_skipped() {
    let small: Vec<u32> = random_words(8, 1000)
        .into_iter()
        .map(|word| word % 256)
        .collect();
    let equal = vec![0xABCD_EF01; 100];

    assert_eq!(
        sorted_u32(&small).2,
        RadixPasses {
            made: 1,
            skipped: 3
        }
    );
    assert_eq!(
        sorted_u32(&equal).2,
        RadixPasses {
            made: 0,
            skipped: 4
        }
    );
}

#[test]
fn slices_of_different_lengths_are_refused() {
    let mut keys = vec![3, 1, 2];
    let mut payload = vec![0, 1];
    let (mut scratch_keys, mut scratch_payload) = (vec![0; 3], vec![0; 3]);
    let mut histogram = [0; RADIX_BUCKETS];

    let refused = radix_sort_u32(
        &mut keys,
        &mut payload,
        &mut scratch_keys,
        &mut scratch_payload,
        &mut histogram,
    );

    assert_eq!(refused, Err(RadixRefusal::LengthsDiffer));
    assert_eq!(keys, vec![3, 1, 2]);
}

#[test]
fn floats_are_refused_before_anything_is_written() {
    let mut keys = vec![3.0_f32, -1.0, 0.5];
    let mut payload = vec![0, 1];
    let mut ordered = vec![7; 3];
    let (mut scratch_keys, mut scratch_payload) = (vec![7; 3], vec![7; 3]);
    let mut histogram = [7; RADIX_BUCKETS];

    let refused = radix_sort_f32(
        &mut keys,
        &mut payload,
        &mut ordered,
        &mut scratch_keys,
        &mut scratch_payload,
        &mut histogram,
    );

    assert_eq!(refused, Err(RadixRefusal::LengthsDiffer));
    assert_eq!(keys, vec![3.0, -1.0, 0.5]);
    assert_eq!(payload, vec![0, 1]);
    assert_eq!(
        (ordered, scratch_keys, scratch_payload),
        (vec![7; 3], vec![7; 3], vec![7; 3])
    );
    assert_eq!(histogram, [7; RADIX_BUCKETS]);
}

#[test]
fn floats_sort_in_ieee_total_order_with_every_bit_pattern_kept() {
    let mut keys: Vec<f32> = random_words(9, 4000)
        .into_iter()
        .map(f32::from_bits)
        .collect();
    keys.extend([
        0.0,
        -0.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        -f32::NAN,
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        f32::from_bits(1),
        1.5,
        1.5,
    ]);
    let mut expected: Vec<(u32, u32)> = keys
        .iter()
        .enumerate()
        .map(|(index, key)| (key.to_bits(), u32::try_from(index).unwrap()))
        .collect();
    expected.sort_by(|left, right| f32::from_bits(left.0).total_cmp(&f32::from_bits(right.0)));
    let mut payload: Vec<u32> = (0..u32::try_from(keys.len()).unwrap()).collect();
    let length = keys.len();
    let (mut ordered, mut scratch_keys, mut scratch_payload) =
        (vec![0; length], vec![0; length], vec![0; length]);
    let mut histogram = [0; RADIX_BUCKETS];

    radix_sort_f32(
        &mut keys,
        &mut payload,
        &mut ordered,
        &mut scratch_keys,
        &mut scratch_payload,
        &mut histogram,
    )
    .unwrap();

    let found: Vec<(u32, u32)> = keys.iter().map(|key| key.to_bits()).zip(payload).collect();
    assert_eq!(found, expected);
}

#[test]
fn the_total_order_bits_rank_negative_zero_before_positive_zero() {
    assert!(total_order_bits(-0.0) < total_order_bits(0.0));
    assert!(total_order_bits(-f32::NAN) < total_order_bits(f32::NEG_INFINITY));
    assert!(total_order_bits(f32::INFINITY) < total_order_bits(f32::NAN));
}
