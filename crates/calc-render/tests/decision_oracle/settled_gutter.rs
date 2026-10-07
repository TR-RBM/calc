use calc_render::{AxisRange, ViewRequest};
use calc_viz::{Column, Scale};

use crate::rendering::{render, render_settled};
use crate::scenes::{
    integer, interval, plane_view, polyline_layer, scene_with, unit_interval, unit_scene,
};

fn settled_plane(x: calc_viz::Interval, y: calc_viz::Interval) -> Vec<ViewRequest> {
    vec![ViewRequest::plane(vec![
        Some(AxisRange {
            range: x,
            scale: Scale::Linear,
        }),
        Some(AxisRange {
            range: y,
            scale: Scale::Linear,
        }),
    ])]
}

fn wide_range() -> calc_viz::Interval {
    interval(integer(-1_000_000), integer(1_000_000))
}

fn wide_view_scene(values: Vec<f64>) -> calc_viz::Scene {
    scene_with(
        vec![plane_view(unit_interval(), wide_range())],
        vec![polyline_layer(vec![0.0, 1.0], values)],
    )
}

#[test]
fn scenes_of_one_settled_view_keep_the_plot_area_whatever_their_value_range() {
    let settled = settled_plane(unit_interval(), wide_range());

    let first = render_settled(&wide_view_scene(vec![-900_000.0, 900_000.0]), &settled);
    let second = render_settled(&wide_view_scene(vec![-1.0, 1.0]), &settled);

    assert_eq!(first.plot_area, second.plot_area);
}

#[test]
fn a_narrower_settled_range_gives_a_wider_plot_area_than_none() {
    let scene = wide_view_scene(vec![-900_000.0, 900_000.0]);

    let settled = render_settled(&scene, &settled_plane(unit_interval(), unit_interval()));
    let unsettled = render(&scene);

    assert!(
        settled.plot_area.width > unsettled.plot_area.width,
        "settled {:?}, from the scene {:?}",
        settled.plot_area,
        unsettled.plot_area
    );
}

#[test]
fn the_plot_area_does_not_depend_on_the_value_bounds_of_a_layer() {
    let mut wide_bounds = polyline_layer(vec![0.0, 1.0], vec![0.5, 0.5]);
    wide_bounds.value_bounds = vec![Column::F64(vec![0.4, 0.4])];

    let bounded = render(&unit_scene(vec![wide_bounds]));
    let plain = render(&unit_scene(vec![polyline_layer(
        vec![0.0, 1.0],
        vec![0.5, 0.5],
    )]));

    assert_eq!(bounded.plot_area, plain.plot_area);
}
