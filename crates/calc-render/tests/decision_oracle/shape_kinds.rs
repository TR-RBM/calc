use calc_viz::{
    Arrows, ColourMap, Column, Emphasis, Graph, Marker, Points, PrecisionLimit, Primitive, Scene,
    TriangleMesh, sequential_colour,
};

use crate::pixels::{LINE_REACH, Mapping, differs, is_drawn_near, share_matching};
use crate::rendering::{Picture, render};
use crate::scenes::{
    layer, line_style, polyline_layer, scalar_grid_layer, unit_interval, unit_scene,
    without_layers, zero_bounds,
};

const ROW_HEIGHT: f64 = 0.37;
const GRID_VALUE: f64 = 0.3;
const UNRESOLVED_GRID_BOUND: f64 = 0.01;
const SMALLEST_SHARE_OF_REFERENCE_COLOUR: f64 = 0.1;

fn any_pixel_differs(picture: &Picture, reference: &Picture) -> bool {
    (0..picture.height)
        .any(|row| (0..picture.width).any(|column| differs(picture, reference, column, row)))
}

fn draws_outside_the_plot_area(scene: &Scene) -> bool {
    let picture = render(scene);
    let empty = render(&without_layers(scene));
    let plot = picture.plot_area;
    let is_inside = |column: u32, row: u32| {
        column >= plot.left
            && column < plot.left + plot.width
            && row >= plot.top
            && row < plot.top + plot.height
    };
    (0..picture.height).any(|row| {
        (0..picture.width)
            .any(|column| !is_inside(column, row) && differs(&picture, &empty, column, row))
    })
}

fn draws_nothing_in_the_plot_area(scene: &Scene) -> bool {
    let picture = render(scene);
    let empty = render(&without_layers(scene));
    let plot = picture.plot_area;
    !(plot.top..plot.top + plot.height).any(|row| {
        (plot.left..plot.left + plot.width).any(|column| differs(&picture, &empty, column, row))
    })
}

fn columns(x: &[f64], y: &[f64]) -> Vec<Column> {
    vec![Column::F64(x.to_vec()), Column::F64(y.to_vec())]
}

#[test]
fn triangle_with_a_nan_vertex_is_not_drawn() {
    let scene = unit_scene(vec![layer(
        Primitive::TriangleMesh(TriangleMesh {
            vertices: columns(&[0.2, 0.8, f64::NAN], &[0.2, 0.2, 0.8]),
            triangles: vec![[0, 1, 2]],
            scalar: None,
        }),
        line_style(),
        vec![zero_bounds(3)],
    )]);

    assert!(draws_nothing_in_the_plot_area(&scene));
}

#[test]
fn point_with_a_nan_coordinate_is_not_drawn() {
    let scene = unit_scene(vec![layer(
        Primitive::Points(Points {
            coordinates: columns(&[f64::NAN], &[ROW_HEIGHT]),
            scalar: None,
        }),
        calc_viz::StyleRole {
            marker: Marker::Dot,
            ..line_style()
        },
        vec![zero_bounds(1)],
    )]);

    assert!(draws_nothing_in_the_plot_area(&scene));
}

fn arrow_picture() -> (Picture, Picture, Mapping) {
    let scene = unit_scene(vec![layer(
        Primitive::Arrows(Arrows {
            bases: columns(&[0.3], &[ROW_HEIGHT]),
            components: columns(&[0.4], &[0.0]),
        }),
        line_style(),
        vec![zero_bounds(1), zero_bounds(1)],
    )]);
    let picture = render(&scene);
    let empty = render(&without_layers(&scene));
    let mapping = Mapping::unit(picture.plot_area);
    (picture, empty, mapping)
}

#[test]
fn arrow_is_drawn_from_its_base_along_its_components() {
    let (picture, empty, mapping) = arrow_picture();

    let undrawn: Vec<u32> = mapping
        .columns_between(0.35, 0.65)
        .filter(|column| {
            !is_drawn_near(
                &picture,
                &empty,
                *column,
                mapping.rows_near(ROW_HEIGHT, LINE_REACH),
            )
        })
        .collect();

    assert!(
        undrawn.is_empty(),
        "columns along the arrow without drawing: {undrawn:?}"
    );
}

