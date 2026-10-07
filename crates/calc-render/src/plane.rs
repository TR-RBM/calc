use calc_numbers::Number;
use calc_viz::{
    Arrows, Band, BitLayout, ColourMap, ColourMapping, Column, ColumnEnclosures, ComplexGrid,
    FigureLayout, Formula, Graph, INSIDE_CELL, Interval, Layer, LayerPosition, LinePattern, Marker,
    Points, Polyline, Primitive, ScalarGrid, Scale, StyleRole, TriangleMesh, UNDECIDED_CELL, View2,
    ViewAxis, categorical_colour, diverging_colour, domain_colour, sequential_colour,
};
use tiny_skia::{Mask, PathBuilder};

use crate::arc::CircularArc;
use crate::axis_units::plane_title_places;
use crate::clip::{Guard, polygon_path, polyline_path};
use crate::colour::Colour;
use crate::depth::DepthBuffer;
use crate::draw::{DrawCommand, DrawList};
use crate::figure_draw::draw_figure;
use crate::geometry::PhysicalRect;
use crate::layout::{LayoutError, ViewLayout};
use crate::mapping::{AxisMap, ScreenPoint, column_value, pixel_floor, pixel_round, to_f32};
use crate::picture_text::PictureText;
use crate::render::{Painter, RenderError, TextRow, joined_title, view_kind_mismatch};
use crate::roles::{self, Fill, Patterns, Role};
use crate::ticks::{Tick, TickRoom, linear_ticks, logarithmic_ticks};

const PLANE_AXES: usize = 2;
const CURVE_WIDTH_LOGICAL: u16 = 2;
const THIN_WIDTH_LOGICAL: u16 = 1;
const MARKER_SIZE_LOGICAL: u16 = 5;
const ROLE_MARKER_SIZE_LOGICAL: u16 = 9;
const STIPPLE_PITCH_LOGICAL: u16 = 3;
const NODE_SIZE_LOGICAL: u16 = 6;
const ARROW_HEAD_LOGICAL: u16 = 6;
const DASH_LOGICAL: u16 = 6;
const DASH_GAP_LOGICAL: u16 = 4;
const COLOUR_MAP_STEPS: f64 = 256.0;
const FULL_TURN: f64 = std::f64::consts::TAU;
pub(crate) const TICK_LABEL_SPACING_CELLS: u32 = 2;
pub(crate) const Y_TICK_SPACING_ROWS: u32 = 3;
const BIT_FIELD_GAPS: usize = 2;

pub(crate) struct PlaneView<'a> {
    pub view: &'a View2,
    pub index: usize,
    pub layout: ViewLayout,
    pub tick_length: u16,
    pub x: AxisMap,
    pub y: AxisMap,
    pub clip: Option<Mask>,
    pub guard: Guard,
    pub patterns: Patterns,
}

impl<'a> PlaneView<'a> {
    pub(crate) fn new(
        painter: &Painter<'_>,
        view: &'a View2,
        layout: &ViewLayout,
        tick_length: u16,
        index: usize,
    ) -> Result<PlaneView<'a>, RenderError> {
        let area = layout.plot_area;
        let x = AxisMap::new(&view.x, f64::from(area.x), f64::from(area.width), false).ok_or(
            RenderError::AxisNotMappable {
                view: index,
                axis: 0,
            },
        )?;
        let y = AxisMap::new(&view.y, f64::from(area.y), f64::from(area.height), true).ok_or(
            RenderError::AxisNotMappable {
                view: index,
                axis: 1,
            },
        )?;
        Ok(PlaneView {
            view,
            index,
            layout: *layout,
            tick_length,
            x,
            y,
            clip: painter.canvas.clip_mask(area),
            guard: Guard::around(area),
            patterns: Patterns::new(painter.theme, painter.scale, (area.x, area.y)),
        })
    }

    pub(crate) fn area(&self) -> PhysicalRect {
        self.layout.plot_area
    }

    pub(crate) fn point(&self, xs: &Column, ys: &Column, index: usize) -> Option<ScreenPoint> {
        Some((self.x.of_column(xs, index)?, self.y.of_column(ys, index)?))
    }
}

pub(crate) fn step_of(axis: &ViewAxis) -> f64 {
    axis.range
        .upper
        .sub_exact(&axis.range.lower)
        .ok()
        .and_then(|width| {
            width
                .div_exact(&Number::from(i64::from(axis.divisions)))
                .ok()
        })
        .map_or(f64::NAN, |step| step.round_to_f64_ties_even())
}

pub(crate) fn is_unresolved(bounds: Option<&Column>, index: usize, step: f64) -> bool {
    bounds
        .and_then(|column| column_value(column, index))
        .is_some_and(|bound| 2.0 * bound >= step)
}

pub(crate) fn bound_at(bounds: Option<&Column>, index: usize) -> Option<f64> {
    bounds
        .and_then(|column| column_value(column, index))
        .filter(|bound| bound.is_finite())
}

pub(crate) fn range_width(range: &Interval) -> f64 {
    range
        .upper
        .sub_exact(&range.lower)
        .map_or(f64::NAN, |width| width.round_to_f64_ties_even())
}

pub(crate) fn scalar_colour(
    mapping: &ColourMapping,
    value: f64,
    position: LayerPosition,
) -> Result<Option<Colour>, RenderError> {
    let colour = match mapping.map {
        ColourMap::Sequential => sequential_colour(&mapping.range, value),
        ColourMap::Diverging => diverging_colour(&mapping.range, value),
        ColourMap::Categorical => categorical_colour(value),
        ColourMap::DomainColouring => return Err(RenderError::ColourMapKindMismatch(position)),
    };
    Ok(colour.map(roles::from_map))
}

pub(crate) fn line_dash(
    painter: &Painter<'_>,
    pattern: LinePattern,
    width: f32,
) -> Result<Option<Option<[f32; 2]>>, RenderError> {
    Ok(match pattern {
        LinePattern::None => None,
        LinePattern::Solid => Some(None),
        LinePattern::Dashed => Some(Some([
            f32::from(painter.physical(DASH_LOGICAL)),
            f32::from(painter.physical(DASH_GAP_LOGICAL)),
        ])),
        LinePattern::Dotted => Some(Some([width, width * 2.0])),
    })
}

