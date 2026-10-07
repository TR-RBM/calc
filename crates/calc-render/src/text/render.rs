use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{GlyphId, MetadataProvider};
use tiny_skia::PathBuilder;

use crate::canvas::Canvas;
use crate::colour::Colour;
use crate::text::font::{BundledFont, FontSet};
use crate::text::placement::PlacedGlyph;

const QUARTERS_PER_PIXEL: u32 = 4;
const QUARTER: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlyphRenderError {
    PositionOutOfRange { font: BundledFont, glyph_id: u32 },
    MissingOutline { font: BundledFont, glyph_id: u32 },
    UnreadableOutline { font: BundledFont, glyph_id: u32 },
    CubicOutline { font: BundledFont, glyph_id: u32 },
}

struct ScreenPen {
    builder: PathBuilder,
    origin_x: f32,
    baseline_y: f32,
    has_cubic_segment: bool,
}

impl OutlinePen for ScreenPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.builder.move_to(self.origin_x + x, self.baseline_y - y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.builder.line_to(self.origin_x + x, self.baseline_y - y);
    }

    fn quad_to(&mut self, control_x: f32, control_y: f32, x: f32, y: f32) {
        self.builder.quad_to(
            self.origin_x + control_x,
            self.baseline_y - control_y,
            self.origin_x + x,
            self.baseline_y - y,
        );
    }

    fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {
        self.has_cubic_segment = true;
    }

    fn close(&mut self) {
        self.builder.close();
    }
}

fn signed_quarters_to_pixels(quarters: i32) -> Option<f32> {
    let whole = i16::try_from(quarters.div_euclid(4)).ok()?;
    let fraction = u8::try_from(quarters.rem_euclid(4)).ok()?;
    Some(f32::from(whole) + f32::from(fraction) * QUARTER)
}

fn unsigned_quarters_to_pixels(quarters: u32) -> Option<f32> {
    let whole = u16::try_from(quarters / QUARTERS_PER_PIXEL).ok()?;
    let fraction = u8::try_from(quarters % QUARTERS_PER_PIXEL).ok()?;
    Some(f32::from(whole) + f32::from(fraction) * QUARTER)
}

