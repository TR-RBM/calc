use calc_viz::{
    ColourMapping, Emphasis, FigureLayout, Frame, Layer, LayerPosition, Primitive, SamplingMethod,
    Scene, SceneError, View, layer_legend,
};

use crate::axis_units::{TextPlacing, TitlePlace, shows_only_figures};
use crate::canvas::Canvas;
use crate::colour::Colour;
use crate::depth::DepthBuffer;
use crate::figure_draw::same_key;
use crate::geometry::{PhysicalRect, ScaleFactor};
use crate::image::Image;
use crate::key_rows::{KeyEntry, draw_footer_row, draw_key_row, footer_entries};
use crate::layout::{
    AxisRange, LayoutError, LayoutRequest, PictureLayout, ViewKind, ViewRequest, plot_layout,
};
use crate::legend::{LegendRequest, draw_legend};
use crate::picture_text::PictureText;
use crate::plane::{PlaneView, draw_plane_axes, draw_plane_layer};
use crate::roles::Role;
use crate::space::{SpaceView, draw_space_axes, draw_space_layer};
use crate::text::font::FontSet;
use crate::text::grid::GridMetrics;
use crate::text::line::{TextDrawError, TextPainter};
use crate::text::placement::GridRowOrigin;
use crate::theme::Theme;

const KEY_ROLES: [Role; 8] = [
    Role::Missing,
    Role::Unresolved,
    Role::MayBeHit,
    Role::Marked,
    Role::Undecided,
    Role::Inside,
    Role::BackFace,
    Role::Provisional,
];

#[derive(Clone, Copy, Debug)]
pub struct RenderRequest<'a> {
    pub scene: &'a Scene,
    pub settled: Option<&'a [ViewRequest]>,
    pub frame: usize,
    pub width: u16,
    pub height: u16,
    pub text_size: u16,
    pub scale: ScaleFactor,
    pub theme: Theme,
    pub decimal_separator: char,
    pub text: &'a PictureText,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rendered {
    pub image: Image,
    pub layout: PictureLayout,
    pub figure_layouts: Vec<Option<FigureLayout>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextRow {
    FigureLabel { label: usize },
    ViewTitle { view: usize },
    Legend { layer: usize },
    AxisTitle { view: usize },
    Key,
    Footer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    Scene(SceneError),
    FrameOutOfRange { frame: usize, frames: usize },
    Layout(LayoutError),
    Text(TextDrawError),
    TextDoesNotFit(TextRow),
    ColourMapMissing(LayerPosition),
    ColourMapKindMismatch(LayerPosition),
    AxisNotMappable { view: usize, axis: usize },
}

pub(crate) struct Painter<'a> {
    pub canvas: &'a mut Canvas,
    pub fonts: &'a FontSet,
    pub metrics: GridMetrics,
    pub theme: Theme,
    pub scale: ScaleFactor,
    pub separator: char,
    pub bounds: PhysicalRect,
    pub shown: Vec<Role>,
    pub may_be_hit: Option<Colour>,
}

impl Painter<'_> {
    pub(crate) fn show(&mut self, role: Role) {
        if !self.shown.contains(&role) {
            self.shown.push(role);
        }
    }

    pub(crate) fn line_width(&self, logical: u16) -> f32 {
        f32::from(self.scale.to_line_width(logical).unwrap_or(u16::MAX))
    }

    pub(crate) fn physical(&self, logical: u16) -> u16 {
        self.scale.to_physical_size(logical).unwrap_or(u16::MAX)
    }

    pub(crate) fn placing(&self) -> TextPlacing {
        TextPlacing::new(self.metrics)
    }

    pub(crate) fn text_width(&self, text: &str) -> u32 {
        self.placing().width(text)
    }

    pub(crate) fn row_height(&self) -> u16 {
        self.metrics.cell.height()
    }

    pub(crate) fn text(
        &mut self,
        text: &str,
        x: u16,
        top: u16,
        colour: Colour,
    ) -> Result<(), RenderError> {
        let painter = TextPainter {
            fonts: self.fonts,
            metrics: self.metrics,
        };
        painter
            .draw_data(self.canvas, text, GridRowOrigin { x, top }, colour)
            .map_err(RenderError::Text)
    }

    pub(crate) fn draw_placed(
        &mut self,
        text: &str,
        place: TitlePlace,
        colour: Colour,
        slot: TextRow,
    ) -> Result<(), RenderError> {
        let (Some(row), Some(left)) = (place.row, place.left) else {
            if place.row.is_some() {
                return Err(RenderError::TextDoesNotFit(slot));
            }
            return Ok(());
        };
        if text.is_empty() {
            return Ok(());
        }
        self.text(text, left, row.y, colour)
    }
}

fn view_request(view: &View) -> ViewRequest {
    match view {
        View::View2(plane) => ViewRequest::plane(
            [&plane.x, &plane.y]
                .into_iter()
                .map(|axis| {
                    Some(AxisRange {
                        range: axis.range.clone(),
                        scale: axis.scale,
                    })
                })
                .collect(),
        ),
        View::View3(_) => ViewRequest::of_kind(ViewKind::Space),
    }
}

