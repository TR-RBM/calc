use std::ops::Range;

use harfrust::{ShapeOptions, UnicodeBuffer};

use crate::text::font::{BundledFont, Face, FontSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapedGlyph {
    pub glyph_id: u32,
    pub x_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cluster {
    pub text_range: Range<usize>,
    pub font: BundledFont,
    pub glyphs: Vec<ShapedGlyph>,
}

pub fn shape_clusters(text: &str, face: &Face) -> Vec<Cluster> {
    let shaper = face.shaper_data().shaper(face.font_ref()).build();
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    let output = shaper.shape(buffer, ShapeOptions::new());

    let mut clusters: Vec<Cluster> = Vec::new();
    for (info, position) in output.glyph_infos().iter().zip(output.glyph_positions()) {
        let start = usize::try_from(info.cluster).unwrap_or(text.len());
        let glyph = ShapedGlyph {
            glyph_id: info.glyph_id,
            x_advance: position.x_advance,
            x_offset: position.x_offset,
            y_offset: position.y_offset,
        };
        match clusters.last_mut() {
            Some(cluster) if cluster.text_range.start == start => cluster.glyphs.push(glyph),
            _ => clusters.push(Cluster {
                text_range: start..text.len(),
                font: face.font(),
                glyphs: vec![glyph],
            }),
        }
    }
    let starts: Vec<usize> = clusters
        .iter()
        .map(|cluster| cluster.text_range.start)
        .collect();
    for (cluster, next_start) in clusters.iter_mut().zip(starts.into_iter().skip(1)) {
        cluster.text_range.end = next_start;
    }
    clusters
}

pub fn shape_with_fallback(text: &str, fonts: &FontSet) -> Vec<Cluster> {
    shape_clusters(text, fonts.monospace())
        .into_iter()
        .flat_map(|cluster| {
            let cluster_text = &text[cluster.text_range.clone()];
            match fonts.first_covering(cluster_text) {
                Some(face) if face.font() != cluster.font => {
                    shifted(shape_clusters(cluster_text, face), cluster.text_range.start)
                }
                _ => vec![cluster],
            }
        })
        .collect()
}

fn shifted(clusters: Vec<Cluster>, offset: usize) -> Vec<Cluster> {
    clusters
        .into_iter()
        .map(|cluster| Cluster {
            text_range: cluster.text_range.start + offset..cluster.text_range.end + offset,
            ..cluster
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONOSPACE_PI: u32 = 395;
    const MONOSPACE_COMBINING_CIRCUMFLEX: u32 = 3481;
    const MONOSPACE_ADVANCE: i32 = 600;
    const MONOSPACE_PI_TOP_ANCHOR_X: i32 = 301;
    const MONOSPACE_CIRCUMFLEX_ANCHOR_X: i32 = 0;
    const MATH_INTEGRAL: u32 = 2847;
    const NOT_DEFINED_GLYPH: u32 = 0;

    fn fonts() -> FontSet {
        FontSet::bundled().expect("bundled fonts load")
    }

    fn glyph_ids(clusters: &[Cluster]) -> Vec<u32> {
        clusters
            .iter()
            .flat_map(|cluster| cluster.glyphs.iter().map(|glyph| glyph.glyph_id))
            .collect()
    }

    #[test]
    fn digits_and_letters_map_to_their_monospace_glyphs() {
        let clusters = shape_with_fallback("x=0", &fonts());

        assert_eq!(glyph_ids(&clusters), vec![91, 32, 19]);
    }

    #[test]
    fn every_monospace_glyph_advances_one_cell() {
        let clusters = shape_with_fallback("x=0", &fonts());

        assert!(
            clusters
                .iter()
                .all(|cluster| cluster.glyphs[0].x_advance == MONOSPACE_ADVANCE)
        );
    }

    #[test]
    fn pi_with_combining_circumflex_is_one_cluster_of_two_glyphs() {
        let clusters = shape_with_fallback("\u{03C0}\u{0302}", &fonts());

        assert_eq!(
            (clusters.len(), glyph_ids(&clusters)),
            (1, vec![MONOSPACE_PI, MONOSPACE_COMBINING_CIRCUMFLEX])
        );
    }

    #[test]
    fn combining_circumflex_attaches_to_top_anchor_of_pi() {
        let clusters = shape_with_fallback("\u{03C0}\u{0302}", &fonts());

        let mark = clusters[0].glyphs[1];
        let expected_x_offset =
            MONOSPACE_PI_TOP_ANCHOR_X - MONOSPACE_CIRCUMFLEX_ANCHOR_X - MONOSPACE_ADVANCE;
        assert_eq!(
            (mark.x_advance, mark.x_offset, mark.y_offset),
            (0, expected_x_offset, 0)
        );
    }

    #[test]
    fn integral_sign_falls_back_to_math_font() {
        let clusters = shape_with_fallback("\u{222B}", &fonts());

        assert_eq!(
            (clusters[0].font, glyph_ids(&clusters)),
            (BundledFont::Math, vec![MATH_INTEGRAL])
        );
    }

    #[test]
    fn fallback_cluster_keeps_its_place_in_the_text() {
        let text = "x\u{222B}x";

        let clusters = shape_with_fallback(text, &fonts());

        let ranges: Vec<Range<usize>> = clusters
            .iter()
            .map(|cluster| cluster.text_range.clone())
            .collect();
        assert_eq!(ranges, vec![0..1, 1..4, 4..5]);
    }

    #[test]
    fn character_outside_both_fonts_stays_in_monospace_as_missing_glyph() {
        let clusters = shape_with_fallback("\u{4E00}", &fonts());

        assert_eq!(
            (clusters[0].font, glyph_ids(&clusters)),
            (BundledFont::Monospace, vec![NOT_DEFINED_GLYPH])
        );
    }
}
