use calc_numbers::Number;
use calc_viz::{
    Arrows, Column, Formula, Graph, Layer, LayerPosition, Points, Polyline, Primitive, ScalarGrid,
    StyleRole, TriangleMesh, View3, ViewAxis, Voxels,
};
use tiny_skia::Mask;

use crate::axis_units::space_title_places;
use crate::clip::{Guard, polyline_path};
use crate::depth::{DepthBuffer, DepthPoint};
use crate::geometry::PhysicalRect;
use crate::layout::ViewLayout;
use crate::mapping::{AxisMap, ScreenPoint, SpaceProjection, column_value};
use crate::plane::{
    GridStyle, bound_at, cell_edges, draw_arrow, draw_graph_nodes, draw_marker, draw_text_at,
    has_grid_limit, is_unresolved, line_dash, mesh_fill, step_of, unresolved_arrow,
};
use crate::render::{Painter, RenderError, TextRow, joined_title, view_kind_mismatch};
use crate::roles::{self, Fill, Patterns, Role};
use crate::ticks::{TickRoom, linear_ticks};

const SPACE_AXES: usize = 3;
const CURVE_WIDTH_LOGICAL: u16 = 2;
const THIN_WIDTH_LOGICAL: u16 = 1;
const HALF_CELL: f64 = 0.5;
const CUBE_LOWER: f64 = -1.0;
const CUBE_UPPER: f64 = 1.0;
const CUBE_SIDE: f64 = 2.0;
const EDGE_TICKS: u32 = 4;

pub(crate) struct SpaceView<'a> {
    pub view: &'a View3,
    pub index: usize,
    pub layout: ViewLayout,
    pub maps: [AxisMap; 3],
    pub projection: SpaceProjection,
    pub clip: Option<Mask>,
    pub guard: Guard,
    pub patterns: Patterns,
}

impl<'a> SpaceView<'a> {
    pub(crate) fn new(
        painter: &Painter<'_>,
        view: &'a View3,
        layout: &ViewLayout,
        index: usize,
    ) -> Result<SpaceView<'a>, RenderError> {
        let area = layout.plot_area;
        let map = |axis: &ViewAxis, number: usize| {
            AxisMap::new(axis, CUBE_LOWER, CUBE_SIDE, false).ok_or(RenderError::AxisNotMappable {
                view: index,
                axis: number,
            })
        };
        let maps = [map(&view.x, 0)?, map(&view.y, 1)?, map(&view.z, 2)?];
        let label_room =
            f64::from(painter.row_height()) + f64::from(painter.metrics.cell.width()) * 3.0;
        let half = f64::from(area.width.min(area.height)) / 2.0 - label_room;
        let centre = (
            f64::from(area.x) + f64::from(area.width) / 2.0,
            f64::from(area.y) + f64::from(area.height) / 2.0,
        );
        let half = if half > 1.0 { half } else { 1.0 };
        Ok(SpaceView {
            view,
            index,
            layout: *layout,
            maps,
            projection: SpaceProjection::new(&view.camera, centre, half),
            clip: painter.canvas.clip_mask(area),
            guard: Guard::around(area),
            patterns: Patterns::new(painter.theme, painter.scale, (area.x, area.y)),
        })
    }

    fn axes(&self) -> [&ViewAxis; 3] {
        [&self.view.x, &self.view.y, &self.view.z]
    }

    fn normalized(&self, columns: &[Column], index: usize) -> Option<[f64; 3]> {
        let [xs, ys, zs] = columns else {
            return None;
        };
        Some([
            self.maps[0].of_column(xs, index)?,
            self.maps[1].of_column(ys, index)?,
            self.maps[2].of_column(zs, index)?,
        ])
    }

    fn project(&self, point: [f64; 3]) -> Option<DepthPoint> {
        self.projection.project(point)
    }

    fn screen(&self, columns: &[Column], index: usize) -> Option<ScreenPoint> {
        let (x, y, _) = self.project(self.normalized(columns, index)?)?;
        Some((x, y))
    }
}