pub(crate) fn fill_path_with(
    painter: &mut Painter<'_>,
    path: &tiny_skia::Path,
    fill: Fill,
    patterns: &Patterns,
    style: &StyleRole,
    clip: Option<&Mask>,
) {
    let theme = painter.theme;
    match fill {
        Fill::Solid(colour) => painter.canvas.fill_path_within(path, colour, clip),
        Fill::Role(role) => {
            painter.show(role);
            painter.canvas.fill_path_pattern(path, clip, &|x, y| {
                Some(roles::settled(&theme, style, patterns.covering(fill, x, y)))
            });
        }
    }
}

pub(crate) fn draw_marker(
    painter: &mut Painter<'_>,
    marker: Marker,
    centre: ScreenPoint,
    (fill, patterns, style): (Fill, &Patterns, &StyleRole),
    clip: Option<&Mask>,
) -> Result<(), RenderError> {
    let width = painter.line_width(THIN_WIDTH_LOGICAL);
    let (marker, size) = match fill {
        Fill::Solid(_) => (marker, MARKER_SIZE_LOGICAL),
        Fill::Role(_) => (Marker::Dot, ROLE_MARKER_SIZE_LOGICAL),
    };
    let radius = f64::from(painter.physical(size)) / 2.0;
    let circle = CircularArc {
        centre,
        radius,
        start: 0.0,
        sweep: FULL_TURN,
    };
    let mut builder = PathBuilder::new();
    let outline_colour = match fill {
        Fill::Solid(colour) => colour,
        Fill::Role(role) => roles::settled(
            &painter.theme,
            style,
            roles::role_tone(&painter.theme, role),
        ),
    };
    match marker {
        Marker::None | Marker::Dot => {
            circle.push_to(&mut builder, true);
            builder.close();
            if let Some(path) = builder.finish() {
                fill_path_with(painter, &path, fill, patterns, style, clip);
                if let Fill::Role(_) = fill {
                    painter
                        .canvas
                        .stroke_path_within(&path, outline_colour, width, None, clip);
                }
            }
        }
        Marker::Circle => {
            circle.push_to(&mut builder, true);
            builder.close();
            if let Some(path) = builder.finish() {
                painter
                    .canvas
                    .stroke_path_within(&path, outline_colour, width, None, clip);
            }
        }
        Marker::Cross => {
            builder.move_to(to_f32(centre.0 - radius), to_f32(centre.1 - radius));
            builder.line_to(to_f32(centre.0 + radius), to_f32(centre.1 + radius));
            builder.move_to(to_f32(centre.0 - radius), to_f32(centre.1 + radius));
            builder.line_to(to_f32(centre.0 + radius), to_f32(centre.1 - radius));
            if let Some(path) = builder.finish() {
                painter
                    .canvas
                    .stroke_path_within(&path, outline_colour, width, None, clip);
            }
        }
    }
    Ok(())
}

pub(crate) fn draw_arrow(
    painter: &mut Painter<'_>,
    guard: &Guard,
    (base, tip): (ScreenPoint, ScreenPoint),
    (colour, dash): (Colour, Option<[f32; 2]>),
    clip: Option<&Mask>,
) -> Result<(), RenderError> {
    let width = painter.line_width(THIN_WIDTH_LOGICAL);
    if let Some(path) = polyline_path(guard, &[vec![base, tip]]) {
        painter
            .canvas
            .stroke_path_within(&path, colour, width, dash, clip);
    }
    let along = (tip.0 - base.0, tip.1 - base.1);
    let length = (along.0 * along.0 + along.1 * along.1).sqrt();
    if length == 0.0 || !length.is_finite() || !guard.contains(tip) {
        return Ok(());
    }
    let head = f64::from(painter.physical(ARROW_HEAD_LOGICAL));
    let unit = (along.0 / length, along.1 / length);
    let back = (tip.0 - unit.0 * head, tip.1 - unit.1 * head);
    let side = (-unit.1 * head / 2.0, unit.0 * head / 2.0);
    let corners = [
        tip,
        (back.0 + side.0, back.1 + side.1),
        (back.0 - side.0, back.1 - side.1),
    ];
    if let Some(path) = polygon_path(guard, &corners) {
        painter.canvas.fill_path_within(&path, colour, clip);
    }
    Ok(())
}

pub(crate) fn draw_plane_layer(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    layer: &Layer,
    position: LayerPosition,
    text: &PictureText,
) -> Result<Option<FigureLayout>, RenderError> {
    match &layer.primitive {
        Primitive::Polyline(polyline) => draw_polyline(painter, plane, layer, polyline)?,
        Primitive::Band(band) => draw_band(painter, plane, layer, band)?,
        Primitive::Points(points) => draw_points(painter, plane, layer, points, position)?,
        Primitive::Arrows(arrows) => draw_arrows(painter, plane, layer, arrows)?,
        Primitive::ScalarGrid(grid) => draw_scalar_grid(painter, plane, layer, grid, position)?,
        Primitive::ComplexGrid(grid) => draw_complex_grid(painter, plane, layer, grid, position)?,
        Primitive::TriangleMesh(mesh) => draw_mesh(painter, plane, layer, mesh, position)?,
        Primitive::Graph(graph) => draw_graph(painter, plane, layer, graph)?,
        Primitive::Formula(formula) => draw_formula(painter, plane, layer, formula)?,
        Primitive::BitLayout(bits) => draw_bits(painter, plane, layer, bits)?,
        Primitive::Figure(figure) => {
            return draw_figure(painter, plane, layer, figure, text).map(Some);
        }
        Primitive::Voxels(_) => return Err(view_kind_mismatch(position, PLANE_AXES)),
    }
    Ok(None)
}

fn coordinate_pair(columns: &[Column]) -> Option<(&Column, &Column)> {
    match columns {
        [xs, ys] => Some((xs, ys)),
        _ => None,
    }
}

fn leaves_on_opposite_sides(before: f64, after: f64, top: f64, bottom: f64) -> bool {
    (before < top && after > bottom) || (before > bottom && after < top)
}

fn column_outline(x: (f64, f64), y: (f64, f64)) -> Vec<ScreenPoint> {
    vec![(x.0, y.0), (x.1, y.0), (x.1, y.1), (x.0, y.1)]
}

fn thickened(y: (f64, f64), thickness: f64) -> (f64, f64) {
    let (top, bottom) = if y.0 <= y.1 { y } else { (y.1, y.0) };
    if bottom - top >= thickness {
        return (top, bottom);
    }
    let middle = (top + bottom) / 2.0;
    (middle - thickness / 2.0, middle + thickness / 2.0)
}

