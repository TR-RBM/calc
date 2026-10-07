use std::f64::consts::{PI, TAU};

use calc_numbers::atan2_f64;
use calc_viz::{
    AngleIndex, ArcLayout, Designation, DotLayout, ElementIndex, Figure, FigureLayout, Label,
    LabelLayout, Layer, LayoutPoint, MarkKind, PointIndex, Quantity, RightAngleMark, SegmentIndex,
    Stroke, StrokeLayout, TickLayout,
};
use tiny_skia::PathBuilder;

use crate::arc::CircularArc;
use crate::clip::polyline_path;
use crate::mapping::{ScreenPoint, pixel_round};
use crate::picture_text::PictureText;
use crate::plane::{PlaneView, draw_arrow};
use crate::render::{Painter, RenderError, TextRow};
use crate::roles;

const THIN_LOGICAL: u16 = 1;
const OUTLINE_LOGICAL: u16 = 2;
const DASH_LOGICAL: u16 = 6;
const DASH_GAP_LOGICAL: u16 = 4;
const ARC_RADIUS_EM: f64 = 2.0;
const ARC_SPACING_EM: f64 = 0.3;
const DOT_DIAMETER_EM: f64 = 0.3;
const SMALLEST_DOT_DIAMETER: f64 = 2.0;
const SQUARE_SIDE_EM: f64 = 0.8;
const TICK_LENGTH_EM: f64 = 0.8;
const TICK_SPACING_EM: f64 = 0.3;
const LABEL_MARGIN_EM: f64 = 0.25;
const PERCENT: f64 = 100.0;
const EQUALS: &str = " = ";
const SOUGHT: &str = "?";
const CLEARANCE: f64 = 1.0;

fn snap_centre(point: ScreenPoint, width: f64) -> ScreenPoint {
    let is_odd = (width % 2.0) == 1.0;
    let snap = |value: f64| {
        if is_odd {
            value.floor() + 0.5
        } else {
            (value + 0.5).floor()
        }
    };
    (snap(point.0), snap(point.1))
}

fn device_pixels(size: f64) -> f64 {
    (size.abs() + 0.5).floor().copysign(size)
}

fn layout_point(point: ScreenPoint) -> LayoutPoint {
    LayoutPoint {
        x: point.0,
        y: point.1,
    }
}

fn unit(vector: ScreenPoint) -> Option<ScreenPoint> {
    let length = (vector.0 * vector.0 + vector.1 * vector.1).sqrt();
    (length > 0.0 && length.is_finite()).then(|| (vector.0 / length, vector.1 / length))
}

fn difference(from: ScreenPoint, to: ScreenPoint) -> ScreenPoint {
    (to.0 - from.0, to.1 - from.1)
}

fn cross(first: ScreenPoint, second: ScreenPoint) -> f64 {
    first.0 * second.1 - first.1 * second.0
}

struct Sizes {
    em: f64,
    thin: f64,
    outline: f64,
    radius: f64,
    spacing: f64,
    margin: f64,
}

struct Placed {
    vertex: ScreenPoint,
    start_arm: ScreenPoint,
    end_arm: ScreenPoint,
    start_angle: f64,
    sweep: f64,
}

impl Placed {
    fn bisector(&self) -> ScreenPoint {
        let middle = self.start_angle + self.sweep / 2.0;
        (calc_numbers::cos_f64(middle), calc_numbers::sin_f64(middle))
    }
}

fn place_angle(
    figure: &Figure,
    points: &[Option<ScreenPoint>],
    index: AngleIndex,
) -> Option<Placed> {
    let angle = figure.angle(index)?;
    let point = |PointIndex(point): PointIndex| {
        usize::try_from(point)
            .ok()
            .and_then(|point| points.get(point).copied().flatten())
    };
    let vertex = point(angle.vertex)?;
    let (mut first, mut second) = (point(angle.first_arm)?, point(angle.second_arm)?);
    let turn = |first: ScreenPoint, second: ScreenPoint| {
        let (along_first, along_second) = (difference(vertex, first), difference(vertex, second));
        let counterclockwise = atan2_f64(
            -cross(along_first, along_second),
            along_first.0 * along_second.0 + along_first.1 * along_second.1,
        );
        if counterclockwise < 0.0 {
            counterclockwise + TAU
        } else {
            counterclockwise
        }
    };
    let mut view_angle = turn(first, second);
    if !angle.is_oriented && view_angle > PI {
        std::mem::swap(&mut first, &mut second);
        view_angle = turn(first, second);
    }
    let along_first = difference(vertex, first);
    Some(Placed {
        vertex,
        start_arm: first,
        end_arm: second,
        start_angle: atan2_f64(along_first.1, along_first.0),
        sweep: -view_angle,
    })
}