pub(crate) fn draw_space_layer(
    painter: &mut Painter<'_>,
    space: &SpaceView<'_>,
    depth: &mut DepthBuffer,
    layer: &Layer,
    position: LayerPosition,
) -> Result<(), RenderError> {
    match &layer.primitive {
        Primitive::Polyline(polyline) => draw_polyline(painter, space, layer, polyline),
        Primitive::Points(points) => draw_points(painter, space, layer, points, position),
        Primitive::Arrows(arrows) => draw_arrows(painter, space, layer, arrows),
        Primitive::ScalarGrid(grid) => draw_volume(painter, space, depth, layer, grid, position),
        Primitive::TriangleMesh(mesh) => draw_mesh(painter, space, depth, layer, mesh, position),
        Primitive::Voxels(voxels) => draw_voxels(painter, space, depth, layer, voxels, position),
        Primitive::Graph(graph) => draw_graph(painter, space, layer, graph),
        Primitive::Formula(formula) => draw_formula(painter, space, layer, formula),
        Primitive::Band(_)
        | Primitive::ComplexGrid(_)
        | Primitive::BitLayout(_)
        | Primitive::Figure(_) => Err(view_kind_mismatch(position, SPACE_AXES)),
    }
}

fn draw_polyline(
    painter: &mut Painter<'_>,
    space: &SpaceView<'_>,
    layer: &Layer,
    polyline: &Polyline,
) -> Result<(), RenderError> {
    let colour = roles::layer_colour(&painter.theme, &layer.style);
    let width = painter.line_width(CURVE_WIDTH_LOGICAL);
    let bounds = layer.value_bounds.first();
    let step = step_of(&space.view.z);
    let length = polyline.coordinates.first().map_or(0, Column::len);
    let mut runs: Vec<Vec<ScreenPoint>> = vec![Vec::new()];
    let mut enclosures: Vec<Vec<ScreenPoint>> = Vec::new();
    for index in 0..length {
        let point = space.normalized(&polyline.coordinates, index);
        let Some(normalized) = point else {
            runs.push(Vec::new());
            continue;
        };
        if is_unresolved(bounds, index, step) {
            runs.push(Vec::new());
            let value = polyline
                .coordinates
                .get(2)
                .and_then(|zs| column_value(zs, index));
            let bound = bound_at(bounds, index);
            if let (Some(value), Some(bound)) = (value, bound) {
                let ends = [value - bound, value + bound].map(|z| {
                    space.maps[2]
                        .of_f64(z)
                        .and_then(|z| space.project([normalized[0], normalized[1], z]))
                        .map(|(x, y, _)| (x, y))
                });
                if let [Some(lower), Some(upper)] = ends {
                    enclosures.push(vec![lower, upper]);
                }
            }
            continue;
        }
        if let (Some((x, y, _)), Some(run)) = (space.project(normalized), runs.last_mut()) {
            run.push((x, y));
        }
    }
    let clip = space.clip.as_ref();
    if !enclosures.is_empty() {
        painter.show(Role::Unresolved);
    }
    if let Some(path) = polyline_path(&space.guard, &enclosures) {
        let edge = roles::settled(
            &painter.theme,
            &layer.style,
            roles::role_tone(&painter.theme, Role::Unresolved),
        );
        let thin = painter.line_width(THIN_WIDTH_LOGICAL);
        painter
            .canvas
            .stroke_path_within(&path, edge, thin, None, clip);
    }
    if let Some(dash) = line_dash(painter, layer.style.line, width)?
        && let Some(path) = polyline_path(&space.guard, &runs)
    {
        painter
            .canvas
            .stroke_path_within(&path, colour, width, dash, clip);
    }
    Ok(())
}

fn draw_points(
    painter: &mut Painter<'_>,
    space: &SpaceView<'_>,
    layer: &Layer,
    points: &Points,
    position: LayerPosition,
) -> Result<(), RenderError> {
    let bounds = layer.value_bounds.first();
    let step = step_of(&space.view.z);
    let grid_limit = has_grid_limit(layer);
    let length = points.coordinates.first().map_or(0, Column::len);
    for index in 0..length {
        let Some(point) = space.screen(&points.coordinates, index) else {
            continue;
        };
        if !space.guard.contains(point) {
            continue;
        }
        let fill = if grid_limit || is_unresolved(bounds, index, step) {
            Fill::Role(Role::Unresolved)
        } else {
            mesh_fill(painter, layer, points.scalar.as_ref(), [index; 3], position)?
        };
        draw_marker(
            painter,
            layer.style.marker,
            point,
            (fill, &space.patterns, &layer.style),
            space.clip.as_ref(),
        )?;
    }
    Ok(())
}