fn draw_columns(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    layer: &Layer,
    (xs, ys): (&Column, &Column),
    columns: &ColumnEnclosures,
) -> Result<(), RenderError> {
    let colour = roles::layer_colour(&painter.theme, &layer.style);
    let faint = roles::band_fill(colour);
    let width = painter.line_width(CURVE_WIDTH_LOGICAL);
    let step = step_of(&plane.view.y);
    let clip = plane.clip.as_ref();
    let theme = painter.theme;
    let patterns = plane.patterns;
    let style = &layer.style;
    let mut runs: Vec<Vec<ScreenPoint>> = vec![Vec::new()];
    for column in 0..columns.lower.len() {
        let (lower, upper) = (columns.lower[column], columns.upper[column]);
        let (hit_lower, hit_upper) = (columns.hit_lower[column], columns.hit_upper[column]);
        let edges = (
            plane.x.of_column(xs, column),
            plane.x.of_column(xs, column + 1),
            plane.y.of_f64(lower),
            plane.y.of_f64(upper),
        );
        let (Some(left), Some(right), Some(bottom), Some(top)) = edges else {
            painter.show(Role::Missing);
            runs.push(Vec::new());
            continue;
        };
        if columns.marked[column] {
            painter.show(Role::Marked);
            runs.push(Vec::new());
            if let Some(path) =
                polygon_path(&plane.guard, &column_outline((left, right), (top, bottom)))
            {
                painter.canvas.fill_path_pattern(&path, clip, &|x, y| {
                    patterns
                        .ink(Role::Marked, x, y)
                        .map(|ink| roles::settled(&theme, style, ink))
                });
            }
            continue;
        }
        if !columns.is_unresolved_at_width(column, step) {
            match (plane.point(xs, ys, column), plane.point(xs, ys, column + 1)) {
                (Some(start), Some(end)) => {
                    if let (Some(hit_bottom), Some(hit_top)) =
                        (plane.y.of_f64(hit_lower), plane.y.of_f64(hit_upper))
                        && (hit_bottom - hit_top).abs() > (end.1 - start.1).abs() + f64::from(width)
                        && let Some(path) = polygon_path(
                            &plane.guard,
                            &column_outline((left, right), (hit_top, hit_bottom)),
                        )
                    {
                        painter.canvas.fill_path_within(&path, colour, clip);
                    }
                    if let Some(run) = runs.last_mut() {
                        if run.is_empty() {
                            run.push(start);
                        }
                        run.push(end);
                    }
                }
                _ => runs.push(Vec::new()),
            }
            continue;
        }
        painter.show(Role::MayBeHit);
        painter.may_be_hit.get_or_insert(faint);
        runs.push(Vec::new());
        if let Some(path) =
            polygon_path(&plane.guard, &column_outline((left, right), (top, bottom)))
        {
            painter.canvas.fill_path_within(&path, faint, clip);
        }
        if let (Some(hit_bottom), Some(hit_top)) =
            (plane.y.of_f64(hit_lower), plane.y.of_f64(hit_upper))
        {
            let span = thickened((hit_top, hit_bottom), f64::from(width));
            if let Some(path) = polygon_path(&plane.guard, &column_outline((left, right), span)) {
                painter.canvas.fill_path_within(&path, colour, clip);
            }
        }
    }
    if let Some(dash) = line_dash(painter, layer.style.line, width)?
        && let Some(path) = polyline_path(&plane.guard, &runs)
    {
        painter
            .canvas
            .stroke_path_within(&path, colour, width, dash, clip);
    }
    Ok(())
}

fn draw_polyline(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    layer: &Layer,
    polyline: &Polyline,
) -> Result<(), RenderError> {
    let Some((xs, ys)) = coordinate_pair(&polyline.coordinates) else {
        return Ok(());
    };
    if let Some(columns) = &layer.columns
        && layer.style.marker == Marker::None
    {
        return draw_columns(painter, plane, layer, (xs, ys), columns);
    }
    let colour = roles::layer_colour(&painter.theme, &layer.style);
    let width = painter.line_width(CURVE_WIDTH_LOGICAL);
    let bounds = layer.value_bounds.first();
    let step = step_of(&plane.view.y);
    let clip = plane.clip.as_ref();
    let mut resolved_runs: Vec<Vec<ScreenPoint>> = Vec::new();
    let mut unresolved_runs: Vec<Vec<(f64, f64, f64)>> = Vec::new();
    let mut current: Vec<ScreenPoint> = Vec::new();
    let mut current_unresolved: Vec<(f64, f64, f64)> = Vec::new();
    let area = plane.layout.plot_area;
    let top = f64::from(area.y);
    let bottom = f64::from(area.y) + f64::from(area.height);
    let mut previous: Option<ScreenPoint> = None;
    for index in 0..xs.len() {
        let point = plane.point(xs, ys, index);
        let unresolved = point.is_some() && is_unresolved(bounds, index, step);
        match (point, unresolved) {
            (Some(point), false) => {
                if let Some(before) = previous
                    && leaves_on_opposite_sides(before.1, point.1, top, bottom)
                    && !current.is_empty()
                {
                    resolved_runs.push(std::mem::take(&mut current));
                }
                previous = Some(point);
                current.push(point);
                if !current_unresolved.is_empty() {
                    unresolved_runs.push(std::mem::take(&mut current_unresolved));
                }
            }
            (Some(point), true) => {
                if !current.is_empty() {
                    resolved_runs.push(std::mem::take(&mut current));
                }
                let value = column_value(ys, index).unwrap_or(f64::NAN);
                let bound = bound_at(bounds, index).unwrap_or(f64::NAN);
                let upper = plane.y.of_f64(value + bound);
                let lower = plane.y.of_f64(value - bound);
                if let (Some(upper), Some(lower)) = (upper, lower) {
                    current_unresolved.push((point.0, upper, lower));
                }
            }
            (None, _) => {
                painter.show(Role::Missing);
                resolved_runs.push(std::mem::take(&mut current));
                unresolved_runs.push(std::mem::take(&mut current_unresolved));
            }
        }
    }
    resolved_runs.push(current);
    unresolved_runs.push(current_unresolved);
    for run in unresolved_runs.iter().filter(|run| !run.is_empty()) {
        draw_enclosure(painter, plane, layer, run)?;
    }
    if let Some(dash) = line_dash(painter, layer.style.line, width)?
        && let Some(path) = polyline_path(&plane.guard, &resolved_runs)
    {
        painter
            .canvas
            .stroke_path_within(&path, colour, width, dash, clip);
    }
    if layer.style.marker != Marker::None {
        for point in resolved_runs.iter().flatten() {
            if plane.guard.contains(*point) {
                draw_marker(
                    painter,
                    layer.style.marker,
                    *point,
                    (Fill::Solid(colour), &plane.patterns, &layer.style),
                    clip,
                )?;
            }
        }
    }
    Ok(())
}