pub(crate) fn draw_figure(
    painter: &mut Painter<'_>,
    plane: &PlaneView<'_>,
    layer: &Layer,
    figure: &Figure,
    text: &PictureText,
) -> Result<FigureLayout, RenderError> {
    let em = f64::from(painter.metrics.font_size_px);
    let sizes = Sizes {
        em,
        thin: f64::from(painter.line_width(THIN_LOGICAL)),
        outline: f64::from(painter.line_width(OUTLINE_LOGICAL)),
        radius: device_pixels(ARC_RADIUS_EM * em),
        spacing: device_pixels(ARC_SPACING_EM * em),
        margin: LABEL_MARGIN_EM * em,
    };
    let colour = roles::layer_colour(&painter.theme, &layer.style);
    let clip = plane.clip.as_ref();
    let [xs, ys] = &figure.points;
    let points: Vec<Option<ScreenPoint>> = (0..figure.point_count())
        .map(|index| plane.point(xs, ys, index))
        .collect();
    let point = |PointIndex(index): PointIndex| {
        usize::try_from(index)
            .ok()
            .and_then(|index| points.get(index).copied().flatten())
    };
    let mut layout = FigureLayout {
        em,
        scale: f64::from(painter.scale.percent()) / PERCENT,
        labels: Vec::new(),
        strokes: Vec::new(),
        arcs: Vec::new(),
        ticks: Vec::new(),
        key: Vec::new(),
    };
    let dash = [
        f32::from(painter.physical(DASH_LOGICAL)),
        f32::from(painter.physical(DASH_GAP_LOGICAL)),
    ];
    for (index, segment) in figure.segments.iter().enumerate() {
        let (Some(from), Some(to)) = (point(segment.from), point(segment.to)) else {
            continue;
        };
        let (width, pattern) = match segment.stroke {
            Stroke::Solid => (sizes.outline, None),
            Stroke::Auxiliary => (sizes.thin, Some(dash)),
        };
        let (from, to) = (snap_centre(from, width), snap_centre(to, width));
        if let Some(path) = polyline_path(&plane.guard, &[vec![from, to]]) {
            painter.canvas.stroke_path_within(
                &path,
                colour,
                crate::mapping::to_f32(width),
                pattern,
                clip,
            );
        }
        layout.strokes.push(StrokeLayout {
            element: Some(ElementIndex::Segment(SegmentIndex(
                u32::try_from(index).unwrap_or(u32::MAX),
            ))),
            points: vec![layout_point(from), layout_point(to)],
            width,
        });
    }
    let pen = MarkPen {
        plane,
        sizes: &sizes,
        colour,
    };
    for mark in &figure.marks {
        match (mark.element, mark.kind) {
            (ElementIndex::Angle(angle), kind) => {
                if let Some(placed) = place_angle(figure, &points, angle) {
                    draw_angle_mark(painter, &pen, &mut layout, (angle, &placed), kind)?;
                }
            }
            (ElementIndex::Segment(segment), MarkKind::EqualTicks { count }) => {
                let ends = figure
                    .segment(segment)
                    .and_then(|segment| Some((point(segment.from)?, point(segment.to)?)));
                if let Some((from, to)) = ends {
                    draw_ticks(painter, &pen, &mut layout, (segment, count), (from, to));
                }
            }
            _ => {}
        }
        let is_new = !layout.key.iter().any(|known| same_key(*known, mark.kind));
        if is_new {
            layout.key.push(mark.kind);
        }
    }
    let centroid = centroid(&points);
    for (index, label) in figure.labels.iter().enumerate() {
        let words = label_text(label, text);
        let width = f64::from(painter.text_width(&words));
        let height = f64::from(painter.row_height());
        let centre = match label.element {
            ElementIndex::Point(vertex) => point(vertex).map(|vertex| {
                let away = unit(difference(centroid, vertex)).unwrap_or((0.0, -1.0));
                let reach = (width * width + height * height).sqrt() / 2.0
                    + sizes.margin
                    + sizes.outline
                    + CLEARANCE;
                (vertex.0 + away.0 * reach, vertex.1 + away.1 * reach)
            }),
            ElementIndex::Segment(segment) => figure
                .segment(segment)
                .and_then(|segment| Some((point(segment.from)?, point(segment.to)?)))
                .and_then(|(from, to)| {
                    let middle = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
                    let along = unit(difference(from, to))?;
                    let mut normal = (-along.1, along.0);
                    let outward = difference(centroid, middle);
                    if normal.0 * outward.0 + normal.1 * outward.1 < 0.0 {
                        normal = (-normal.0, -normal.1);
                    }
                    let has_ticks = layout.ticks.iter().any(|tick| tick.segment == segment);
                    let tick_reach = if has_ticks {
                        device_pixels(TICK_LENGTH_EM * sizes.em) / 2.0
                    } else {
                        0.0
                    };
                    let extent = (normal.0.abs() * width + normal.1.abs() * height) / 2.0;
                    let reach =
                        extent + sizes.margin + sizes.outline / 2.0 + tick_reach + CLEARANCE;
                    Some((middle.0 + normal.0 * reach, middle.1 + normal.1 * reach))
                }),
            ElementIndex::Angle(angle) => place_angle(figure, &points, angle).map(|placed| {
                angle_label_centre(&sizes, &mut layout, angle, &placed, width, height)
            }),
        };
        let Some(centre) = centre else {
            continue;
        };
        let left = pixel_round(centre.0 - width / 2.0);
        let top = pixel_round(centre.1 - height / 2.0);
        let (left_edge, top_edge) = (f64::from(left), f64::from(top));
        let bounds = painter.bounds;
        let fits = left_edge >= f64::from(bounds.x)
            && top_edge >= f64::from(bounds.y)
            && left_edge + width <= f64::from(bounds.right())
            && top_edge + height <= f64::from(bounds.bottom());
        if !fits {
            return Err(RenderError::TextDoesNotFit(TextRow::FigureLabel {
                label: index,
            }));
        }
        painter.text(&words, left, top, painter.theme.text)?;
        layout.labels.push(LabelLayout {
            label: index,
            corners: [
                LayoutPoint {
                    x: left_edge,
                    y: top_edge,
                },
                LayoutPoint {
                    x: left_edge + width,
                    y: top_edge,
                },
                LayoutPoint {
                    x: left_edge + width,
                    y: top_edge + height,
                },
                LayoutPoint {
                    x: left_edge,
                    y: top_edge + height,
                },
            ],
        });
    }
    Ok(layout)
}