fn draw_arrows(
    painter: &mut Painter<'_>,
    space: &SpaceView<'_>,
    layer: &Layer,
    arrows: &Arrows,
) -> Result<(), RenderError> {
    let steps = [
        step_of(&space.view.x),
        step_of(&space.view.y),
        step_of(&space.view.z),
    ];
    let length = arrows.bases.first().map_or(0, Column::len);
    for index in 0..length {
        let Some(base) = space.screen(&arrows.bases, index) else {
            continue;
        };
        let tip: Option<Vec<f64>> = arrows
            .bases
            .iter()
            .zip(&arrows.components)
            .zip(&space.maps)
            .map(|((bases, components), map)| match (bases, components) {
                (Column::F32(bases), Column::F32(components)) => {
                    map.of_f32(*bases.get(index)? + *components.get(index)?)
                }
                _ => map.of_f64(column_value(bases, index)? + column_value(components, index)?),
            })
            .collect();
        let Some([x, y, z]) = tip
            .as_deref()
            .and_then(|tip| <[f64; 3]>::try_from(tip).ok())
        else {
            continue;
        };
        let Some((tip_x, tip_y, _)) = space.project([x, y, z]) else {
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
            &space.guard,
            (base, (tip_x, tip_y)),
            stroke,
            space.clip.as_ref(),
        )?;
    }
    Ok(())
}

fn fill_box(
    painter: &mut Painter<'_>,
    space: &SpaceView<'_>,
    depth: &mut DepthBuffer,
    (lower, upper): ([f64; 3], [f64; 3]),
    (fill, style): (Fill, &StyleRole),
) {
    if let Fill::Role(role) = fill {
        painter.show(role);
    }
    let theme = painter.theme;
    let patterns = space.patterns;
    let paint = |x: u16, y: u16| roles::settled(&theme, style, patterns.covering(fill, x, y));
    let corner = |x: bool, y: bool, z: bool| {
        [
            if x { upper[0] } else { lower[0] },
            if y { upper[1] } else { lower[1] },
            if z { upper[2] } else { lower[2] },
        ]
    };
    let faces: [([f64; 3], [[bool; 3]; 4]); 6] = [
        (
            [-1.0, 0.0, 0.0],
            [
                [false, false, false],
                [false, true, false],
                [false, true, true],
                [false, false, true],
            ],
        ),
        (
            [1.0, 0.0, 0.0],
            [
                [true, false, false],
                [true, true, false],
                [true, true, true],
                [true, false, true],
            ],
        ),
        (
            [0.0, -1.0, 0.0],
            [
                [false, false, false],
                [true, false, false],
                [true, false, true],
                [false, false, true],
            ],
        ),
        (
            [0.0, 1.0, 0.0],
            [
                [false, true, false],
                [true, true, false],
                [true, true, true],
                [false, true, true],
            ],
        ),
        (
            [0.0, 0.0, -1.0],
            [
                [false, false, false],
                [true, false, false],
                [true, true, false],
                [false, true, false],
            ],
        ),
        (
            [0.0, 0.0, 1.0],
            [
                [false, false, true],
                [true, false, true],
                [true, true, true],
                [false, true, true],
            ],
        ),
    ];
    for (normal, corners) in faces {
        let points = corners.map(|[x, y, z]| corner(x, y, z));
        if !space.projection.faces_viewer(normal, points[0]) {
            continue;
        }
        let projected: Option<Vec<DepthPoint>> =
            points.iter().map(|point| space.project(*point)).collect();
        if let Some([first, second, third, fourth]) = projected
            .as_deref()
            .and_then(|corners| <[DepthPoint; 4]>::try_from(corners).ok())
        {
            depth.fill_triangle(painter.canvas, [first, second, third], &paint);
            depth.fill_triangle(painter.canvas, [first, third, fourth], &paint);
        }
    }
}

