const BYTES_PER_PIXEL: usize = 4;
const PAM_HEADER_START: &str = "P7\nWIDTH ";
const PAM_HEADER_HEIGHT: &str = "\nHEIGHT ";
const PAM_HEADER_END: &str = "\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n";
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
const PNG_BIT_DEPTH: u8 = 8;
const PNG_COLOUR_TYPE_RGBA: u8 = 6;
const PNG_COMPRESSION_DEFLATE: u8 = 0;
const PNG_FILTER_METHOD: u8 = 0;
const PNG_NO_INTERLACE: u8 = 0;
const PNG_ROW_FILTER_NONE: u8 = 0;
const PNG_ROW_FILTER_SUB: u8 = 1;
const PNG_ROW_FILTER_UP: u8 = 2;
const PNG_ROW_FILTER_AVERAGE: u8 = 3;
const PNG_ROW_FILTER_PAETH: u8 = 4;
const PNG_ROW_FILTERS: [u8; 5] = [
    PNG_ROW_FILTER_NONE,
    PNG_ROW_FILTER_SUB,
    PNG_ROW_FILTER_UP,
    PNG_ROW_FILTER_AVERAGE,
    PNG_ROW_FILTER_PAETH,
];
const ZLIB_HEADER: [u8; 2] = [0x78, 0x01];
const BYTES_PER_PIXEL_U64: u64 = 4;
const ZLIB_FRAME_BYTES: u64 = 6;
const ADLER_MODULUS: u32 = 65_521;
const ADLER_RUN: usize = 5_552;
const CRC_POLYNOMIAL: u32 = 0xedb8_8320;
const CRC_TABLE: [u32; 256] = crc_table();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageError {
    EmptySize {
        width: u16,
        height: u16,
    },
    ByteCount {
        width: u16,
        height: u16,
        found: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PngError {
    StreamTooLong { width: u16, height: u16 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    width: u16,
    height: u16,
    rgba: Vec<u8>,
}

impl Image {
    pub fn new(width: u16, height: u16, rgba: Vec<u8>) -> Result<Image, ImageError> {
        if width == 0 || height == 0 {
            return Err(ImageError::EmptySize { width, height });
        }
        let expected = usize::from(width)
            .checked_mul(usize::from(height))
            .and_then(|pixels| pixels.checked_mul(BYTES_PER_PIXEL));
        if expected == Some(rgba.len()) {
            Ok(Image {
                width,
                height,
                rgba,
            })
        } else {
            Err(ImageError::ByteCount {
                width,
                height,
                found: rgba.len(),
            })
        }
    }

    pub(crate) const fn from_canvas(width: u16, height: u16, rgba: Vec<u8>) -> Image {
        Image {
            width,
            height,
            rgba,
        }
    }

    pub const fn width(&self) -> u16 {
        self.width
    }

    pub const fn height(&self) -> u16 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn pixel(&self, x: u16, y: u16) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let start = (usize::from(y) * usize::from(self.width) + usize::from(x)) * BYTES_PER_PIXEL;
        let bytes = self.rgba.get(start..start + BYTES_PER_PIXEL)?;
        <[u8; 4]>::try_from(bytes).ok()
    }
}

pub fn encode_pam(image: &Image) -> Vec<u8> {
    let header = format!(
        "{PAM_HEADER_START}{}{PAM_HEADER_HEIGHT}{}{PAM_HEADER_END}",
        image.width, image.height
    );
    let mut bytes = Vec::with_capacity(header.len() + image.rgba.len());
    bytes.extend_from_slice(header.as_bytes());
    bytes.extend_from_slice(&image.rgba);
    bytes
}

pub fn encode_png(image: &Image) -> Result<Vec<u8>, PngError> {
    png_stream_length(image.width, image.height)?;
    let filtered = filtered_rows(image);
    let stream = zlib_deflate(&filtered);
    let stream_length = u32::try_from(stream.len()).map_err(|_| PngError::StreamTooLong {
        width: image.width,
        height: image.height,
    })?;
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&u32::from(image.width).to_be_bytes());
    header.extend_from_slice(&u32::from(image.height).to_be_bytes());
    header.extend_from_slice(&[
        PNG_BIT_DEPTH,
        PNG_COLOUR_TYPE_RGBA,
        PNG_COMPRESSION_DEFLATE,
        PNG_FILTER_METHOD,
        PNG_NO_INTERLACE,
    ]);
    let mut bytes = Vec::with_capacity(stream.len() + 64);
    bytes.extend_from_slice(&PNG_SIGNATURE);
    push_chunk(&mut bytes, *b"IHDR", &header, 13);
    push_chunk(&mut bytes, *b"IDAT", &stream, stream_length);
    push_chunk(&mut bytes, *b"IEND", &[], 0);
    Ok(bytes)
}

fn png_stream_length(width: u16, height: u16) -> Result<u32, PngError> {
    let rows = u64::from(height) * (u64::from(width) * BYTES_PER_PIXEL_U64 + 1);
    let length = rows + ZLIB_FRAME_BYTES;
    u32::try_from(length).map_err(|_| PngError::StreamTooLong { width, height })
}

fn push_chunk(bytes: &mut Vec<u8>, kind: [u8; 4], data: &[u8], length: u32) {
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(&kind);
    bytes.extend_from_slice(data);
    let checksum = crc32_update(crc32_update(u32::MAX, &kind), data) ^ u32::MAX;
    bytes.extend_from_slice(&checksum.to_be_bytes());
}

fn zlib_deflate(data: &[u8]) -> Vec<u8> {
    let deflated = crate::deflate::deflate(data);
    let mut stream = Vec::with_capacity(deflated.len() + 6);
    stream.extend_from_slice(&ZLIB_HEADER);
    stream.extend_from_slice(&deflated);
    stream.extend_from_slice(&adler32(data).to_be_bytes());
    stream
}

fn filtered_rows(image: &Image) -> Vec<u8> {
    let row_bytes = usize::from(image.width) * BYTES_PER_PIXEL;
    let mut filtered = Vec::with_capacity(usize::from(image.height) * (row_bytes + 1));
    let mut above = vec![0_u8; row_bytes];
    let mut coded = vec![0_u8; row_bytes];
    for row in image.rgba.chunks(row_bytes) {
        let mut best: Option<(u8, Vec<u8>, u64)> = None;
        for filter in PNG_ROW_FILTERS {
            code_row(filter, row, &above, &mut coded);
            let weight = coded
                .iter()
                .map(|byte| u64::from(byte.wrapping_add(128).abs_diff(128)))
                .sum();
            if best.as_ref().is_none_or(|(_, _, kept)| weight < *kept) {
                best = Some((filter, coded.clone(), weight));
            }
        }
        let (filter, bytes, _) = best.unwrap_or((PNG_ROW_FILTER_NONE, row.to_vec(), 0));
        filtered.push(filter);
        filtered.extend_from_slice(&bytes);
        above.copy_from_slice(row);
    }
    filtered
}

fn code_row(filter: u8, row: &[u8], above: &[u8], coded: &mut [u8]) {
    for index in 0..row.len() {
        let raw = row[index];
        let left = index
            .checked_sub(BYTES_PER_PIXEL)
            .map_or(0, |earlier| row[earlier]);
        let up = above[index];
        let up_left = index
            .checked_sub(BYTES_PER_PIXEL)
            .map_or(0, |earlier| above[earlier]);
        coded[index] = match filter {
            PNG_ROW_FILTER_SUB => raw.wrapping_sub(left),
            PNG_ROW_FILTER_UP => raw.wrapping_sub(up),
            PNG_ROW_FILTER_AVERAGE => {
                let average = (u16::from(left) + u16::from(up)) / 2;
                raw.wrapping_sub(u8::try_from(average).unwrap_or_default())
            }
            PNG_ROW_FILTER_PAETH => raw.wrapping_sub(paeth(left, up, up_left)),
            _ => raw,
        };
    }
}

fn paeth(left: u8, up: u8, up_left: u8) -> u8 {
    let estimate = i32::from(left) + i32::from(up) - i32::from(up_left);
    let from_left = estimate.abs_diff(i32::from(left));
    let from_up = estimate.abs_diff(i32::from(up));
    let from_up_left = estimate.abs_diff(i32::from(up_left));
    if from_left <= from_up && from_left <= from_up_left {
        left
    } else if from_up <= from_up_left {
        up
    } else {
        up_left
    }
}

fn adler32(data: &[u8]) -> u32 {
    let mut low = 1_u32;
    let mut high = 0_u32;
    for run in data.chunks(ADLER_RUN) {
        for byte in run {
            low += u32::from(*byte);
            high += low;
        }
        low %= ADLER_MODULUS;
        high %= ADLER_MODULUS;
    }
    (high << 16) | low
}

const fn crc_table() -> [u32; 256] {
    let mut table = [0_u32; 256];
    let mut index = 0;
    let mut seed = 0_u32;
    while index < 256 {
        let mut value = seed;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 1 == 1 {
                (value >> 1) ^ CRC_POLYNOMIAL
            } else {
                value >> 1
            };
            bit += 1;
        }
        table[index] = value;
        index += 1;
        seed += 1;
    }
    table
}