pub(crate) fn same_key(known: MarkKind, kind: MarkKind) -> bool {
    match (known, kind) {
        (MarkKind::RightAngle(known), MarkKind::RightAngle(style)) => known == style,
        _ => std::mem::discriminant(&known) == std::mem::discriminant(&kind),
    }
}

fn centroid(points: &[Option<ScreenPoint>]) -> ScreenPoint {
    let valid: Vec<ScreenPoint> = points.iter().flatten().copied().collect();
    let count = f64::from(u32::try_from(valid.len()).unwrap_or(u32::MAX).max(1));
    let sum = valid
        .iter()
        .fold((0.0, 0.0), |sum, point| (sum.0 + point.0, sum.1 + point.1));
    (sum.0 / count, sum.1 / count)
}

pub(crate) fn label_text(label: &Label, text: &PictureText) -> String {
    let mut words = label.name.clone();
    match &label.quantity {
        Quantity::Named => {}
        Quantity::Given(value) | Quantity::Answered(value) => {
            words.push_str(EQUALS);
            words.push_str(&value.printed);
        }
        Quantity::Sought { .. } => {
            words.push_str(EQUALS);
            words.push_str(SOUGHT);
        }
    }
    let designation = label.designation.map(|designation| match designation {
        Designation::Hypotenuse => &text.designations.hypotenuse,
        Designation::Leg => &text.designations.leg,
        Designation::Height => &text.designations.height,
    });
    if let Some(designation) = designation.filter(|designation| !designation.is_empty()) {
        words.push(' ');
        words.push_str(designation);
    }
    words
}