fn draw_volume(
    painter: &mut Painter<'_>,
    space: &SpaceView<'_>,
    depth: &mut DepthBuffer,
    layer: &Layer,
    grid: &ScalarGrid,
    position: LayerPosition,
) -> Result<(), RenderError> {
    let style = GridStyle::new(layer, position)?;
    let ([x_region, y_region, z_region], [x_count, y_count, z_count]) =
        (grid.cells.region.as_slice(), grid.cells.counts.as_slice())
    else {
        return Err(view_kind_mismatch(position, SPACE_AXES));
    };
    let edges = [
        cell_edges(&space.maps[0], &space.view.x, x_region, *x_count),
        cell_edges(&space.maps[1], &space.view.y, y_region, *y_count),
        cell_edges(&space.maps[2], &space.view.z, z_region, *z_count),
    ];
    let columns = usize::try_from(*x_count).unwrap_or(0);
    let rows = usize::try_from(*y_count).unwrap_or(0);
    for (layer_index, z_pair) in edges[2].windows(2).enumerate() {
        for (row, y_pair) in edges[1].windows(2).enumerate() {
            for (column, x_pair) in edges[0].windows(2).enumerate() {
                let bounds = [x_pair, y_pair, z_pair].map(|pair| match pair {
                    [Some(low), Some(high)] => Some((*low, *high)),
                    _ => None,
                });
                let [Some(x), Some(y), Some(z)] = bounds else {
                    continue;
                };
                let index = (layer_index * rows + row) * columns + column;
                let fill = style.cell_fill(painter, grid, index)?;
                fill_box(
                    painter,
                    space,
                    depth,
                    ([x.0, y.0, z.0], [x.1, y.1, z.1]),
                    (fill, &layer.style),
                );
            }
        }
    }
    Ok(())
}

fn draw_mesh(
    painter: &mut Painter<'_>,
    space: &SpaceView<'_>,
    depth: &mut DepthBuffer,
    layer: &Layer,
    mesh: &TriangleMesh,
    position: LayerPosition,
) -> Result<(), RenderError> {
    let bounds = layer.value_bounds.first();
    let step = step_of(&space.view.z);
    for triangle in &mesh.triangles {
        let indices = triangle.map(|index| usize::try_from(index).unwrap_or(usize::MAX));
        let normalized = indices.map(|index| space.normalized(&mesh.vertices, index));
        let [Some(first), Some(second), Some(third)] = normalized else {
            painter.show(Role::Missing);
            continue;
        };
        let along = [
            second[0] - first[0],
            second[1] - first[1],
            second[2] - first[2],
        ];
        let across = [
            third[0] - first[0],
            third[1] - first[1],
            third[2] - first[2],
        ];
        let normal = [
            along[1] * across[2] - along[2] * across[1],
            along[2] * across[0] - along[0] * across[2],
            along[0] * across[1] - along[1] * across[0],
        ];
        let fill = if !space.projection.faces_viewer(normal, first) {
            Fill::Role(Role::BackFace)
        } else if indices
            .iter()
            .any(|index| is_unresolved(bounds, *index, step))
        {
            Fill::Role(Role::Unresolved)
        } else {
            mesh_fill(painter, layer, mesh.scalar.as_ref(), indices, position)?
        };
        if let Fill::Role(role) = fill {
            painter.show(role);
        }
        let theme = painter.theme;
        let patterns = space.patterns;
        let style = &layer.style;
        let inside_box = cut_at_box(&[first, second, third]);
        let projected: Option<Vec<DepthPoint>> = inside_box
            .iter()
            .map(|point| space.project(*point))
            .collect();
        let Some(projected) = projected else {
            continue;
        };
        let Some((anchor, rest)) = projected.split_first() else {
            continue;
        };
        for pair in rest.windows(2) {
            let [second_corner, third_corner] = pair else {
                continue;
            };
            depth.fill_triangle(
                painter.canvas,
                [*anchor, *second_corner, *third_corner],
                &|x, y| roles::settled(&theme, style, patterns.covering(fill, x, y)),
            );
        }
    }
    Ok(())
}

