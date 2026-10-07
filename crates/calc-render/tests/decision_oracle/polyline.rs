use calc_viz::{Band, Column, Emphasis, Marker, Points, PrecisionLimit, Primitive};

use crate::pixels::{LINE_REACH, Mapping, drawn_rows, is_drawn_near};
use crate::rendering::render;
use crate::scenes::{layer, line_style, polyline_layer, unit_scene, without_layers, zero_bounds};

const LINE_HEIGHT: f64 = 0.37;
const FAR_HEIGHT: f64 = 0.8;
const BOUND: f64 = 0.2;
const INSIDE_BAND_OFFSET: f64 = 0.15;
const BAND_WINDOW_COLUMNS: usize = 8;

fn horizontal_line_scene() -> calc_viz::Scene {
    unit_scene(vec![polyline_layer(
        vec![0.0, 1.0],
        vec![LINE_HEIGHT, LINE_HEIGHT],
    )])
}

#[test]
fn horizontal_polyline_is_drawn_on_the_row_of_its_height() {
    let scene = horizontal_line_scene();
    let picture = render(&scene);
    let empty = render(&without_layers(&scene));
    let mapping = Mapping::unit(picture.plot_area);

    let columns = mapping.columns_between(0.1, 0.9);
    let undrawn: Vec<u32> = columns
        .filter(|column| {
            !is_drawn_near(
                &picture,
                &empty,
                *column,
                mapping.rows_near(LINE_HEIGHT, LINE_REACH),
            )
        })
        .collect();

    assert!(undrawn.is_empty(), "columns without the line: {undrawn:?}");
}

#[test]
fn horizontal_polyline_leaves_rows_away_from_its_height_untouched() {
    let scene = horizontal_line_scene();
    let picture = render(&scene);
    let empty = render(&without_layers(&scene));
    let mapping = Mapping::unit(picture.plot_area);
    let line_row = mapping.row(LINE_HEIGHT);

    let stray: Vec<(u32, u32)> = mapping
        .columns_between(0.1, 0.9)
        .flat_map(|column| {
            drawn_rows(
                &picture,
                &empty,
                column,
                picture.plot_area.top..picture.plot_area.top + picture.plot_area.height,
            )
            .into_iter()
            .filter(|row| (f64::from(*row) - line_row).abs() > LINE_REACH + 1.0)
            .map(move |row| (column, row))
        })
        .collect();

    assert!(
        stray.is_empty(),
        "pixels drawn away from the line: {stray:?}"
    );
}

#[test]
fn larger_y_is_drawn_higher_in_the_image() {
    let scene = unit_scene(vec![polyline_layer(
        vec![0.0, 1.0],
        vec![FAR_HEIGHT, FAR_HEIGHT],
    )]);
    let picture = render(&scene);
    let empty = render(&without_layers(&scene));
    let mapping = Mapping::unit(picture.plot_area);
    let column = mapping.columns_between(0.5, 0.6).start;

    let rows = drawn_rows(&picture, &empty, column, mapping.rows_between(0.0, 1.0));

    assert!(
        !rows.is_empty() && rows.iter().all(|row| f64::from(*row) < mapping.row(0.5)),
        "rows drawn for y = 0.8: {rows:?}"
    );
}

fn broken_line_scene(gap: f64) -> calc_viz::Scene {
    unit_scene(vec![polyline_layer(
        vec![0.0, 0.25, 0.5, 0.75, 1.0],
        vec![LINE_HEIGHT, LINE_HEIGHT, gap, LINE_HEIGHT, LINE_HEIGHT],
    )])
}

fn gap_columns_drawn(gap: f64) -> Vec<u32> {
    let scene = broken_line_scene(gap);
    let picture = render(&scene);
    let empty = render(&without_layers(&scene));
    let mapping = Mapping::unit(picture.plot_area);
    mapping
        .columns_between(0.25, 0.75)
        .filter(|column| {
            is_drawn_near(
                &picture,
                &empty,
                *column,
                mapping.rows_near(LINE_HEIGHT, 3.0 * LINE_REACH),
            )
        })
        .collect()
}

#[test]
fn polyline_is_broken_at_a_nan_coordinate() {
    let drawn = gap_columns_drawn(f64::NAN);

    assert!(
        drawn.is_empty(),
        "columns drawn between the neighbours of the NaN: {drawn:?}"
    );
}

#[test]
fn polyline_is_broken_at_an_infinite_coordinate() {
    let drawn = gap_columns_drawn(f64::INFINITY);

    assert!(
        drawn.is_empty(),
        "columns drawn between the neighbours of the infinity: {drawn:?}"
    );
}

