use calc_viz::{
    Angle, AngleIndex, Column, ElementIndex, Figure, FigureLayout, FigureScale, KindColour, Label,
    LayoutPoint, Mark, MarkKind, NameKind, PointIndex, Primitive, Quantity, Segment, Stroke,
    check_label_layout,
};

use crate::pixels::{LINE_REACH, Mapping, differs, is_drawn_near, whole_pixel};
use crate::rendering::{Picture, render};
use crate::scenes::{layer, line_style, unit_scene, without_layers};

const SEGMENT_HEIGHT: f64 = 0.37;
const BOX_INSET: f64 = 1.0;

fn figure_scene(figure: Figure) -> calc_viz::Scene {
    unit_scene(vec![layer(
        Primitive::Figure(Box::new(figure)),
        calc_viz::StyleRole {
            kind: KindColour::Exact,
            ..line_style()
        },
        Vec::new(),
    )])
}

fn segment_figure(stroke: Stroke) -> Figure {
    Figure {
        points: [
            Column::F64(vec![0.1, 0.9]),
            Column::F64(vec![SEGMENT_HEIGHT, SEGMENT_HEIGHT]),
        ],
        segments: vec![Segment {
            from: PointIndex(0),
            to: PointIndex(1),
            stroke,
        }],
        angles: Vec::new(),
        marks: Vec::new(),
        labels: Vec::new(),
        scale: FigureScale::Sketch,
    }
}

fn columns_with_stroke(stroke: Stroke) -> (usize, usize) {
    let scene = figure_scene(segment_figure(stroke));
    let picture = render(&scene);
    let empty = render(&without_layers(&scene));
    let mapping = Mapping::unit(picture.plot_area);
    let columns = mapping.columns_between(0.2, 0.8);
    let total = columns.len();
    let drawn = columns
        .filter(|column| {
            is_drawn_near(
                &picture,
                &empty,
                *column,
                mapping.rows_near(SEGMENT_HEIGHT, LINE_REACH),
            )
        })
        .count();
    (drawn, total)
}

#[test]
fn solid_segment_is_drawn_without_gaps() {
    let (drawn, total) = columns_with_stroke(Stroke::Solid);

    assert!(
        total > 0 && drawn == total,
        "{drawn} of {total} columns drawn"
    );
}

#[test]
fn auxiliary_segment_is_drawn_dashed() {
    let (drawn, total) = columns_with_stroke(Stroke::Auxiliary);

    assert!(
        drawn > 0 && drawn < total,
        "{drawn} of {total} columns drawn"
    );
}

fn vertex_label(point: u32, name: &str) -> Label {
    Label {
        element: ElementIndex::Point(PointIndex(point)),
        names: NameKind::Vertex,
        name: name.to_string(),
        quantity: Quantity::Named,
        designation: None,
    }
}

fn triangle(labels: Vec<Label>, marks: Vec<Mark>) -> Figure {
    let solid = |from, to| Segment {
        from: PointIndex(from),
        to: PointIndex(to),
        stroke: Stroke::Solid,
    };
    Figure {
        points: [
            Column::F64(vec![0.2, 0.8, 0.5]),
            Column::F64(vec![0.2, 0.2, 0.8]),
        ],
        segments: vec![solid(0, 1), solid(1, 2), solid(2, 0)],
        angles: vec![Angle {
            vertex: PointIndex(0),
            first_arm: PointIndex(1),
            second_arm: PointIndex(2),
            is_oriented: false,
        }],
        marks,
        labels,
        scale: FigureScale::Sketch,
    }
}

fn named_triangle() -> Figure {
    triangle(
        vec![
            vertex_label(0, "A"),
            vertex_label(1, "B"),
            vertex_label(2, "C"),
        ],
        Vec::new(),
    )
}

fn layout_of(picture: &Picture) -> FigureLayout {
    picture
        .figure_layout
        .clone()
        .expect("the reference renderer reports the layout of a figure")
}

#[test]
fn reference_layout_of_a_spacious_triangle_passes_the_label_layout_check() {
    let figure = named_triangle();
    let picture = render(&figure_scene(figure.clone()));

    let faults = check_label_layout(&figure, &layout_of(&picture));

    assert!(faults.is_empty(), "{faults:?}");
}

#[test]
fn every_label_of_the_figure_has_a_box_in_the_layout() {
    let figure = named_triangle();
    let picture = render(&figure_scene(figure));

    let mut labels: Vec<usize> = layout_of(&picture)
        .labels
        .iter()
        .map(|label| label.label)
        .collect();
    labels.sort_unstable();

    assert_eq!(labels, vec![0, 1, 2]);
}