fn arm_ends(placed: &Placed) -> (LayoutPoint, LayoutPoint) {
    (layout_point(placed.end_arm), layout_point(placed.start_arm))
}

struct MarkPen<'a, 'b> {
    plane: &'a PlaneView<'b>,
    sizes: &'a Sizes,
    colour: crate::colour::Colour,
}

fn draw_angle_mark(
    painter: &mut Painter<'_>,
    pen: &MarkPen<'_, '_>,
    layout: &mut FigureLayout,
    (angle, placed): (AngleIndex, &Placed),
    kind: MarkKind,
) -> Result<(), RenderError> {
    let (plane, sizes, colour) = (pen.plane, pen.sizes, pen.colour);
    let clip = plane.clip.as_ref();
    let width = crate::mapping::to_f32(sizes.thin);
    let (first_arm_end, second_arm_end) = arm_ends(placed);
    let mut arc_layout = ArcLayout {
        angle,
        vertex: layout_point(placed.vertex),
        first_arm_end,
        second_arm_end,
        radius: sizes.radius,
        arc_count: 0,
        arc_spacing: sizes.spacing,
        dot: None,
        square_side: None,
        is_name_outside: false,
    };
    let element = Some(ElementIndex::Angle(angle));
    let arc = |radius: f64| CircularArc {
        centre: placed.vertex,
        radius,
        start: placed.start_angle,
        sweep: placed.sweep,
    };
    let stroke_arc =
        |painter: &mut Painter<'_>, layout: &mut FigureLayout, circular: CircularArc| {
            let mut builder = PathBuilder::new();
            circular.push_to(&mut builder, true);
            if let Some(path) = builder.finish() {
                painter
                    .canvas
                    .stroke_path_within(&path, colour, width, None, clip);
            }
            layout.strokes.push(StrokeLayout {
                element,
                points: circular
                    .layout_points()
                    .into_iter()
                    .map(layout_point)
                    .collect(),
                width: sizes.thin,
            });
        };
    match kind {
        MarkKind::AngleArc { count } => {
            for index in 0..count.max(1) {
                stroke_arc(
                    painter,
                    layout,
                    arc(sizes.radius + f64::from(index) * sizes.spacing),
                );
            }
            arc_layout.arc_count = count.max(1);
        }
        MarkKind::Direction => {
            let circular = arc(sizes.radius);
            stroke_arc(painter, layout, circular);
            let end_angle = placed.start_angle + placed.sweep;
            let tip = circular.point_at(end_angle);
            let tangent = (
                calc_numbers::sin_f64(end_angle),
                -calc_numbers::cos_f64(end_angle),
            );
            let reach = sizes.spacing * 4.0;
            let base = (tip.0 - tangent.0 * reach, tip.1 - tangent.1 * reach);
            draw_arrow(painter, &plane.guard, (base, tip), (colour, None), clip)?;
            arc_layout.arc_count = 1;
        }
        MarkKind::RightAngle(RightAngleMark::ArcWithDot) => {
            stroke_arc(painter, layout, arc(sizes.radius));
            let bisector = placed.bisector();
            let rounded = device_pixels(DOT_DIAMETER_EM * sizes.em);
            let diameter = if rounded < SMALLEST_DOT_DIAMETER {
                SMALLEST_DOT_DIAMETER
            } else {
                rounded
            };
            let centre = (
                placed.vertex.0 + bisector.0 * sizes.radius / 2.0,
                placed.vertex.1 + bisector.1 * sizes.radius / 2.0,
            );
            let dot = CircularArc {
                centre,
                radius: diameter / 2.0,
                start: 0.0,
                sweep: TAU,
            };
            let mut builder = PathBuilder::new();
            dot.push_to(&mut builder, true);
            builder.close();
            if let Some(path) = builder.finish() {
                painter.canvas.fill_path_within(&path, colour, clip);
            }
            arc_layout.arc_count = 1;
            arc_layout.dot = Some(DotLayout {
                centre: layout_point(centre),
                diameter,
            });
        }
        MarkKind::RightAngle(RightAngleMark::Square) => {
            let side = device_pixels(SQUARE_SIDE_EM * sizes.em);
            let (Some(first), Some(second)) = (
                unit(difference(placed.vertex, placed.start_arm)),
                unit(difference(placed.vertex, placed.end_arm)),
            ) else {
                return Ok(());
            };
            let corner = |a: f64, b: f64| {
                (
                    placed.vertex.0 + first.0 * a + second.0 * b,
                    placed.vertex.1 + first.1 * a + second.1 * b,
                )
            };
            let square = vec![corner(side, 0.0), corner(side, side), corner(0.0, side)];
            if let Some(path) = polyline_path(&plane.guard, std::slice::from_ref(&square)) {
                painter
                    .canvas
                    .stroke_path_within(&path, colour, width, None, clip);
            }
            layout.strokes.push(StrokeLayout {
                element,
                points: square.into_iter().map(layout_point).collect(),
                width: sizes.thin,
            });
            arc_layout.square_side = Some(side);
        }
        MarkKind::EqualTicks { .. } => return Ok(()),
    }
    layout.arcs.push(arc_layout);
    Ok(())
}

