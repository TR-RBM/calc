use calc_viz::{Colour, ColourMap, diverging_colour, sequential_colour};

use crate::pixels::matches_colour;
use crate::rendering::{Picture, render};
use crate::scenes::{polyline_layer, scalar_grid_layer, unit_interval, unit_scene, without_layers};

const SEQUENTIAL_SWATCHES: i32 = 16;
const DIVERGING_SWATCHES: i32 = 17;
const LINE_HEIGHT: f64 = 0.37;

fn swatch_centres(count: i32) -> Vec<f64> {
    (0..count)
        .map(|index| (f64::from(index) + 0.5) / f64::from(count))
        .collect()
}

fn legend_columns(picture: &Picture, colour: Colour) -> Vec<u32> {
    let first_row = picture.plot_area.top + picture.plot_area.height;
    (first_row..picture.height)
        .flat_map(|row| (0..picture.width).map(move |column| (column, row)))
        .filter(|(column, row)| {
            picture
                .pixel(*column, *row)
                .is_some_and(|pixel| matches_colour(pixel, colour))
        })
        .map(|(column, _)| column)
        .collect()
}

fn middle_column(mut columns: Vec<u32>) -> Option<u32> {
    columns.sort_unstable();
    columns.get(columns.len() / 2).copied()
}

fn swatch_positions(picture: &Picture, colours: &[Colour]) -> Vec<Option<u32>> {
    colours
        .iter()
        .map(|colour| middle_column(legend_columns(picture, *colour)))
        .collect()
}

fn is_left_to_right(positions: &[Option<u32>]) -> bool {
    positions.iter().all(Option::is_some)
        && positions
            .windows(2)
            .all(|pair| matches!(pair, [Some(left), Some(right)] if left < right))
}

fn grid_picture(map: ColourMap) -> Picture {
    render(&unit_scene(vec![scalar_grid_layer([1, 1], vec![0.5], map)]))
}

#[test]
fn colour_mapped_layer_takes_a_legend_row_from_the_plot_area() {
    let mapped = grid_picture(ColourMap::Sequential);
    let unmapped = render(&unit_scene(vec![polyline_layer(
        vec![0.0, 1.0],
        vec![LINE_HEIGHT, LINE_HEIGHT],
    )]));

    assert!(
        mapped.plot_area.height < unmapped.plot_area.height,
        "mapped {:?}, unmapped {:?}",
        mapped.plot_area,
        unmapped.plot_area
    );
}

#[test]
fn second_colour_mapped_layer_takes_a_further_legend_row() {
    let one = grid_picture(ColourMap::Sequential);
    let two = render(&unit_scene(vec![
        scalar_grid_layer([1, 1], vec![0.5], ColourMap::Sequential),
        scalar_grid_layer([1, 1], vec![0.25], ColourMap::Sequential),
    ]));

    assert!(
        two.plot_area.height < one.plot_area.height,
        "one layer {:?}, two layers {:?}",
        one.plot_area,
        two.plot_area
    );
}

#[test]
fn layer_without_a_colour_map_takes_no_legend_row() {
    let scene = unit_scene(vec![polyline_layer(
        vec![0.0, 1.0],
        vec![LINE_HEIGHT, LINE_HEIGHT],
    )]);

    let with_line = render(&scene).plot_area;
    let without_layers = render(&without_layers(&scene)).plot_area;

    assert_eq!(with_line, without_layers);
}

#[test]
fn sequential_legend_shows_sixteen_reference_swatches_from_lower_to_upper_left_to_right() {
    let picture = grid_picture(ColourMap::Sequential);
    let colours: Vec<Colour> = swatch_centres(SEQUENTIAL_SWATCHES)
        .into_iter()
        .map(|centre| sequential_colour(&unit_interval(), centre).expect("inside the range"))
        .collect();

    let positions = swatch_positions(&picture, &colours);

    assert!(
        is_left_to_right(&positions),
        "swatch columns: {positions:?}"
    );
}

#[test]
fn diverging_legend_shows_seventeen_reference_swatches_from_lower_to_upper_left_to_right() {
    let picture = grid_picture(ColourMap::Diverging);
    let colours: Vec<Colour> = swatch_centres(DIVERGING_SWATCHES)
        .into_iter()
        .map(|centre| diverging_colour(&unit_interval(), centre).expect("inside the range"))
        .collect();

    let positions = swatch_positions(&picture, &colours);

    assert!(
        is_left_to_right(&positions),
        "swatch columns: {positions:?}"
    );
}
