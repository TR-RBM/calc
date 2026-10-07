pub const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
pub const IHDR: [u8; 4] = *b"IHDR";
pub const IDAT: [u8; 4] = *b"IDAT";
pub const IEND: [u8; 4] = *b"IEND";
const CRC_POLYNOMIAL: u32 = 0xedb8_8320;
const ADLER_MODULUS: u32 = 65_521;
const ZLIB_CHECK_DIVISOR: u16 = 31;
const DEFLATE_METHOD: u8 = 8;
const LOW_NIBBLE: u8 = 0x0f;
const PRESET_DICTIONARY_FLAG: u8 = 0x20;
const FINAL_BLOCK_FLAG: u8 = 1;
const BLOCK_TYPE_SHIFT: u32 = 1;
const BLOCK_TYPE_MASK: u8 = 3;
const STORED_BLOCK_TYPE: u8 = 0;
const FIXED_BLOCK_TYPE: u8 = 1;
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
const ZLIB_HEADER_LENGTH: usize = 2;
const ADLER_LENGTH: usize = 4;
const CHUNK_LENGTH_BYTES: usize = 4;
const CHUNK_KIND_BYTES: usize = 4;
const CHUNK_CRC_BYTES: usize = 4;

pub fn pam_header(width: u32, height: u32) -> Vec<u8> {
    format!("P7\nWIDTH {width}\nHEIGHT {height}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n")
        .into_bytes()
}

pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ CRC_POLYNOMIAL
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

pub fn adler32(bytes: &[u8]) -> u32 {
    let (mut low, mut high) = (1u32, 0u32);
    for byte in bytes {
        low = (low + u32::from(*byte)) % ADLER_MODULUS;
        high = (high + low) % ADLER_MODULUS;
    }
    (high << 16) | low
}

fn big_endian_u32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(..4)?.try_into().ok()?))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PngChunk {
    pub kind: [u8; 4],
    pub data: Vec<u8>,
    pub stored_crc: u32,
}

impl PngChunk {
    pub fn has_valid_crc(&self) -> bool {
        let mut covered = self.kind.to_vec();
        covered.extend_from_slice(&self.data);
        crc32(&covered) == self.stored_crc
    }
}

