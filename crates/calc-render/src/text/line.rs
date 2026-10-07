use crate::canvas::Canvas;
use crate::colour::Colour;
use crate::text::font::FontSet;
use crate::text::grid::GridMetrics;
use crate::text::placement::{GridRowOrigin, PlacedGlyph, PlacementError, place_on_grid};
use crate::text::render::{GlyphRenderError, draw_glyphs};
use crate::text::shape::shape_with_fallback;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextDrawError {
    Placement(PlacementError),
    Render(GlyphRenderError),
}

pub struct TextPainter<'a> {
    pub fonts: &'a FontSet,
    pub metrics: GridMetrics,
}

impl TextPainter<'_> {
    pub fn draw_data(
        &self,
        canvas: &mut Canvas,
        text: &str,
        origin: GridRowOrigin,
        colour: Colour,
    ) -> Result<(), TextDrawError> {
        if text.is_empty() {
            return Ok(());
        }
        let clusters = shape_with_fallback(text, self.fonts);
        let glyphs: Vec<PlacedGlyph> =
            place_on_grid(text, &clusters, self.fonts, self.metrics, origin)
                .map_err(TextDrawError::Placement)?
                .into_iter()
                .flat_map(|cluster| cluster.glyphs)
                .collect();
        draw_glyphs(canvas, &glyphs, self.fonts, colour).map_err(TextDrawError::Render)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::grid::grid_metrics;

    const BACKGROUND: Colour = Colour::opaque(255, 255, 255);
    const INK: Colour = Colour::opaque(0, 0, 0);

    #[test]
    fn text_is_drawn_from_its_origin_cell() {
        let fonts = FontSet::bundled().expect("bundled fonts load");
        let painter = TextPainter {
            fonts: &fonts,
            metrics: grid_metrics(&fonts, 20).expect("metrics in range"),
        };
        let mut canvas = Canvas::new(48, 27, BACKGROUND).expect("non-empty canvas");

        painter
            .draw_data(
                &mut canvas,
                "\u{2588}",
                GridRowOrigin { x: 24, top: 0 },
                INK,
            )
            .expect("text draws");

        assert_eq!(
            [canvas.pixel(6, 13), canvas.pixel(30, 13)],
            [Some(BACKGROUND), Some(INK)]
        );
    }

    #[test]
    fn empty_text_draws_nothing() {
        let fonts = FontSet::bundled().expect("bundled fonts load");
        let painter = TextPainter {
            fonts: &fonts,
            metrics: grid_metrics(&fonts, 20).expect("metrics in range"),
        };
        let mut canvas = Canvas::new(12, 27, BACKGROUND).expect("non-empty canvas");

        let result = painter.draw_data(&mut canvas, "", GridRowOrigin { x: 0, top: 0 }, INK);

        assert_eq!((result, canvas.pixel(6, 13)), (Ok(()), Some(BACKGROUND)));
    }
}
