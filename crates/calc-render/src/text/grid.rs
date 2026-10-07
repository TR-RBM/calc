use crate::geometry::{CellSize, CellSizeError};
use crate::text::font::{FontError, FontSet};

pub const QUARTERS_PER_PIXEL: i64 = 4;
const CELL_REFERENCE_CHARACTER: char = '0';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridError {
    Font(FontError),
    Cell(CellSizeError),
    OutOfRange { font_size_px: u16 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridMetrics {
    pub cell: CellSize,
    pub baseline: u16,
    pub font_size_px: u16,
}

pub fn round_divide(numerator: i64, denominator: i64) -> i64 {
    (numerator * 2 + denominator).div_euclid(denominator * 2)
}

pub fn font_units_to_quarters(units: i64, size_quarters: i64, units_per_em: u16) -> i64 {
    round_divide(units * size_quarters, i64::from(units_per_em))
}

pub fn grid_metrics(fonts: &FontSet, font_size_px: u16) -> Result<GridMetrics, GridError> {
    let face = fonts.monospace();
    let units_per_em = i64::from(face.units_per_em());
    let size = i64::from(font_size_px);
    let advance = face
        .advance_of(CELL_REFERENCE_CHARACTER)
        .map_err(GridError::Font)?;
    let metrics = face.vertical_metrics();
    let line_height =
        i64::from(metrics.ascender) - i64::from(metrics.descender) + i64::from(metrics.line_gap);
    let to_pixels = |units: i64| {
        u16::try_from(round_divide(units * size, units_per_em))
            .map_err(|_| GridError::OutOfRange { font_size_px })
    };
    let cell = CellSize::new(to_pixels(i64::from(advance))?, to_pixels(line_height)?)
        .map_err(GridError::Cell)?;
    Ok(GridMetrics {
        cell,
        baseline: to_pixels(i64::from(metrics.ascender))?,
        font_size_px,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics_at(font_size_px: u16) -> GridMetrics {
        let fonts = FontSet::bundled().expect("bundled fonts load");
        grid_metrics(&fonts, font_size_px).expect("metrics in range")
    }

    #[test]
    fn cell_width_is_digit_advance_rounded_to_whole_pixels() {
        assert_eq!(metrics_at(20).cell.width(), 12);
    }

    #[test]
    fn cell_width_rounds_half_pixel_up() {
        assert_eq!(metrics_at(15).cell.width(), 9);
    }

    #[test]
    fn cell_height_is_line_height_rounded_to_whole_pixels() {
        assert_eq!(metrics_at(20).cell.height(), 27);
    }

    #[test]
    fn baseline_is_ascender_rounded_to_whole_pixels() {
        assert_eq!(metrics_at(20).baseline, 21);
    }

    #[test]
    fn zero_font_size_has_no_cell() {
        let fonts = FontSet::bundled().expect("bundled fonts load");

        assert_eq!(
            grid_metrics(&fonts, 0),
            Err(GridError::Cell(CellSizeError::ZeroWidth))
        );
    }

    #[test]
    fn negative_font_units_round_to_nearest_quarter() {
        assert_eq!(font_units_to_quarters(-299, 80, 1000), -24);
    }

    #[test]
    fn rounding_of_an_exact_half_goes_up() {
        assert_eq!(round_divide(5, 2), 3);
    }
}