pub fn draw_glyphs(
    canvas: &mut Canvas,
    glyphs: &[PlacedGlyph],
    fonts: &FontSet,
    colour: Colour,
) -> Result<(), GlyphRenderError> {
    for glyph in glyphs {
        let font = glyph.font;
        let glyph_id = glyph.glyph_id;
        let out_of_range = GlyphRenderError::PositionOutOfRange { font, glyph_id };
        let origin_x = signed_quarters_to_pixels(glyph.origin_x_quarters).ok_or(out_of_range)?;
        let baseline_y =
            signed_quarters_to_pixels(glyph.baseline_y_quarters).ok_or(out_of_range)?;
        let size = unsigned_quarters_to_pixels(glyph.size_quarters).ok_or(out_of_range)?;

        let outline = fonts
            .face(font)
            .font_ref()
            .outline_glyphs()
            .get(GlyphId::new(glyph_id))
            .ok_or(GlyphRenderError::MissingOutline { font, glyph_id })?;
        let mut pen = ScreenPen {
            builder: PathBuilder::new(),
            origin_x,
            baseline_y,
            has_cubic_segment: false,
        };
        outline
            .draw(
                DrawSettings::unhinted(Size::new(size), LocationRef::default()),
                &mut pen,
            )
            .map_err(|_| GlyphRenderError::UnreadableOutline { font, glyph_id })?;
        if pen.has_cubic_segment {
            return Err(GlyphRenderError::CubicOutline { font, glyph_id });
        }
        if let Some(path) = pen.builder.finish() {
            canvas.fill_antialiased_path(&path, colour);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::grid::grid_metrics;
    use crate::text::placement::{GridRowOrigin, place_on_grid};
    use crate::text::shape::shape_with_fallback;

    const BACKGROUND: Colour = Colour::opaque(255, 255, 255);
    const INK: Colour = Colour::opaque(0, 0, 0);

    fn render(text: &str) -> Canvas {
        let fonts = FontSet::bundled().expect("bundled fonts load");
        let metrics = grid_metrics(&fonts, 20).expect("metrics in range");
        let clusters = shape_with_fallback(text, &fonts);
        let placed = place_on_grid(
            text,
            &clusters,
            &fonts,
            metrics,
            GridRowOrigin { x: 0, top: 0 },
        )
        .expect("placement in range");
        let glyphs: Vec<PlacedGlyph> = placed
            .into_iter()
            .flat_map(|cluster| cluster.glyphs)
            .collect();
        let mut canvas = Canvas::new(48, 27, BACKGROUND).expect("non-empty canvas");
        draw_glyphs(&mut canvas, &glyphs, &fonts, INK).expect("glyphs render");
        canvas
    }

    const ROW_HEIGHT: u16 = 27;

    fn ink_rows_outside_middle_row(text: &str) -> Vec<u16> {
        let fonts = FontSet::bundled().expect("bundled fonts load");
        let metrics = grid_metrics(&fonts, 20).expect("metrics in range");
        let clusters = shape_with_fallback(text, &fonts);
        let placed = place_on_grid(
            text,
            &clusters,
            &fonts,
            metrics,
            GridRowOrigin {
                x: 0,
                top: ROW_HEIGHT,
            },
        )
        .expect("placement in range");
        let glyphs: Vec<PlacedGlyph> = placed
            .into_iter()
            .flat_map(|cluster| cluster.glyphs)
            .collect();
        let mut canvas = Canvas::new(48, 3 * ROW_HEIGHT, BACKGROUND).expect("non-empty canvas");
        draw_glyphs(&mut canvas, &glyphs, &fonts, INK).expect("glyphs render");
        (0..ROW_HEIGHT)
            .chain(2 * ROW_HEIGHT..3 * ROW_HEIGHT)
            .filter(|&y| (0..48).any(|x| canvas.pixel(x, y) != Some(BACKGROUND)))
            .collect()
    }

    #[test]
    fn pi_with_circumflex_stays_inside_its_row() {
        assert_eq!(
            ink_rows_outside_middle_row("\u{03C0}\u{0302}"),
            Vec::<u16>::new()
        );
    }

    #[test]
    fn integral_from_math_font_stays_inside_its_row() {
        assert_eq!(ink_rows_outside_middle_row("\u{222B}"), Vec::<u16>::new());
    }

    #[test]
    fn letter_with_diaeresis_and_acute_taller_than_the_row_is_scaled_into_it() {
        assert_eq!(ink_rows_outside_middle_row("\u{01D7}"), Vec::<u16>::new());
    }

    #[test]
    fn full_block_covers_the_middle_of_its_cell() {
        let canvas = render("\u{2588}");

        assert_eq!(canvas.pixel(6, 13), Some(INK));
    }

    #[test]
    fn full_block_leaves_the_next_cell_empty() {
        let canvas = render("\u{2588}");

        assert_eq!(canvas.pixel(18, 13), Some(BACKGROUND));
    }

    #[test]
    fn space_draws_nothing() {
        let canvas = render(" ");

        assert_eq!(canvas.pixel(6, 13), Some(BACKGROUND));
    }

    #[test]
    fn circumflex_adds_ink_above_pi() {
        let plain = render("\u{03C0}");
        let marked = render("\u{03C0}\u{0302}");

        let top_rows_differ =
            (0..8).any(|y| (0..12).any(|x| plain.pixel(x, y) != marked.pixel(x, y)));
        assert!(top_rows_differ);
    }

    #[test]
    fn negative_quarter_position_converts_to_fractional_pixels() {
        assert_eq!(signed_quarters_to_pixels(-5), Some(-1.25));
    }

    #[test]
    fn position_beyond_sixteen_bits_is_rejected() {
        assert_eq!(signed_quarters_to_pixels(i32::MAX), None);
    }
}