fn draw_ticks(
    painter: &mut Painter<'_>,
    pen: &MarkPen<'_, '_>,
    layout: &mut FigureLayout,
    (segment, count): (SegmentIndex, u8),
    (from, to): (ScreenPoint, ScreenPoint),
) {
    let (plane, sizes, colour) = (pen.plane, pen.sizes, pen.colour);
    let Some(along) = unit(difference(from, to)) else {
        return;
    };
    let normal = (-along.1, along.0);
    let length = device_pixels(TICK_LENGTH_EM * sizes.em);
    let spacing = device_pixels(TICK_SPACING_EM * sizes.em);
    let middle = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
    let count = count.max(1);
    let first_offset = -spacing * f64::from(count - 1) / 2.0;
    let mut runs = Vec::new();
    for index in 0..count {
        let offset = first_offset + spacing * f64::from(index);
        let centre = (middle.0 + along.0 * offset, middle.1 + along.1 * offset);
        let half = length / 2.0;
        let tick = vec![
            (centre.0 - normal.0 * half, centre.1 - normal.1 * half),
            (centre.0 + normal.0 * half, centre.1 + normal.1 * half),
        ];
        layout.strokes.push(StrokeLayout {
            element: Some(ElementIndex::Segment(segment)),
            points: tick.iter().copied().map(layout_point).collect(),
            width: sizes.thin,
        });
        runs.push(tick);
    }
    if let Some(path) = polyline_path(&plane.guard, &runs) {
        painter.canvas.stroke_path_within(
            &path,
            colour,
            crate::mapping::to_f32(sizes.thin),
            None,
            plane.clip.as_ref(),
        );
    }
    layout.ticks.push(TickLayout {
        segment,
        count,
        length,
        spacing,
    });
}

fn angle_label_centre(
    sizes: &Sizes,
    layout: &mut FigureLayout,
    angle: AngleIndex,
    placed: &Placed,
    width: f64,
    height: f64,
) -> ScreenPoint {
    let bisector = placed.bisector();
    let half_diagonal = (width * width + height * height).sqrt() / 2.0;
    let arc = layout.arcs.iter_mut().find(|arc| arc.angle == angle);
    let (count, has_dot) = arc.as_ref().map_or((0, false), |arc| {
        (
            arc.arc_count,
            arc.dot.is_some() || arc.square_side.is_some(),
        )
    });
    let outer = sizes.radius + f64::from(count.saturating_sub(1)) * sizes.spacing;
    let inside_distance = sizes.radius - sizes.margin - half_diagonal;
    let half_sweep = placed.sweep.abs() / 2.0;
    let fits_inside = !has_dot
        && inside_distance > 0.0
        && half_sweep < PI / 2.0
        && inside_distance * calc_numbers::sin_f64(half_sweep) >= half_diagonal + sizes.margin;
    let distance = if fits_inside {
        inside_distance
    } else {
        outer + sizes.margin + half_diagonal + sizes.thin + CLEARANCE
    };
    if let Some(arc) = arc {
        arc.is_name_outside = !fits_inside;
    }
    (
        placed.vertex.0 + bisector.0 * distance,
        placed.vertex.1 + bisector.1 * distance,
    )
}