#[test]
fn polyline_pieces_beside_a_nan_are_still_drawn() {
    let scene = broken_line_scene(f64::NAN);
    let picture = render(&scene);
    let empty = render(&without_layers(&scene));
    let mapping = Mapping::unit(picture.plot_area);

    let undrawn: Vec<u32> = mapping
        .columns_between(0.0, 0.25)
        .filter(|column| {
            !is_drawn_near(
                &picture,
                &empty,
                *column,
                mapping.rows_near(LINE_HEIGHT, LINE_REACH),
            )
        })
        .collect();

    assert!(
        undrawn.is_empty(),
        "columns of the first piece without the line: {undrawn:?}"
    );
}

fn line_with_bounds(bound: f64) -> calc_viz::Scene {
    let mut line = polyline_layer(vec![0.0, 1.0], vec![LINE_HEIGHT, LINE_HEIGHT]);
    line.value_bounds = vec![Column::F64(vec![bound, bound])];
    if bound > 0.0 {
        line.precision = Some(PrecisionLimit {
            grid_exhausted: Vec::new(),
            unresolved_samples: 2,
            unknown_bounds: 0,
            marked_columns: 0,
            varies_below_bounds: false,
        });
    }
    unit_scene(vec![line])
}

fn band_windows_drawn(bound: f64) -> (usize, usize) {
    let scene = line_with_bounds(bound);
    let picture = render(&scene);
    let empty = render(&without_layers(&scene));
    let mapping = Mapping::unit(picture.plot_area);
    let band_rows =
        mapping.rows_between(LINE_HEIGHT + INSIDE_BAND_OFFSET / 3.0, LINE_HEIGHT + BOUND);
    let columns: Vec<u32> = mapping.columns_between(0.1, 0.9).collect();
    let windows: Vec<&[u32]> = columns.chunks(BAND_WINDOW_COLUMNS).collect();
    let drawn = windows
        .iter()
        .filter(|window| {
            window.iter().any(|column| {
                band_rows
                    .clone()
                    .any(|row| crate::pixels::differs(&picture, &empty, *column, row))
            })
        })
        .count();
    (drawn, windows.len())
}

#[test]
fn unresolved_curve_is_drawn_as_the_band_of_its_bounds() {
    let (drawn, total) = band_windows_drawn(BOUND);

    assert!(
        total > 0 && drawn == total,
        "{drawn} of {total} column windows drawn inside the band"
    );
}

#[test]
fn resolved_curve_draws_nothing_where_the_band_would_be() {
    let (drawn, total) = band_windows_drawn(0.0);

    assert!(
        total > 0 && drawn == 0,
        "{drawn} of {total} column windows drawn above a resolved line"
    );
}

#[test]
fn band_fills_between_lower_and_upper() {
    let scene = unit_scene(vec![layer(
        Primitive::Band(Band {
            abscissa: Column::F64(vec![0.0, 1.0]),
            lower: Column::F64(vec![0.25, 0.25]),
            upper: Column::F64(vec![0.75, 0.75]),
        }),
        line_style(),
        vec![zero_bounds(2), zero_bounds(2)],
    )]);
    let picture = render(&scene);
    let empty = render(&without_layers(&scene));
    let mapping = Mapping::unit(picture.plot_area);
    let columns = mapping.columns_between(0.1, 0.9);

    let inside_undrawn = columns
        .clone()
        .filter(|column| !is_drawn_near(&picture, &empty, *column, mapping.rows_near(0.5, 1.0)))
        .count();
    let outside_drawn = columns
        .filter(|column| is_drawn_near(&picture, &empty, *column, mapping.rows_near(0.9, 1.0)))
        .count();

    assert!(
        inside_undrawn == 0 && outside_drawn == 0,
        "undrawn inside: {inside_undrawn}, drawn above the band: {outside_drawn}"
    );
}

fn point_pixel(grid_exhausted: bool) -> Option<[u8; 4]> {
    let mut points = layer(
        Primitive::Points(Points {
            coordinates: vec![Column::F64(vec![0.5]), Column::F64(vec![LINE_HEIGHT])],
            scalar: None,
        }),
        calc_viz::StyleRole {
            marker: Marker::Dot,
            emphasis: Emphasis::Normal,
            ..line_style()
        },
        vec![zero_bounds(1)],
    );
    if grid_exhausted {
        points.precision = Some(PrecisionLimit {
            grid_exhausted: vec![0],
            unresolved_samples: 0,
            unknown_bounds: 0,
            marked_columns: 0,
            varies_below_bounds: false,
        });
    }
    let picture = render(&unit_scene(vec![points]));
    let mapping = Mapping::unit(picture.plot_area);
    let column = crate::pixels::whole_pixel(mapping.column(0.5))?;
    let row = crate::pixels::whole_pixel(mapping.row(LINE_HEIGHT))?;
    picture.pixel(column, row)
}

#[test]
fn points_of_an_exhausted_grid_are_drawn_in_another_role_than_ordinary_points() {
    let ordinary = point_pixel(false);
    let exhausted = point_pixel(true);

    assert_ne!(ordinary, exhausted);
}
