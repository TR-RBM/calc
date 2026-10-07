use std::f64::consts::FRAC_PI_2;

use calc_viz::{Figure, FigureScale, Frame, MarkKind, Primitive, RightAngleMark};
use tiny_skia::PathBuilder;

use crate::arc::CircularArc;
use crate::colour::Colour;
use crate::draw::{DrawCommand, DrawList};
use crate::geometry::PhysicalRect;
use crate::mapping::{pixel_floor, to_f32};
use crate::picture_text::PictureText;
use crate::render::{Painter, RenderError, TextRow};
use crate::roles::{self, Fill, Patterns, Role};

const ENTRY_GAP_CELLS: u32 = 2;
const WORD_GAP_CELLS: u32 = 1;
const GLYPH_INSET_LOGICAL: u16 = 2;
const THIN_LOGICAL: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyEntry {
    Mark(MarkKind),
    Role(Role),
}

struct RowCursor {
    row: PhysicalRect,
    x: u32,
    slot: TextRow,
}

impl RowCursor {
    fn claim(&mut self, width: u32) -> Result<u16, RenderError> {
        let start = self.x;
        let end = start + width;
        if end > u32::from(self.row.right()) {
            return Err(RenderError::TextDoesNotFit(self.slot));
        }
        self.x = end;
        u16::try_from(start).map_err(|_| RenderError::TextDoesNotFit(self.slot))
    }

    fn gap(&mut self, width: u32) {
        self.x += width;
    }
}

fn words(
    painter: &mut Painter<'_>,
    cursor: &mut RowCursor,
    text: &str,
    colour: Colour,
) -> Result<(), RenderError> {
    if text.is_empty() {
        return Ok(());
    }
    let x = cursor.claim(painter.text_width(text))?;
    painter.text(text, x, cursor.row.y, colour)
}

fn swatch(
    painter: &mut Painter<'_>,
    cursor: &mut RowCursor,
    role: Role,
) -> Result<(), RenderError> {
    let inset = painter.physical(GLYPH_INSET_LOGICAL);
    let size = painter.row_height().saturating_sub(2 * inset);
    let x = cursor.claim(u32::from(painter.row_height()))?;
    let (left, top) = (x + inset, cursor.row.y + inset);
    let theme = painter.theme;
    let patterns = Patterns::new(theme, painter.scale, (left, top));
    let fill = match role {
        Role::Provisional => Fill::Solid(roles::provisional(&theme, theme.text)),
        Role::MayBeHit => Fill::Solid(roles::over_window(
            &theme,
            painter
                .may_be_hit
                .unwrap_or_else(|| roles::band_fill(theme.text_secondary)),
        )),
        _ => Fill::Role(role),
    };
    painter
        .canvas
        .paint_rect(PhysicalRect::new(left, top, size, size), &|x, y| {
            patterns.covering(fill, x, y)
        });
    let line = pixel_floor(f64::from(painter.line_width(THIN_LOGICAL)));
    let mut list = DrawList::new();
    list.push(DrawCommand::FrameRect {
        rect: PhysicalRect::new(left, top, size, size),
        line_width: line,
        colour: theme.text_tertiary,
    });
    painter.canvas.draw(&list);
    Ok(())
}

fn role_word(role: Role, text: &PictureText) -> &str {
    match role {
        Role::Missing => &text.roles.missing,
        Role::Unresolved => &text.roles.unresolved,
        Role::MayBeHit => &text.roles.may_be_hit,
        Role::Marked => &text.roles.marked,
        Role::Undecided => &text.escape_time.undecided,
        Role::Inside => &text.escape_time.inside,
        Role::BackFace => &text.roles.back_face,
        Role::Provisional => &text.roles.provisional,
    }
}