#[cfg(test)]
mod tests {
    use calc_viz::{
        Angle, CoherentValue, Designation, Dimension, Label, LabelValue, NameKind, PointIndex,
    };

    use super::*;

    fn label(quantity: Quantity, designation: Option<Designation>) -> Label {
        Label {
            element: ElementIndex::Segment(SegmentIndex(0)),
            names: NameKind::Side,
            name: String::from("c"),
            quantity,
            designation,
        }
    }

    fn given(printed: &str) -> Quantity {
        Quantity::Given(LabelValue {
            value: CoherentValue::Rational(calc_numbers::Number::from(5_i64)),
            dimension: Dimension::DIMENSIONLESS,
            printed: String::from(printed),
        })
    }

    #[test]
    fn device_pixels_round_ties_away_from_zero() {
        assert_eq!((device_pixels(4.5), device_pixels(-4.5)), (5.0, -5.0));
    }

    #[test]
    fn odd_stroke_centre_sits_on_a_half_pixel() {
        assert_eq!(snap_centre((10.2, 7.9), 1.0), (10.5, 7.5));
    }

    #[test]
    fn even_stroke_centre_sits_on_a_whole_pixel() {
        assert_eq!(snap_centre((10.2, 7.6), 2.0), (10.0, 8.0));
    }

    #[test]
    fn named_label_shows_only_its_name() {
        assert_eq!(
            label_text(&label(Quantity::Named, None), &PictureText::default()),
            "c"
        );
    }

    #[test]
    fn given_label_shows_name_and_printed_value() {
        assert_eq!(
            label_text(&label(given("5 cm"), None), &PictureText::default()),
            "c = 5 cm"
        );
    }

    #[test]
    fn sought_label_shows_a_question_mark() {
        let sought = label(Quantity::Sought { step: None }, None);

        assert_eq!(label_text(&sought, &PictureText::default()), "c = ?");
    }

    #[test]
    fn designation_word_follows_the_label() {
        let mut text = PictureText::default();
        text.designations.hypotenuse = String::from("hypotenuse");

        let words = label_text(
            &label(Quantity::Named, Some(Designation::Hypotenuse)),
            &text,
        );

        assert_eq!(words, "c hypotenuse");
    }

    #[test]
    fn right_angle_marks_of_different_styles_are_different_keys() {
        assert!(!same_key(
            MarkKind::RightAngle(RightAngleMark::Square),
            MarkKind::RightAngle(RightAngleMark::ArcWithDot)
        ));
    }

    #[test]
    fn arcs_with_different_counts_share_one_key() {
        assert!(same_key(
            MarkKind::AngleArc { count: 1 },
            MarkKind::AngleArc { count: 3 }
        ));
    }

    fn figure_with_angle(is_oriented: bool) -> Figure {
        Figure {
            points: [
                calc_viz::Column::F64(vec![0.0, 1.0, 0.0]),
                calc_viz::Column::F64(vec![0.0, 0.0, 1.0]),
            ],
            segments: Vec::new(),
            angles: vec![Angle {
                vertex: PointIndex(0),
                first_arm: PointIndex(2),
                second_arm: PointIndex(1),
                is_oriented,
            }],
            marks: Vec::new(),
            labels: Vec::new(),
            scale: calc_viz::FigureScale::Sketch,
        }
    }

    fn screen_points() -> Vec<Option<ScreenPoint>> {
        vec![Some((0.0, 0.0)), Some((10.0, 0.0)), Some((0.0, -10.0))]
    }

    #[test]
    fn unoriented_reflex_angle_is_drawn_as_its_interior_angle() {
        let placed =
            place_angle(&figure_with_angle(false), &screen_points(), AngleIndex(0)).unwrap();

        assert!((placed.sweep.abs() - PI / 2.0).abs() < 1e-12);
    }

    #[test]
    fn oriented_angle_keeps_its_counterclockwise_sweep() {
        let placed =
            place_angle(&figure_with_angle(true), &screen_points(), AngleIndex(0)).unwrap();

        assert!((placed.sweep.abs() - 3.0 * PI / 2.0).abs() < 1e-12);
    }
}