fn crc32_update(state: u32, data: &[u8]) -> u32 {
    data.iter().fold(state, |state, byte| {
        let slot = usize::from(state.to_le_bytes()[0] ^ byte);
        (state >> 8) ^ CRC_TABLE[slot]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u16, height: u16) -> Image {
        let bytes = usize::from(width) * usize::from(height) * BYTES_PER_PIXEL;
        let rgba = (0..bytes)
            .map(|index| u8::try_from(index % 251).unwrap())
            .collect();
        Image::new(width, height, rgba).unwrap()
    }

    fn inflated(stream: &[u8]) -> Vec<u8> {
        crate::deflate::inflate(&stream[2..stream.len() - 4])
    }

    fn decoded(png: &[u8], width: u16, height: u16) -> Vec<u8> {
        let rows = inflated(&chunk(png, b"IDAT"));
        let row_bytes = usize::from(width) * BYTES_PER_PIXEL;
        let mut pixels: Vec<u8> = Vec::with_capacity(usize::from(height) * row_bytes);
        for (index, row) in rows.chunks(row_bytes + 1).enumerate() {
            let filter = row[0];
            let start = index * row_bytes;
            for at in 0..row_bytes {
                let left = at
                    .checked_sub(BYTES_PER_PIXEL)
                    .map_or(0, |earlier| pixels[start + earlier]);
                let up = index
                    .checked_sub(1)
                    .map_or(0, |_| pixels[start - row_bytes + at]);
                let up_left = index
                    .checked_sub(1)
                    .zip(at.checked_sub(BYTES_PER_PIXEL))
                    .map_or(0, |(_, earlier)| pixels[start - row_bytes + earlier]);
                let coded = row[at + 1];
                let raw = match filter {
                    PNG_ROW_FILTER_SUB => coded.wrapping_add(left),
                    PNG_ROW_FILTER_UP => coded.wrapping_add(up),
                    PNG_ROW_FILTER_AVERAGE => {
                        let average = (u16::from(left) + u16::from(up)) / 2;
                        coded.wrapping_add(u8::try_from(average).unwrap())
                    }
                    PNG_ROW_FILTER_PAETH => coded.wrapping_add(paeth(left, up, up_left)),
                    _ => coded,
                };
                pixels.push(raw);
            }
        }
        pixels
    }

    fn chunk(png: &[u8], kind: &[u8; 4]) -> Vec<u8> {
        let mut position = PNG_SIGNATURE.len();
        loop {
            let length = usize::try_from(u32::from_be_bytes(
                png[position..position + 4].try_into().unwrap(),
            ))
            .unwrap();
            if &png[position + 4..position + 8] == kind {
                return png[position + 8..position + 8 + length].to_vec();
            }
            position += 12 + length;
        }
    }

    #[test]
    fn image_with_too_few_bytes_is_rejected() {
        let result = Image::new(2, 2, vec![0; 15]);

        assert_eq!(
            result,
            Err(ImageError::ByteCount {
                width: 2,
                height: 2,
                found: 15
            })
        );
    }

    #[test]
    fn image_without_height_is_rejected() {
        let result = Image::new(2, 0, Vec::new());

        assert_eq!(
            result,
            Err(ImageError::EmptySize {
                width: 2,
                height: 0
            })
        );
    }

    #[test]
    fn png_stream_beyond_a_chunk_length_is_rejected() {
        let result = png_stream_length(u16::MAX, u16::MAX);

        assert_eq!(
            result,
            Err(PngError::StreamTooLong {
                width: u16::MAX,
                height: u16::MAX
            })
        );
    }

    #[test]
    fn png_stream_length_counts_the_filtered_rows_and_the_zlib_frame() {
        assert_eq!(png_stream_length(3, 2), Ok(2 * 13 + 6));
    }

    #[test]
    fn pixel_is_read_row_by_row_from_the_top() {
        let image = Image::new(2, 2, (0..16).collect()).unwrap();

        assert_eq!(image.pixel(1, 1), Some([12, 13, 14, 15]));
    }

    #[test]
    fn pixel_outside_the_image_is_absent() {
        assert_eq!(image(2, 2).pixel(2, 0), None);
    }

    #[test]
    fn pam_header_names_size_depth_and_tuple_type() {
        let pam = encode_pam(&image(2, 1));

        assert!(pam.starts_with(
            b"P7\nWIDTH 2\nHEIGHT 1\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n"
        ));
    }

    #[test]
    fn pam_ends_with_the_pixels() {
        let pam = encode_pam(&Image::new(1, 1, vec![1, 2, 3, 255]).unwrap());

        assert!(pam.ends_with(&[1, 2, 3, 255]));
    }

    #[test]
    fn png_starts_with_its_signature_and_header_chunk() {
        let png = encode_png(&image(3, 2)).unwrap();

        assert_eq!(&png[..16], b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR");
    }

    #[test]
    fn png_header_holds_size_depth_and_colour_type() {
        let png = encode_png(&image(300, 2)).unwrap();

        assert_eq!(
            chunk(&png, b"IHDR"),
            [0, 0, 1, 44, 0, 0, 0, 2, 8, 6, 0, 0, 0]
        );
    }

    #[test]
    fn png_ends_with_the_empty_end_chunk() {
        let png = encode_png(&image(3, 2)).unwrap();

        assert!(png.ends_with(&[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82]));
    }

    #[test]
    fn png_data_stream_starts_with_the_zlib_header() {
        let png = encode_png(&image(3, 2)).unwrap();

        assert_eq!(chunk(&png, b"IDAT")[..2], ZLIB_HEADER);
    }

    #[test]
    fn png_pixels_come_back_from_the_stream() {
        let image = image(37, 11);

        let png = encode_png(&image).unwrap();

        assert_eq!(decoded(&png, 37, 11), image.rgba());
    }

    #[test]
    fn png_rows_carry_a_filter_of_the_standard_set() {
        let png = encode_png(&image(37, 11)).unwrap();

        let rows = inflated(&chunk(&png, b"IDAT"));

        let filters: Vec<u8> = rows
            .chunks(37 * BYTES_PER_PIXEL + 1)
            .map(|row| row[0])
            .collect();
        assert!(
            filters
                .iter()
                .all(|filter| PNG_ROW_FILTERS.contains(filter))
        );
    }

    #[test]
    fn a_flat_picture_is_far_smaller_than_its_pixels() {
        let flat = Image::new(200, 100, vec![250; 200 * 100 * BYTES_PER_PIXEL]).unwrap();

        let png = encode_png(&flat).unwrap();

        assert!(png.len() < flat.rgba().len() / 20);
    }

    #[test]
    fn the_same_image_gives_the_same_png() {
        let image = image(37, 11);

        assert_eq!(encode_png(&image), encode_png(&image));
    }

    #[test]
    fn png_data_stream_ends_with_the_adler_checksum_of_the_rows() {
        let png = encode_png(&image(3, 2)).unwrap();
        let stream = chunk(&png, b"IDAT");

        let rows = inflated(&stream);

        assert_eq!(stream[stream.len() - 4..], adler32(&rows).to_be_bytes());
    }

    #[test]
    fn adler_checksum_matches_the_zlib_reference_value() {
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
    }

    #[test]
    fn crc_matches_the_reference_check_value() {
        assert_eq!(crc32_update(u32::MAX, b"123456789") ^ u32::MAX, 0xcbf4_3926);
    }
}