fn draw_enclosure(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    layer: &Layer,
    run: &[(f64, f64, f64)],
) -> Result<(), RenderError> {
    let clip = plane.clip.as_ref();
    let theme = painter.theme;
    let edge = roles::settled(
        &theme,
        &layer.style,
        roles::role_tone(&theme, Role::Unresolved),
    );
    let width = painter.line_width(THIN_WIDTH_LOGICAL);
    painter.show(Role::Unresolved);
    if let [(x, upper, lower)] = run {
        if let Some(path) = polyline_path(&plane.guard, &[vec![(*x, *upper), (*x, *lower)]]) {
            painter
                .canvas
                .stroke_path_within(&path, edge, width, None, clip);
        }
        return Ok(());
    }
    let upper_edge: Vec<ScreenPoint> = run.iter().map(|(x, upper, _)| (*x, *upper)).collect();
    let lower_edge: Vec<ScreenPoint> = run.iter().map(|(x, _, lower)| (*x, *lower)).collect();
    let mut outline = upper_edge.clone();
    outline.extend(lower_edge.iter().rev());
    if let Some(path) = polygon_path(&plane.guard, &outline) {
        let patterns = plane.patterns;
        let style = &layer.style;
        painter.canvas.fill_path_pattern(&path, clip, &|x, y| {
            patterns
                .ink(Role::Unresolved, x, y)
                .map(|ink| roles::settled(&theme, style, ink))
        });
    }
    if let Some(path) = polyline_path(&plane.guard, &[upper_edge, lower_edge]) {
        painter
            .canvas
            .stroke_path_within(&path, edge, width, None, clip);
    }
    Ok(())
}

fn draw_band(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    layer: &Layer,
    band: &Band,
) -> Result<(), RenderError> {
    let colour = roles::band_fill(roles::layer_colour(&painter.theme, &layer.style));
    let lower_bounds = layer.value_bounds.first();
    let upper_bounds = layer.value_bounds.get(1);
    let step = step_of(&plane.view.y);
    let mut runs: Vec<Vec<(f64, f64, f64)>> = vec![Vec::new()];
    let mut enclosures: Vec<Vec<(f64, f64, f64)>> = vec![Vec::new()];
    for index in 0..band.abscissa.len() {
        let x = plane.x.of_column(&band.abscissa, index);
        let lower = plane.y.of_column(&band.lower, index);
        let upper = plane.y.of_column(&band.upper, index);
        let (Some(x), Some(lower), Some(upper)) = (x, lower, upper) else {
            painter.show(Role::Missing);
            runs.push(Vec::new());
            enclosures.push(Vec::new());
            continue;
        };
        if let Some(run) = runs.last_mut() {
            run.push((x, upper, lower));
        }
        let unresolved =
            is_unresolved(lower_bounds, index, step) || is_unresolved(upper_bounds, index, step);
        if unresolved {
            let widened = |column: &Column, bounds: Option<&Column>, sign: f64| {
                let value = column_value(column, index)?;
                plane
                    .y
                    .of_f64(value + sign * bound_at(bounds, index).unwrap_or(0.0))
            };
            if let (Some(upper), Some(lower), Some(run)) = (
                widened(&band.upper, upper_bounds, 1.0),
                widened(&band.lower, lower_bounds, -1.0),
                enclosures.last_mut(),
            ) {
                run.push((x, upper, lower));
            }
        } else if enclosures.last().is_some_and(|run| !run.is_empty()) {
            enclosures.push(Vec::new());
        }
    }
    for run in enclosures.iter().filter(|run| !run.is_empty()) {
        draw_enclosure(painter, plane, layer, run)?;
    }
    for run in runs.iter().filter(|run| run.len() >= 2) {
        let mut outline: Vec<ScreenPoint> = run.iter().map(|(x, upper, _)| (*x, *upper)).collect();
        outline.extend(run.iter().rev().map(|(x, _, lower)| (*x, *lower)));
        if let Some(path) = polygon_path(&plane.guard, &outline) {
            painter
                .canvas
                .fill_path_within(&path, colour, plane.clip.as_ref());
        }
    }
    Ok(())
}

fn point_fill(
    painter: &Painter<'_>,
    layer: &Layer,
    scalar: Option<&Column>,
    index: usize,
    position: LayerPosition,
) -> Result<Fill, RenderError> {
    let base = roles::layer_colour(&painter.theme, &layer.style);
    let (Some(scalar), Some(mapping)) = (scalar, layer.style.colour_map.as_ref()) else {
        return Ok(Fill::Solid(base));
    };
    let value = column_value(scalar, index).unwrap_or(f64::NAN);
    let mapped = scalar_colour(mapping, value, position)?.filter(|_| value.is_finite());
    Ok(mapped.map_or(Fill::Role(Role::Missing), |colour| {
        Fill::Solid(roles::emphasised(&painter.theme, &layer.style, colour))
    }))
}

pub(crate) fn has_grid_limit(layer: &Layer) -> bool {
    layer
        .precision
        .as_ref()
        .is_some_and(|precision| !precision.grid_exhausted.is_empty())
}

fn draw_points(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    layer: &Layer,
    points: &Points,
    position: LayerPosition,
) -> Result<(), RenderError> {
    let Some((xs, ys)) = coordinate_pair(&points.coordinates) else {
        return Ok(());
    };
    let bounds = layer.value_bounds.first();
    let step = step_of(&plane.view.y);
    let grid_limit = has_grid_limit(layer);
    for index in 0..xs.len() {
        let Some(point) = plane.point(xs, ys, index) else {
            painter.show(Role::Missing);
            continue;
        };
        if !plane.guard.contains(point) {
            continue;
        }
        let fill = if grid_limit || is_unresolved(bounds, index, step) {
            Fill::Role(Role::Unresolved)
        } else {
            point_fill(painter, layer, points.scalar.as_ref(), index, position)?
        };
        draw_marker(
            painter,
            layer.style.marker,
            point,
            (fill, &plane.patterns, &layer.style),
            plane.clip.as_ref(),
        )?;
    }
    Ok(())
}

