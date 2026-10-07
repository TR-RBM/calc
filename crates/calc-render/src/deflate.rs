const WINDOW: usize = 32_768;
const HASH_BITS: u32 = 15;
const HASH_SIZE: usize = 1 << HASH_BITS;
const HASH_MASK: usize = HASH_SIZE - 1;
const MATCH_LEAST: usize = 3;
const MATCH_MOST: usize = 258;
const CHAIN_MOST: usize = 128;
const NO_POSITION: usize = usize::MAX;
const END_OF_BLOCK: u16 = 256;
const FIRST_LENGTH_SYMBOL: u16 = 257;
const FIXED_BLOCK_HEADER: u32 = 0b01;
const FINAL_BLOCK: u32 = 1;

const LENGTH_BASE: [usize; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DISTANCE_BASE: [usize; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12_289, 16_385, 24_577,
];
const DISTANCE_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

struct Bits {
    bytes: Vec<u8>,
    held: u32,
    count: u32,
}

impl Bits {
    fn new(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
            held: 0,
            count: 0,
        }
    }

    fn push(&mut self, value: u32, width: u32) {
        self.held |= (value & ((1 << width) - 1)) << self.count;
        self.count += width;
        while self.count >= 8 {
            let byte = u8::try_from(self.held & 0xff).unwrap_or_default();
            self.bytes.push(byte);
            self.held >>= 8;
            self.count -= 8;
        }
    }

    fn push_code(&mut self, code: u32, width: u32) {
        let mut reversed = 0;
        for bit in 0..width {
            reversed |= ((code >> (width - 1 - bit)) & 1) << bit;
        }
        self.push(reversed, width);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.count > 0 {
            let byte = u8::try_from(self.held & 0xff).unwrap_or_default();
            self.bytes.push(byte);
        }
        self.bytes
    }
}

fn literal_code(symbol: u16) -> (u32, u32) {
    let symbol = u32::from(symbol);
    match symbol {
        0..=143 => (0x30 + symbol, 8),
        144..=255 => (0x190 + symbol - 144, 9),
        256..=279 => (symbol - 256, 7),
        _ => (0xc0 + symbol - 280, 8),
    }
}

fn length_symbol(length: usize) -> (u16, u32, u32) {
    let index = LENGTH_BASE
        .iter()
        .rposition(|base| *base <= length)
        .unwrap_or(0);
    let extra = LENGTH_EXTRA[index];
    let offset = u32::try_from(length - LENGTH_BASE[index]).unwrap_or_default();
    (
        FIRST_LENGTH_SYMBOL + u16::try_from(index).unwrap_or_default(),
        offset,
        extra,
    )
}

fn distance_symbol(distance: usize) -> (u32, u32, u32) {
    let index = DISTANCE_BASE
        .iter()
        .rposition(|base| *base <= distance)
        .unwrap_or(0);
    let extra = DISTANCE_EXTRA[index];
    let offset = u32::try_from(distance - DISTANCE_BASE[index]).unwrap_or_default();
    (u32::try_from(index).unwrap_or_default(), offset, extra)
}

fn hash_of(data: &[u8], position: usize) -> usize {
    let first = usize::from(data[position]);
    let second = usize::from(data[position + 1]);
    let third = usize::from(data[position + 2]);
    ((first << 10) ^ (second << 5) ^ third) & HASH_MASK
}

fn match_length(data: &[u8], earlier: usize, position: usize, most: usize) -> usize {
    let mut length = 0;
    while length < most && data[earlier + length] == data[position + length] {
        length += 1;
    }
    length
}

fn longest_match(
    data: &[u8],
    heads: &[usize],
    chain: &[usize],
    position: usize,
) -> Option<(usize, usize)> {
    let most = MATCH_MOST.min(data.len() - position);
    if most < MATCH_LEAST {
        return None;
    }
    let mut earlier = heads[hash_of(data, position)];
    let mut best: Option<(usize, usize)> = None;
    let mut steps = 0;
    while earlier != NO_POSITION && steps < CHAIN_MOST {
        if position - earlier > WINDOW {
            break;
        }
        let length = match_length(data, earlier, position, most);
        if length >= MATCH_LEAST && best.is_none_or(|(kept, _)| length > kept) {
            best = Some((length, position - earlier));
            if length == most {
                break;
            }
        }
        earlier = chain[earlier & (WINDOW - 1)];
        steps += 1;
    }
    best
}

pub(crate) fn deflate(data: &[u8]) -> Vec<u8> {
    let mut bits = Bits::new(data.len() / 2 + 16);
    bits.push(FINAL_BLOCK, 1);
    bits.push(FIXED_BLOCK_HEADER, 2);
    let mut heads = vec![NO_POSITION; HASH_SIZE];
    let mut chain = vec![NO_POSITION; WINDOW];
    let mut position = 0;
    while position < data.len() {
        let found = if position + MATCH_LEAST <= data.len() {
            longest_match(data, &heads, &chain, position)
        } else {
            None
        };
        let step = match found {
            Some((length, distance)) => {
                let (symbol, offset, extra) = length_symbol(length);
                let (code, width) = literal_code(symbol);
                bits.push_code(code, width);
                if extra > 0 {
                    bits.push(offset, extra);
                }
                let (code, offset, extra) = distance_symbol(distance);
                bits.push_code(code, 5);
                if extra > 0 {
                    bits.push(offset, extra);
                }
                length
            }
            None => {
                let (code, width) = literal_code(u16::from(data[position]));
                bits.push_code(code, width);
                1
            }
        };
        for index in position..position + step {
            if index + MATCH_LEAST <= data.len() {
                let hash = hash_of(data, index);
                chain[index & (WINDOW - 1)] = heads[hash];
                heads[hash] = index;
            }
        }
        position += step;
    }
    let (code, width) = literal_code(END_OF_BLOCK);
    bits.push_code(code, width);
    bits.finish()
}

