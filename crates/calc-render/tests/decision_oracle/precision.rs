use crate::pixels::{LINE_REACH, Mapping, drawn_rows};
use crate::rendering::render;
use crate::scenes::{
    integer, interval, plane_view, polyline_layer, scene_with, unit_interval, without_layers,
};

const ORIGIN: i64 = 16_777_216;
const OFFSET: f64 = 0.37;

fn deep_zoom_scene() -> calc_viz::Scene {
    let origin = 16_777_216.0;
    scene_with(
        vec![plane_view(
            unit_interval(),
            interval(integer(ORIGIN), integer(ORIGIN + 1)),
        )],
        vec![polyline_layer(
            vec![0.0, 1.0],
            vec![origin + OFFSET, origin + OFFSET],
        )],
    )
}

#[test]
fn f64_line_above_an_f32_grid_point_is_drawn_at_its_offset_from_the_view_origin() {
    let scene = deep_zoom_scene();
    let picture = render(&scene);
    let empty = render(&without_layers(&scene));
    let mapping = Mapping {
        y_lower: 16_777_216.0,
        y_upper: 16_777_217.0,
        ..Mapping::unit(picture.plot_area)
    };
    let column = mapping.columns_between(0.4, 0.6).start;
    let expected_row = mapping.row(16_777_216.0 + OFFSET);

    let rows = drawn_rows(
        &picture,
        &empty,
        column,
        mapping.rows_between(16_777_216.0, 16_777_217.0),
    );

    assert!(
        !rows.is_empty()
            && rows
                .iter()
                .all(|row| (f64::from(*row) - expected_row).abs() <= LINE_REACH),
        "rows drawn: {rows:?}, expected near row {expected_row}"
    );
}
