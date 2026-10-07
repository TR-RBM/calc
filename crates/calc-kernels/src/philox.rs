pub const PHILOX4X32_10_ROUNDS: u32 = 10;

const MULTIPLIER_0: u32 = 0xD251_1F53;
const MULTIPLIER_1: u32 = 0xCD9E_8D57;
const KEY_STEP_0: u32 = 0x9E37_79B9;
const KEY_STEP_1: u32 = 0xBB67_AE85;
const WORDS_PER_BLOCK: usize = 4;

fn high_and_low(first: u32, second: u32) -> (u32, u32) {
    let product = u64::from(first) * u64::from(second);
    let [low, high] = split(product);
    (high, low)
}

fn split(value: u64) -> [u32; 2] {
    let bytes = value.to_le_bytes();
    [
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
    ]
}

fn round(counter: [u32; 4], key: [u32; 2]) -> [u32; 4] {
    let (high_0, low_0) = high_and_low(MULTIPLIER_0, counter[0]);
    let (high_1, low_1) = high_and_low(MULTIPLIER_1, counter[2]);
    [
        high_1 ^ counter[1] ^ key[0],
        low_1,
        high_0 ^ counter[3] ^ key[1],
        low_0,
    ]
}

pub fn philox4x32_10(counter: [u32; 4], key: [u32; 2]) -> [u32; 4] {
    let mut block = round(counter, key);
    let mut key = key;
    for _ in 1..PHILOX4X32_10_ROUNDS {
        key = [
            key[0].wrapping_add(KEY_STEP_0),
            key[1].wrapping_add(KEY_STEP_1),
        ];
        block = round(block, key);
    }
    block
}

pub fn philox4x32_10_at(seed: u64, stream: u64, index: u64) -> [u32; 4] {
    let [index_low, index_high] = split(index);
    let [stream_low, stream_high] = split(stream);
    philox4x32_10(
        [index_low, index_high, stream_low, stream_high],
        split(seed),
    )
}

pub fn philox4x32_10_fill(seed: u64, stream: u64, first_index: u64, words: &mut [u32]) {
    let mut index = first_index;
    for chunk in words.chunks_mut(WORDS_PER_BLOCK) {
        let block = philox4x32_10_at(seed, stream, index);
        chunk.copy_from_slice(&block[..chunk.len()]);
        index = index.wrapping_add(1);
    }
}
