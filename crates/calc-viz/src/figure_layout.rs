use std::mem::discriminant;

use crate::figure::{AngleIndex, ElementIndex, Figure, MarkKind, SegmentIndex};
use crate::figure_check::FigureFault;

const LABEL_MARGIN_EM: f64 = 0.25;
const ARC_RADIUS_EM: f64 = 2.0;
const DOT_DIAMETER_EM: f64 = 0.3;
const SMALLEST_DOT_DIAMETER: f64 = 2.0;
const SQUARE_SIDE_EM: f64 = 0.8;
const ARC_SPACING_EM: f64 = 0.3;
const TICK_LENGTH_EM: f64 = 0.8;
const TICK_SPACING_EM: f64 = 0.3;
const THIN_STROKE_WIDTH: f64 = 1.0;
const OUTLINE_STROKE_WIDTH: f64 = 2.0;
const THINNEST_LINE: f64 = 1.0;
const POSITION_TOLERANCE: f64 = 0.5;
const FIRST_FURTHER_ARC: u8 = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelLayout {
    pub label: usize,
    pub corners: [LayoutPoint; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrokeLayout {
    pub element: Option<ElementIndex>,
    pub points: Vec<LayoutPoint>,
    pub width: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DotLayout {
    pub centre: LayoutPoint,
    pub diameter: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcLayout {
    pub angle: AngleIndex,
    pub vertex: LayoutPoint,
    pub first_arm_end: LayoutPoint,
    pub second_arm_end: LayoutPoint,
    pub radius: f64,
    pub arc_count: u8,
    pub arc_spacing: f64,
    pub dot: Option<DotLayout>,
    pub square_side: Option<f64>,
    pub is_name_outside: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickLayout {
    pub segment: SegmentIndex,
    pub count: u8,
    pub length: f64,
    pub spacing: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FigureLayout {
    pub em: f64,
    pub scale: f64,
    pub labels: Vec<LabelLayout>,
    pub strokes: Vec<StrokeLayout>,
    pub arcs: Vec<ArcLayout>,
    pub ticks: Vec<TickLayout>,
    pub key: Vec<MarkKind>,
}

fn lesser(first: f64, second: f64) -> f64 {
    if second < first { second } else { first }
}

fn greater(first: f64, second: f64) -> f64 {
    if second > first { second } else { first }
}

fn smallest(values: impl Iterator<Item = f64>) -> f64 {
    values.fold(f64::INFINITY, lesser)
}

fn difference(from: LayoutPoint, to: LayoutPoint) -> LayoutPoint {
    LayoutPoint {
        x: to.x - from.x,
        y: to.y - from.y,
    }
}

fn length(vector: LayoutPoint) -> f64 {
    (vector.x * vector.x + vector.y * vector.y).sqrt()
}

fn distance(first: LayoutPoint, second: LayoutPoint) -> f64 {
    length(difference(first, second))
}

fn cross(first: LayoutPoint, second: LayoutPoint) -> f64 {
    first.x * second.y - first.y * second.x
}

fn dot_product(first: LayoutPoint, second: LayoutPoint) -> f64 {
    first.x * second.x + first.y * second.y
}

fn unit(vector: LayoutPoint) -> LayoutPoint {
    let size = length(vector);
    LayoutPoint {
        x: vector.x / size,
        y: vector.y / size,
    }
}

fn device_pixels(size: f64) -> f64 {
    (size.abs() + 0.5).floor().copysign(size)
}

fn point_segment_distance(point: LayoutPoint, start: LayoutPoint, end: LayoutPoint) -> f64 {
    let along = difference(start, end);
    let squared = dot_product(along, along);
    if squared == 0.0 {
        return distance(point, start);
    }
    let offset = lesser(
        greater(dot_product(difference(start, point), along) / squared, 0.0),
        1.0,
    );
    distance(
        point,
        LayoutPoint {
            x: start.x + along.x * offset,
            y: start.y + along.y * offset,
        },
    )
}

fn segments_cross(first: (LayoutPoint, LayoutPoint), second: (LayoutPoint, LayoutPoint)) -> bool {
    let side =
        |a: LayoutPoint, b: LayoutPoint, c: LayoutPoint| cross(difference(a, b), difference(a, c));
    let before = side(first.0, first.1, second.0) * side(first.0, first.1, second.1);
    let after = side(second.0, second.1, first.0) * side(second.0, second.1, first.1);
    before < 0.0 && after < 0.0
}

fn segment_distance(first: (LayoutPoint, LayoutPoint), second: (LayoutPoint, LayoutPoint)) -> f64 {
    if segments_cross(first, second) {
        return 0.0;
    }
    smallest(
        [
            point_segment_distance(first.0, second.0, second.1),
            point_segment_distance(first.1, second.0, second.1),
            point_segment_distance(second.0, first.0, first.1),
            point_segment_distance(second.1, first.0, first.1),
        ]
        .into_iter(),
    )
}

fn edges(corners: &[LayoutPoint; 4]) -> [(LayoutPoint, LayoutPoint); 4] {
    let [first, second, third, fourth] = *corners;
    [
        (first, second),
        (second, third),
        (third, fourth),
        (fourth, first),
    ]
}

fn contains(corners: &[LayoutPoint; 4], point: LayoutPoint) -> bool {
    let turns =
        edges(corners).map(|(start, end)| cross(difference(start, end), difference(start, point)));
    turns.iter().all(|turn| *turn >= 0.0) || turns.iter().all(|turn| *turn <= 0.0)
}

fn polyline_segments(points: &[LayoutPoint]) -> Vec<(LayoutPoint, LayoutPoint)> {
    points
        .windows(2)
        .filter_map(|pair| match pair {
            [start, end] => Some((*start, *end)),
            _ => None,
        })
        .collect()
}

fn label_polyline_distance(label: &[LayoutPoint; 4], points: &[LayoutPoint]) -> f64 {
    if points.iter().any(|point| contains(label, *point)) {
        return 0.0;
    }
    let segments = polyline_segments(points);
    let to_segments = smallest(segments.iter().flat_map(|segment| {
        edges(label)
            .into_iter()
            .map(move |edge| segment_distance(edge, *segment))
    }));
    let to_points = smallest(points.iter().flat_map(|point| {
        edges(label)
            .into_iter()
            .map(move |(start, end)| point_segment_distance(*point, start, end))
    }));
    lesser(to_segments, to_points)
}

fn label_label_distance(first: &[LayoutPoint; 4], second: &[LayoutPoint; 4]) -> f64 {
    let overlaps = first.iter().any(|corner| contains(second, *corner))
        || second.iter().any(|corner| contains(first, *corner));
    if overlaps {
        return 0.0;
    }
    smallest(
        edges(first)
            .into_iter()
            .flat_map(|edge| edges(second).map(move |other| segment_distance(edge, other))),
    )
}

fn centre(corners: &[LayoutPoint; 4]) -> LayoutPoint {
    let [first, second, third, fourth] = *corners;
    LayoutPoint {
        x: (first.x + second.x + third.x + fourth.x) / 4.0,
        y: (first.y + second.y + third.y + fourth.y) / 4.0,
    }
}

fn label_extent(corners: &[LayoutPoint; 4]) -> f64 {
    let [first, second, third, _] = *corners;
    greater(distance(first, second), distance(second, third))
}

fn check_margins(layout: &FigureLayout, faults: &mut Vec<FigureFault>) {
    let margin = LABEL_MARGIN_EM * layout.em;
    for (position, label) in layout.labels.iter().enumerate() {
        let is_too_close = layout.strokes.iter().any(|stroke| {
            label_polyline_distance(&label.corners, &stroke.points) < margin + stroke.width / 2.0
        });
        if is_too_close {
            faults.push(FigureFault::LabelTooCloseToStroke { label: label.label });
        }
        for other in layout.labels.iter().skip(position + 1) {
            if label_label_distance(&label.corners, &other.corners) < margin {
                faults.push(FigureFault::LabelsTooClose {
                    first: label.label,
                    second: other.label,
                });
            }
        }
    }
}

fn line_width(logical: f64, scale: f64) -> f64 {
    greater((logical * scale).floor(), THINNEST_LINE)
}

fn check_stroke_widths(layout: &FigureLayout, faults: &mut Vec<FigureFault>) {
    let thin = line_width(THIN_STROKE_WIDTH, layout.scale);
    let outline = line_width(OUTLINE_STROKE_WIDTH, layout.scale);
    for (index, stroke) in layout.strokes.iter().enumerate() {
        if stroke.width != thin && stroke.width != outline {
            faults.push(FigureFault::StrokeWidthWrong { stroke: index });
        }
    }
}

fn is_in_sector(arc: &ArcLayout, point: LayoutPoint, radius: f64) -> bool {
    let first = difference(arc.vertex, arc.first_arm_end);
    let second = difference(arc.vertex, arc.second_arm_end);
    let offset = difference(arc.vertex, point);
    let after_first = cross(first, offset) >= 0.0;
    let before_second = cross(offset, second) >= 0.0;
    let is_between = if cross(first, second) >= 0.0 {
        after_first && before_second
    } else {
        after_first || before_second
    };
    is_between && length(offset) <= radius
}

fn bisector(arc: &ArcLayout) -> LayoutPoint {
    let first = unit(difference(arc.vertex, arc.first_arm_end));
    let second = unit(difference(arc.vertex, arc.second_arm_end));
    let sum = LayoutPoint {
        x: first.x + second.x,
        y: first.y + second.y,
    };
    let inner = if length(sum) == 0.0 {
        LayoutPoint {
            x: -first.y,
            y: first.x,
        }
    } else {
        unit(sum)
    };
    if cross(first, second) < 0.0 {
        LayoutPoint {
            x: -inner.x,
            y: -inner.y,
        }
    } else {
        inner
    }
}

fn check_arc_sizes(layout: &FigureLayout, arc: &ArcLayout, faults: &mut Vec<FigureFault>) {
    let em = layout.em;
    if arc.radius != device_pixels(ARC_RADIUS_EM * em) {
        faults.push(FigureFault::ArcRadiusWrong { angle: arc.angle });
    }
    if arc.arc_count >= FIRST_FURTHER_ARC && arc.arc_spacing != device_pixels(ARC_SPACING_EM * em) {
        faults.push(FigureFault::ArcSpacingWrong { angle: arc.angle });
    }
    if let Some(side) = arc.square_side
        && side != device_pixels(SQUARE_SIDE_EM * em)
    {
        faults.push(FigureFault::SquareSizeWrong { angle: arc.angle });
    }
    if let Some(dot) = arc.dot {
        let direction = bisector(arc);
        let expected_diameter = greater(device_pixels(DOT_DIAMETER_EM * em), SMALLEST_DOT_DIAMETER);
        let expected_centre = LayoutPoint {
            x: arc.vertex.x + direction.x * arc.radius / 2.0,
            y: arc.vertex.y + direction.y * arc.radius / 2.0,
        };
        let is_misplaced = dot.diameter != expected_diameter
            || distance(dot.centre, expected_centre) > POSITION_TOLERANCE;
        if is_misplaced {
            faults.push(FigureFault::DotMisplaced { angle: arc.angle });
        }
    }
}

fn check_arc_name(
    figure: &Figure,
    layout: &FigureLayout,
    arc: &ArcLayout,
    faults: &mut Vec<FigureFault>,
) {
    let margin = LABEL_MARGIN_EM * layout.em;
    let outer_radius = arc.radius + f64::from(arc.arc_count.saturating_sub(1)) * arc.arc_spacing;
    let name = figure
        .labels
        .iter()
        .enumerate()
        .filter(|(_, label)| label.element == ElementIndex::Angle(arc.angle))
        .find_map(|(index, _)| layout.labels.iter().find(|placed| placed.label == index));
    let name_extent = name.map_or(0.0, |name| label_extent(&name.corners));
    let shortest_arm = lesser(
        distance(arc.vertex, arc.first_arm_end),
        distance(arc.vertex, arc.second_arm_end),
    );
    if shortest_arm < outer_radius + name_extent + margin {
        faults.push(FigureFault::ArmTooShort { angle: arc.angle });
    }
    let Some(name) = name else {
        return;
    };
    if arc.is_name_outside {
        let direction = bisector(arc);
        let offset = difference(arc.vertex, centre(&name.corners));
        let is_on_bisector = cross(direction, offset).abs() <= POSITION_TOLERANCE
            && dot_product(direction, offset) > 0.0;
        let is_beyond = name
            .corners
            .iter()
            .all(|corner| distance(arc.vertex, *corner) > outer_radius);
        if !(is_on_bisector && is_beyond) {
            faults.push(FigureFault::AngleNameOffBisector { label: name.label });
        }
    } else {
        let fits = name
            .corners
            .iter()
            .all(|corner| is_in_sector(arc, *corner, arc.radius - margin));
        if !fits {
            faults.push(FigureFault::ArcTooSmall { angle: arc.angle });
        }
    }
}

fn check_ticks(layout: &FigureLayout, faults: &mut Vec<FigureFault>) {
    for tick in &layout.ticks {
        let length_is_wrong = tick.length != device_pixels(TICK_LENGTH_EM * layout.em);
        let spacing_is_wrong =
            tick.count >= 2 && tick.spacing != device_pixels(TICK_SPACING_EM * layout.em);
        if length_is_wrong || spacing_is_wrong {
            faults.push(FigureFault::TickSizeWrong {
                segment: tick.segment,
            });
        }
    }
}

fn element_distance(
    layout: &FigureLayout,
    element: ElementIndex,
    point: LayoutPoint,
) -> Option<f64> {
    let distances: Vec<f64> = layout
        .strokes
        .iter()
        .filter(|stroke| stroke.element == Some(element))
        .map(|stroke| {
            let to_segments = smallest(
                polyline_segments(&stroke.points)
                    .iter()
                    .map(|(start, end)| point_segment_distance(point, *start, *end)),
            );
            let to_points = smallest(stroke.points.iter().map(|end| distance(point, *end)));
            lesser(to_segments, to_points)
        })
        .collect();
    (!distances.is_empty()).then(|| smallest(distances.into_iter()))
}

fn check_label_nearness(figure: &Figure, layout: &FigureLayout, faults: &mut Vec<FigureFault>) {
    for placed in &layout.labels {
        let Some(label) = figure.labels.get(placed.label) else {
            continue;
        };
        if matches!(label.element, ElementIndex::Angle(_)) {
            continue;
        }
        let point = centre(&placed.corners);
        let Some(own) = element_distance(layout, label.element, point) else {
            continue;
        };
        let is_nearer_elsewhere = layout
            .strokes
            .iter()
            .filter_map(|stroke| stroke.element)
            .filter(|element| {
                *element != label.element && discriminant(element) == discriminant(&label.element)
            })
            .filter_map(|element| element_distance(layout, element, point))
            .any(|other| other < own);
        if is_nearer_elsewhere {
            faults.push(FigureFault::LabelAwayFromElement {
                label: placed.label,
            });
        }
    }
}

fn check_key(figure: &Figure, layout: &FigureLayout, faults: &mut Vec<FigureFault>) {
    for (index, mark) in figure.marks.iter().enumerate() {
        let is_named = layout.key.iter().any(|key| match (key, mark.kind) {
            (MarkKind::RightAngle(key_style), MarkKind::RightAngle(style)) => *key_style == style,
            _ => discriminant(key) == discriminant(&mark.kind),
        });
        if !is_named {
            faults.push(FigureFault::KeyMissesMark { mark: index });
        }
    }
}

pub fn check_label_layout(figure: &Figure, layout: &FigureLayout) -> Vec<FigureFault> {
    let mut faults = Vec::new();
    check_margins(layout, &mut faults);
    check_stroke_widths(layout, &mut faults);
    for arc in &layout.arcs {
        check_arc_sizes(layout, arc, &mut faults);
        check_arc_name(figure, layout, arc, &mut faults);
    }
    check_ticks(layout, &mut faults);
    check_label_nearness(figure, layout, &mut faults);
    check_key(figure, layout, &mut faults);
    faults
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::{Label, NameKind, PointIndex, Quantity, RightAngleMark};
    use crate::figure_check::tests::right_triangle;

    const EM: f64 = 10.0;

    fn at(x: f64, y: f64) -> LayoutPoint {
        LayoutPoint { x, y }
    }

    fn square_box(label: usize, lower: (f64, f64), upper: (f64, f64)) -> LabelLayout {
        LabelLayout {
            label,
            corners: [
                at(lower.0, lower.1),
                at(upper.0, lower.1),
                at(upper.0, upper.1),
                at(lower.0, upper.1),
            ],
        }
    }

    fn side_stroke(segment: u32, from: LayoutPoint, to: LayoutPoint) -> StrokeLayout {
        StrokeLayout {
            element: Some(ElementIndex::Segment(SegmentIndex(segment))),
            points: vec![from, to],
            width: 2.0,
        }
    }

    fn right_angle_arc() -> ArcLayout {
        let half_diagonal = 20.0 / 2.0 / 2.0_f64.sqrt();
        ArcLayout {
            angle: AngleIndex(1),
            vertex: at(0.0, 0.0),
            first_arm_end: at(160.0, 0.0),
            second_arm_end: at(0.0, 120.0),
            radius: 20.0,
            arc_count: 1,
            arc_spacing: 0.0,
            dot: Some(DotLayout {
                centre: at(half_diagonal, half_diagonal),
                diameter: 3.0,
            }),
            square_side: None,
            is_name_outside: false,
        }
    }

    fn triangle_layout() -> FigureLayout {
        FigureLayout {
            em: EM,
            scale: 1.0,
            labels: vec![
                square_box(0, (165.0, -15.0), (175.0, -5.0)),
                square_box(1, (-15.0, 125.0), (-5.0, 135.0)),
                square_box(2, (-15.0, -15.0), (-5.0, -5.0)),
                square_box(3, (-15.0, 55.0), (-5.0, 65.0)),
                square_box(4, (75.0, -15.0), (85.0, -5.0)),
                square_box(5, (82.2, 64.6), (92.2, 74.6)),
            ],
            strokes: vec![
                side_stroke(0, at(0.0, 120.0), at(0.0, 0.0)),
                side_stroke(1, at(160.0, 0.0), at(0.0, 0.0)),
                side_stroke(2, at(160.0, 0.0), at(0.0, 120.0)),
            ],
            arcs: vec![right_angle_arc()],
            ticks: Vec::new(),
            key: vec![MarkKind::RightAngle(RightAngleMark::ArcWithDot)],
        }
    }

    fn named_right_angle() -> Figure {
        let mut figure = right_triangle();
        figure.labels.push(Label {
            element: ElementIndex::Angle(AngleIndex(1)),
            names: NameKind::Angle,
            name: String::from("γ"),
            quantity: Quantity::Named,
            designation: None,
        });
        figure
    }

    fn faults(layout: &FigureLayout) -> Vec<FigureFault> {
        check_label_layout(&named_right_angle(), layout)
    }

    #[test]
    fn well_placed_triangle_has_no_layout_faults() {
        let faults = check_label_layout(&right_triangle(), &triangle_layout());

        assert_eq!(faults, Vec::new());
    }

    #[test]
    fn label_crossing_a_side_is_too_close() {
        let mut layout = triangle_layout();
        layout.labels[3] = square_box(3, (-3.0, 55.0), (7.0, 65.0));

        let result = check_label_layout(&right_triangle(), &layout);

        assert!(result.contains(&FigureFault::LabelTooCloseToStroke { label: 3 }));
    }

    #[test]
    fn label_within_the_margin_from_the_edge_of_a_wide_stroke_is_too_close() {
        let mut layout = triangle_layout();
        layout.labels[3] = square_box(3, (-13.0, 55.0), (-3.0, 65.0));

        let result = check_label_layout(&right_triangle(), &layout);

        assert!(result.contains(&FigureFault::LabelTooCloseToStroke { label: 3 }));
    }

    #[test]
    fn label_at_the_margin_from_the_edge_of_a_thin_stroke_passes() {
        let mut layout = triangle_layout();
        layout.strokes[0].width = 1.0;
        layout.labels[3] = square_box(3, (-13.0, 55.0), (-3.0, 65.0));

        let result = check_label_layout(&right_triangle(), &layout);

        assert_eq!(result, Vec::new());
    }

    #[test]
    fn rotated_label_parallel_to_the_hypotenuse_is_judged_by_its_corners() {
        let mut layout = triangle_layout();
        let normal = at(0.6, 0.8);
        let along = at(-0.8, 0.6);
        let base = at(80.0 + normal.x * 4.0, 60.0 + normal.y * 4.0);
        let corner = |a: f64, n: f64| {
            at(
                base.x + along.x * a + normal.x * n,
                base.y + along.y * a + normal.y * n,
            )
        };
        layout.labels[5] = LabelLayout {
            label: 5,
            corners: [
                corner(-5.0, 0.0),
                corner(5.0, 0.0),
                corner(5.0, 4.0),
                corner(-5.0, 4.0),
            ],
        };

        let result = check_label_layout(&right_triangle(), &layout);

        assert_eq!(result, Vec::new());
    }

    #[test]
    fn labels_closer_than_the_margin_are_too_close() {
        let mut layout = triangle_layout();
        layout.labels[4] = square_box(4, (75.0, -15.0), (85.0, -5.0));
        layout
            .labels
            .push(square_box(0, (87.0, -15.0), (97.0, -5.0)));

        let result = check_label_layout(&right_triangle(), &layout);

        assert!(result.contains(&FigureFault::LabelsTooClose {
            first: 4,
            second: 0
        }));
    }

    #[test]
    fn stroke_of_three_pixels_is_a_wrong_width() {
        let mut layout = triangle_layout();
        layout.strokes[1].width = 3.0;

        let result = check_label_layout(&right_triangle(), &layout);

        assert!(result.contains(&FigureFault::StrokeWidthWrong { stroke: 1 }));
    }

    #[test]
    fn arc_radius_other_than_two_em_is_wrong() {
        let mut layout = triangle_layout();
        layout.arcs[0].radius = 18.0;

        let result = check_label_layout(&right_triangle(), &layout);

        assert!(result.contains(&FigureFault::ArcRadiusWrong {
            angle: AngleIndex(1)
        }));
    }

    #[test]
    fn further_arcs_spaced_other_than_three_tenths_em_are_wrong() {
        let mut layout = triangle_layout();
        layout.arcs[0].arc_count = 2;
        layout.arcs[0].arc_spacing = 4.0;

        let result = check_label_layout(&right_triangle(), &layout);

        assert!(result.contains(&FigureFault::ArcSpacingWrong {
            angle: AngleIndex(1)
        }));
    }

    #[test]
    fn right_angle_square_other_than_eight_tenths_em_is_wrong() {
        let mut layout = triangle_layout();
        layout.arcs[0].square_side = Some(7.0);

        let result = check_label_layout(&right_triangle(), &layout);

        assert!(result.contains(&FigureFault::SquareSizeWrong {
            angle: AngleIndex(1)
        }));
    }

    #[test]
    fn dot_away_from_half_the_radius_on_the_bisector_is_misplaced() {
        let mut layout = triangle_layout();
        if let Some(dot) = &mut layout.arcs[0].dot {
            dot.centre = at(10.0, 2.0);
        }

        let result = check_label_layout(&right_triangle(), &layout);

        assert!(result.contains(&FigureFault::DotMisplaced {
            angle: AngleIndex(1)
        }));
    }

    #[test]
    fn sizes_round_to_device_pixels_with_ties_away_from_zero() {
        let mut layout = triangle_layout();
        layout.em = 12.5;
        layout.arcs[0].radius = 25.0;
        if let Some(dot) = &mut layout.arcs[0].dot {
            let half_diagonal = 25.0 / 2.0 / 2.0_f64.sqrt();
            dot.centre = at(half_diagonal, half_diagonal);
            dot.diameter = 4.0;
        }
        layout.labels.clear();

        let result = check_label_layout(&right_triangle(), &layout);

        assert_eq!(result, Vec::new());
    }

    #[test]
    fn arm_shorter_than_the_arc_and_its_name_is_too_short() {
        let mut layout = triangle_layout();
        layout.arcs[0].first_arm_end = at(15.0, 0.0);

        let result = check_label_layout(&right_triangle(), &layout);

        assert!(result.contains(&FigureFault::ArmTooShort {
            angle: AngleIndex(1)
        }));
    }

    #[test]
    fn name_reaching_past_the_arc_inside_does_not_fit() {
        let mut layout = triangle_layout();
        layout.labels.push(square_box(6, (8.0, 8.0), (16.0, 16.0)));

        let result = faults(&layout);

        assert!(result.contains(&FigureFault::ArcTooSmall {
            angle: AngleIndex(1)
        }));
    }

    #[test]
    fn small_name_inside_the_arc_fits() {
        let mut layout = triangle_layout();
        layout.arcs[0].dot = None;
        layout.labels.push(square_box(6, (5.0, 5.0), (9.0, 9.0)));

        let result = faults(&layout);

        assert!(!result.contains(&FigureFault::ArcTooSmall {
            angle: AngleIndex(1)
        }));
    }

    #[test]
    fn name_outside_the_arc_off_its_bisector_is_a_fault() {
        let mut layout = triangle_layout();
        layout.arcs[0].is_name_outside = true;
        layout.labels.push(square_box(6, (30.0, 5.0), (34.0, 9.0)));

        let result = faults(&layout);

        assert!(result.contains(&FigureFault::AngleNameOffBisector { label: 6 }));
    }

    #[test]
    fn name_outside_the_arc_on_its_bisector_passes() {
        let mut layout = triangle_layout();
        layout.arcs[0].is_name_outside = true;
        layout
            .labels
            .push(square_box(6, (19.2, 19.2), (23.2, 23.2)));

        let result = faults(&layout);

        assert_eq!(result, Vec::new());
    }

    #[test]
    fn tick_shorter_than_eight_tenths_em_is_wrong() {
        let mut layout = triangle_layout();
        layout.ticks.push(TickLayout {
            segment: SegmentIndex(0),
            count: 1,
            length: 7.0,
            spacing: 0.0,
        });

        let result = check_label_layout(&right_triangle(), &layout);

        assert!(result.contains(&FigureFault::TickSizeWrong {
            segment: SegmentIndex(0)
        }));
    }

    #[test]
    fn side_name_nearer_to_another_side_is_away_from_its_element() {
        let mut layout = triangle_layout();
        layout.labels[3] = square_box(3, (40.0, -15.0), (50.0, -5.0));

        let result = check_label_layout(&right_triangle(), &layout);

        assert!(result.contains(&FigureFault::LabelAwayFromElement { label: 3 }));
    }

    #[test]
    fn mark_missing_from_the_key_is_a_fault() {
        let mut layout = triangle_layout();
        layout.key.clear();

        let result = check_label_layout(&right_triangle(), &layout);

        assert!(result.contains(&FigureFault::KeyMissesMark { mark: 0 }));
    }

    #[test]
    fn vertex_labels_without_strokes_of_their_own_are_not_judged_by_nearness() {
        let mut figure = right_triangle();
        figure.labels[0].element = ElementIndex::Point(PointIndex(0));
        let layout = triangle_layout();

        let result = check_label_layout(&figure, &layout);

        assert!(!result.contains(&FigureFault::LabelAwayFromElement { label: 0 }));
    }

    #[test]
    fn lines_snapped_at_one_and_a_half_scale_pass() {
        let mut layout = triangle_layout();
        layout.scale = 1.5;
        for stroke in &mut layout.strokes {
            stroke.width = 3.0;
        }
        layout.strokes[0].width = 1.0;
        layout.labels.clear();

        let result = check_label_layout(&right_triangle(), &layout);

        assert_eq!(result, Vec::new());
    }

    #[test]
    fn two_pixel_line_at_one_and_a_half_scale_is_a_wrong_width() {
        let mut layout = triangle_layout();
        layout.scale = 1.5;
        layout.labels.clear();

        let result = check_label_layout(&right_triangle(), &layout);

        assert!(result.contains(&FigureFault::StrokeWidthWrong { stroke: 0 }));
    }
}
