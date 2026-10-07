use calc_viz::{Layer, Primitive, Scene, View};

use crate::geometry::PhysicalRect;
use crate::layout::{PictureLayout, ViewLayout};
use crate::picture_text::PictureText;
use crate::render::joined_title;
use crate::text::cells::cells_of;
use crate::text::grid::GridMetrics;

pub(crate) const TITLE_SEPARATOR: &str = "  ";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Alignment {
    Start,
    Centre,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TextPlacing {
    metrics: GridMetrics,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TitlePlace {
    pub row: Option<PhysicalRect>,
    pub left: Option<u16>,
    pub unit: Option<PhysicalRect>,
}

impl TextPlacing {
    pub(crate) const fn new(metrics: GridMetrics) -> Self {
        Self { metrics }
    }

    pub(crate) fn width(&self, text: &str) -> u32 {
        u32::from(cells_of(text)) * u32::from(self.metrics.cell.width())
    }

    pub(crate) fn left_in_row(
        &self,
        text: &str,
        row: PhysicalRect,
        alignment: Alignment,
    ) -> Option<u16> {
        if text.is_empty() {
            return Some(row.x);
        }
        let spare = u32::from(row.width).checked_sub(self.width(text))?;
        let offset = match alignment {
            Alignment::Start => 0,
            Alignment::Centre => spare / 2,
        };
        u16::try_from(u32::from(row.x) + offset).ok()
    }

    pub(crate) fn tail_rect(
        &self,
        text: &str,
        tail: &str,
        (left, row): (u16, PhysicalRect),
    ) -> Option<PhysicalRect> {
        if tail.is_empty() || !text.ends_with(tail) {
            return None;
        }
        let head = text.get(..text.len() - tail.len())?;
        let start = u16::try_from(u32::from(left) + self.width(head)).ok()?;
        let width = u16::try_from(self.width(tail)).ok()?;
        Some(PhysicalRect::new(start, row.y, width, row.height))
    }

    fn placed(
        &self,
        text: &str,
        row: Option<PhysicalRect>,
        alignment: Alignment,
        tail: &str,
    ) -> TitlePlace {
        let Some(row) = row else {
            return TitlePlace::default();
        };
        let left = self.left_in_row(text, row, alignment);
        TitlePlace {
            row: Some(row),
            left,
            unit: left.and_then(|left| self.tail_rect(text, tail, (left, row))),
        }
    }
}

pub(crate) fn plane_title_places(
    placing: &TextPlacing,
    layout: &ViewLayout,
    titles: [&str; 2],
    symbols: [&str; 2],
) -> [TitlePlace; 2] {
    let area = layout.plot_area;
    let under_area = layout
        .axis_title_row
        .map(|row| PhysicalRect::new(area.x, row.y, area.width, row.height));
    [
        placing.placed(titles[0], under_area, Alignment::Centre, symbols[0]),
        placing.placed(
            titles[1],
            Some(layout.title_row),
            Alignment::Start,
            symbols[1],
        ),
    ]
}

pub(crate) fn space_title_places(
    placing: &TextPlacing,
    layout: &ViewLayout,
    titles: &[String],
    symbols: &[String],
) -> (String, TitlePlace, Vec<Option<PhysicalRect>>) {
    let row = layout.title_row;
    let joined = titles.join(TITLE_SEPARATOR);
    let place = placing.placed(&joined, Some(row), Alignment::Start, "");
    let mut units = Vec::with_capacity(titles.len());
    let mut before = String::new();
    for (index, title) in titles.iter().enumerate() {
        if index > 0 {
            before.push_str(TITLE_SEPARATOR);
        }
        let symbol = symbols.get(index).cloned().unwrap_or_default();
        let shown = format!("{before}{title}");
        units.push(
            place
                .left
                .and_then(|left| placing.tail_rect(&shown, &symbol, (left, row))),
        );
        before.push_str(title);
    }
    (joined, place, units)
}

pub fn axis_unit_rects(
    layout: &PictureLayout,
    scene: &Scene,
    frame: usize,
    text: &PictureText,
) -> Vec<Vec<Option<PhysicalRect>>> {
    let placing = TextPlacing::new(layout.metrics);
    let empty = Vec::new();
    let layers = scene
        .frames
        .get(frame)
        .map_or(&empty, |frame| &frame.layers);
    scene
        .views
        .iter()
        .zip(&layout.views)
        .enumerate()
        .map(|(index, (view, view_layout))| {
            let titles = text.axis_titles.get(index);
            match view {
                View::View2(plane) => {
                    if shows_only_figures(layers, index) {
                        return Vec::new();
                    }
                    let symbols = [plane.x.unit.symbol(), plane.y.unit.symbol()];
                    let joined = [
                        joined_title(
                            titles.and_then(|titles| titles.first()),
                            &text.unit_joiner,
                            symbols[0],
                        ),
                        joined_title(
                            titles.and_then(|titles| titles.get(1)),
                            &text.unit_joiner,
                            symbols[1],
                        ),
                    ];
                    plane_title_places(&placing, view_layout, [&joined[0], &joined[1]], symbols)
                        .iter()
                        .map(|place| place.unit)
                        .collect()
                }
                View::View3(space) => {
                    let axes = [&space.x, &space.y, &space.z];
                    let symbols: Vec<String> = axes
                        .iter()
                        .map(|axis| axis.unit.symbol().to_owned())
                        .collect();
                    let shown: Vec<String> = axes
                        .iter()
                        .enumerate()
                        .map(|(axis, _)| {
                            joined_title(
                                titles.and_then(|titles| titles.get(axis)),
                                &text.unit_joiner,
                                &symbols[axis],
                            )
                        })
                        .filter(|title| !title.is_empty())
                        .collect();
                    space_title_places(&placing, view_layout, &shown, &symbols).2
                }
            }
        })
        .collect()
}

pub(crate) fn shows_only_figures(layers: &[Layer], view: usize) -> bool {
    let of_view: Vec<&Layer> = layers
        .iter()
        .filter(|layer| usize::try_from(layer.view.0).ok() == Some(view))
        .collect();
    !of_view.is_empty()
        && of_view
            .iter()
            .all(|layer| matches!(layer.primitive, Primitive::Figure(_)))
}

#[cfg(test)]
mod tests {
    use calc_viz::{Primitive, SamplingDiagnostics, SamplingMethod};

    use super::*;
    use crate::canvas::Canvas;
    use crate::geometry::ScaleFactor;
    use crate::render::{DrawnScene, RenderRequest, draw_scene};
    use crate::test_scenes::{
        HEIGHT, WIDTH, f64s, layer, metre_plane, metre_space, plane, record, scene, scene_with,
        style,
    };
    use crate::text::font::FontSet;
    use crate::theme::Theme;

    fn drawn(scene: &Scene, text: &PictureText) -> DrawnScene {
        let fonts = FontSet::bundled().expect("bundled fonts load");
        let mut canvas =
            Canvas::new(WIDTH, HEIGHT, Theme::light().window).expect("non-empty canvas");
        draw_scene(
            &mut canvas,
            (0, 0),
            &RenderRequest {
                scene,
                settled: None,
                frame: 0,
                width: WIDTH,
                height: HEIGHT,
                text_size: 14,
                scale: ScaleFactor::from_percent(100).expect("non-zero scale"),
                theme: Theme::light(),
                decimal_separator: '.',
                text,
            },
            &fonts,
        )
        .expect("the scene draws")
    }

    fn axis_text(names: &[&str]) -> PictureText {
        PictureText {
            axis_titles: vec![names.iter().map(|name| (*name).to_owned()).collect()],
            unit_joiner: String::from("in"),
            ..PictureText::default()
        }
    }

    fn curve() -> Primitive {
        Primitive::Polyline(calc_viz::Polyline {
            coordinates: vec![f64s(&[0.0, 10.0]), f64s(&[5.0, 5.0])],
        })
    }

    fn measured(
        drawn: &DrawnScene,
        scene: &Scene,
        text: &PictureText,
    ) -> Vec<Vec<Option<PhysicalRect>>> {
        axis_unit_rects(&drawn.layout, scene, 0, text)
    }

    #[test]
    fn a_plane_view_measures_the_rectangles_it_draws() {
        let text = axis_text(&["length", "height"]);
        let scene = scene(
            metre_plane(0, 10),
            vec![layer(curve(), style(), vec![f64s(&[0.0, 0.0])])],
        );

        let drawn = drawn(&scene, &text);

        assert_eq!(measured(&drawn, &scene, &text), drawn.axis_units);
    }

    #[test]
    fn a_space_view_measures_the_rectangles_it_draws() {
        let text = axis_text(&["length", "width", "height"]);
        let scene = scene_with(
            record(SamplingMethod::UniformGrid, SamplingDiagnostics::default()),
            metre_space(0, 1, 30, 20),
            Vec::new(),
        );

        let drawn = drawn(&scene, &text);

        assert_eq!(measured(&drawn, &scene, &text), drawn.axis_units);
    }

    #[test]
    fn an_axis_without_a_unit_measures_no_rectangle() {
        let text = axis_text(&["x", "y"]);
        let scene = scene(
            plane(0, 10),
            vec![layer(curve(), style(), vec![f64s(&[0.0, 0.0])])],
        );

        let drawn = drawn(&scene, &text);

        assert_eq!(measured(&drawn, &scene, &text), vec![vec![None, None]]);
    }

    #[test]
    fn a_measured_rectangle_holds_the_width_of_the_symbol() {
        let text = axis_text(&["length", "height"]);
        let scene = scene(
            metre_plane(0, 10),
            vec![layer(curve(), style(), vec![f64s(&[0.0, 0.0])])],
        );
        let drawn = drawn(&scene, &text);

        let units = measured(&drawn, &scene, &text);

        let cell = drawn.layout.metrics.cell.width();
        assert_eq!(units[0][0].map(|rect| rect.width), Some(cell));
    }
}