fn cut_at_box(corners: &[[f64; 3]]) -> Vec<[f64; 3]> {
    let mut polygon = corners.to_vec();
    for axis in 0..SPACE_AXES {
        for (bound, keeps) in [(CUBE_LOWER, true), (CUBE_UPPER, false)] {
            let is_inside = |point: &[f64; 3]| {
                if keeps {
                    point[axis] >= bound
                } else {
                    point[axis] <= bound
                }
            };
            let input = std::mem::take(&mut polygon);
            let Some(mut previous) = input.last().copied() else {
                return Vec::new();
            };
            for point in input {
                let crossing = |from: [f64; 3], to: [f64; 3]| {
                    let fraction = (bound - from[axis]) / (to[axis] - from[axis]);
                    let mut cut = [0.0; 3];
                    for (index, slot) in cut.iter_mut().enumerate() {
                        *slot = from[index] + (to[index] - from[index]) * fraction;
                    }
                    cut[axis] = bound;
                    cut
                };
                match (is_inside(&previous), is_inside(&point)) {
                    (true, true) => polygon.push(point),
                    (true, false) => polygon.push(crossing(previous, point)),
                    (false, true) => {
                        polygon.push(crossing(previous, point));
                        polygon.push(point);
                    }
                    (false, false) => {}
                }
                previous = point;
            }
        }
    }
    polygon
}

fn draw_voxels(
    painter: &mut Painter<'_>,
    space: &SpaceView<'_>,
    depth: &mut DepthBuffer,
    layer: &Layer,
    voxels: &Voxels,
    position: LayerPosition,
) -> Result<(), RenderError> {
    let bounds = layer.value_bounds.first();
    for (index, cell) in voxels.occupied.iter().enumerate() {
        let edges: Option<Vec<(f64, f64)>> = cell
            .iter()
            .zip(&space.maps)
            .map(|(coordinate, map)| {
                let centre = Number::from(*coordinate).round_to_f64_ties_even();
                Some((
                    map.of_f64(centre - HALF_CELL)?,
                    map.of_f64(centre + HALF_CELL)?,
                ))
            })
            .collect();
        let Some([x, y, z]) = edges
            .as_deref()
            .and_then(|edges| <[(f64, f64); 3]>::try_from(edges).ok())
        else {
            continue;
        };
        let value = voxels
            .scalar
            .as_ref()
            .and_then(|scalar| column_value(scalar, index));
        let unresolved = match (value, bound_at(bounds, index)) {
            (Some(value), Some(bound)) => bound >= value.abs(),
            _ => false,
        };
        let fill = if unresolved {
            Fill::Role(Role::Unresolved)
        } else if voxels.scalar.is_some() && !value.is_some_and(f64::is_finite) {
            Fill::Role(Role::Missing)
        } else {
            mesh_fill(painter, layer, voxels.scalar.as_ref(), [index; 3], position)?
        };
        fill_box(
            painter,
            space,
            depth,
            ([x.0, y.0, z.0], [x.1, y.1, z.1]),
            (fill, &layer.style),
        );
    }
    Ok(())
}

fn draw_graph(
    painter: &mut Painter<'_>,
    space: &SpaceView<'_>,
    layer: &Layer,
    graph: &Graph,
) -> Result<(), RenderError> {
    let length = graph.positions.first().map_or(0, Column::len);
    let nodes: Vec<Option<ScreenPoint>> = (0..length)
        .map(|index| space.screen(&graph.positions, index))
        .collect();
    draw_graph_nodes(
        painter,
        &space.guard,
        space.clip.as_ref(),
        layer,
        graph,
        &nodes,
    )
}