pub fn png_chunks(bytes: &[u8]) -> Option<Vec<PngChunk>> {
    let mut rest = bytes.strip_prefix(&PNG_SIGNATURE)?;
    let mut chunks = Vec::new();
    while !rest.is_empty() {
        let length = usize::try_from(big_endian_u32(rest)?).ok()?;
        let kind_start = CHUNK_LENGTH_BYTES;
        let data_start = kind_start + CHUNK_KIND_BYTES;
        let data_end = data_start + length;
        let kind: [u8; 4] = rest.get(kind_start..data_start)?.try_into().ok()?;
        let data = rest.get(data_start..data_end)?.to_vec();
        let stored_crc = big_endian_u32(rest.get(data_end..)?)?;
        chunks.push(PngChunk {
            kind,
            data,
            stored_crc,
        });
        rest = rest.get(data_end + CHUNK_CRC_BYTES..)?;
    }
    Some(chunks)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageHeader {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub colour_type: u8,
    pub compression: u8,
    pub filter: u8,
    pub interlace: u8,
}

pub fn image_header(data: &[u8]) -> Option<ImageHeader> {
    let fields = data.get(8..13)?;
    Some(ImageHeader {
        width: big_endian_u32(data)?,
        height: big_endian_u32(data.get(4..)?)?,
        bit_depth: fields[0],
        colour_type: fields[1],
        compression: fields[2],
        filter: fields[3],
        interlace: fields[4],
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InflateFault {
    Truncated,
    NotDeflate,
    PresetDictionary,
    HeaderCheck,
    NotStoredBlock,
    NotFixedBlock,
    UnknownSymbol,
    DistanceBeyondOutput,
    LengthComplement,
    TrailingBytes,
    AdlerMismatch,
}

pub fn inflate_stored(zlib: &[u8]) -> Result<Vec<u8>, InflateFault> {
    let header = zlib
        .get(..ZLIB_HEADER_LENGTH)
        .ok_or(InflateFault::Truncated)?;
    if header[0] & LOW_NIBBLE != DEFLATE_METHOD {
        return Err(InflateFault::NotDeflate);
    }
    if header[1] & PRESET_DICTIONARY_FLAG != 0 {
        return Err(InflateFault::PresetDictionary);
    }
    if u16::from_be_bytes([header[0], header[1]]) % ZLIB_CHECK_DIVISOR != 0 {
        return Err(InflateFault::HeaderCheck);
    }
    let mut position = ZLIB_HEADER_LENGTH;
    let mut output = Vec::new();
    loop {
        let block = *zlib.get(position).ok_or(InflateFault::Truncated)?;
        if (block >> BLOCK_TYPE_SHIFT) & BLOCK_TYPE_MASK != STORED_BLOCK_TYPE {
            return Err(InflateFault::NotStoredBlock);
        }
        let lengths = zlib
            .get(position + 1..position + 5)
            .ok_or(InflateFault::Truncated)?;
        let length = u16::from_le_bytes([lengths[0], lengths[1]]);
        let complement = u16::from_le_bytes([lengths[2], lengths[3]]);
        if length != !complement {
            return Err(InflateFault::LengthComplement);
        }
        let start = position + 5;
        let end = start + usize::from(length);
        output.extend_from_slice(zlib.get(start..end).ok_or(InflateFault::Truncated)?);
        position = end;
        if block & FINAL_BLOCK_FLAG == FINAL_BLOCK_FLAG {
            break;
        }
    }
    let trailer = zlib.get(position..).ok_or(InflateFault::Truncated)?;
    if trailer.len() != ADLER_LENGTH {
        return Err(InflateFault::TrailingBytes);
    }
    if big_endian_u32(trailer) != Some(adler32(&output)) {
        return Err(InflateFault::AdlerMismatch);
    }
    Ok(output)
}

pub fn inflate_fixed(zlib: &[u8]) -> Result<Vec<u8>, InflateFault> {
    let header = zlib
        .get(..ZLIB_HEADER_LENGTH)
        .ok_or(InflateFault::Truncated)?;
    if header[0] & LOW_NIBBLE != DEFLATE_METHOD {
        return Err(InflateFault::NotDeflate);
    }
    if u16::from_be_bytes([header[0], header[1]]) % ZLIB_CHECK_DIVISOR != 0 {
        return Err(InflateFault::HeaderCheck);
    }
    let body = zlib
        .get(ZLIB_HEADER_LENGTH..zlib.len().saturating_sub(ADLER_LENGTH))
        .ok_or(InflateFault::Truncated)?;
    let mut bits = Reader {
        bytes: body,
        position: 0,
        bit: 0,
    };
    let mut output: Vec<u8> = Vec::new();
    loop {
        let is_final = bits.take(1)? == 1;
        if u8::try_from(bits.take(2)?).unwrap_or(u8::MAX) != FIXED_BLOCK_TYPE {
            return Err(InflateFault::NotFixedBlock);
        }
        loop {
            let symbol = bits.symbol()?;
            if symbol == 256 {
                break;
            }
            if symbol < 256 {
                output.push(u8::try_from(symbol).unwrap_or_default());
                continue;
            }
            let index = usize::from(symbol) - 257;
            let base = *LENGTH_BASE.get(index).ok_or(InflateFault::UnknownSymbol)?;
            let length = base + usize::try_from(bits.take(LENGTH_EXTRA[index])?).unwrap_or(0);
            let code = usize::try_from(bits.take_code(5)?).unwrap_or(0);
            let base = *DISTANCE_BASE.get(code).ok_or(InflateFault::UnknownSymbol)?;
            let distance = base + usize::try_from(bits.take(DISTANCE_EXTRA[code])?).unwrap_or(0);
            if distance > output.len() {
                return Err(InflateFault::DistanceBeyondOutput);
            }
            for _ in 0..length {
                let byte = output[output.len() - distance];
                output.push(byte);
            }
        }
        if is_final {
            break;
        }
    }
    let trailer = zlib
        .get(zlib.len().saturating_sub(ADLER_LENGTH)..)
        .ok_or(InflateFault::Truncated)?;
    if big_endian_u32(trailer) != Some(adler32(&output)) {
        return Err(InflateFault::AdlerMismatch);
    }
    Ok(output)
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
    bit: u32,
}

impl Reader<'_> {
    fn next_bit(&mut self) -> Result<u32, InflateFault> {
        let byte = *self
            .bytes
            .get(self.position)
            .ok_or(InflateFault::Truncated)?;
        let bit = (u32::from(byte) >> self.bit) & 1;
        self.bit += 1;
        if self.bit == 8 {
            self.bit = 0;
            self.position += 1;
        }
        Ok(bit)
    }

    fn take(&mut self, width: u32) -> Result<u32, InflateFault> {
        let mut value = 0;
        for index in 0..width {
            value |= self.next_bit()? << index;
        }
        Ok(value)
    }

    fn take_code(&mut self, width: u32) -> Result<u32, InflateFault> {
        let mut value = 0;
        for _ in 0..width {
            value = (value << 1) | self.next_bit()?;
        }
        Ok(value)
    }

    fn symbol(&mut self) -> Result<u16, InflateFault> {
        let mut code = self.take_code(7)?;
        if code <= 0b001_0111 {
            return u16::try_from(code + 256).map_err(|_| InflateFault::UnknownSymbol);
        }
        code = (code << 1) | self.next_bit()?;
        if (0b0011_0000..=0b1011_1111).contains(&code) {
            return u16::try_from(code - 0b0011_0000).map_err(|_| InflateFault::UnknownSymbol);
        }
        if (0b1100_0000..=0b1100_0111).contains(&code) {
            return u16::try_from(code - 0b1100_0000 + 280)
                .map_err(|_| InflateFault::UnknownSymbol);
        }
        code = (code << 1) | self.next_bit()?;
        if (0b1_1001_0000..=0b1_1111_1111).contains(&code) {
            return u16::try_from(code - 0b1_1001_0000 + 144)
                .map_err(|_| InflateFault::UnknownSymbol);
        }
        Err(InflateFault::UnknownSymbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_of_check_string_is_the_published_value() {
        let crc = crc32(b"123456789");

        assert_eq!(crc, 0xcbf4_3926);
    }

    #[test]
    fn adler32_of_wikipedia_is_the_published_value() {
        let checksum = adler32(b"Wikipedia");

        assert_eq!(checksum, 0x11e6_0398);
    }

    #[test]
    fn stored_zlib_stream_inflates_to_its_payload() {
        let payload = b"abc";
        let mut stream = vec![0x78, 0x01, 0x01, 0x03, 0x00, 0xfc, 0xff];
        stream.extend_from_slice(payload);
        stream.extend_from_slice(&adler32(payload).to_be_bytes());

        let inflated = inflate_stored(&stream);

        assert_eq!(inflated, Ok(payload.to_vec()));
    }

    #[test]
    fn stored_stream_with_wrong_adler_is_refused() {
        let mut stream = vec![0x78, 0x01, 0x01, 0x01, 0x00, 0xfe, 0xff, b'a'];
        stream.extend_from_slice(&0u32.to_be_bytes());

        let inflated = inflate_stored(&stream);

        assert_eq!(inflated, Err(InflateFault::AdlerMismatch));
    }

    #[test]
    fn compressed_block_is_refused() {
        let stream = vec![0x78, 0x01, 0x03, 0x00];

        let inflated = inflate_stored(&stream);

        assert_eq!(inflated, Err(InflateFault::NotStoredBlock));
    }

    #[test]
    fn chunk_with_altered_data_fails_its_crc() {
        let mut chunk = PngChunk {
            kind: IEND,
            data: Vec::new(),
            stored_crc: crc32(b"IEND"),
        };
        chunk.data.push(0);

        assert!(!chunk.has_valid_crc());
    }
}