fn mark_glyph(
    painter: &mut Painter<'_>,
    cursor: &mut RowCursor,
    kind: MarkKind,
) -> Result<(), RenderError> {
    let height = painter.row_height();
    let inset = f64::from(painter.physical(GLYPH_INSET_LOGICAL));
    let width = painter.line_width(THIN_LOGICAL);
    let x = f64::from(cursor.claim(u32::from(height))?);
    let top = f64::from(cursor.row.y);
    let size = f64::from(height) - 2.0 * inset;
    let corner = (x + inset, top + inset + size);
    let colour = painter.theme.text;
    let mut builder = PathBuilder::new();
    builder.move_to(to_f32(corner.0 + size), to_f32(corner.1));
    builder.line_to(to_f32(corner.0), to_f32(corner.1));
    builder.line_to(to_f32(corner.0), to_f32(corner.1 - size));
    let quarter = |radius: f64| CircularArc {
        centre: corner,
        radius,
        start: 0.0,
        sweep: -FRAC_PI_2,
    };
    match kind {
        MarkKind::AngleArc { .. } | MarkKind::Direction => {
            quarter(size * 0.6).push_to(&mut builder, true)
        }
        MarkKind::RightAngle(RightAngleMark::ArcWithDot) => {
            quarter(size * 0.6).push_to(&mut builder, true);
            let dot = CircularArc {
                centre: (corner.0 + size * 0.2, corner.1 - size * 0.2),
                radius: f64::from(width),
                start: 0.0,
                sweep: std::f64::consts::TAU,
            };
            dot.push_to(&mut builder, true);
        }
        MarkKind::RightAngle(RightAngleMark::Square) => {
            let side = size * 0.4;
            builder.move_to(to_f32(corner.0 + side), to_f32(corner.1));
            builder.line_to(to_f32(corner.0 + side), to_f32(corner.1 - side));
            builder.line_to(to_f32(corner.0), to_f32(corner.1 - side));
        }
        MarkKind::EqualTicks { .. } => {
            let middle = corner.0 + size / 2.0;
            builder.move_to(to_f32(middle), to_f32(corner.1 - size * 0.3));
            builder.line_to(to_f32(middle), to_f32(corner.1 + size * 0.3 - inset));
        }
    }
    if let Some(path) = builder.finish() {
        painter
            .canvas
            .stroke_path_within(&path, colour, width, None, None);
    }
    Ok(())
}

fn mark_word(kind: MarkKind, text: &PictureText) -> &str {
    match kind {
        MarkKind::AngleArc { .. } => &text.mark_keys.angle_arc,
        MarkKind::RightAngle(RightAngleMark::Square) => &text.mark_keys.right_angle_square,
        MarkKind::RightAngle(RightAngleMark::ArcWithDot) => {
            &text.mark_keys.right_angle_arc_with_dot
        }
        MarkKind::EqualTicks { .. } => &text.mark_keys.equal_ticks,
        MarkKind::Direction => &text.mark_keys.direction,
    }
}

pub(crate) fn draw_key_row(
    painter: &mut Painter<'_>,
    row: PhysicalRect,
    entries: &[KeyEntry],
    text: &PictureText,
) -> Result<(), RenderError> {
    let cell = u32::from(painter.metrics.cell.width());
    let mut cursor = RowCursor {
        row,
        x: u32::from(row.x),
        slot: TextRow::Key,
    };
    let theme = painter.theme;
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            cursor.gap(ENTRY_GAP_CELLS * cell);
        }
        match entry {
            KeyEntry::Mark(kind) => {
                mark_glyph(painter, &mut cursor, *kind)?;
                cursor.gap(WORD_GAP_CELLS * cell);
                words(
                    painter,
                    &mut cursor,
                    mark_word(*kind, text),
                    theme.text_secondary,
                )?;
            }
            KeyEntry::Role(role) => {
                swatch(painter, &mut cursor, *role)?;
                cursor.gap(WORD_GAP_CELLS * cell);
                words(
                    painter,
                    &mut cursor,
                    role_word(*role, text),
                    theme.text_secondary,
                )?;
            }
        }
    }
    Ok(())
}

fn figures(frame: &Frame) -> impl Iterator<Item = &Figure> {
    frame
        .layers
        .iter()
        .filter_map(|layer| match &layer.primitive {
            Primitive::Figure(figure) => Some(figure.as_ref()),
            _ => None,
        })
}