fn draw_formula(
    painter: &mut Painter<'_>,
    space: &SpaceView<'_>,
    layer: &Layer,
    formula: &Formula,
) -> Result<(), RenderError> {
    let anchor: Option<Vec<f64>> = formula
        .anchor
        .iter()
        .zip(space.axes())
        .zip(&space.maps)
        .map(|((value, axis), map)| map.of_exact(axis, value))
        .collect();
    let Some([x, y, z]) = anchor
        .as_deref()
        .and_then(|anchor| <[f64; 3]>::try_from(anchor).ok())
    else {
        return Ok(());
    };
    let Some((screen_x, screen_y, _)) = space.project([x, y, z]) else {
        return Ok(());
    };
    let colour = roles::emphasised(&painter.theme, &layer.style, painter.theme.text);
    draw_text_at(
        painter,
        space.layout.plot_area,
        &formula.expression,
        (screen_x, screen_y),
        colour,
    )
}

pub(crate) fn draw_space_axes(
    painter: &mut Painter<'_>,
    space: &SpaceView<'_>,
    titles: Option<&Vec<String>>,
    joiner: &str,
) -> Result<Vec<Option<PhysicalRect>>, RenderError> {
    let theme = painter.theme;
    let width = painter.line_width(THIN_WIDTH_LOGICAL);
    let corners = [CUBE_LOWER, CUBE_UPPER];
    let mut edges: Vec<Vec<ScreenPoint>> = Vec::new();
    for axis in 0..SPACE_AXES {
        for first in corners {
            for second in corners {
                let point = |along: f64| {
                    let mut point = [0.0; 3];
                    point[axis] = along;
                    point[(axis + 1) % SPACE_AXES] = first;
                    point[(axis + 2) % SPACE_AXES] = second;
                    space.project(point).map(|(x, y, _)| (x, y))
                };
                if let (Some(start), Some(end)) = (point(CUBE_LOWER), point(CUBE_UPPER)) {
                    edges.push(vec![start, end]);
                }
            }
        }
    }
    if let Some(path) = polyline_path(&space.guard, &edges) {
        painter
            .canvas
            .stroke_path_within(&path, theme.line, width, None, space.clip.as_ref());
    }
    let centre = space.project([0.0, 0.0, 0.0]).map(|(x, y, _)| (x, y));
    for (axis_index, (axis, map)) in space.axes().into_iter().zip(&space.maps).enumerate() {
        for tick in linear_ticks(
            &axis.range,
            &TickRoom::at_most(EDGE_TICKS),
            painter.separator,
        ) {
            let Some(along) = map.of_exact(axis, &tick.value) else {
                continue;
            };
            let mut point = [CUBE_LOWER; 3];
            point[axis_index] = along;
            let (Some((x, y, _)), Some(centre)) = (space.project(point), centre) else {
                continue;
            };
            let away = (x - centre.0, y - centre.1);
            let distance = (away.0 * away.0 + away.1 * away.1).sqrt();
            if distance == 0.0 {
                continue;
            }
            let offset = f64::from(painter.row_height());
            let label_centre = (
                x + away.0 / distance * offset,
                y + away.1 / distance * offset,
            );
            let label_width = f64::from(painter.text_width(&tick.label));
            draw_text_at(
                painter,
                space.layout.plot_area,
                &tick.label,
                (label_centre.0 - label_width / 2.0, label_centre.1),
                theme.text_secondary,
            )?;
        }
    }
    let title_text: Vec<String> = space
        .axes()
        .into_iter()
        .enumerate()
        .map(|(index, axis)| {
            joined_title(
                titles.and_then(|titles| titles.get(index)),
                joiner,
                axis.unit.symbol(),
            )
        })
        .filter(|title| !title.is_empty())
        .collect();
    let symbols: Vec<String> = space
        .axes()
        .into_iter()
        .map(|axis| axis.unit.symbol().to_owned())
        .collect();
    let (joined, place, units) =
        space_title_places(&painter.placing(), &space.layout, &title_text, &symbols);
    painter.draw_placed(
        &joined,
        place,
        theme.text,
        TextRow::ViewTitle { view: space.index },
    )?;
    Ok(units)
}

#[cfg(test)]
mod tests {
    use calc_viz::View;