fn tip_position(map: &AxisMap, bases: &Column, components: &Column, index: usize) -> Option<f64> {
    match (bases, components) {
        (Column::F32(bases), Column::F32(components)) => {
            map.of_f32(*bases.get(index)? + *components.get(index)?)
        }
        _ => map.of_f64(column_value(bases, index)? + column_value(components, index)?),
    }
}

fn draw_arrows(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    layer: &Layer,
    arrows: &Arrows,
) -> Result<(), RenderError> {
    let (Some((base_x, base_y)), Some((component_x, component_y))) = (
        coordinate_pair(&arrows.bases),
        coordinate_pair(&arrows.components),
    ) else {
        return Ok(());
    };
    let steps = [step_of(&plane.view.x), step_of(&plane.view.y)];
    for index in 0..base_x.len() {
        let Some(base) = plane.point(base_x, base_y, index) else {
            continue;
        };
        let tip = (
            tip_position(&plane.x, base_x, component_x, index),
            tip_position(&plane.y, base_y, component_y, index),
        );
        let (Some(tip_x), Some(tip_y)) = tip else {
            continue;
        };
        let unresolved = layer
            .value_bounds
            .iter()
            .zip(steps)
            .any(|(bounds, step)| is_unresolved(Some(bounds), index, step));
        let stroke = unresolved_arrow(painter, layer, unresolved);
        draw_arrow(
            painter,
            &plane.guard,
            (base, (tip_x, tip_y)),
            stroke,
            plane.clip.as_ref(),
        )?;
    }
    Ok(())
}

pub(crate) fn unresolved_arrow(
    painter: &mut Painter<'_>,
    layer: &Layer,
    unresolved: bool,
) -> (Colour, Option<[f32; 2]>) {
    if !unresolved {
        return (roles::layer_colour(&painter.theme, &layer.style), None);
    }
    painter.show(Role::Unresolved);
    let dot = painter.line_width(THIN_WIDTH_LOGICAL);
    let pitch = f32::from(painter.physical(STIPPLE_PITCH_LOGICAL));
    (
        roles::settled(
            &painter.theme,
            &layer.style,
            roles::role_tone(&painter.theme, Role::Unresolved),
        ),
        Some([dot, pitch - dot]),
    )
}

pub(crate) fn cell_edges(
    map: &AxisMap,
    axis: &ViewAxis,
    region: &Interval,
    count: u32,
) -> Vec<Option<f64>> {
    let width = region.upper.sub_exact(&region.lower).ok();
    (0..=count)
        .map(|index| {
            let edge = width
                .as_ref()?
                .mul_exact(&Number::from(i64::from(index)))
                .ok()?
                .div_exact(&Number::from(i64::from(count)))
                .ok()?
                .add_exact(&region.lower)
                .ok()?;
            map.of_exact(axis, &edge)
        })
        .collect()
}

fn pixel_span(first: Option<f64>, second: Option<f64>, low: u16, high: u16) -> Option<(u16, u16)> {
    let (first, second) = (first?, second?);
    let (start, end) = if first <= second {
        (first, second)
    } else {
        (second, first)
    };
    let start = pixel_round(start).clamp(low, high);
    let end = pixel_round(end).clamp(low, high);
    (end > start).then_some((start, end))
}

fn paint_cells(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    (region, counts): (&[Interval], &[u32]),
    style: &StyleRole,
    mut fill_of: impl FnMut(&Painter<'_>, usize) -> Result<Fill, RenderError>,
) -> Result<(), RenderError> {
    let ([x_region, y_region], [x_count, y_count]) = (region, counts) else {
        return Ok(());
    };
    let area = plane.area();
    let x_edges = cell_edges(&plane.x, &plane.view.x, x_region, *x_count);
    let y_edges = cell_edges(&plane.y, &plane.view.y, y_region, *y_count);
    let columns = usize::try_from(*x_count).unwrap_or(0);
    let area_width = usize::from(area.width);
    let mut inside = vec![false; area_width * usize::from(area.height)];
    let theme = painter.theme;
    let patterns = plane.patterns;
    for (row, y_pair) in y_edges.windows(2).enumerate() {
        let [bottom, top] = y_pair else {
            continue;
        };
        let Some((y_start, y_end)) = pixel_span(*top, *bottom, area.y, area.bottom()) else {
            continue;
        };
        for (column, x_pair) in x_edges.windows(2).enumerate() {
            let [left, right] = x_pair else {
                continue;
            };
            let Some((x_start, x_end)) = pixel_span(*left, *right, area.x, area.right()) else {
                continue;
            };
            let fill = fill_of(painter, row * columns + column)?;
            if let Fill::Role(role) = fill {
                painter.show(role);
            }
            if fill == Fill::Role(Role::Inside) {
                for y in y_start..y_end {
                    let offset = usize::from(y - area.y) * area_width;
                    for x in x_start..x_end {
                        if let Some(flag) = inside.get_mut(offset + usize::from(x - area.x)) {
                            *flag = true;
                        }
                    }
                }
            }
            painter.canvas.paint_rect(
                PhysicalRect::new(x_start, y_start, x_end - x_start, y_end - y_start),
                &|x, y| roles::settled(&theme, style, patterns.covering(fill, x, y)),
            );
        }
    }
    let line = painter.line_width(THIN_WIDTH_LOGICAL);
    mark_inside_edges(painter, area, &inside, pixel_floor(f64::from(line)), style);
    Ok(())
}

fn mark_inside_edges(
    painter: &mut Painter<'_>,
    area: PhysicalRect,
    inside: &[bool],
    line: u16,
    style: &StyleRole,
) {
    let width = usize::from(area.width);
    let height = usize::from(area.height);
    let is_inside = |column: usize, row: usize| {
        column < width && row < height && inside.get(row * width + column).copied().unwrap_or(false)
    };
    let ground = roles::settled(&painter.theme, style, painter.theme.window);
    let reach = usize::from(line);
    for row in 0..height {
        for column in 0..width {
            if !is_inside(column, row) {
                continue;
            }
            let borders_outside = (1..=reach).any(|distance| {
                let left = column
                    .checked_sub(distance)
                    .is_some_and(|left| !is_inside(left, row));
                let up = row
                    .checked_sub(distance)
                    .is_some_and(|up| !is_inside(column, up));
                let right = column + distance < width && !is_inside(column + distance, row);
                let down = row + distance < height && !is_inside(column, row + distance);
                left || up || right || down
            });
            if borders_outside {
                let x = area
                    .x
                    .saturating_add(u16::try_from(column).unwrap_or(u16::MAX));
                let y = area
                    .y
                    .saturating_add(u16::try_from(row).unwrap_or(u16::MAX));
                painter
                    .canvas
                    .set_pixels(PhysicalRect::new(x, y, 1, 1), ground);
            }
        }
    }
}

fn draw_scalar_grid(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    layer: &Layer,
    grid: &ScalarGrid,
    position: LayerPosition,
) -> Result<(), RenderError> {
    let style = GridStyle::new(layer, position)?;
    paint_cells(
        painter,
        plane,
        (&grid.cells.region, &grid.cells.counts),
        &layer.style,
        |painter, index| style.cell_fill(painter, grid, index),
    )
}

pub(crate) struct GridStyle<'a> {
    layer: &'a Layer,
    mapping: &'a ColourMapping,
    step: f64,
    position: LayerPosition,
}

