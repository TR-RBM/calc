use crate::text::cells::cell_span;
use crate::text::font::{BundledFont, FontError, FontSet};
use crate::text::grid::{GridMetrics, QUARTERS_PER_PIXEL, font_units_to_quarters};
use crate::text::shape::Cluster;

const PLACEMENT_SLACK_QUARTERS: i64 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementError {
    Font(FontError),
    OutOfRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedGlyph {
    pub font: BundledFont,
    pub glyph_id: u32,
    pub origin_x_quarters: i32,
    pub baseline_y_quarters: i32,
    pub size_quarters: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacedCluster {
    pub first_cell: u16,
    pub cells: u16,
    pub glyphs: Vec<PlacedGlyph>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridRowOrigin {
    pub x: u16,
    pub top: u16,
}

#[derive(Clone, Copy)]
struct InkBox {
    left: i64,
    right: i64,
    bottom: i64,
    top: i64,
}

impl InkBox {
    fn union(self, other: InkBox) -> InkBox {
        InkBox {
            left: self.left.min(other.left),
            right: self.right.max(other.right),
            bottom: self.bottom.min(other.bottom),
            top: self.top.max(other.top),
        }
    }
}

struct ClusterExtent {
    advance: i64,
    ink: Option<InkBox>,
}

pub fn place_on_grid(
    text: &str,
    clusters: &[Cluster],
    fonts: &FontSet,
    metrics: GridMetrics,
    origin: GridRowOrigin,
) -> Result<Vec<PlacedCluster>, PlacementError> {
    let cell_quarters = i64::from(metrics.cell.width()) * QUARTERS_PER_PIXEL;
    let full_size_quarters = i64::from(metrics.font_size_px) * QUARTERS_PER_PIXEL;
    let baseline_quarters =
        (i64::from(origin.top) + i64::from(metrics.baseline)) * QUARTERS_PER_PIXEL;
    let mut next_cell: u16 = 0;
    let mut placed = Vec::with_capacity(clusters.len());

    for cluster in clusters {
        let face = fonts.face(cluster.font);
        let units_per_em = face.units_per_em();
        let cells = text
            .get(cluster.text_range.clone())
            .and_then(|cluster_text| cluster_text.chars().next())
            .map_or(1, cell_span);
        let span_quarters = i64::from(cells) * cell_quarters;
        let extent = cluster_extent(cluster, fonts)?;

        let units = i64::from(units_per_em);
        let above_baseline_quarters = i64::from(metrics.baseline) * QUARTERS_PER_PIXEL;
        let below_baseline_quarters =
            (i64::from(metrics.cell.height()) - i64::from(metrics.baseline)) * QUARTERS_PER_PIXEL;
        let size_quarters = extent.ink.map_or(full_size_quarters, |ink| {
            [
                (ink.right - ink.left, span_quarters),
                (ink.top, above_baseline_quarters),
                (-ink.bottom, below_baseline_quarters),
            ]
            .into_iter()
            .filter(|&(ink_units, room)| ink_units * full_size_quarters > room * units)
            .map(|(ink_units, room)| {
                ((room - PLACEMENT_SLACK_QUARTERS).max(0) * units).div_euclid(ink_units)
            })
            .fold(full_size_quarters, i64::min)
        });
        let scale = |units: i64| font_units_to_quarters(units, size_quarters, units_per_em);

        let cell_start_quarters = (i64::from(origin.x)
            + i64::from(next_cell) * i64::from(metrics.cell.width()))
            * QUARTERS_PER_PIXEL;
        let scaled_advance = scale(extent.advance);
        let pen_start = match extent.ink {
            Some(InkBox { left, right, .. }) if scaled_advance > span_quarters => {
                let units_per_em = i64::from(units_per_em);
                let centred_numerator = cell_start_quarters * units_per_em
                    + (span_quarters * units_per_em - (right - left) * size_quarters).div_euclid(2)
                    - left * size_quarters;
                -(-centred_numerator).div_euclid(units_per_em)
            }
            _ => cell_start_quarters + (span_quarters - scaled_advance).div_euclid(2),
        };

        let mut pen_units: i64 = 0;
        let mut glyphs = Vec::with_capacity(cluster.glyphs.len());
        for glyph in &cluster.glyphs {
            let origin_x = pen_start + scale(pen_units + i64::from(glyph.x_offset));
            let baseline_y = baseline_quarters - scale(i64::from(glyph.y_offset));
            glyphs.push(PlacedGlyph {
                font: cluster.font,
                glyph_id: glyph.glyph_id,
                origin_x_quarters: to_i32(origin_x)?,
                baseline_y_quarters: to_i32(baseline_y)?,
                size_quarters: u32::try_from(size_quarters)
                    .map_err(|_| PlacementError::OutOfRange)?,
            });
            pen_units += i64::from(glyph.x_advance);
        }

        placed.push(PlacedCluster {
            first_cell: next_cell,
            cells,
            glyphs,
        });
        next_cell = next_cell
            .checked_add(cells)
            .ok_or(PlacementError::OutOfRange)?;
    }
    Ok(placed)
}

fn cluster_extent(cluster: &Cluster, fonts: &FontSet) -> Result<ClusterExtent, PlacementError> {
    let face = fonts.face(cluster.font);
    let mut pen: i64 = 0;
    let mut ink: Option<InkBox> = None;
    for glyph in &cluster.glyphs {
        let glyph_origin = pen + i64::from(glyph.x_offset);
        let glyph_raise = i64::from(glyph.y_offset);
        if let Some(extent) = face
            .ink_extent(glyph.glyph_id)
            .map_err(PlacementError::Font)?
        {
            let glyph_ink = InkBox {
                left: glyph_origin + i64::from(extent.left),
                right: glyph_origin + i64::from(extent.right),
                bottom: glyph_raise + i64::from(extent.bottom),
                top: glyph_raise + i64::from(extent.top),
            };
            ink = Some(ink.map_or(glyph_ink, |known| known.union(glyph_ink)));
        }
        pen += i64::from(glyph.x_advance);
    }
    Ok(ClusterExtent { advance: pen, ink })
}

fn to_i32(quarters: i64) -> Result<i32, PlacementError> {
    i32::try_from(quarters).map_err(|_| PlacementError::OutOfRange)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::grid::grid_metrics;
    use crate::text::shape::shape_with_fallback;

    const FONT_SIZE_PX: u16 = 20;
    const CELL_QUARTERS: i32 = 48;
    const FULL_SIZE_QUARTERS: u32 = 80;
    const ORIGIN: GridRowOrigin = GridRowOrigin { x: 0, top: 0 };

    fn place(text: &str) -> Vec<PlacedCluster> {
        let fonts = FontSet::bundled().expect("bundled fonts load");
        let metrics = grid_metrics(&fonts, FONT_SIZE_PX).expect("metrics in range");
        let clusters = shape_with_fallback(text, &fonts);
        place_on_grid(text, &clusters, &fonts, metrics, ORIGIN).expect("placement in range")
    }

    fn origins(placed: &[PlacedCluster]) -> Vec<i32> {
        placed
            .iter()
            .flat_map(|cluster| cluster.glyphs.iter().map(|glyph| glyph.origin_x_quarters))
            .collect()
    }

    #[test]
    fn monospace_glyphs_start_at_their_cells() {
        let placed = place("x=0");

        assert_eq!(origins(&placed), vec![0, CELL_QUARTERS, 2 * CELL_QUARTERS]);
    }

    #[test]
    fn glyphs_sit_on_the_row_baseline() {
        let placed = place("x");

        assert_eq!(placed[0].glyphs[0].baseline_y_quarters, 21 * 4);
    }

    #[test]
    fn row_origin_moves_every_glyph() {
        let fonts = FontSet::bundled().expect("bundled fonts load");
        let metrics = grid_metrics(&fonts, FONT_SIZE_PX).expect("metrics in range");
        let clusters = shape_with_fallback("x", &fonts);

        let placed = place_on_grid(
            "x",
            &clusters,
            &fonts,
            metrics,
            GridRowOrigin { x: 3, top: 10 },
        )
        .expect("placement in range");

        let glyph = placed[0].glyphs[0];
        assert_eq!(
            (glyph.origin_x_quarters, glyph.baseline_y_quarters),
            (12, 31 * 4)
        );
    }

    #[test]
    fn combining_circumflex_is_placed_over_pi_within_its_cell() {
        let placed = place("\u{03C0}\u{0302}");

        let mark_origin_units: i64 = 600 - 299;
        let expected_mark_quarters = (mark_origin_units * 80 + 500) / 1000;
        assert_eq!(
            (placed[0].cells, origins(&placed)),
            (
                1,
                vec![0, i32::try_from(expected_mark_quarters).expect("small")]
            )
        );
    }

    #[test]
    fn character_after_pi_with_circumflex_starts_in_the_next_cell() {
        let placed = place("\u{03C0}\u{0302}x");

        assert_eq!(placed[1].glyphs[0].origin_x_quarters, CELL_QUARTERS);
    }

    #[test]
    fn integral_wider_than_its_cell_is_scaled_down() {
        let placed = place("\u{222B}");

        assert!(placed[0].glyphs[0].size_quarters < FULL_SIZE_QUARTERS);
    }

    #[test]
    fn scaled_integral_ink_fits_inside_its_cell() {
        let placed = place("\u{222B}");

        let glyph = placed[0].glyphs[0];
        let size = i64::from(glyph.size_quarters);
        let ink_left = i64::from(glyph.origin_x_quarters) + (51 * size).div_euclid(1000);
        let ink_right = i64::from(glyph.origin_x_quarters) + (785 * size + 999).div_euclid(1000);
        assert!(ink_left >= 0 && ink_right <= i64::from(CELL_QUARTERS));
    }

    #[test]
    fn monospace_glyph_keeps_full_size() {
        let placed = place("x");

        assert_eq!(placed[0].glyphs[0].size_quarters, FULL_SIZE_QUARTERS);
    }

    #[test]
    fn summation_sign_falls_back_and_fits_its_cell() {
        let placed = place("\u{2211}");

        let glyph = placed[0].glyphs[0];
        let size = i64::from(glyph.size_quarters);
        let ink_left = i64::from(glyph.origin_x_quarters) + (51 * size).div_euclid(1000);
        let ink_right = i64::from(glyph.origin_x_quarters) + (801 * size + 999).div_euclid(1000);
        assert!(ink_left >= 0 && ink_right <= i64::from(CELL_QUARTERS));
    }

    #[test]
    fn wide_character_takes_two_cells() {
        let placed = place("\u{4E00}x");

        assert_eq!((placed[0].cells, placed[1].first_cell), (2, 2));
    }
}