#[test]
fn arrow_is_not_drawn_behind_its_base() {
    let (picture, empty, mapping) = arrow_picture();

    let drawn: Vec<u32> = mapping
        .columns_between(0.0, 0.25)
        .filter(|column| {
            is_drawn_near(
                &picture,
                &empty,
                *column,
                mapping.rows_near(ROW_HEIGHT, LINE_REACH),
            )
        })
        .collect();

    assert!(
        drawn.is_empty(),
        "columns behind the base with drawing: {drawn:?}"
    );
}

#[test]
fn graph_edge_is_drawn_between_its_nodes() {
    let scene = unit_scene(vec![layer(
        Primitive::Graph(Graph {
            positions: columns(&[0.2, 0.8], &[ROW_HEIGHT, ROW_HEIGHT]),
            edges: vec![[0, 1]],
            is_directed: false,
        }),
        line_style(),
        Vec::new(),
    )]);
    let picture = render(&scene);
    let empty = render(&without_layers(&scene));
    let mapping = Mapping::unit(picture.plot_area);

    let undrawn: Vec<u32> = mapping
        .columns_between(0.35, 0.65)
        .filter(|column| {
            !is_drawn_near(
                &picture,
                &empty,
                *column,
                mapping.rows_near(ROW_HEIGHT, LINE_REACH),
            )
        })
        .collect();

    assert!(
        undrawn.is_empty(),
        "columns along the edge without drawing: {undrawn:?}"
    );
}

#[test]
fn unresolved_grid_cell_is_not_drawn_in_its_map_colour() {
    let mut grid = scalar_grid_layer([1, 1], vec![GRID_VALUE], ColourMap::Sequential);
    grid.value_bounds = vec![Column::F64(vec![UNRESOLVED_GRID_BOUND])];
    grid.precision = Some(PrecisionLimit {
        grid_exhausted: Vec::new(),
        unresolved_samples: 1,
        unknown_bounds: 0,
        marked_columns: 0,
        varies_below_bounds: false,
    });
    let picture = render(&unit_scene(vec![grid]));
    let mapping = Mapping::unit(picture.plot_area);
    let reference = sequential_colour(&unit_interval(), GRID_VALUE).expect("value in range");

    let share = share_matching(
        &picture,
        mapping.columns_between(0.0, 1.0),
        mapping.rows_between(0.0, 1.0),
        reference,
    );

    assert!(
        share < SMALLEST_SHARE_OF_REFERENCE_COLOUR,
        "share in the map colour: {share}"
    );
}

#[test]
fn picture_awaiting_its_bounds_is_drawn_in_the_provisional_role() {
    let normal = unit_scene(vec![polyline_layer(
        vec![0.0, 1.0],
        vec![ROW_HEIGHT, ROW_HEIGHT],
    )]);
    let mut provisional_layer = polyline_layer(vec![0.0, 1.0], vec![ROW_HEIGHT, ROW_HEIGHT]);
    provisional_layer.value_bounds = Vec::new();
    provisional_layer.style.emphasis = Emphasis::Provisional;
    let provisional = unit_scene(vec![provisional_layer]);

    assert!(any_pixel_differs(&render(&provisional), &render(&normal)));
}

fn removed_point_scene() -> Scene {
    unit_scene(vec![layer(
        Primitive::Points(Points {
            coordinates: columns(&[f64::NAN], &[ROW_HEIGHT]),
            scalar: None,
        }),
        calc_viz::StyleRole {
            marker: Marker::Dot,
            ..line_style()
        },
        vec![zero_bounds(1)],
    )])
}

fn removed_triangle_scene() -> Scene {
    unit_scene(vec![layer(
        Primitive::TriangleMesh(TriangleMesh {
            vertices: columns(&[0.2, 0.8, f64::NAN], &[0.2, 0.2, 0.8]),
            triangles: vec![[0, 1, 2]],
            scalar: None,
        }),
        line_style(),
        vec![zero_bounds(3)],
    )])
}

#[test]
fn point_removed_for_a_nan_coordinate_adds_a_key_swatch() {
    assert!(draws_outside_the_plot_area(&removed_point_scene()));
}

#[test]
fn triangle_removed_for_a_nan_vertex_adds_a_key_swatch() {
    assert!(draws_outside_the_plot_area(&removed_triangle_scene()));
}