impl<'a> GridStyle<'a> {
    pub(crate) fn new(
        layer: &'a Layer,
        position: LayerPosition,
    ) -> Result<GridStyle<'a>, RenderError> {
        let mapping = layer
            .style
            .colour_map
            .as_ref()
            .ok_or(RenderError::ColourMapMissing(position))?;
        Ok(GridStyle {
            layer,
            mapping,
            step: range_width(&mapping.range) / COLOUR_MAP_STEPS,
            position,
        })
    }

    pub(crate) fn cell_fill(
        &self,
        painter: &Painter<'_>,
        grid: &ScalarGrid,
        index: usize,
    ) -> Result<Fill, RenderError> {
        let class = grid
            .classes
            .as_ref()
            .and_then(|classes| classes.get(index).copied());
        Ok(match class {
            Some(INSIDE_CELL) => Fill::Role(Role::Inside),
            Some(UNDECIDED_CELL) => Fill::Role(Role::Undecided),
            _ => {
                let value = column_value(&grid.scalar, index).unwrap_or(f64::NAN);
                let bounds = self.layer.value_bounds.first();
                if class.is_none() && value.is_finite() && is_unresolved(bounds, index, self.step) {
                    Fill::Role(Role::Unresolved)
                } else {
                    match scalar_colour(self.mapping, value, self.position)? {
                        Some(colour) if value.is_finite() => Fill::Solid(roles::emphasised(
                            &painter.theme,
                            &self.layer.style,
                            colour,
                        )),
                        _ => Fill::Role(Role::Missing),
                    }
                }
            }
        })
    }
}

fn draw_complex_grid(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    layer: &Layer,
    grid: &ComplexGrid,
    position: LayerPosition,
) -> Result<(), RenderError> {
    let mapping = layer
        .style
        .colour_map
        .as_ref()
        .ok_or(RenderError::ColourMapMissing(position))?;
    if mapping.map != ColourMap::DomainColouring {
        return Err(RenderError::ColourMapKindMismatch(position));
    }
    let step = range_width(&mapping.range) / COLOUR_MAP_STEPS;
    let bounds = [layer.value_bounds.first(), layer.value_bounds.get(1)];
    paint_cells(
        painter,
        plane,
        (&grid.cells.region, &grid.cells.counts),
        &layer.style,
        |painter, index| {
            let theme = &painter.theme;
            let real = column_value(&grid.real, index).unwrap_or(f64::NAN);
            let imaginary = column_value(&grid.imaginary, index).unwrap_or(f64::NAN);
            let unresolved = bounds
                .iter()
                .any(|bounds| is_unresolved(*bounds, index, step));
            Ok(if !real.is_finite() || !imaginary.is_finite() {
                Fill::Role(Role::Missing)
            } else if unresolved {
                Fill::Role(Role::Unresolved)
            } else {
                domain_colour(&mapping.range, real, imaginary).map_or(
                    Fill::Role(Role::Missing),
                    |colour| {
                        Fill::Solid(roles::emphasised(
                            theme,
                            &layer.style,
                            roles::from_map(colour),
                        ))
                    },
                )
            })
        },
    )
}

fn draw_mesh(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    layer: &Layer,
    mesh: &TriangleMesh,
    position: LayerPosition,
) -> Result<(), RenderError> {
    let Some((xs, ys)) = coordinate_pair(&mesh.vertices) else {
        return Ok(());
    };
    let bounds = layer.value_bounds.first();
    let step = step_of(&plane.view.y);
    let mut depth = DepthBuffer::new(plane.area());
    let mut order = 0.0;
    for triangle in &mesh.triangles {
        order -= 1.0;
        let indices = triangle.map(|index| usize::try_from(index).unwrap_or(usize::MAX));
        let corners: Option<Vec<ScreenPoint>> = indices
            .iter()
            .map(|index| plane.point(xs, ys, *index))
            .collect();
        let Some(corners) = corners else {
            painter.show(Role::Missing);
            continue;
        };
        let fill = if indices
            .iter()
            .any(|index| is_unresolved(bounds, *index, step))
        {
            Fill::Role(Role::Unresolved)
        } else {
            mesh_fill(painter, layer, mesh.scalar.as_ref(), indices, position)?
        };
        let [first, second, third] = corners.as_slice() else {
            continue;
        };
        if let Fill::Role(role) = fill {
            painter.show(role);
        }
        let theme = painter.theme;
        let patterns = plane.patterns;
        let style = &layer.style;
        depth.fill_triangle(
            painter.canvas,
            [first, second, third].map(|corner| (corner.0, corner.1, order)),
            &|x, y| roles::settled(&theme, style, patterns.covering(fill, x, y)),
        );
    }
    Ok(())
}

pub(crate) fn mesh_fill(
    painter: &Painter<'_>,
    layer: &Layer,
    scalar: Option<&Column>,
    indices: [usize; 3],
    position: LayerPosition,
) -> Result<Fill, RenderError> {
    let base = roles::layer_colour(&painter.theme, &layer.style);
    let (Some(scalar), Some(mapping)) = (scalar, layer.style.colour_map.as_ref()) else {
        return Ok(Fill::Solid(base));
    };
    let values = indices.map(|index| column_value(scalar, index).unwrap_or(f64::NAN));
    let mean = (values[0] + values[1] + values[2]) / 3.0;
    Ok(match scalar_colour(mapping, mean, position)? {
        Some(colour) if mean.is_finite() => {
            Fill::Solid(roles::emphasised(&painter.theme, &layer.style, colour))
        }
        _ => Fill::Role(Role::Missing),
    })
}