#[cfg(test)]
pub(crate) fn inflate(stream: &[u8]) -> Vec<u8> {
    let mut reader = BitReader {
        bytes: stream,
        position: 0,
        bit: 0,
    };
    let mut data: Vec<u8> = Vec::new();
    loop {
        let is_final = reader.take(1) == 1;
        let kind = reader.take(2);
        assert_eq!(kind, FIXED_BLOCK_HEADER, "only fixed blocks are written");
        loop {
            let symbol = reader.symbol();
            if symbol == END_OF_BLOCK {
                break;
            }
            if symbol < END_OF_BLOCK {
                data.push(u8::try_from(symbol).expect("a literal byte"));
                continue;
            }
            let index = usize::from(symbol - FIRST_LENGTH_SYMBOL);
            let length = LENGTH_BASE[index]
                + usize::try_from(reader.take(LENGTH_EXTRA[index])).expect("extra bits");
            let code = usize::try_from(reader.take_code(5)).expect("a distance code");
            let distance = DISTANCE_BASE[code]
                + usize::try_from(reader.take(DISTANCE_EXTRA[code])).expect("extra bits");
            for _ in 0..length {
                let byte = data[data.len() - distance];
                data.push(byte);
            }
        }
        if is_final {
            return data;
        }
    }
}

#[cfg(test)]
struct BitReader<'a> {
    bytes: &'a [u8],
    position: usize,
    bit: u32,
}

#[cfg(test)]
impl BitReader<'_> {
    fn take(&mut self, width: u32) -> u32 {
        let mut value = 0;
        for index in 0..width {
            value |= self.next_bit() << index;
        }
        value
    }

    fn take_code(&mut self, width: u32) -> u32 {
        let mut value = 0;
        for _ in 0..width {
            value = (value << 1) | self.next_bit();
        }
        value
    }

    fn next_bit(&mut self) -> u32 {
        let byte = u32::from(self.bytes[self.position]);
        let bit = (byte >> self.bit) & 1;
        self.bit += 1;
        if self.bit == 8 {
            self.bit = 0;
            self.position += 1;
        }
        bit
    }

    fn symbol(&mut self) -> u16 {
        let mut code = self.take_code(7);
        if code <= 0b001_0111 {
            return u16::try_from(code).expect("an end or length symbol") + 256;
        }
        code = (code << 1) | self.next_bit();
        if (0b0011_0000..=0b1011_1111).contains(&code) {
            return u16::try_from(code - 0b0011_0000).expect("a literal");
        }
        if (0b1100_0000..=0b1100_0111).contains(&code) {
            return u16::try_from(code - 0b1100_0000).expect("a length symbol") + 280;
        }
        code = (code << 1) | self.next_bit();
        u16::try_from(code - 0b1_1001_0000).expect("a high literal") + 144
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(data: &[u8]) -> Vec<u8> {
        inflate(&deflate(data))
    }

    #[test]
    fn literals_come_back_unchanged() {
        let data: Vec<u8> = (0..=255).collect();

        assert_eq!(round_trip(&data), data);
    }

    #[test]
    fn a_repeated_run_comes_back_unchanged() {
        let data = vec![7_u8; 1000];

        assert_eq!(round_trip(&data), data);
    }

    #[test]
    fn a_repeated_run_is_shorter_than_the_data() {
        let data = vec![7_u8; 1000];

        assert!(deflate(&data).len() < data.len() / 10);
    }

    #[test]
    fn text_with_repeats_comes_back_unchanged() {
        let mut data = Vec::new();
        for index in 0..500 {
            data.extend_from_slice(b"the picture carries its unit ");
            data.push(u8::try_from(index % 251).expect("a byte"));
        }

        assert_eq!(round_trip(&data), data);
    }

    #[test]
    fn a_match_at_the_furthest_distance_comes_back_unchanged() {
        let mut data = vec![0_u8; WINDOW];
        data[..8].copy_from_slice(b"boundary");
        data.extend_from_slice(b"boundary");

        assert_eq!(round_trip(&data), data);
    }

    #[test]
    fn empty_data_comes_back_empty() {
        assert_eq!(round_trip(&[]), Vec::<u8>::new());
    }

    #[test]
    fn the_same_data_gives_the_same_stream() {
        let data: Vec<u8> = (0..4096)
            .map(|index| u8::try_from(index % 37).unwrap())
            .collect();

        assert_eq!(deflate(&data), deflate(&data));
    }
}
