use crate::encoding::{
    IDAT, IEND, IHDR, ImageHeader, image_header, inflate_fixed, pam_header, png_chunks,
};
use crate::pixels::OPAQUE;
use crate::rendering::{IMAGE_HEIGHT, IMAGE_WIDTH, pam_bytes, png_bytes, render};
use crate::scenes::{polyline_layer, scalar_grid_layer, unit_scene};

const PNG_BIT_DEPTH: u8 = 8;
const PNG_COLOUR_TYPE_RGBA: u8 = 6;
const SUB_FILTER: u8 = 1;
const UP_FILTER: u8 = 2;
const AVERAGE_FILTER: u8 = 3;
const PAETH_FILTER: u8 = 4;
const ROW_BYTES_PER_PIXEL: usize = 4;

fn sample_scene() -> calc_viz::Scene {
    unit_scene(vec![
        scalar_grid_layer([2, 1], vec![0.2, 0.9], calc_viz::ColourMap::Sequential),
        polyline_layer(vec![0.0, 1.0], vec![0.1, 0.9]),
    ])
}

#[test]
fn image_has_the_requested_size() {
    let picture = render(&sample_scene());

    assert_eq!((picture.width, picture.height), (IMAGE_WIDTH, IMAGE_HEIGHT));
}

#[test]
fn image_holds_four_bytes_per_pixel() {
    let picture = render(&sample_scene());

    assert_eq!(
        picture.rgba.len(),
        usize::try_from(IMAGE_WIDTH * IMAGE_HEIGHT * 4).expect("fits")
    );
}

#[test]
fn every_pixel_is_opaque() {
    let picture = render(&sample_scene());

    assert!(picture.alpha_values().all(|alpha| alpha == OPAQUE));
}

#[test]
fn same_scene_renders_to_the_same_bytes_twice() {
    let first = pam_bytes(&sample_scene());
    let second = pam_bytes(&sample_scene());

    assert!(first == second);
}

#[test]
fn pam_is_the_t128_header_followed_by_the_pixels() {
    let scene = sample_scene();
    let picture = render(&scene);
    let mut expected = pam_header(picture.width, picture.height);
    expected.extend_from_slice(&picture.rgba);

    let encoded = pam_bytes(&scene);

    assert!(encoded == expected, "PAM differs from header plus pixels");
}

fn chunk_kinds(bytes: &[u8]) -> Option<Vec<[u8; 4]>> {
    Some(png_chunks(bytes)?.iter().map(|chunk| chunk.kind).collect())
}

#[test]
fn png_is_signature_ihdr_one_idat_and_iend() {
    let kinds = chunk_kinds(&png_bytes(&sample_scene()));

    assert_eq!(kinds, Some(vec![IHDR, IDAT, IEND]));
}

#[test]
fn every_png_chunk_has_a_valid_crc() {
    let chunks = png_chunks(&png_bytes(&sample_scene())).expect("chunks parse");

    assert!(chunks.iter().all(|chunk| chunk.has_valid_crc()));
}

#[test]
fn png_header_is_eight_bit_rgba_of_the_image_size() {
    let chunks = png_chunks(&png_bytes(&sample_scene())).expect("chunks parse");

    let header = chunks.first().and_then(|chunk| image_header(&chunk.data));

    assert_eq!(
        header,
        Some(ImageHeader {
            width: IMAGE_WIDTH,
            height: IMAGE_HEIGHT,
            bit_depth: PNG_BIT_DEPTH,
            colour_type: PNG_COLOUR_TYPE_RGBA,
            compression: 0,
            filter: 0,
            interlace: 0,
        })
    );
}

#[test]
fn png_data_is_fixed_huffman_deflate_of_the_filtered_rows() {
    let scene = sample_scene();
    let picture = render(&scene);
    let chunks = png_chunks(&png_bytes(&scene)).expect("chunks parse");
    let data = chunks
        .iter()
        .find(|chunk| chunk.kind == IDAT)
        .expect("one IDAT")
        .data
        .clone();
    let row_length = usize::try_from(picture.width).expect("fits") * ROW_BYTES_PER_PIXEL;

    let inflated = inflate_fixed(&data);

    let rows = inflated.expect("the IDAT inflates as a fixed Huffman stream");
    assert_eq!(
        rows.len(),
        usize::try_from(picture.height).expect("fits") * (row_length + 1)
    );
}

#[test]
fn png_rows_carry_one_of_the_five_filters() {
    let scene = sample_scene();
    let picture = render(&scene);
    let chunks = png_chunks(&png_bytes(&scene)).expect("chunks parse");
    let data = chunks
        .iter()
        .find(|chunk| chunk.kind == IDAT)
        .expect("one IDAT")
        .data
        .clone();
    let row_length = usize::try_from(picture.width).expect("fits") * ROW_BYTES_PER_PIXEL;

    let rows = inflate_fixed(&data).expect("the IDAT inflates");

    let filters: Vec<u8> = rows.chunks(row_length + 1).map(|row| row[0]).collect();
    assert!(filters.iter().all(|filter| *filter <= PAETH_FILTER));
}

#[test]
fn png_rows_unfilter_to_the_pixels_of_the_picture() {
    let scene = sample_scene();
    let picture = render(&scene);
    let chunks = png_chunks(&png_bytes(&scene)).expect("chunks parse");
    let data = chunks
        .iter()
        .find(|chunk| chunk.kind == IDAT)
        .expect("one IDAT")
        .data
        .clone();
    let row_length = usize::try_from(picture.width).expect("fits") * ROW_BYTES_PER_PIXEL;

    let rows = inflate_fixed(&data).expect("the IDAT inflates");

    assert_eq!(unfiltered(&rows, row_length), picture.rgba);
}

fn unfiltered(rows: &[u8], row_length: usize) -> Vec<u8> {
    let mut pixels: Vec<u8> = Vec::with_capacity(rows.len());
    for (index, row) in rows.chunks(row_length + 1).enumerate() {
        let start = index * row_length;
        for at in 0..row_length {
            let left = at
                .checked_sub(ROW_BYTES_PER_PIXEL)
                .map_or(0, |earlier| pixels[start + earlier]);
            let up = index
                .checked_sub(1)
                .map_or(0, |_| pixels[start - row_length + at]);
            let up_left = index
                .checked_sub(1)
                .zip(at.checked_sub(ROW_BYTES_PER_PIXEL))
                .map_or(0, |(_, earlier)| pixels[start - row_length + earlier]);
            let coded = row[at + 1];
            let raw = match row[0] {
                SUB_FILTER => coded.wrapping_add(left),
                UP_FILTER => coded.wrapping_add(up),
                AVERAGE_FILTER => {
                    let average = (u16::from(left) + u16::from(up)) / 2;
                    coded.wrapping_add(u8::try_from(average).expect("a byte"))
                }
                PAETH_FILTER => coded.wrapping_add(paeth(left, up, up_left)),
                _ => coded,
            };
            pixels.push(raw);
        }
    }
    pixels
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