fn draw_graph(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    layer: &Layer,
    graph: &Graph,
) -> Result<(), RenderError> {
    let Some((xs, ys)) = coordinate_pair(&graph.positions) else {
        return Ok(());
    };
    let nodes: Vec<Option<ScreenPoint>> = (0..xs.len())
        .map(|index| plane.point(xs, ys, index))
        .collect();
    draw_graph_nodes(
        painter,
        &plane.guard,
        plane.clip.as_ref(),
        layer,
        graph,
        &nodes,
    )
}

pub(crate) fn draw_graph_nodes(
    painter: &mut Painter<'_>,
    guard: &Guard,
    clip: Option<&Mask>,
    layer: &Layer,
    graph: &Graph,
    nodes: &[Option<ScreenPoint>],
) -> Result<(), RenderError> {
    let colour = roles::layer_colour(&painter.theme, &layer.style);
    let width = painter.line_width(THIN_WIDTH_LOGICAL);
    let node_radius = f64::from(painter.physical(NODE_SIZE_LOGICAL)) / 2.0;
    let node = |index: u32| {
        usize::try_from(index)
            .ok()
            .and_then(|index| nodes.get(index).copied().flatten())
    };
    for [from, to] in &graph.edges {
        let (Some(start), Some(end)) = (node(*from), node(*to)) else {
            continue;
        };
        if graph.is_directed {
            let along = (end.0 - start.0, end.1 - start.1);
            let length = (along.0 * along.0 + along.1 * along.1).sqrt();
            if length > node_radius {
                let shortened = (
                    end.0 - along.0 / length * node_radius,
                    end.1 - along.1 / length * node_radius,
                );
                draw_arrow(painter, guard, (start, shortened), (colour, None), clip)?;
            }
        } else if let Some(path) = polyline_path(guard, &[vec![start, end]]) {
            painter
                .canvas
                .stroke_path_within(&path, colour, width, None, clip);
        }
    }
    for point in nodes.iter().flatten() {
        if guard.contains(*point) {
            let circle = CircularArc {
                centre: *point,
                radius: node_radius,
                start: 0.0,
                sweep: FULL_TURN,
            };
            let mut builder = PathBuilder::new();
            circle.push_to(&mut builder, true);
            builder.close();
            if let Some(path) = builder.finish() {
                painter.canvas.fill_path_within(&path, colour, clip);
            }
        }
    }
    Ok(())
}

pub(crate) fn draw_text_at(
    painter: &mut Painter<'_>,
    area: PhysicalRect,
    text: &str,
    anchor: ScreenPoint,
    colour: Colour,
) -> Result<(), RenderError> {
    let width = painter.text_width(text);
    let height = u32::from(painter.row_height());
    let left = anchor.0;
    let top = anchor.1 - f64::from(painter.row_height()) / 2.0;
    let fits = left >= f64::from(area.x)
        && top >= f64::from(area.y)
        && left + f64::from(width) <= f64::from(area.right())
        && top + f64::from(height) <= f64::from(area.bottom());
    if !fits {
        return Ok(());
    }
    painter.text(text, pixel_round(left), pixel_round(top), colour)
}

fn draw_formula(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    layer: &Layer,
    formula: &Formula,
) -> Result<(), RenderError> {
    let [x, y] = formula.anchor.as_slice() else {
        return Ok(());
    };
    let (Some(x), Some(y)) = (
        plane.x.of_exact(&plane.view.x, x),
        plane.y.of_exact(&plane.view.y, y),
    ) else {
        return Ok(());
    };
    let colour = roles::emphasised(&painter.theme, &layer.style, painter.theme.text);
    draw_text_at(painter, plane.area(), &formula.expression, (x, y), colour)
}

fn draw_bits(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    layer: &Layer,
    bits: &BitLayout,
) -> Result<(), RenderError> {
    let area = plane.area();
    let slots = u32::try_from(bits.bits.len() + BIT_FIELD_GAPS).unwrap_or(u32::MAX);
    let cell_limit = 2 * u32::from(painter.row_height());
    let size = (u32::from(area.width) / slots)
        .min(cell_limit)
        .min(u32::from(area.height));
    let size = u16::try_from(size).unwrap_or(0);
    if size == 0 {
        return Err(RenderError::Layout(LayoutError::TooSmall {
            width: area.width,
            height: area.height,
        }));
    }
    let colour = roles::layer_colour(&painter.theme, &layer.style);
    let frame_width = pixel_floor(f64::from(painter.line_width(THIN_WIDTH_LOGICAL)));
    let used = u32::from(size) * slots;
    let left = u32::from(area.x) + (u32::from(area.width) - used) / 2;
    let top = area.y + (area.height - size) / 2;
    let exponent_start = usize::try_from(bits.exponent_start).unwrap_or(usize::MAX);
    let fraction_start = usize::try_from(bits.fraction_start).unwrap_or(usize::MAX);
    let shows_digits = size >= painter.metrics.cell.width() && size >= painter.row_height();
    let mut list = DrawList::new();
    let mut digits = Vec::new();
    for (index, bit) in bits.bits.iter().enumerate() {
        let gaps = usize::from(index >= exponent_start) + usize::from(index >= fraction_start);
        let slot = u32::try_from(index + gaps).unwrap_or(u32::MAX);
        let x = u16::try_from(left + slot * u32::from(size)).unwrap_or(u16::MAX);
        let rect = PhysicalRect::new(x, top, size, size);
        if *bit {
            list.push(DrawCommand::FillRect { rect, colour });
        } else {
            list.push(DrawCommand::FrameRect {
                rect,
                line_width: frame_width,
                colour,
            });
        }
        if shows_digits {
            digits.push((*bit, x));
        }
    }
    painter.canvas.draw(&list);
    let cell_width = painter.metrics.cell.width();
    let digit_top = top + size.saturating_sub(painter.row_height()) / 2;
    for (bit, x) in digits {
        let (text, ink) = if bit {
            ("1", painter.theme.window)
        } else {
            ("0", painter.theme.text)
        };
        painter.text(
            text,
            x + size.saturating_sub(cell_width) / 2,
            digit_top,
            ink,
        )?;
    }
    Ok(())
}