    use super::*;
    use crate::canvas::Canvas;
    use crate::colour::Colour;
    use crate::layout::{LayoutRequest, ViewKind, ViewRequest, plot_layout};
    use crate::test_scenes::space;
    use crate::text::font::FontSet;
    use crate::text::grid::grid_metrics;
    use crate::theme::Theme;

    const INK: Colour = Colour::opaque(200, 10, 10);

    fn canvas() -> Canvas {
        Canvas::new(400, 300, Theme::light().window).unwrap()
    }

    fn painter<'a>(canvas: &'a mut Canvas, fonts: &'a FontSet) -> Painter<'a> {
        Painter {
            canvas,
            fonts,
            metrics: grid_metrics(fonts, 14).unwrap(),
            theme: Theme::light(),
            scale: crate::geometry::ScaleFactor::from_percent(100).unwrap(),
            separator: '.',
            bounds: crate::geometry::PhysicalRect::new(0, 0, 400, 300),
            shown: Vec::new(),
            may_be_hit: None,
        }
    }

    fn view_layout(fonts: &FontSet) -> ViewLayout {
        plot_layout(
            &LayoutRequest {
                width: 400,
                height: 300,
                text_size: 14,
                scale: crate::geometry::ScaleFactor::from_percent(100).unwrap(),
                views: vec![ViewRequest::of_kind(ViewKind::Space)],
                legends: Vec::new(),
            },
            fonts,
        )
        .unwrap()
        .views[0]
    }

    #[test]
    fn box_seen_from_above_shows_its_top_face_at_the_centre() {
        let fonts = FontSet::bundled().unwrap();
        let mut canvas = canvas();
        let mut painter = painter(&mut canvas, &fonts);
        let layout = view_layout(&fonts);
        let View::View3(view) = space(-1, 1, 0, 90) else {
            unreachable!()
        };
        let space_view = SpaceView::new(&painter, &view, &layout, 0).unwrap();
        let mut depth = DepthBuffer::new(layout.plot_area);

        fill_box(
            &mut painter,
            &space_view,
            &mut depth,
            ([-0.5; 3], [0.5; 3]),
            (Fill::Solid(INK), &crate::test_scenes::style()),
        );

        let centre = (
            layout.plot_area.x + layout.plot_area.width / 2,
            layout.plot_area.y + layout.plot_area.height / 2,
        );
        assert_eq!(painter.canvas.pixel(centre.0, centre.1), Some(INK));
    }

    #[test]
    fn triangle_inside_the_box_is_kept_whole() {
        let corners = [[0.0, 0.0, 0.0], [0.5, 0.0, 0.0], [0.0, 0.5, 0.5]];

        assert_eq!(cut_at_box(&corners), corners.to_vec());
    }

    #[test]
    fn triangle_rising_above_the_box_is_cut_at_its_top_face() {
        let cut = cut_at_box(&[[0.0, 0.0, 0.0], [0.5, 0.0, 0.0], [0.0, 0.0, 3.0]]);

        assert!(cut.iter().all(|point| point[2] <= 1.0) && cut.len() == 4);
    }

    #[test]
    fn triangle_outside_the_box_vanishes() {
        assert!(cut_at_box(&[[0.0, 0.0, 2.0], [0.5, 0.0, 2.0], [0.0, 0.5, 3.0]]).is_empty());
    }

    #[test]
    fn normalized_coordinates_span_the_cube() {
        let fonts = FontSet::bundled().unwrap();
        let mut canvas = canvas();
        let painter = painter(&mut canvas, &fonts);
        let layout = view_layout(&fonts);
        let View::View3(view) = space(0, 4, 30, 20) else {
            unreachable!()
        };
        let space_view = SpaceView::new(&painter, &view, &layout, 0).unwrap();
        let columns = [
            calc_viz::Column::F64(vec![0.0, 4.0]),
            calc_viz::Column::F64(vec![2.0, 2.0]),
            calc_viz::Column::F64(vec![4.0, 0.0]),
        ];

        assert_eq!(
            (
                space_view.normalized(&columns, 0),
                space_view.normalized(&columns, 1)
            ),
            (Some([-1.0, 0.0, 1.0]), Some([1.0, 0.0, -1.0]))
        );
    }
}