fn box_bounds(corners: &[LayoutPoint; 4]) -> (f64, f64, f64, f64) {
    let smallest = |values: [f64; 4]| {
        values
            .into_iter()
            .fold(f64::INFINITY, |a, v| if v < a { v } else { a })
    };
    let largest = |values: [f64; 4]| {
        values
            .into_iter()
            .fold(f64::NEG_INFINITY, |a, v| if v > a { v } else { a })
    };
    let xs = corners.map(|corner| corner.x);
    let ys = corners.map(|corner| corner.y);
    (smallest(xs), smallest(ys), largest(xs), largest(ys))
}

#[test]
fn label_boxes_lie_inside_the_image() {
    let picture = render(&figure_scene(named_triangle()));

    let outside: Vec<usize> = layout_of(&picture)
        .labels
        .iter()
        .filter(|label| {
            let (left, top, right, bottom) = box_bounds(&label.corners);
            left < 0.0
                || top < 0.0
                || right > f64::from(picture.width)
                || bottom > f64::from(picture.height)
        })
        .map(|label| label.label)
        .collect();

    assert!(outside.is_empty(), "labels outside the image: {outside:?}");
}

#[test]
fn text_of_each_label_is_drawn_inside_its_box() {
    let labelled = render(&figure_scene(named_triangle()));
    let unlabelled = render(&figure_scene(triangle(Vec::new(), Vec::new())));

    let empty_boxes: Vec<usize> = layout_of(&labelled)
        .labels
        .iter()
        .filter(|label| {
            let (left, top, right, bottom) = box_bounds(&label.corners);
            let columns = whole_pixel(left + BOX_INSET).unwrap_or(0)
                ..whole_pixel(right - BOX_INSET).unwrap_or(0);
            let rows = whole_pixel(top + BOX_INSET).unwrap_or(0)
                ..whole_pixel(bottom - BOX_INSET).unwrap_or(0);
            !rows
                .flat_map(|row| columns.clone().map(move |column| (column, row)))
                .any(|(column, row)| differs(&labelled, &unlabelled, column, row))
        })
        .map(|label| label.label)
        .collect();

    assert!(
        empty_boxes.is_empty(),
        "label boxes without drawn text: {empty_boxes:?}"
    );
}

fn bisector_pixel(layout: &FigureLayout, fraction_of_radius: f64) -> Option<(u32, u32)> {
    let arc = layout.arcs.iter().find(|arc| arc.angle == AngleIndex(0))?;
    let unit = |end: LayoutPoint| {
        let (dx, dy) = (end.x - arc.vertex.x, end.y - arc.vertex.y);
        let length = (dx * dx + dy * dy).sqrt();
        (dx / length, dy / length)
    };
    let (first_x, first_y) = unit(arc.first_arm_end);
    let (second_x, second_y) = unit(arc.second_arm_end);
    let (sum_x, sum_y) = (first_x + second_x, first_y + second_y);
    let sum_length = (sum_x * sum_x + sum_y * sum_y).sqrt();
    let distance = arc.radius * fraction_of_radius;
    let column = whole_pixel(arc.vertex.x + sum_x / sum_length * distance)?;
    let row = whole_pixel(arc.vertex.y + sum_y / sum_length * distance)?;
    Some((column, row))
}

fn arc_figures() -> (Picture, Picture) {
    let marked = triangle(
        Vec::new(),
        vec![Mark {
            element: ElementIndex::Angle(AngleIndex(0)),
            kind: MarkKind::AngleArc { count: 1 },
        }],
    );
    (
        render(&figure_scene(marked)),
        render(&figure_scene(triangle(Vec::new(), Vec::new()))),
    )
}

#[test]
fn angle_arc_fills_no_sector() {
    let (marked, unmarked) = arc_figures();

    let (column, row) = bisector_pixel(&layout_of(&marked), 0.5).expect("the arc is in the layout");

    assert!(!differs(&marked, &unmarked, column, row));
}

#[test]
fn angle_arc_is_drawn_at_its_reported_radius() {
    let (marked, unmarked) = arc_figures();

    let (column, row) = bisector_pixel(&layout_of(&marked), 1.0).expect("the arc is in the layout");
    let near: Vec<(u32, u32)> = (row.saturating_sub(2)..row + 3)
        .flat_map(|near_row| {
            (column.saturating_sub(2)..column + 3).map(move |near_column| (near_column, near_row))
        })
        .collect();

    assert!(near.iter().any(|(near_column, near_row)| differs(
        &marked,
        &unmarked,
        *near_column,
        *near_row
    )));
}