fn axis_ticks(axis: &ViewAxis, room: &TickRoom, separator: char) -> Vec<Tick> {
    match axis.scale {
        Scale::Linear => linear_ticks(&axis.range, room, separator),
        Scale::Logarithmic => logarithmic_ticks(&axis.range, room, separator),
    }
}

pub(crate) fn draw_plane_axes(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    titles: Option<&Vec<String>>,
    joiner: &str,
) -> Result<Vec<Option<PhysicalRect>>, RenderError> {
    let area = plane.area();
    let theme = painter.theme;
    let line = painter.line_width(THIN_WIDTH_LOGICAL);
    let line_pixels = pixel_floor(f64::from(line));
    let frame = PhysicalRect::new(
        area.x.saturating_sub(line_pixels),
        area.y.saturating_sub(line_pixels),
        area.width.saturating_add(2 * line_pixels),
        area.height.saturating_add(2 * line_pixels),
    );
    let mut list = DrawList::new();
    list.push(DrawCommand::FrameRect {
        rect: frame,
        line_width: line_pixels,
        colour: theme.line,
    });
    let x_room = TickRoom::beside_labels(
        u32::from(area.width),
        painter.metrics.cell.width(),
        TICK_LABEL_SPACING_CELLS,
    );
    let x_ticks = axis_ticks(&plane.view.x, &x_room, painter.separator);
    let y_room = TickRoom::spaced(
        u32::from(area.height),
        Y_TICK_SPACING_ROWS * u32::from(painter.row_height()),
    );
    let y_ticks = axis_ticks(&plane.view.y, &y_room, painter.separator);
    let tick_length = plane.tick_length;
    let mut labels = Vec::new();
    for tick in &x_ticks {
        let Some(x) = plane.x.of_exact(&plane.view.x, &tick.value) else {
            continue;
        };
        let x = pixel_floor(x);
        if x < area.x || x > area.right() {
            continue;
        }
        list.push(DrawCommand::FillRect {
            rect: PhysicalRect::new(
                x.min(area.right().saturating_sub(line_pixels)),
                frame.bottom(),
                line_pixels,
                tick_length,
            ),
            colour: theme.line,
        });
        if let Some(row) = plane.layout.tick_label_row {
            let width = u16::try_from(painter.text_width(&tick.label)).unwrap_or(u16::MAX);
            let left = x.saturating_sub(width / 2);
            labels.push((tick.label.clone(), left, row.y));
        }
    }
    for tick in &y_ticks {
        let Some(y) = plane.y.of_exact(&plane.view.y, &tick.value) else {
            continue;
        };
        let y = pixel_floor(y);
        if y < area.y || y > area.bottom() {
            continue;
        }
        let tick_left = frame.x.saturating_sub(tick_length);
        list.push(DrawCommand::FillRect {
            rect: PhysicalRect::new(
                tick_left,
                y.min(area.bottom().saturating_sub(line_pixels)),
                tick_length,
                line_pixels,
            ),
            colour: theme.line,
        });
        if let Some(gutter) = plane.layout.gutter {
            let width = u16::try_from(painter.text_width(&tick.label)).unwrap_or(u16::MAX);
            let left = gutter.right().saturating_sub(width);
            let top = y.saturating_sub(painter.row_height() / 2);
            labels.push((tick.label.clone(), left, top));
        }
    }
    painter.canvas.draw(&list);
    for (label, left, top) in labels {
        painter.text(&label, left, top, theme.text_secondary)?;
    }
    let title = |axis: usize| titles.and_then(|titles| titles.get(axis));
    let symbols = [plane.view.x.unit.symbol(), plane.view.y.unit.symbol()];
    let x_title = joined_title(title(0), joiner, symbols[0]);
    let y_title = joined_title(title(1), joiner, symbols[1]);
    let places = plane_title_places(
        &painter.placing(),
        &plane.layout,
        [&x_title, &y_title],
        symbols,
    );
    let slots = [
        TextRow::AxisTitle { view: plane.index },
        TextRow::ViewTitle { view: plane.index },
    ];
    for ((title, place), slot) in [&x_title, &y_title].into_iter().zip(places).zip(slots) {
        painter.draw_placed(title, place, theme.text, slot)?;
    }
    Ok(places.iter().map(|place| place.unit).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_scenes::{axis, f64s, interval};

    #[test]
    fn step_is_the_visible_width_over_the_divisions() {
        assert_eq!(step_of(&axis(0, 10)), 0.05);
    }

    #[test]
    fn bound_of_at_least_half_a_step_is_unresolved() {
        assert!(is_unresolved(Some(&f64s(&[0.5])), 0, 1.0));
    }

    #[test]
    fn bound_below_half_a_step_is_resolved() {
        assert!(!is_unresolved(Some(&f64s(&[0.49])), 0, 1.0));
    }

    #[test]
    fn unknown_bound_is_not_unresolved() {
        assert!(!is_unresolved(Some(&f64s(&[f64::NAN])), 0, 1.0));
    }

    #[test]
    fn cell_edges_split_the_region_evenly() {
        let view = axis(0, 10);
        let map = AxisMap::new(&view, 0.0, 100.0, false).unwrap();

        let edges = cell_edges(&map, &view, &interval(0, 10), 4);

        assert_eq!(
            edges,
            vec![Some(0.0), Some(25.0), Some(50.0), Some(75.0), Some(100.0)]
        );
    }

    #[test]
    fn pixel_span_is_ordered_and_clamped_to_the_area() {
        assert_eq!(pixel_span(Some(40.4), Some(-3.0), 10, 30), Some((10, 30)));
    }

    #[test]
    fn empty_pixel_span_is_absent() {
        assert_eq!(pixel_span(Some(12.2), Some(12.4), 0, 30), None);
    }

    #[test]
    fn domain_colouring_is_not_a_scalar_map() {
        let mapping = ColourMapping {
            map: ColourMap::DomainColouring,
            range: interval(0, 1),
        };
        let position = LayerPosition { frame: 0, layer: 2 };

        assert_eq!(
            scalar_colour(&mapping, 0.5, position),
            Err(RenderError::ColourMapKindMismatch(position))
        );
    }

    #[test]
    fn scalar_outside_the_map_takes_the_colour_of_its_end() {
        let mapping = ColourMapping {
            map: ColourMap::Sequential,
            range: interval(0, 1),
        };
        let position = LayerPosition { frame: 0, layer: 0 };

        assert_eq!(
            scalar_colour(&mapping, 7.0, position),
            scalar_colour(&mapping, 1.0, position)
        );
    }
}
