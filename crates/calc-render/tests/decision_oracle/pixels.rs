use std::ops::Range;

use calc_numbers::Number;
use calc_viz::Colour;

use crate::rendering::{BYTES_PER_PIXEL, Picture, PixelRect};

pub const LINE_REACH: f64 = 4.0;
pub const CHANNEL_TOLERANCE: f64 = 1.0;
pub const CHANNEL_SCALE: f64 = 255.0;
pub const OPAQUE: u8 = 255;
const ALPHA_CHANNEL: usize = 3;

pub fn whole_pixel(value: f64) -> Option<u32> {
    let floored = value.floor();
    match Number::F64(floored).to_exact().ok()? {
        Number::Integer(integer) => u32::try_from(integer.to_i64()?).ok(),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Mapping {
    pub plot: PixelRect,
    pub x_lower: f64,
    pub x_upper: f64,
    pub y_lower: f64,
    pub y_upper: f64,
}

impl Mapping {
    pub fn unit(plot: PixelRect) -> Self {
        Self {
            plot,
            x_lower: 0.0,
            x_upper: 1.0,
            y_lower: 0.0,
            y_upper: 1.0,
        }
    }

    pub fn column(&self, x: f64) -> f64 {
        f64::from(self.plot.left)
            + (x - self.x_lower) / (self.x_upper - self.x_lower) * f64::from(self.plot.width)
    }

    pub fn row(&self, y: f64) -> f64 {
        f64::from(self.plot.top)
            + (self.y_upper - y) / (self.y_upper - self.y_lower) * f64::from(self.plot.height)
    }

    pub fn columns_between(&self, left: f64, right: f64) -> Range<u32> {
        let first = whole_pixel(self.column(left) + LINE_REACH).unwrap_or(0);
        let last = whole_pixel(self.column(right) - LINE_REACH).unwrap_or(0);
        first..last.max(first)
    }

    pub fn rows_between(&self, lower: f64, upper: f64) -> Range<u32> {
        let first = whole_pixel(self.row(upper) + LINE_REACH).unwrap_or(0);
        let last = whole_pixel(self.row(lower) - LINE_REACH).unwrap_or(0);
        first..last.max(first)
    }

    pub fn rows_near(&self, y: f64, reach: f64) -> Range<u32> {
        let centre = self.row(y);
        let first = whole_pixel((centre - reach).max(0.0)).unwrap_or(0);
        let last = whole_pixel(centre + reach).unwrap_or(0) + 1;
        first..last
    }
}

impl Picture {
    pub fn pixel(&self, column: u32, row: u32) -> Option<[u8; 4]> {
        if column >= self.width || row >= self.height {
            return None;
        }
        let start = usize::try_from((row * self.width + column) * BYTES_PER_PIXEL).ok()?;
        let bytes = self.rgba.get(start..start + 4)?;
        Some([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    pub fn alpha_values(&self) -> impl Iterator<Item = u8> + '_ {
        self.rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel[ALPHA_CHANNEL])
    }
}

pub fn differs(picture: &Picture, reference: &Picture, column: u32, row: u32) -> bool {
    picture.pixel(column, row) != reference.pixel(column, row)
}

pub fn drawn_rows(
    picture: &Picture,
    reference: &Picture,
    column: u32,
    rows: Range<u32>,
) -> Vec<u32> {
    rows.filter(|row| differs(picture, reference, column, *row))
        .collect()
}

pub fn is_drawn_near(
    picture: &Picture,
    reference: &Picture,
    column: u32,
    rows: Range<u32>,
) -> bool {
    !drawn_rows(picture, reference, column, rows).is_empty()
}

pub fn channel_distance(pixel: [u8; 4], colour: Colour) -> f64 {
    let channels = [colour.red, colour.green, colour.blue];
    pixel
        .iter()
        .zip(channels)
        .map(|(byte, channel)| (f64::from(*byte) - channel * CHANNEL_SCALE).abs())
        .fold(0.0, |largest, distance| {
            if distance > largest {
                distance
            } else {
                largest
            }
        })
}

pub fn expected_byte(channel: f64) -> f64 {
    (channel.clamp(0.0, 1.0) * CHANNEL_SCALE).round_ties_even()
}

pub fn matches_colour(pixel: [u8; 4], colour: Colour) -> bool {
    pixel
        .iter()
        .zip([colour.red, colour.green, colour.blue])
        .all(|(byte, channel)| {
            (f64::from(*byte) - expected_byte(channel)).abs() <= CHANNEL_TOLERANCE
        })
}

pub fn share_matching(
    picture: &Picture,
    columns: Range<u32>,
    rows: Range<u32>,
    colour: Colour,
) -> f64 {
    let mut total = 0u32;
    let mut matching = 0u32;
    for row in rows {
        for column in columns.clone() {
            total += 1;
            if picture
                .pixel(column, row)
                .is_some_and(|pixel| matches_colour(pixel, colour))
            {
                matching += 1;
            }
        }
    }
    if total == 0 {
        return 0.0;
    }
    f64::from(matching) / f64::from(total)
}

pub fn most_common_pixel(
    picture: &Picture,
    columns: Range<u32>,
    rows: Range<u32>,
) -> Option<[u8; 4]> {
    let mut counts: Vec<([u8; 4], u32)> = Vec::new();
    for row in rows {
        for column in columns.clone() {
            let pixel = picture.pixel(column, row)?;
            match counts.iter_mut().find(|(seen, _)| *seen == pixel) {
                Some((_, count)) => *count += 1,
                None => counts.push((pixel, 1)),
            }
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(pixel, _)| pixel)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plot() -> PixelRect {
        PixelRect {
            left: 10,
            top: 20,
            width: 100,
            height: 50,
        }
    }

    #[test]
    fn upper_view_edge_maps_to_top_row_of_plot() {
        let mapping = Mapping::unit(plot());

        let row = mapping.row(1.0);

        assert_eq!(row, 20.0);
    }

    #[test]
    fn lower_view_edge_maps_to_bottom_boundary_of_plot() {
        let mapping = Mapping::unit(plot());

        let row = mapping.row(0.0);

        assert_eq!(row, 70.0);
    }

    #[test]
    fn whole_pixel_floors_fractional_position() {
        let pixel = whole_pixel(12.75);

        assert_eq!(pixel, Some(12));
    }

    #[test]
    fn colour_within_rounding_of_a_byte_matches() {
        let colour = Colour {
            red: 0.5,
            green: 0.0,
            blue: 1.0,
        };

        let is_match = matches_colour([128, 0, 255, 255], colour);

        assert!(is_match);
    }

    #[test]
    fn colour_two_bytes_away_does_not_match() {
        let colour = Colour {
            red: 0.5,
            green: 0.0,
            blue: 1.0,
        };

        let is_match = matches_colour([130, 0, 255, 255], colour);

        assert!(!is_match);
    }
}