pub(crate) fn joined_title(title: Option<&String>, joiner: &str, symbol: &str) -> String {
    match (
        title.map(String::as_str).filter(|title| !title.is_empty()),
        symbol.is_empty(),
    ) {
        (Some(title), true) => title.to_owned(),
        (Some(title), false) if joiner.is_empty() => format!("{title} {symbol}"),
        (Some(title), false) => format!("{title} {joiner} {symbol}"),
        (None, _) => symbol.to_owned(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DrawnScene {
    pub layout: PictureLayout,
    pub figure_layouts: Vec<Option<FigureLayout>>,
    pub axis_units: Vec<Vec<Option<PhysicalRect>>>,
}

type DrawnViews = (Vec<Option<FigureLayout>>, Vec<Vec<Option<PhysicalRect>>>);

pub fn render_scene(request: &RenderRequest<'_>, fonts: &FontSet) -> Result<Rendered, RenderError> {
    let mut canvas =
        Canvas::new(request.width, request.height, request.theme.window).map_err(|_| {
            RenderError::Layout(LayoutError::TooSmall {
                width: request.width,
                height: request.height,
            })
        })?;
    let drawn = draw_scene(&mut canvas, (0, 0), request, fonts)?;
    Ok(Rendered {
        image: canvas.to_image(),
        layout: drawn.layout,
        figure_layouts: drawn.figure_layouts,
    })
}

pub fn draw_scene(
    canvas: &mut Canvas,
    origin: (u16, u16),
    request: &RenderRequest<'_>,
    fonts: &FontSet,
) -> Result<DrawnScene, RenderError> {
    let scene = request.scene;
    scene.check().map_err(RenderError::Scene)?;
    let frame = scene
        .frames
        .get(request.frame)
        .ok_or(RenderError::FrameOutOfRange {
            frame: request.frame,
            frames: scene.frames.len(),
        })?;
    let legend_layers = legend_layers(scene, frame, request.text);
    let mut layout = plot_layout(
        &LayoutRequest {
            width: request.width,
            height: request.height,
            text_size: request.text_size,
            scale: request.scale,
            views: match request.settled {
                Some(settled) => settled.to_vec(),
                None => scene.views.iter().map(view_request).collect(),
            },
            legends: legend_layers
                .iter()
                .map(|(_, request, _)| request.clone())
                .collect(),
        },
        fonts,
    )
    .map_err(RenderError::Layout)?;
    layout.move_to(origin);
    let bounds = PhysicalRect::new(origin.0, origin.1, request.width, request.height);
    canvas.paint_rect(bounds, &|_, _| request.theme.window);
    let mut painter = Painter {
        canvas,
        fonts,
        metrics: layout.metrics,
        theme: request.theme,
        scale: request.scale,
        separator: request.decimal_separator,
        bounds,
        shown: Vec::new(),
        may_be_hit: None,
    };
    let (figure_layouts, axis_units) = draw_views(&mut painter, request, &layout, frame)?;
    let mut key: Vec<KeyEntry> = Vec::new();
    for kind in figure_layouts
        .iter()
        .flatten()
        .flat_map(|figure| figure.key.iter().copied())
    {
        let is_named = key
            .iter()
            .any(|entry| matches!(entry, KeyEntry::Mark(known) if same_key(*known, kind)));
        if !is_named {
            key.push(KeyEntry::Mark(kind));
        }
    }
    let is_escape_time = matches!(scene.record.method, SamplingMethod::EscapeTime { .. });
    if frame
        .layers
        .iter()
        .any(|layer| layer.style.emphasis == Emphasis::Provisional)
    {
        painter.show(Role::Provisional);
    }
    for ((layer, legend, mapping), legend_layout) in legend_layers.iter().zip(&layout.legends) {
        let beyond = frame
            .layers
            .get(*layer)
            .map_or([false, false], |layer| values_beyond(layer, &mapping.range));
        draw_legend(
            &mut painter,
            legend_layout,
            (legend, mapping, beyond),
            *layer,
        )?;
    }
    for role in KEY_ROLES {
        let is_class = is_escape_time && matches!(role, Role::Inside | Role::Undecided);
        if painter.shown.contains(&role) && !is_class && !key.contains(&KeyEntry::Role(role)) {
            key.push(KeyEntry::Role(role));
        }
    }
    draw_key_row(&mut painter, layout.key_row, &key, request.text)?;
    let footer = footer_entries(frame, request.text, is_escape_time);
    draw_footer_row(&mut painter, layout.footer_row, &footer)?;
    Ok(DrawnScene {
        layout,
        figure_layouts,
        axis_units,
    })
}

fn values_beyond(layer: &Layer, range: &calc_viz::Interval) -> [bool; 2] {
    let scalar = match &layer.primitive {
        Primitive::ScalarGrid(grid) => Some(&grid.scalar),
        Primitive::TriangleMesh(mesh) => mesh.scalar.as_ref(),
        Primitive::Voxels(voxels) => voxels.scalar.as_ref(),
        Primitive::Points(points) => points.scalar.as_ref(),
        _ => None,
    };
    let Some(scalar) = scalar else {
        return [false, false];
    };
    let values: Vec<f64> = match scalar {
        calc_viz::Column::F32(values) => values.iter().map(|value| f64::from(*value)).collect(),
        calc_viz::Column::F64(values) => values.clone(),
    };
    let lower = range.lower.round_to_f64_ties_even();
    let upper = range.upper.round_to_f64_ties_even();
    let order = |value: f64, end: &calc_numbers::Number| {
        calc_numbers::Number::F64(value)
            .to_exact()
            .ok()
            .and_then(|value| crate::ticks::compare(&value, end))
    };
    let below = values.iter().any(|value| {
        value.is_finite()
            && *value <= lower
            && order(*value, &range.lower) == Some(std::cmp::Ordering::Less)
    });
    let above = values.iter().any(|value| {
        value.is_finite()
            && *value >= upper
            && order(*value, &range.upper) == Some(std::cmp::Ordering::Greater)
    });
    [below, above]
}

fn legend_layers(
    scene: &Scene,
    frame: &Frame,
    text: &PictureText,
) -> Vec<(usize, LegendRequest, ColourMapping)> {
    let mut legends = Vec::new();
    for view in 0..scene.views.len() {
        for (index, layer) in frame.layers.iter().enumerate() {
            if usize::try_from(layer.view.0).ok() != Some(view) {
                continue;
            }
            let (Some(legend), Some(mapping)) =
                (layer_legend(layer), layer.style.colour_map.as_ref())
            else {
                continue;
            };
            legends.push((
                index,
                LegendRequest {
                    legend,
                    title: text.legend_titles.get(index).cloned().unwrap_or_default(),
                    bar_words: [
                        text.domain_legend.argument.clone(),
                        text.domain_legend.modulus.clone(),
                    ],
                    class_names: [
                        text.escape_time.inside.clone(),
                        text.escape_time.undecided.clone(),
                    ],
                },
                mapping.clone(),
            ));
        }
    }
    legends
}

fn draw_views(
    painter: &mut Painter<'_>,
    request: &RenderRequest<'_>,
    layout: &PictureLayout,
    frame: &Frame,
) -> Result<DrawnViews, RenderError> {
    let scene = request.scene;
    let mut figure_layouts = vec![None; frame.layers.len()];
    let mut axis_units: Vec<Vec<Option<PhysicalRect>>> = Vec::with_capacity(scene.views.len());
    for (view_index, (view, view_layout)) in scene.views.iter().zip(&layout.views).enumerate() {
        let titles = request.text.axis_titles.get(view_index);
        let layers: Vec<(usize, &Layer)> = frame
            .layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| usize::try_from(layer.view.0).ok() == Some(view_index))
            .collect();
        let only_figures = shows_only_figures(&frame.layers, view_index);
        match view {
            View::View2(plane) => {
                let plane_view =
                    PlaneView::new(painter, plane, view_layout, layout.tick_length, view_index)?;
                for (layer_index, layer) in layers {
                    let position = LayerPosition {
                        frame: request.frame,
                        layer: layer_index,
                    };
                    let drawn =
                        draw_plane_layer(painter, &plane_view, layer, position, request.text)?;
                    if let Some(slot) = figure_layouts.get_mut(layer_index) {
                        *slot = drawn;
                    }
                }
                let units = if only_figures {
                    Vec::new()
                } else {
                    draw_plane_axes(painter, &plane_view, titles, &request.text.unit_joiner)?
                };
                axis_units.push(units);
            }
            View::View3(space) => {
                let space_view = SpaceView::new(painter, space, view_layout, view_index)?;
                let mut depth = DepthBuffer::new(view_layout.plot_area);
                for (layer_index, layer) in layers {
                    let position = LayerPosition {
                        frame: request.frame,
                        layer: layer_index,
                    };
                    draw_space_layer(painter, &space_view, &mut depth, layer, position)?;
                }
                axis_units.push(draw_space_axes(
                    painter,
                    &space_view,
                    titles,
                    &request.text.unit_joiner,
                )?);
            }
        }
    }
    Ok((figure_layouts, axis_units))
}

pub(crate) fn view_kind_mismatch(position: LayerPosition, axes: usize) -> RenderError {
    RenderError::Scene(SceneError::ViewKindMismatch {
        layer: position,
        axes,
    })
}

#[cfg(test)]
mod tests {
    use calc_numbers::Number;
    use calc_viz::{
        Arrows, Band, BitLayoutRequest, CellGrid, ColourMap, ColourMapping, ComplexGrid,
        EscapeTimeForm, FormulaRequest, Graph, IterationLimitRule, KindColour, Points, Polyline,
        PrecisionLimit, ResultId, SamplingDiagnostics, ScalarGrid, TriangleMesh, Voxels,
        bit_layout_scene, check_label_layout, domain_colour, formula_scene, sequential_colour,
    };

    use calc_viz::{StyleRole, View2};

    use super::*;
    use crate::roles;
    use crate::test_scenes::*;

    fn polyline(xs: &[f64], ys: &[f64]) -> Primitive {
        Primitive::Polyline(Polyline {
            coordinates: vec![f64s(xs), f64s(ys)],
        })
    }

    fn pixel_near(rendered: &Rendered, (x, y): (u16, u16), colour: [u8; 4]) -> bool {
        (y.saturating_sub(1)..=y + 1).any(|row| rendered.image.pixel(x, row) == Some(colour))
    }

    fn role_pixel(rendered: &Rendered, role: Role, x: u16, y: u16) -> crate::colour::Colour {
        let area = rendered.layout.views[0].plot_area;
        roles::Patterns::new(
            Theme::light(),
            ScaleFactor::from_percent(100).unwrap(),
            (area.x, area.y),
        )
        .covering(roles::Fill::Role(role), x, y)
    }

    fn kind() -> [u8; 4] {
        rgba(Theme::light().kinds.numeric)
    }

    fn grid(scalar: &[f64], classes: Option<Vec<u8>>) -> ScalarGrid {
        ScalarGrid {
            cells: CellGrid {
                region: vec![interval(0, 10), interval(0, 10)],
                counts: vec![2, 2],
            },
            scalar: f64s(scalar),
            classes,
        }
    }

    fn curve_scene() -> Scene {
        scene(
            plane(0, 10),
            vec![layer(
                polyline(&[0.0, 10.0], &[5.0, 5.0]),
                style(),
                vec![f64s(&[0.0, 0.0])],
            )],
        )
    }

    fn drawn_into(
        canvas: &mut Canvas,
        origin: (u16, u16),
        scene: &Scene,
        size: (u16, u16),
    ) -> DrawnScene {
        let fonts = FontSet::bundled().unwrap();
        draw_scene(
            canvas,
            origin,
            &RenderRequest {
                settled: None,
                scene,
                frame: 0,
                width: size.0,
                height: size.1,
                text_size: 14,
                scale: ScaleFactor::from_percent(100).unwrap(),
                theme: Theme::light(),
                decimal_separator: '.',
                text: &PictureText::default(),
            },
            &fonts,
        )
        .unwrap()
    }

    fn drawn_units(scene: &Scene, text: &PictureText) -> Vec<Vec<Option<PhysicalRect>>> {
        let fonts = FontSet::bundled().unwrap();
        let mut canvas = Canvas::new(WIDTH, HEIGHT, Theme::light().window).unwrap();
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
                scale: ScaleFactor::from_percent(100).unwrap(),
                theme: Theme::light(),
                decimal_separator: '.',
                text,
            },
            &fonts,
        )
        .unwrap()
        .axis_units
    }

    fn metre_axis_text() -> PictureText {
        PictureText {
            axis_titles: vec![vec![String::from("length"), String::from("height")]],
            unit_joiner: String::from("in"),
            ..PictureText::default()
        }
    }

    #[test]
    fn a_space_view_reports_a_rectangle_for_each_axis_with_a_unit() {
        let scene = scene_with(
            record(SamplingMethod::UniformGrid, SamplingDiagnostics::default()),
            metre_space(0, 1, 30, 20),
            Vec::new(),
        );
        let text = PictureText {
            axis_titles: vec![vec![
                String::from("length"),
                String::from("width"),
                String::from("height"),
            ]],
            unit_joiner: String::from("in"),
            ..PictureText::default()
        };

        let units = drawn_units(&scene, &text);

        let drawn: Vec<bool> = units[0].iter().map(Option::is_some).collect();
        assert_eq!(drawn, vec![true, true, true]);
    }

    #[test]
    fn an_axis_title_reports_the_rectangle_of_its_unit() {
        let scene = scene(
            metre_plane(0, 10),
            vec![layer(
                polyline(&[0.0, 10.0], &[5.0, 5.0]),
                style(),
                vec![f64s(&[0.0, 0.0])],
            )],
        );
        let text = metre_axis_text();

        let units = drawn_units(&scene, &text);

        let across = units[0][0].expect("the across axis has a unit");
        assert!(across.width > 0 && across.height > 0);
    }

    #[test]
    fn an_axis_without_a_unit_reports_no_rectangle() {
        let scene = scene(
            plane(0, 10),
            vec![layer(
                polyline(&[0.0, 10.0], &[5.0, 5.0]),
                style(),
                vec![f64s(&[0.0, 0.0])],
            )],
        );
        let text = metre_axis_text();

        let units = drawn_units(&scene, &text);

        assert_eq!(units[0], vec![None, None]);
    }

    #[test]
    fn a_unit_rectangle_sits_at_the_end_of_its_title() {
        let scene = scene(
            metre_plane(0, 10),
            vec![layer(
                polyline(&[0.0, 10.0], &[5.0, 5.0]),
                style(),
                vec![f64s(&[0.0, 0.0])],
            )],
        );
        let text = metre_axis_text();
        let fonts = FontSet::bundled().unwrap();
        let metrics = crate::text::grid::grid_metrics(&fonts, 14).unwrap();

        let units = drawn_units(&scene, &text);

        let across = units[0][0].expect("the across axis has a unit");
        assert_eq!(across.width, metrics.cell.width());
    }

    #[test]
    fn a_scene_drawn_at_an_offset_has_the_pixels_of_the_rendered_picture() {
        let scene = curve_scene();
        let rendered = render_sized(&scene, 320, 240, &PictureText::default()).unwrap();
        let mut canvas = Canvas::new(400, 300, Theme::light().window).unwrap();

        drawn_into(&mut canvas, (40, 30), &scene, (320, 240));

        let same = (0..240).all(|row| {
            (0..320).all(|column| {
                canvas.pixel(column + 40, row + 30).map(rgba) == rendered.image.pixel(column, row)
            })
        });
        assert!(same);
    }

    #[test]
    fn a_scene_drawn_at_an_offset_lays_out_its_plot_area_at_that_offset() {
        let scene = curve_scene();
        let rendered = render_sized(&scene, 320, 240, &PictureText::default()).unwrap();
        let mut canvas = Canvas::new(400, 300, Theme::light().window).unwrap();

        let drawn = drawn_into(&mut canvas, (40, 30), &scene, (320, 240));

        let area = drawn.layout.views[0].plot_area;
        let rendered_area = rendered.layout.views[0].plot_area;
        assert_eq!(
            (area.x, area.y, area.width, area.height),
            (
                rendered_area.x + 40,
                rendered_area.y + 30,
                rendered_area.width,
                rendered_area.height
            )
        );
    }

    #[test]
    fn a_scene_drawn_at_an_offset_leaves_the_canvas_around_it_alone() {
        let scene = curve_scene();
        let mut canvas = Canvas::new(400, 300, Colour::opaque(1, 2, 3)).unwrap();

        drawn_into(&mut canvas, (40, 30), &scene, (320, 240));

        assert_eq!(
            (canvas.pixel(39, 29), canvas.pixel(360, 270)),
            (Some(Colour::opaque(1, 2, 3)), Some(Colour::opaque(1, 2, 3)))
        );
    }

    #[test]
    fn horizontal_polyline_is_drawn_in_its_kind_colour() {
        let scene = scene(
            plane(0, 10),
            vec![layer(
                polyline(&[0.0, 10.0], &[5.0, 5.0]),
                style(),
                vec![f64s(&[0.0, 0.0])],
            )],
        );

        let rendered = render(&scene);

        assert!(pixel_near(
            &rendered,
            view_pixel(&rendered, 5.0, 5.0, 0.0, 10.0),
            kind()
        ));
    }

    #[test]
    fn nan_coordinate_breaks_the_polyline() {
        let scene = scene(
            plane(0, 10),
            vec![layer(
                polyline(&[0.0, 4.0, 6.0, 10.0], &[5.0, 5.0, f64::NAN, 5.0]),
                style(),
                vec![f64s(&[0.0; 4])],
            )],
        );

        let rendered = render(&scene);

        assert!(!pixel_near(
            &rendered,
            view_pixel(&rendered, 7.0, 5.0, 0.0, 10.0),
            kind()
        ));
    }

    #[test]
    fn unresolved_samples_are_drawn_as_their_enclosure_instead_of_a_line() {
        let scene = scene(
            plane(0, 10),
            vec![layer(
                polyline(&[0.0, 5.0, 10.0], &[5.0, 5.0, 5.0]),
                style(),
                vec![f64s(&[1.0, 1.0, 1.0])],
            )],
        );

        let rendered = render(&scene);
        let (x, y) = view_pixel(&rendered, 5.0, 5.5, 0.0, 10.0);
        let block: Vec<Option<[u8; 4]>> = (x - 2..=x + 2)
            .flat_map(|column| (y - 2..=y + 2).map(move |row| (column, row)))
            .map(|(column, row)| rendered.image.pixel(column, row))
            .collect();

        assert_eq!(
            (
                block.contains(&Some(rgba(Theme::light().text_secondary))),
                block.contains(&Some(kind()))
            ),
            (true, false)
        );
    }

    #[test]
    fn band_fills_between_its_lower_and_upper_values() {
        let band = Primitive::Band(Band {
            abscissa: f64s(&[0.0, 10.0]),
            lower: f64s(&[2.0, 2.0]),
            upper: f64s(&[8.0, 8.0]),
        });
        let scene = scene(
            plane(0, 10),
            vec![layer(band, style(), vec![f64s(&[0.0; 2]), f64s(&[0.0; 2])])],
        );

        let rendered = render(&scene);
        let inside = view_pixel(&rendered, 5.0, 5.0, 0.0, 10.0);
        let outside = view_pixel(&rendered, 5.0, 9.0, 0.0, 10.0);

        assert_eq!(
            (
                rendered.image.pixel(inside.0, inside.1) == Some(window()),
                rendered.image.pixel(outside.0, outside.1)
            ),
            (false, Some(window()))
        );
    }

    #[test]
    fn point_is_drawn_as_a_marker() {
        let points = Primitive::Points(Points {
            coordinates: vec![f64s(&[5.0]), f64s(&[5.0])],
            scalar: None,
        });
        let scene = scene(
            plane(0, 10),
            vec![layer(points, style(), vec![f64s(&[0.0])])],
        );

        let rendered = render(&scene);
        let (x, y) = view_pixel(&rendered, 5.0, 5.0, 0.0, 10.0);

        assert_ne!(rendered.image.pixel(x, y), Some(window()));
    }

    fn with_columns(mut layer: Layer, columns: calc_viz::ColumnEnclosures) -> Layer {
        layer.columns = Some(columns);
        layer
    }

    fn two_columns(
        lower: [f64; 2],
        upper: [f64; 2],
        marked: [bool; 2],
    ) -> calc_viz::ColumnEnclosures {
        calc_viz::ColumnEnclosures {
            lower: lower.to_vec(),
            upper: upper.to_vec(),
            hit_lower: vec![f64::NAN, f64::NAN],
            hit_upper: vec![f64::NAN, f64::NAN],
            marked: marked.to_vec(),
            varies_below_bounds: false,
        }
    }

    #[test]
    fn a_column_resolved_to_a_pixel_is_drawn_as_the_curve() {
        let scene = scene(
            plane(0, 10),
            vec![with_columns(
                layer(
                    polyline(&[0.0, 5.0, 10.0], &[5.0, 5.0, 5.0]),
                    style(),
                    vec![f64s(&[0.0, 0.0, 0.0])],
                ),
                two_columns([5.0, 5.0], [5.0, 5.0], [false, false]),
            )],
        );

        let rendered = render(&scene);

        assert!(pixel_near(
            &rendered,
            view_pixel(&rendered, 2.5, 5.0, 0.0, 10.0),
            kind()
        ));
    }

    #[test]
    fn a_column_that_may_be_hit_is_filled_faintly_and_not_drawn_as_the_curve() {
        let scene = scene(
            plane(0, 10),
            vec![with_columns(
                layer(
                    polyline(&[0.0, 5.0, 10.0], &[5.0, 5.0, 5.0]),
                    style(),
                    vec![f64s(&[0.0, 0.0, 0.0])],
                ),
                two_columns([1.0, 1.0], [9.0, 9.0], [false, false]),
            )],
        );

        let rendered = render(&scene);
        let (x, y) = view_pixel(&rendered, 2.5, 8.0, 0.0, 10.0);
        let faint = rgba(roles::over_window(
            &Theme::light(),
            roles::band_fill(Theme::light().kinds.numeric),
        ));

        assert_eq!(rendered.image.pixel(x, y), Some(faint));
        assert!(!pixel_near(
            &rendered,
            view_pixel(&rendered, 2.5, 5.0, 0.0, 10.0),
            kind()
        ));
    }

    #[test]
    fn a_marked_column_is_hatched_and_not_drawn_as_the_curve() {
        let scene = scene(
            plane(0, 10),
            vec![with_columns(
                layer(
                    polyline(&[0.0, 5.0, 10.0], &[5.0, 5.0, 5.0]),
                    style(),
                    vec![f64s(&[3.0, 3.0, 3.0])],
                ),
                two_columns([2.0, 2.0], [8.0, 8.0], [true, true]),
            )],
        );

        let rendered = render(&scene);
        let (x, y) = view_pixel(&rendered, 2.5, 5.0, 0.0, 10.0);
        let block: Vec<Option<[u8; 4]>> = (x - 3..=x + 3)
            .flat_map(|column| (y - 3..=y + 3).map(move |row| (column, row)))
            .map(|(column, row)| rendered.image.pixel(column, row))
            .collect();

        assert_eq!(
            (
                block.contains(&Some(rgba(Theme::light().text_secondary))),
                block.contains(&Some(kind()))
            ),
            (true, false)
        );
    }

    #[test]
    fn points_at_the_grid_limit_are_drawn_unresolved() {
        let points = Primitive::Points(Points {
            coordinates: vec![f64s(&[5.0]), f64s(&[5.0])],
            scalar: None,
        });
        let mut limited = layer(points, style(), vec![f64s(&[0.0])]);
        limited.precision = Some(PrecisionLimit {
            grid_exhausted: vec![0],
            ..PrecisionLimit::default()
        });
        let scene = scene(plane(0, 10), vec![limited]);

        let rendered = render(&scene);
        let (x, y) = view_pixel(&rendered, 5.0, 5.0, 0.0, 10.0);

        assert!((x - 2..=x + 2).any(|column| (y - 2..=y + 2).any(|row| {
            rendered.image.pixel(column, row) == Some(rgba(Theme::light().text_secondary))
        })));
    }

    #[test]
    fn arrow_is_drawn_from_its_base_along_its_components() {
        let arrows = Primitive::Arrows(Arrows {
            bases: vec![f64s(&[2.0]), f64s(&[5.0])],
            components: vec![f64s(&[6.0]), f64s(&[0.0])],
        });
        let scene = scene(
            plane(0, 10),
            vec![layer(arrows, style(), vec![f64s(&[0.0]), f64s(&[0.0])])],
        );

        let rendered = render(&scene);
        let (x, y) = view_pixel(&rendered, 5.0, 5.0, 0.0, 10.0);

        assert!(
            (y.saturating_sub(1)..=y + 1).any(|row| rendered.image.pixel(x, row) != Some(window()))
        );
    }

    #[test]
    fn scalar_grid_cell_takes_its_colour_map_colour() {
        let scene = scene(
            plane(0, 10),
            vec![layer(
                Primitive::ScalarGrid(grid(&[0.0, 1.0, 2.0, 3.0], None)),
                sequential(0, 3),
                vec![f64s(&[0.0; 4])],
            )],
        );

        let rendered = render(&scene);
        let (x, y) = view_pixel(&rendered, 7.5, 7.5, 0.0, 10.0);
        let expected = roles::from_map(sequential_colour(&interval(0, 3), 3.0).unwrap());

        assert_eq!(rendered.image.pixel(x, y), Some(rgba(expected)));
    }

    #[test]
    fn nan_scalar_cell_is_drawn_as_missing() {
        let scene = scene(
            plane(0, 10),
            vec![layer(
                Primitive::ScalarGrid(grid(&[f64::NAN, 1.0, 2.0, 3.0], None)),
                sequential(0, 3),
                vec![f64s(&[0.0; 4])],
            )],
        );

        let rendered = render(&scene);
        let (x, y) = view_pixel(&rendered, 2.5, 2.5, 0.0, 10.0);

        assert_eq!(
            rendered.image.pixel(x, y),
            Some(rgba(role_pixel(&rendered, Role::Missing, x, y)))
        );
    }

    fn escape_time_scene() -> Scene {
        let diagnostics = SamplingDiagnostics {
            escaped_cells: 2,
            inside_cells: 1,
            undecided_cells: 1,
            ..SamplingDiagnostics::default()
        };
        let method = SamplingMethod::EscapeTime {
            form: EscapeTimeForm::QuadraticParameter,
            limit_rule: IterationLimitRule::Fixed { iterations: 16 },
            iterations_used: 16,
        };
        scene_with(
            record(method, diagnostics),
            plane(0, 10),
            vec![layer(
                Primitive::ScalarGrid(grid(
                    &[1.0, f64::NAN, f64::NAN, 2.0],
                    Some(vec![0, 1, 2, 0]),
                )),
                sequential(0, 16),
                vec![f64s(&[f64::NAN; 4])],
            )],
        )
    }

    #[test]
    fn inside_cell_is_drawn_in_the_inside_role() {
        let rendered = render(&escape_time_scene());
        let (x, y) = view_pixel(&rendered, 7.5, 2.5, 0.0, 10.0);

        assert_eq!(rendered.image.pixel(x, y), Some(rgba(Theme::light().text)));
    }

    #[test]
    fn undecided_cell_is_drawn_in_the_undecided_role() {
        let rendered = render(&escape_time_scene());
        let (x, y) = view_pixel(&rendered, 2.5, 7.5, 0.0, 10.0);

        assert_eq!(
            rendered.image.pixel(x, y),
            Some(rgba(role_pixel(&rendered, Role::Undecided, x, y)))
        );
    }

    #[test]
    fn inside_region_is_edged_with_a_window_line() {
        let rendered = render(&escape_time_scene());
        let area = rendered.layout.views[0].plot_area;
        let (x, y) = view_pixel(&rendered, 7.5, 2.5, 0.0, 10.0);
        let edge = crate::mapping::pixel_round(f64::from(area.x) + f64::from(area.width) / 2.0);

        assert_eq!(
            (rendered.image.pixel(edge, y), rendered.image.pixel(x, y)),
            (Some(window()), Some(rgba(Theme::light().text)))
        );
    }

    #[test]
    fn missing_cell_puts_its_role_in_the_key_row() {
        let scene = scene(
            plane(0, 10),
            vec![layer(
                Primitive::ScalarGrid(grid(&[f64::NAN, 1.0, 2.0, 3.0], None)),
                sequential(0, 3),
                vec![f64s(&[0.0; 4])],
            )],
        );

        let rendered = render(&scene);
        let row = rendered.layout.key_row;

        assert!((row.x..row.x + row.height).any(|x| {
            (row.y..row.bottom())
                .any(|y| rendered.image.pixel(x, y) == Some(rgba(Theme::light().text_secondary)))
        }));
    }

    fn key_shows_missing(rendered: &Rendered) -> bool {
        let row = rendered.layout.key_row;
        (row.x..row.x + row.height).any(|x| {
            (row.y..row.bottom())
                .any(|y| rendered.image.pixel(x, y) == Some(rgba(Theme::light().text_secondary)))
        })
    }

    #[test]
    fn plane_triangle_removed_for_a_missing_vertex_puts_the_missing_role_in_the_key() {
        let mesh = Primitive::TriangleMesh(TriangleMesh {
            vertices: vec![f64s(&[1.0, 9.0, f64::NAN]), f64s(&[1.0, 1.0, 9.0])],
            triangles: vec![[0, 1, 2]],
            scalar: None,
        });
        let scene = scene(
            plane(0, 10),
            vec![layer(mesh, style(), vec![f64s(&[0.0; 3])])],
        );

        let rendered = render(&scene);

        assert!(key_shows_missing(&rendered));
    }

    #[test]
    fn space_triangle_removed_for_a_missing_vertex_puts_the_missing_role_in_the_key() {
        let mesh = Primitive::TriangleMesh(TriangleMesh {
            vertices: vec![
                f64s(&[-1.0, 1.0, 0.0]),
                f64s(&[-1.0, -1.0, 1.0]),
                f64s(&[0.0, f64::INFINITY, 0.0]),
            ],
            triangles: vec![[0, 1, 2]],
            scalar: None,
        });
        let scene = scene(
            space(-1, 1, 30, 40),
            vec![layer(mesh, style(), vec![f64s(&[0.0; 3])])],
        );

        let rendered = render(&scene);

        assert!(key_shows_missing(&rendered));
    }

    #[test]
    fn complete_triangle_leaves_the_missing_role_out_of_the_key() {
        let rendered = render(&flat_triangle([0, 1, 2]));

        assert!(!key_shows_missing(&rendered));
    }

    #[test]
    fn complex_grid_cell_takes_its_domain_colour() {
        let complex = Primitive::ComplexGrid(ComplexGrid {
            cells: CellGrid {
                region: vec![interval(0, 10), interval(0, 10)],
                counts: vec![1, 1],
            },
            real: f64s(&[1.0]),
            imaginary: f64s(&[1.0]),
        });
        let domain = StyleRole {
            colour_map: Some(ColourMapping {
                map: ColourMap::DomainColouring,
                range: interval(0, 1),
            }),
            ..style()
        };
        let scene = scene(
            plane(0, 10),
            vec![layer(complex, domain, vec![f64s(&[0.0]), f64s(&[0.0])])],
        );

        let rendered = render_sized(&scene, 640, 480, &PictureText::default()).unwrap();
        let (x, y) = view_pixel(&rendered, 5.0, 5.0, 0.0, 10.0);
        let expected = roles::from_map(domain_colour(&interval(0, 1), 1.0, 1.0).unwrap());

        assert_eq!(rendered.image.pixel(x, y), Some(rgba(expected)));
    }

    #[test]
    fn plane_triangle_is_filled_in_its_kind_colour() {
        let mesh = Primitive::TriangleMesh(TriangleMesh {
            vertices: vec![f64s(&[1.0, 9.0, 5.0]), f64s(&[1.0, 1.0, 9.0])],
            triangles: vec![[0, 1, 2]],
            scalar: None,
        });
        let scene = scene(
            plane(0, 10),
            vec![layer(mesh, style(), vec![f64s(&[0.0; 3])])],
        );

        let rendered = render(&scene);
        let (x, y) = view_pixel(&rendered, 5.0, 4.0, 0.0, 10.0);

        assert_eq!(rendered.image.pixel(x, y), Some(kind()));
    }

    fn flat_triangle(order: [u32; 3]) -> Scene {
        let mesh = Primitive::TriangleMesh(TriangleMesh {
            vertices: vec![
                f64s(&[-1.0, 1.0, 0.0]),
                f64s(&[-1.0, -1.0, 1.0]),
                f64s(&[0.0, 0.0, 0.0]),
            ],
            triangles: vec![order],
            scalar: None,
        });
        scene(
            space(-1, 1, 0, 90),
            vec![layer(mesh, style(), vec![f64s(&[0.0; 3])])],
        )
    }

    fn triangle_centre(rendered: &Rendered) -> (u16, u16) {
        let area = rendered.layout.views[0].plot_area;
        (area.x + area.width / 2, area.y + area.height / 2)
    }

    #[test]
    fn space_triangle_turned_towards_the_eye_is_drawn_in_its_kind_colour() {
        let rendered = render(&flat_triangle([0, 1, 2]));
        let (x, y) = triangle_centre(&rendered);

        assert_eq!(rendered.image.pixel(x, y), Some(kind()));
    }

    #[test]
    fn back_face_of_a_space_triangle_is_drawn_in_the_back_face_colour() {
        let rendered = render(&flat_triangle([0, 2, 1]));
        let (x, y) = triangle_centre(&rendered);
        let theme = Theme::light();

        assert_eq!(rendered.image.pixel(x, y), Some(rgba(theme.text_tertiary)));
    }

    #[test]
    fn nearer_voxel_covers_the_farther_one() {
        let voxels = Primitive::Voxels(Voxels {
            occupied: vec![[0, 0, 0], [0, 0, 1]],
            scalar: Some(f64s(&[1.0, 2.0])),
        });
        let View::View3(mut view) = space(-1, 1, 0, 90) else {
            unreachable!()
        };
        view.z = axis(-1, 2);
        let scene = scene(
            View::View3(view),
            vec![layer(voxels, sequential(0, 3), vec![f64s(&[0.0, 0.0])])],
        );

        let rendered = render(&scene);
        let (x, y) = triangle_centre(&rendered);
        let nearer = roles::from_map(sequential_colour(&interval(0, 3), 2.0).unwrap());

        assert_eq!(rendered.image.pixel(x, y), Some(rgba(nearer)));
    }

    #[test]
    fn graph_node_is_drawn_at_its_position() {
        let graph = Primitive::Graph(Graph {
            positions: vec![f64s(&[2.0, 8.0]), f64s(&[5.0, 5.0])],
            edges: vec![[0, 1]],
            is_directed: true,
        });
        let scene = scene(plane(0, 10), vec![layer(graph, style(), Vec::new())]);

        let rendered = render(&scene);
        let (x, y) = view_pixel(&rendered, 2.0, 5.0, 0.0, 10.0);

        assert_eq!(rendered.image.pixel(x, y), Some(kind()));
    }

    #[test]
    fn formula_text_is_drawn_at_its_anchor() {
        let scene = formula_scene(&FormulaRequest {
            result: ResultId(1),
            text: String::from("x^2"),
            anchor: [Number::from(0_i64), Number::from(0_i64)],
            view_x: interval(-1, 1),
            view_y: interval(-1, 1),
            kind: KindColour::Symbolic,
            references: Vec::new(),
        })
        .unwrap();

        let rendered = render(&scene);
        let area = rendered.layout.views[0].plot_area;
        let (left, middle) = (area.x + area.width / 2, area.y + area.height / 2);
        let row = rendered.layout.metrics.cell.height() / 2;

        assert!((left..left + 3 * 9).any(|x| {
            (middle - row..middle + row).any(|y| rendered.image.pixel(x, y) != Some(window()))
        }));
    }

    #[test]
    fn bit_layout_narrower_than_its_digits_draws_its_cells() {
        let scene = bit_layout_scene(&BitLayoutRequest {
            result: ResultId(1),
            expression_text: String::from("1.5"),
            value: Number::F64(1.5),
            kind: KindColour::Numeric,
            references: Vec::new(),
        })
        .unwrap();

        let result = render_sized(&scene, 400, 300, &PictureText::default());

        assert!(result.is_ok());
    }

    #[test]
    fn provisional_layer_is_drawn_halfway_to_the_window() {
        let provisional = StyleRole {
            emphasis: calc_viz::Emphasis::Provisional,
            ..style()
        };
        let scene = scene(
            plane(0, 10),
            vec![layer(
                polyline(&[0.0, 10.0], &[5.0, 5.0]),
                provisional,
                Vec::new(),
            )],
        );

        let rendered = render(&scene);
        let theme = Theme::light();
        let expected = rgba(roles::provisional(&theme, theme.kinds.numeric));

        assert!(pixel_near(
            &rendered,
            view_pixel(&rendered, 5.0, 5.0, 0.0, 10.0),
            expected
        ));
    }

    fn right_triangle() -> calc_viz::Figure {
        use calc_viz::{
            Angle, AngleIndex, ElementIndex, Figure, FigureScale, Label, Mark, MarkKind, NameKind,
            PointIndex, Quantity, RightAngleMark, Segment, SegmentIndex, Stroke,
        };
        let segment = |from, to| Segment {
            from: PointIndex(from),
            to: PointIndex(to),
            stroke: Stroke::Solid,
        };
        let vertex = |point, name: &str| Label {
            element: ElementIndex::Point(PointIndex(point)),
            names: NameKind::Vertex,
            name: String::from(name),
            quantity: Quantity::Named,
            designation: None,
        };
        Figure {
            points: [f64s(&[0.0, 4.0, 0.0]), f64s(&[0.0, 0.0, 3.0])],
            segments: vec![segment(0, 1), segment(1, 2), segment(2, 0)],
            angles: vec![Angle {
                vertex: PointIndex(0),
                first_arm: PointIndex(1),
                second_arm: PointIndex(2),
                is_oriented: false,
            }],
            marks: vec![Mark {
                element: ElementIndex::Angle(AngleIndex(0)),
                kind: MarkKind::RightAngle(RightAngleMark::Square),
            }],
            labels: vec![
                vertex(0, "C"),
                vertex(1, "A"),
                vertex(2, "B"),
                Label {
                    element: ElementIndex::Segment(SegmentIndex(1)),
                    names: NameKind::Side,
                    name: String::from("c"),
                    quantity: Quantity::Sought { step: None },
                    designation: None,
                },
            ],
            scale: FigureScale::Sketch,
        }
    }

    #[test]
    fn figure_layout_of_a_right_triangle_has_no_layout_fault() {
        let figure = right_triangle();
        let scene = scene(
            plane(-2, 6),
            vec![layer(
                Primitive::Figure(Box::new(figure.clone())),
                style(),
                Vec::new(),
            )],
        );

        let rendered = render(&scene);
        let layout = rendered.figure_layouts[0].as_ref().unwrap();

        assert_eq!(check_label_layout(&figure, layout), Vec::new());
    }

    #[test]
    fn figure_layout_reports_every_placed_label() {
        let figure = right_triangle();
        let scene = scene(
            plane(-2, 6),
            vec![layer(
                Primitive::Figure(Box::new(figure)),
                style(),
                Vec::new(),
            )],
        );

        let rendered = render(&scene);

        assert_eq!(rendered.figure_layouts[0].as_ref().unwrap().labels.len(), 4);
    }

    #[test]
    fn precision_notice_is_written_in_the_footer_row() {
        let mut limited = layer(
            polyline(&[0.0, 10.0], &[5.0, 5.0]),
            style(),
            vec![f64s(&[0.0, 0.0])],
        );
        limited.precision = Some(PrecisionLimit {
            unresolved_samples: 1,
            ..PrecisionLimit::default()
        });
        let scene = scene(plane(0, 10), vec![limited]);
        let mut text = PictureText::default();
        text.precision.value_limit = String::from("limit");

        let rendered = render_sized(&scene, WIDTH, HEIGHT, &text).unwrap();
        let row = rendered.layout.footer_row;

        assert!(
            (row.x..row.x + 45).any(
                |x| (row.y..row.bottom()).any(|y| rendered.image.pixel(x, y) != Some(window()))
            )
        );
    }

    #[test]
    fn sequential_legend_bar_starts_with_the_colour_of_its_lowest_swatch() {
        let scene = scene(
            plane(0, 10),
            vec![layer(
                Primitive::ScalarGrid(grid(&[0.0, 1.0, 2.0, 3.0], None)),
                sequential(0, 16),
                vec![f64s(&[0.0; 4])],
            )],
        );

        let rendered = render_sized(&scene, 640, 480, &PictureText::default()).unwrap();
        let row = rendered.layout.legends[0].rows[0];
        let sizes = crate::legend::sizes(rendered.layout.metrics);
        let first_swatch = row.x + u16::try_from(sizes.reserve + sizes.swatch / 2).unwrap();
        let expected = roles::from_map(sequential_colour(&interval(0, 16), 0.5).unwrap());

        assert_eq!(
            rendered.image.pixel(first_swatch, row.y + 1),
            Some(rgba(expected))
        );
    }

    #[test]
    fn escape_time_classes_stand_in_the_legend_row_not_the_key_row() {
        let rendered = render(&escape_time_scene());
        let key = rendered.layout.key_row;
        let text = rgba(Theme::light().text);

        let key_has_inside_swatch = (key.x..key.right())
            .any(|x| (key.y..key.bottom()).any(|y| rendered.image.pixel(x, y) == Some(text)));

        assert_eq!(
            (key_has_inside_swatch, rendered.layout.legends.len()),
            (false, 1)
        );
    }

    #[test]
    fn legend_wider_than_its_row_is_rejected() {
        let scene = scene(
            plane(0, 10),
            vec![layer(
                Primitive::ScalarGrid(grid(&[0.0, 1.0, 2.0, 3.0], None)),
                sequential(0, 3),
                vec![f64s(&[0.0; 4])],
            )],
        );
        let text = PictureText {
            legend_titles: vec!["value ".repeat(10)],
            ..PictureText::default()
        };

        assert_eq!(
            render_sized(&scene, WIDTH, HEIGHT, &text),
            Err(RenderError::TextDoesNotFit(TextRow::Legend { layer: 0 }))
        );
    }

    #[test]
    fn frame_beyond_the_scene_is_rejected() {
        let scene = scene(plane(0, 10), Vec::new());
        let fonts = FontSet::bundled().unwrap();
        let text = PictureText::default();
        let request = RenderRequest {
            scene: &scene,
            settled: None,
            frame: 1,
            width: WIDTH,
            height: HEIGHT,
            text_size: 14,
            scale: ScaleFactor::from_percent(100).unwrap(),
            theme: Theme::light(),
            decimal_separator: '.',
            text: &text,
        };

        assert_eq!(
            render_scene(&request, &fonts),
            Err(RenderError::FrameOutOfRange {
                frame: 1,
                frames: 1
            })
        );
    }

    #[test]
    fn scene_that_fails_its_check_is_rejected() {
        let mut scene = scene(plane(0, 10), Vec::new());
        scene.format_version = 1;

        assert_eq!(
            render_sized(&scene, WIDTH, HEIGHT, &PictureText::default()),
            Err(RenderError::Scene(SceneError::UnknownFormatVersion(1)))
        );
    }

    #[test]
    fn picture_too_small_for_its_layout_is_rejected() {
        let scene = scene(plane(0, 10), Vec::new());

        assert_eq!(
            render_sized(&scene, 40, 40, &PictureText::default()),
            Err(RenderError::Layout(LayoutError::TooSmall {
                width: 40,
                height: 40
            }))
        );
    }

    #[test]
    fn footer_text_wider_than_its_row_is_rejected() {
        let mut limited = layer(
            polyline(&[0.0, 10.0], &[5.0, 5.0]),
            style(),
            vec![f64s(&[0.0, 0.0])],
        );
        limited.precision = Some(PrecisionLimit {
            unresolved_samples: 1,
            ..PrecisionLimit::default()
        });
        let scene = scene(plane(0, 10), vec![limited]);
        let mut text = PictureText::default();
        text.precision.value_limit = "limit ".repeat(20);

        assert_eq!(
            render_sized(&scene, WIDTH, HEIGHT, &text),
            Err(RenderError::TextDoesNotFit(TextRow::Footer))
        );
    }

    #[test]
    fn text_beyond_the_glyph_position_range_is_a_text_error() {
        let scene = scene(plane(0, 10), Vec::new());

        let result = render_sized(&scene, 40_000, 200, &PictureText::default());

        assert!(matches!(result, Err(RenderError::Text(_))));
    }

    #[test]
    fn scalar_grid_without_a_colour_map_is_rejected() {
        let scene = scene(
            plane(0, 10),
            vec![layer(
                Primitive::ScalarGrid(grid(&[0.0; 4], None)),
                style(),
                vec![f64s(&[0.0; 4])],
            )],
        );

        assert_eq!(
            render_sized(&scene, WIDTH, HEIGHT, &PictureText::default()),
            Err(RenderError::ColourMapMissing(LayerPosition {
                frame: 0,
                layer: 0
            }))
        );
    }

    #[test]
    fn scalar_grid_with_domain_colouring_is_rejected() {
        let domain = StyleRole {
            colour_map: Some(ColourMapping {
                map: ColourMap::DomainColouring,
                range: interval(0, 1),
            }),
            ..style()
        };
        let scene = scene(
            plane(0, 10),
            vec![layer(
                Primitive::ScalarGrid(grid(&[0.0; 4], None)),
                domain,
                vec![f64s(&[0.0; 4])],
            )],
        );

        assert_eq!(
            render_sized(&scene, WIDTH, HEIGHT, &PictureText::default()),
            Err(RenderError::ColourMapKindMismatch(LayerPosition {
                frame: 0,
                layer: 0
            }))
        );
    }

    #[test]
    fn logarithmic_axis_from_zero_cannot_be_mapped() {
        let mut view = View2 {
            x: axis(0, 10),
            y: axis(0, 10),
        };
        view.x.scale = calc_viz::Scale::Logarithmic;
        let scene = scene(View::View2(Box::new(view)), Vec::new());

        assert_eq!(
            render_sized(&scene, WIDTH, HEIGHT, &PictureText::default()),
            Err(RenderError::AxisNotMappable { view: 0, axis: 0 })
        );
    }
}
