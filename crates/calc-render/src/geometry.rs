const PERCENT_PER_WHOLE: u32 = 100;
const HALF_PERCENT_PER_WHOLE: u32 = 50;
const THINNEST_LINE: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl PhysicalRect {
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub const fn right(&self) -> u16 {
        self.x.saturating_add(self.width)
    }

    pub const fn bottom(&self) -> u16 {
        self.y.saturating_add(self.height)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScaleError {
    ZeroScale,
    PhysicalSizeTooLarge { logical: u16 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScaleFactor {
    percent: u16,
}

impl ScaleFactor {
    pub fn from_percent(percent: u16) -> Result<Self, ScaleError> {
        if percent == 0 {
            return Err(ScaleError::ZeroScale);
        }
        Ok(Self { percent })
    }

    pub const fn percent(self) -> u16 {
        self.percent
    }

    pub fn to_physical_size(self, logical: u16) -> Result<u16, ScaleError> {
        let scaled = u32::from(logical) * u32::from(self.percent) + HALF_PERCENT_PER_WHOLE;
        u16::try_from(scaled / PERCENT_PER_WHOLE)
            .map_err(|_| ScaleError::PhysicalSizeTooLarge { logical })
    }

    pub fn to_line_width(self, logical: u16) -> Result<u16, ScaleError> {
        let scaled = u32::from(logical) * u32::from(self.percent);
        let whole_pixels = u16::try_from(scaled / PERCENT_PER_WHOLE)
            .map_err(|_| ScaleError::PhysicalSizeTooLarge { logical })?;
        Ok(whole_pixels.max(THINNEST_LINE))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellSizeError {
    ZeroWidth,
    ZeroHeight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellSize {
    width: u16,
    height: u16,
}

impl CellSize {
    pub fn new(width: u16, height: u16) -> Result<Self, CellSizeError> {
        if width == 0 {
            return Err(CellSizeError::ZeroWidth);
        }
        if height == 0 {
            return Err(CellSizeError::ZeroHeight);
        }
        Ok(Self { width, height })
    }

    pub const fn width(self) -> u16 {
        self.width
    }

    pub const fn height(self) -> u16 {
        self.height
    }

    pub const fn cells_across(self, physical_width: u16) -> u16 {
        physical_width / self.width
    }

    pub const fn width_of_cells(self, cells: u16) -> Option<u16> {
        cells.checked_mul(self.width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scale(percent: u16) -> ScaleFactor {
        ScaleFactor::from_percent(percent).expect("non-zero scale")
    }

    #[test]
    fn full_scale_keeps_logical_size() {
        assert_eq!(scale(100).to_physical_size(3), Ok(3));
    }

    #[test]
    fn fractional_scale_rounds_size_to_nearest_physical_pixel() {
        assert_eq!(scale(125).to_physical_size(3), Ok(4));
    }

    #[test]
    fn half_pixel_size_rounds_up() {
        assert_eq!(scale(150).to_physical_size(1), Ok(2));
    }

    #[test]
    fn one_pixel_line_stays_one_physical_pixel_at_one_and_a_half_scale() {
        assert_eq!(scale(150).to_line_width(1), Ok(1));
    }

    #[test]
    fn one_pixel_line_stays_one_physical_pixel_at_one_and_a_quarter_scale() {
        assert_eq!(scale(125).to_line_width(1), Ok(1));
    }

    #[test]
    fn one_pixel_line_is_two_physical_pixels_at_double_scale() {
        assert_eq!(scale(200).to_line_width(1), Ok(2));
    }

    #[test]
    fn line_width_is_never_below_one_physical_pixel() {
        assert_eq!(scale(50).to_line_width(1), Ok(1));
    }

    #[test]
    fn zero_scale_is_rejected() {
        assert_eq!(ScaleFactor::from_percent(0), Err(ScaleError::ZeroScale));
    }

    #[test]
    fn physical_size_beyond_u16_is_rejected() {
        assert_eq!(
            scale(400).to_physical_size(u16::MAX),
            Err(ScaleError::PhysicalSizeTooLarge { logical: u16::MAX })
        );
    }

    #[test]
    fn line_width_beyond_u16_is_rejected() {
        assert_eq!(
            scale(400).to_line_width(u16::MAX),
            Err(ScaleError::PhysicalSizeTooLarge { logical: u16::MAX })
        );
    }

    #[test]
    fn cell_with_zero_width_is_rejected() {
        assert_eq!(CellSize::new(0, 16), Err(CellSizeError::ZeroWidth));
    }

    #[test]
    fn cell_with_zero_height_is_rejected() {
        assert_eq!(CellSize::new(8, 0), Err(CellSizeError::ZeroHeight));
    }

    #[test]
    fn partial_cell_is_not_counted_across_a_width() {
        let cell = CellSize::new(8, 16).expect("valid cell");

        assert_eq!(cell.cells_across(803), 100);
    }

    #[test]
    fn rect_right_edge_saturates_at_u16_limit() {
        let rect = PhysicalRect::new(u16::MAX, 0, 10, 10);

        assert_eq!(rect.right(), u16::MAX);
    }

    #[test]
    fn rect_without_height_is_empty() {
        assert!(PhysicalRect::new(0, 0, 10, 0).is_empty());
    }
}
