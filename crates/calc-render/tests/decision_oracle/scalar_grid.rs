use calc_viz::{
    CellGrid, Colour, ColourMap, Column, ComplexGrid, Primitive, domain_colour, sequential_colour,
};

use crate::pixels::{Mapping, channel_distance, most_common_pixel, share_matching};
use crate::rendering::render;
use crate::scenes::{
    colour_map_style, layer, scalar_grid_layer, unit_interval, unit_scene, zero_bounds,
};

const MAJORITY: f64 = 0.9;
const MAP_SAMPLES: i32 = 4096;
const SMALLEST_DISTANCE_FROM_MAP: f64 = 3.0;

fn sequential(value: f64) -> Colour {
    sequential_colour(&unit_interval(), value).expect("values inside the range have a colour")
}

fn quadrant_share(value: f64, grid_values: Vec<f64>, x: (f64, f64), y: (f64, f64)) -> f64 {
    let picture = render(&unit_scene(vec![scalar_grid_layer(
        [2, 2],
        grid_values,
        ColourMap::Sequential,
    )]));
    let mapping = Mapping::unit(picture.plot_area);
    share_matching(
        &picture,
        mapping.columns_between(x.0, x.1),
        mapping.rows_between(y.0, y.1),
        sequential(value),
    )
}

fn row_major_values() -> Vec<f64> {
    vec![0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]
}

#[test]
fn single_cell_takes_the_reference_colour_of_its_value() {
    let picture = render(&unit_scene(vec![scalar_grid_layer(
        [1, 1],
        vec![0.3],
        ColourMap::Sequential,
    )]));
    let mapping = Mapping::unit(picture.plot_area);

    let share = share_matching(
        &picture,
        mapping.columns_between(0.0, 1.0),
        mapping.rows_between(0.0, 1.0),
        sequential(0.3),
    );

    assert!(
        share >= MAJORITY,
        "share of the cell in its reference colour: {share}"
    );
}

#[test]
fn first_scalar_of_the_grid_is_the_cell_at_lower_x_and_lower_y() {
    let share = quadrant_share(0.0, row_major_values(), (0.0, 0.5), (0.0, 0.5));

    assert!(share >= MAJORITY, "share: {share}");
}

#[test]
fn second_scalar_of_the_grid_is_the_cell_at_upper_x_and_lower_y() {
    let share = quadrant_share(1.0 / 3.0, row_major_values(), (0.5, 1.0), (0.0, 0.5));

    assert!(share >= MAJORITY, "share: {share}");
}

#[test]
fn third_scalar_of_the_grid_is_the_cell_at_lower_x_and_upper_y() {
    let share = quadrant_share(2.0 / 3.0, row_major_values(), (0.0, 0.5), (0.5, 1.0));

    assert!(share >= MAJORITY, "share: {share}");
}

#[test]
fn fourth_scalar_of_the_grid_is_the_cell_at_upper_x_and_upper_y() {
    let share = quadrant_share(1.0, row_major_values(), (0.5, 1.0), (0.5, 1.0));

    assert!(share >= MAJORITY, "share: {share}");
}

#[test]
fn later_layer_is_drawn_over_earlier_layer() {
    let picture = render(&unit_scene(vec![
        scalar_grid_layer([1, 1], vec![0.0], ColourMap::Sequential),
        scalar_grid_layer([1, 1], vec![1.0], ColourMap::Sequential),
    ]));
    let mapping = Mapping::unit(picture.plot_area);

    let share = share_matching(
        &picture,
        mapping.columns_between(0.0, 1.0),
        mapping.rows_between(0.0, 1.0),
        sequential(1.0),
    );

    assert!(
        share >= MAJORITY,
        "share of the later layer's colour: {share}"
    );
}

fn smallest_distance_from_sequential_map(pixel: [u8; 4]) -> f64 {
    (0..=MAP_SAMPLES)
        .map(|index| channel_distance(pixel, sequential(f64::from(index) / f64::from(MAP_SAMPLES))))
        .fold(f64::INFINITY, |smallest, distance| {
            if distance < smallest {
                distance
            } else {
                smallest
            }
        })
}

#[test]
fn nan_cell_is_drawn_in_a_colour_that_is_not_on_the_map() {
    let picture = render(&unit_scene(vec![scalar_grid_layer(
        [2, 1],
        vec![f64::NAN, 0.5],
        ColourMap::Sequential,
    )]));
    let mapping = Mapping::unit(picture.plot_area);

    let pixel = most_common_pixel(
        &picture,
        mapping.columns_between(0.0, 0.5),
        mapping.rows_between(0.0, 1.0),
    )
    .expect("the NaN cell has pixels");
    let distance = smallest_distance_from_sequential_map(pixel);

    assert!(
        distance >= SMALLEST_DISTANCE_FROM_MAP,
        "NaN cell pixel {pixel:?} lies {distance} from the map"
    );
}

#[test]
fn complex_cell_takes_the_domain_colouring_reference_colour() {
    let (real, imaginary) = (0.3, -0.4);
    let scene = unit_scene(vec![layer(
        Primitive::ComplexGrid(ComplexGrid {
            cells: CellGrid {
                region: vec![unit_interval(), unit_interval()],
                counts: vec![1, 1],
            },
            real: Column::F64(vec![real]),
            imaginary: Column::F64(vec![imaginary]),
        }),
        colour_map_style(ColourMap::DomainColouring, unit_interval()),
        vec![zero_bounds(1), zero_bounds(1)],
    )]);
    let picture = render(&scene);
    let mapping = Mapping::unit(picture.plot_area);
    let expected =
        domain_colour(&unit_interval(), real, imaginary).expect("finite value has a colour");

    let share = share_matching(
        &picture,
        mapping.columns_between(0.0, 1.0),
        mapping.rows_between(0.0, 1.0),
        expected,
    );

    assert!(share >= MAJORITY, "share of the domain colour: {share}");
}
