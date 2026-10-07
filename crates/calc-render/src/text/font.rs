use harfrust::ShaperData;
use skrifa::raw::TableProvider;
use skrifa::{FontRef, GlyphId, MetadataProvider};

const MONOSPACE_DATA: &[u8] = include_bytes!("../../fonts/NotoSansMono-Regular.ttf");
const MATH_DATA: &[u8] = include_bytes!("../../fonts/NotoSansMath-Regular.ttf");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BundledFont {
    Monospace,
    Math,
}

impl BundledFont {
    pub const fn data(self) -> &'static [u8] {
        match self {
            Self::Monospace => MONOSPACE_DATA,
            Self::Math => MATH_DATA,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontTable {
    Head,
    Hhea,
    Hmtx,
    Glyf,
    Loca,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontError {
    Unreadable { font: BundledFont },
    MissingTable { font: BundledFont, table: FontTable },
    MissingGlyph { font: BundledFont, character: char },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerticalMetrics {
    pub ascender: i16,
    pub descender: i16,
    pub line_gap: i16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InkExtent {
    pub left: i16,
    pub right: i16,
    pub bottom: i16,
    pub top: i16,
}

pub struct Face {
    font: BundledFont,
    font_ref: FontRef<'static>,
    shaper_data: ShaperData,
    units_per_em: u16,
    vertical_metrics: VerticalMetrics,
}

impl Face {
    pub fn load(font: BundledFont) -> Result<Self, FontError> {
        let font_ref = FontRef::new(font.data()).map_err(|_| FontError::Unreadable { font })?;
        let units_per_em = font_ref
            .head()
            .map_err(|_| FontError::MissingTable {
                font,
                table: FontTable::Head,
            })?
            .units_per_em();
        let hhea = font_ref.hhea().map_err(|_| FontError::MissingTable {
            font,
            table: FontTable::Hhea,
        })?;
        let vertical_metrics = VerticalMetrics {
            ascender: hhea.ascender().to_i16(),
            descender: hhea.descender().to_i16(),
            line_gap: hhea.line_gap().to_i16(),
        };
        let shaper_data = ShaperData::new(&font_ref);
        Ok(Self {
            font,
            font_ref,
            shaper_data,
            units_per_em,
            vertical_metrics,
        })
    }

    pub const fn font(&self) -> BundledFont {
        self.font
    }

    pub const fn font_ref(&self) -> &FontRef<'static> {
        &self.font_ref
    }

    pub const fn shaper_data(&self) -> &ShaperData {
        &self.shaper_data
    }

    pub const fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    pub const fn vertical_metrics(&self) -> VerticalMetrics {
        self.vertical_metrics
    }

    pub fn covers(&self, character: char) -> bool {
        self.font_ref.charmap().map(character).is_some()
    }

    pub fn advance_of(&self, character: char) -> Result<u16, FontError> {
        let missing = FontError::MissingGlyph {
            font: self.font,
            character,
        };
        let glyph_id = self.font_ref.charmap().map(character).ok_or(missing)?;
        self.font_ref
            .hmtx()
            .map_err(|_| FontError::MissingTable {
                font: self.font,
                table: FontTable::Hmtx,
            })?
            .advance(glyph_id)
            .ok_or(missing)
    }

    pub fn ink_extent(&self, glyph_id: u32) -> Result<Option<InkExtent>, FontError> {
        let glyf = self.font_ref.glyf().map_err(|_| FontError::MissingTable {
            font: self.font,
            table: FontTable::Glyf,
        })?;
        let loca = self
            .font_ref
            .loca(None)
            .map_err(|_| FontError::MissingTable {
                font: self.font,
                table: FontTable::Loca,
            })?;
        let glyph =
            loca.get_glyf(GlyphId::new(glyph_id), &glyf)
                .map_err(|_| FontError::MissingTable {
                    font: self.font,
                    table: FontTable::Glyf,
                })?;
        Ok(glyph.map(|glyph| InkExtent {
            left: glyph.x_min(),
            right: glyph.x_max(),
            bottom: glyph.y_min(),
            top: glyph.y_max(),
        }))
    }
}

pub struct FontSet {
    monospace: Face,
    math: Face,
}

impl FontSet {
    pub fn bundled() -> Result<Self, FontError> {
        Ok(Self {
            monospace: Face::load(BundledFont::Monospace)?,
            math: Face::load(BundledFont::Math)?,
        })
    }

    pub const fn face(&self, font: BundledFont) -> &Face {
        match font {
            BundledFont::Monospace => &self.monospace,
            BundledFont::Math => &self.math,
        }
    }

    pub const fn monospace(&self) -> &Face {
        &self.monospace
    }

    pub fn first_covering(&self, text: &str) -> Option<&Face> {
        [&self.monospace, &self.math]
            .into_iter()
            .find(|face| text.chars().all(|character| face.covers(character)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fonts");
    const OFL_LICENCE: &str = "OFL-1.1";

    fn fonts() -> FontSet {
        FontSet::bundled().expect("bundled fonts load")
    }

    fn source_rows() -> Vec<Vec<String>> {
        let sources = std::fs::read_to_string(format!("{FONT_DIRECTORY}/SOURCES.md"))
            .expect("SOURCES.md is readable");
        sources
            .lines()
            .filter(|line| line.starts_with("| ") && !line.starts_with("| File"))
            .map(|line| {
                line.trim_matches('|')
                    .split('|')
                    .map(|cell| cell.trim().to_owned())
                    .collect()
            })
            .collect()
    }

    fn font_file_names() -> Vec<String> {
        std::fs::read_dir(FONT_DIRECTORY)
            .expect("font directory is readable")
            .map(|entry| entry.expect("directory entry").file_name())
            .filter_map(|name| name.into_string().ok())
            .filter(|name| name.ends_with(".ttf"))
            .collect()
    }

    #[test]
    fn monospace_font_has_one_thousand_units_per_em() {
        assert_eq!(fonts().monospace().units_per_em(), 1000);
    }

    #[test]
    fn monospace_font_vertical_metrics_match_its_hhea_table() {
        assert_eq!(
            fonts().monospace().vertical_metrics(),
            VerticalMetrics {
                ascender: 1069,
                descender: -293,
                line_gap: 0
            }
        );
    }

    #[test]
    fn monospace_digit_advance_is_six_hundred_units() {
        assert_eq!(fonts().monospace().advance_of('0'), Ok(600));
    }

    #[test]
    fn missing_character_has_no_advance() {
        assert_eq!(
            fonts().monospace().advance_of('\u{222B}'),
            Err(FontError::MissingGlyph {
                font: BundledFont::Monospace,
                character: '\u{222B}'
            })
        );
    }

    #[test]
    fn pi_ink_spans_its_glyph_bounding_box() {
        assert_eq!(
            fonts().monospace().ink_extent(395),
            Ok(Some(InkExtent {
                left: 37,
                right: 562,
                bottom: -10,
                top: 536
            }))
        );
    }

    #[test]
    fn space_has_no_ink() {
        assert_eq!(fonts().monospace().ink_extent(3), Ok(None));
    }

    #[test]
    fn plain_text_is_covered_by_monospace_font() {
        let fonts = fonts();

        let face = fonts.first_covering("x = 2").map(Face::font);

        assert_eq!(face, Some(BundledFont::Monospace));
    }

    #[test]
    fn integral_is_covered_by_math_font() {
        let fonts = fonts();

        let face = fonts.first_covering("\u{222B}").map(Face::font);

        assert_eq!(face, Some(BundledFont::Math));
    }

    #[test]
    fn text_outside_both_fonts_has_no_covering_face() {
        assert!(fonts().first_covering("\u{4E00}").is_none());
    }

    #[test]
    fn every_font_file_has_a_sources_row() {
        let rows = source_rows();

        let unlisted: Vec<String> = font_file_names()
            .into_iter()
            .filter(|name| !rows.iter().any(|row| row.first() == Some(name)))
            .collect();

        assert_eq!(unlisted, Vec::<String>::new());
    }

    #[test]
    fn every_sources_row_names_the_open_font_licence() {
        let rows = source_rows();

        assert!(
            rows.iter()
                .all(|row| row.get(4).map(String::as_str) == Some(OFL_LICENCE))
        );
    }

    #[test]
    fn open_font_licence_text_is_bundled() {
        assert!(std::path::Path::new(&format!("{FONT_DIRECTORY}/OFL.txt")).is_file());
    }
}