pub(crate) fn footer_entries<'a>(
    frame: &Frame,
    text: &'a PictureText,
    is_escape_time: bool,
) -> Vec<&'a str> {
    let precision = frame
        .layers
        .iter()
        .filter_map(|layer| layer.precision.as_ref());
    let mut grid_limit = false;
    let mut value_limit = false;
    let mut unknown = false;
    let mut marked = false;
    let mut below = false;
    let explains_width = !text.precision.width_columns.is_empty()
        && frame.layers.iter().any(|layer| layer.columns.is_some());
    for limit in precision {
        grid_limit |= !limit.grid_exhausted.is_empty();
        value_limit |= limit.unresolved_samples > 0;
        unknown |= limit.unknown_bounds > 0;
        marked |= limit.marked_columns > 0;
        below |= limit.varies_below_bounds;
    }
    let is_sketch = figures(frame).any(|figure| figure.scale == FigureScale::Sketch);
    [
        (is_sketch, text.sketch.as_str()),
        (grid_limit, text.precision.grid_limit.as_str()),
        (value_limit && !marked, text.precision.value_limit.as_str()),
        (
            unknown && !is_escape_time && !explains_width,
            text.precision.unknown_bounds.as_str(),
        ),
        (true, text.precision.width_columns.as_str()),
        (marked, text.precision.marked_columns.as_str()),
        (
            below && !marked,
            text.precision.varies_below_bounds.as_str(),
        ),
        (is_escape_time, text.escape_time.undecided_share.as_str()),
    ]
    .into_iter()
    .filter(|(applies, words)| *applies && !words.is_empty())
    .map(|(_, words)| words)
    .collect()
}

pub(crate) fn draw_footer_row(
    painter: &mut Painter<'_>,
    row: PhysicalRect,
    entries: &[&str],
) -> Result<(), RenderError> {
    let cell = u32::from(painter.metrics.cell.width());
    let mut cursor = RowCursor {
        row,
        x: u32::from(row.x),
        slot: TextRow::Footer,
    };
    let colour = painter.theme.text_secondary;
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            cursor.gap(ENTRY_GAP_CELLS * cell);
        }
        words(painter, &mut cursor, entry, colour)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use calc_viz::{PrecisionLimit, Primitive};

    use super::*;
    use crate::test_scenes::{f64s, layer, style};

    fn frame_with(precision: PrecisionLimit) -> Frame {
        let mut limited = layer(
            Primitive::Polyline(calc_viz::Polyline {
                coordinates: vec![f64s(&[0.0]), f64s(&[0.0])],
            }),
            style(),
            Vec::new(),
        );
        limited.precision = Some(precision);
        Frame {
            parameter_values: Vec::new(),
            layers: vec![limited],
        }
    }

    fn text() -> PictureText {
        let mut text = PictureText::default();
        text.precision.grid_limit = String::from("grid");
        text.precision.value_limit = String::from("value");
        text.precision.unknown_bounds = String::from("unknown");
        text.escape_time.undecided_share = String::from("share");
        text
    }

    #[test]
    fn footer_names_each_limit_a_layer_reached() {
        let frame = frame_with(PrecisionLimit {
            grid_exhausted: vec![0],
            unresolved_samples: 2,
            unknown_bounds: 0,
            marked_columns: 0,
            varies_below_bounds: false,
        });

        assert_eq!(footer_entries(&frame, &text(), false), ["grid", "value"]);
    }

    #[test]
    fn escape_time_footer_shows_the_share_instead_of_unknown_bounds() {
        let frame = frame_with(PrecisionLimit {
            unknown_bounds: 4,
            ..PrecisionLimit::default()
        });

        assert_eq!(footer_entries(&frame, &text(), true), ["share"]);
    }

    #[test]
    fn footer_leaves_out_an_empty_word() {
        let frame = frame_with(PrecisionLimit {
            unresolved_samples: 1,
            ..PrecisionLimit::default()
        });

        assert!(footer_entries(&frame, &PictureText::default(), false).is_empty());
    }

    #[test]
    fn cursor_past_its_row_is_text_that_does_not_fit() {
        let mut cursor = RowCursor {
            row: PhysicalRect::new(10, 0, 20, 10),
            x: 10,
            slot: TextRow::Key,
        };

        assert_eq!(
            cursor.claim(21),
            Err(RenderError::TextDoesNotFit(TextRow::Key))
        );
    }

    #[test]
    fn cursor_claims_from_where_the_last_claim_ended() {
        let mut cursor = RowCursor {
            row: PhysicalRect::new(10, 0, 20, 10),
            x: 10,
            slot: TextRow::Key,
        };
        cursor.claim(5).unwrap();

        assert_eq!(cursor.claim(5), Ok(15));
    }
}
