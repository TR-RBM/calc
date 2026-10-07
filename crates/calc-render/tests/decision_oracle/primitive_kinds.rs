use calc_numbers::Number;
use calc_viz::{
    Arrows, BitLayoutRequest, Camera, Column, FormulaRequest, Graph, KindColour, Points, Primitive,
    Projection, ResultId, Scene, TriangleMesh, View, View3, Voxels, bit_layout_scene,
    formula_scene,
};

use crate::pixels::differs;
use crate::rendering::{Picture, render};
use crate::scenes::{
    axis, fraction, integer, interval, layer, line_style, polyline_layer, scene_with,
    unit_interval, unit_scene, without_layers, zero_bounds,
};

const AZIMUTH_DEGREES: i64 = 30;
const ELEVATION_DEGREES: i64 = 20;
const VOXEL_BOX_UPPER: i64 = 4;
const ELEVEN_CELLS_AT_TEXT_SIZE: u32 = 11 * 14;

fn draws_something(scene: &Scene) -> bool {
    let picture = render(scene);
    let empty = render(&without_layers(scene));
    any_pixel_differs(&picture, &empty)
}

fn any_pixel_differs(picture: &Picture, reference: &Picture) -> bool {
    (0..picture.height)
        .any(|row| (0..picture.width).any(|column| differs(picture, reference, column, row)))
}

fn columns(values: [&[f64]; 2]) -> Vec<Column> {
    values
        .iter()
        .map(|column| Column::F64(column.to_vec()))
        .collect()
}

#[test]
fn points_are_drawn() {
    let scene = unit_scene(vec![layer(
        Primitive::Points(Points {
            coordinates: columns([&[0.3, 0.6], &[0.4, 0.7]]),
            scalar: None,
        }),
        calc_viz::StyleRole {
            marker: calc_viz::Marker::Dot,
            ..line_style()
        },
        vec![zero_bounds(2)],
    )]);

    assert!(draws_something(&scene));
}

#[test]
fn arrows_are_drawn() {
    let scene = unit_scene(vec![layer(
        Primitive::Arrows(Arrows {
            bases: columns([&[0.3], &[0.3]]),
            components: columns([&[0.4], &[0.2]]),
        }),
        line_style(),
        vec![zero_bounds(1), zero_bounds(1)],
    )]);

    assert!(draws_something(&scene));
}

#[test]
fn triangle_mesh_is_drawn() {
    let scene = unit_scene(vec![layer(
        Primitive::TriangleMesh(TriangleMesh {
            vertices: columns([&[0.2, 0.8, 0.5], &[0.2, 0.2, 0.8]]),
            triangles: vec![[0, 1, 2]],
            scalar: None,
        }),
        line_style(),
        vec![zero_bounds(3)],
    )]);

    assert!(draws_something(&scene));
}

#[test]
fn graph_is_drawn() {
    let scene = unit_scene(vec![layer(
        Primitive::Graph(Graph {
            positions: columns([&[0.2, 0.8], &[0.5, 0.5]]),
            edges: vec![[0, 1]],
            is_directed: true,
        }),
        line_style(),
        Vec::new(),
    )]);

    assert!(draws_something(&scene));
}

#[test]
fn voxels_are_drawn_in_a_space_view() {
    let view = View::View3(Box::new(View3 {
        x: axis(interval(integer(0), integer(VOXEL_BOX_UPPER))),
        y: axis(interval(integer(0), integer(VOXEL_BOX_UPPER))),
        z: axis(interval(integer(0), integer(VOXEL_BOX_UPPER))),
        camera: Camera {
            azimuth_degrees: integer(AZIMUTH_DEGREES),
            elevation_degrees: integer(ELEVATION_DEGREES),
            projection: Projection::Orthographic,
        },
    }));
    let scene = scene_with(
        vec![view],
        vec![layer(
            Primitive::Voxels(Voxels {
                occupied: vec![[1, 1, 1], [2, 1, 1]],
                scalar: None,
            }),
            line_style(),
            Vec::new(),
        )],
    );

    assert!(draws_something(&scene));
}

#[test]
fn formula_is_drawn() {
    let scene = formula_scene(&FormulaRequest {
        result: ResultId(0),
        text: "x^2 + 1".to_string(),
        anchor: [fraction(1, 2), fraction(1, 2)],
        view_x: unit_interval(),
        view_y: unit_interval(),
        kind: KindColour::Exact,
        references: Vec::new(),
    })
    .expect("formula scene");

    assert!(draws_something(&scene));
}

#[test]
fn bit_layout_is_drawn() {
    let scene = bit_layout_scene(&BitLayoutRequest {
        result: ResultId(0),
        expression_text: "1.5".to_string(),
        value: Number::F64(1.5),
        kind: KindColour::Numeric,
        references: Vec::new(),
    })
    .expect("bit layout scene");

    assert!(draws_something(&scene));
}

fn plot_area_of(scene: &Scene) -> crate::rendering::PixelRect {
    render(scene).plot_area
}

#[test]
fn plot_area_does_not_depend_on_the_samples() {
    let sparse = plot_area_of(&unit_scene(vec![polyline_layer(
        vec![0.0, 1.0],
        vec![0.5, 0.5],
    )]));
    let dense = plot_area_of(&unit_scene(vec![polyline_layer(
        (0..64).map(|index| f64::from(index) / 63.0).collect(),
        (0..64).map(|index| f64::from(index % 7) / 9.0).collect(),
    )]));

    assert_eq!(sparse, dense);
}

#[test]
fn a_settled_range_with_narrower_ticks_gives_a_wider_plot_area() {
    let narrow_ticks = render(&unit_scene(vec![polyline_layer(
        vec![0.0, 1.0],
        vec![0.5, 0.5],
    )]));
    let wide_ticks = render(&scene_with(
        vec![crate::scenes::plane_view(
            interval(integer(-1_000_000), integer(1_000_000)),
            interval(integer(-1_000_000), integer(1_000_000)),
        )],
        vec![polyline_layer(
            vec![-1_000_000.0, 1_000_000.0],
            vec![-999_999.5, 999_999.5],
        )],
    ));

    assert!(
        narrow_ticks.plot_area.width > wide_ticks.plot_area.width,
        "narrow ticks {:?}, wide ticks {:?}",
        narrow_ticks.plot_area,
        wide_ticks.plot_area
    );
}

#[test]
fn a_gutter_is_never_wider_than_the_eleven_cells_of_the_format() {
    let narrow_ticks = render(&unit_scene(vec![polyline_layer(
        vec![0.0, 1.0],
        vec![0.5, 0.5],
    )]));
    let widest = render(&scene_with(
        vec![crate::scenes::plane_view(
            interval(integer(-1_000_000_000), integer(1_000_000_000)),
            interval(integer(-1_000_000_000), integer(1_000_000_000)),
        )],
        vec![polyline_layer(
            vec![-1_000_000_000.0, 1_000_000_000.0],
            vec![-999_999_999.5, 999_999_999.5],
        )],
    ));

    assert!(
        widest.plot_area.left <= narrow_ticks.plot_area.left + ELEVEN_CELLS_AT_TEXT_SIZE,
        "widest gutter starts at {}, narrow at {}",
        widest.plot_area.left,
        narrow_ticks.plot_area.left
    );
}
