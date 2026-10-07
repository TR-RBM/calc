use calc_kernels::{philox4x32_10, philox4x32_10_at, philox4x32_10_fill};

const KNOWN_ANSWERS: &str = include_str!("data/random123-kat-philox4x32-10.txt");

struct KnownAnswer {
    counter: [u32; 4],
    key: [u32; 2],
    output: [u32; 4],
}

fn known_answers() -> Vec<KnownAnswer> {
    KNOWN_ANSWERS
        .lines()
        .filter(|line| line.starts_with("philox4x32 10 "))
        .map(|line| {
            let words: Vec<u32> = line
                .split_whitespace()
                .skip(2)
                .map(|word| u32::from_str_radix(word, 16).unwrap())
                .collect();
            KnownAnswer {
                counter: [words[0], words[1], words[2], words[3]],
                key: [words[4], words[5]],
                output: [words[6], words[7], words[8], words[9]],
            }
        })
        .collect()
}

fn joined(high: u32, low: u32) -> u64 {
    (u64::from(high) << 32) | u64::from(low)
}

#[test]
fn every_published_vector_is_reproduced() {
    let answers = known_answers();

    assert_eq!(answers.len(), 3);
    for answer in answers {
        assert_eq!(philox4x32_10(answer.counter, answer.key), answer.output);
    }
}

#[test]
fn seed_stream_and_index_place_their_words_low_first() {
    for answer in known_answers() {
        let seed = joined(answer.key[1], answer.key[0]);
        let index = joined(answer.counter[1], answer.counter[0]);
        let stream = joined(answer.counter[3], answer.counter[2]);

        assert_eq!(philox4x32_10_at(seed, stream, index), answer.output);
    }
}

#[test]
fn the_filled_slice_holds_the_blocks_of_consecutive_indices() {
    let mut words = [0_u32; 10];

    philox4x32_10_fill(7, 3, 40, &mut words);

    let mut expected = Vec::new();
    for index in 40..43 {
        expected.extend(philox4x32_10_at(7, 3, index));
    }
    assert_eq!(words.as_slice(), &expected[..10]);
}

#[test]
fn the_index_wraps_within_its_stream_after_the_last_one() {
    let mut words = [0_u32; 8];

    philox4x32_10_fill(1, 2, u64::MAX, &mut words);

    assert_eq!(words[..4], philox4x32_10_at(1, 2, u64::MAX));
    assert_eq!(words[4..], philox4x32_10_at(1, 2, 0));
}

#[test]
fn an_empty_slice_is_left_alone() {
    let mut words: [u32; 0] = [];

    philox4x32_10_fill(1, 2, 3, &mut words);

    assert!(words.is_empty());
}

#[test]
fn the_gp_oracle_values_are_reproduced_bit_for_bit() {
    for (seed, stream, index, expected) in [
        (
            1,
            2,
            3,
            [0xde08_bf52, 0x663e_ff4f, 0x8759_c4e2, 0xbdd5_e548],
        ),
        (
            7,
            3,
            40,
            [0xc350_cafb, 0xa0f1_5ce6, 0xa735_c1af, 0xade9_bd81],
        ),
        (
            123_456_789,
            0,
            (1 << 40) + 5,
            [0x31c6_bec0, 0x65f1_4307, 0x57df_818d, 0x66aa_6da5],
        ),
        (
            1 << 63,
            1 << 32,
            (1 << 32) - 1,
            [0x996a_04db, 0x09e6_3ffe, 0x2a29_5467, 0x5802_19e6],
        ),
    ] {
        assert_eq!(philox4x32_10_at(seed, stream, index), expected);
    }
}
