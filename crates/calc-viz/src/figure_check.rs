use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use calc_numbers::{Integer, Interval, Number, Truth, checked_atan2_f64};

use crate::exact_order::compare_exact;
use crate::figure::{
    AngleIndex, CoherentValue, Designation, ElementIndex, Figure, FigureScale, Label, MarkKind,
    NameKind, PointIndex, Quantity, RightAngleMark, SegmentIndex,
};
use crate::primitive::Column;
use crate::view::Dimension;

const LENGTH_EXPONENT: usize = 0;
const DRAWING_TOLERANCE_NUMERATOR_LOW: i64 = 49;
const DRAWING_TOLERANCE_NUMERATOR_HIGH: i64 = 51;
const DRAWING_TOLERANCE_DENOMINATOR: i64 = 50;
const DEGREES_PER_HALF_TURN: f64 = 180.0;
const FIRST_EQUALITY_ARC_COUNT: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameTriple {
    pub vertex: String,
    pub side: String,
    pub angle: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VertexOrder {
    Counterclockwise,
    Unordered,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FigureNotation {
    pub triples: Vec<NameTriple>,
    pub angle_by_points: Vec<String>,
    pub vertex_order: VertexOrder,
    pub right_angle_mark: RightAngleMark,
    pub allows_equality_marks: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskName {
    pub name: String,
    pub kind: NameKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FigureTask {
    pub names: Vec<TaskName>,
    pub right_angles: Vec<AngleIndex>,
    pub designations: Vec<(String, Designation)>,
    pub equalities: Vec<(ElementIndex, ElementIndex)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FigureFault {
    VertexLabelNotOnPoint { label: usize },
    VertexLabelsShareName { first: usize, second: usize },
    VertexLabelsSharePoint { first: usize, second: usize },
    SideOnWrongSegment { label: usize },
    AngleAtWrongVertex { label: usize },
    VerticesNotCounterclockwise,
    TaskNameMissing { name: String },
    LabelNameUnused { label: usize },
    RightAngleMarkWithoutRightAngle { mark: usize },
    RightAngleUnmarked { angle: AngleIndex },
    RightAngleMarkStyle { mark: usize },
    HypotenuseTouchesRightAngle { segment: SegmentIndex },
    ValueMissing { label: usize },
    DimensionMismatch { label: usize },
    ScaleMismatch { label: usize },
    ScaleUndecided { label: usize },
    NameNamesTwoElements { name: String },
    TaskNameKindDiffers { name: String },
    EqualityMarksNotAllowed { mark: usize },
    EqualityNotGiven { first: usize, second: usize },
    DirectionOnUnorientedAngle { mark: usize },
    SeveralSoughtWithoutStep,
    SoughtStepRepeated { step: u32 },
    MarkNotUsed { mark: usize },
    LabelTooCloseToStroke { label: usize },
    LabelsTooClose { first: usize, second: usize },
    StrokeWidthWrong { stroke: usize },
    ArcRadiusWrong { angle: AngleIndex },
    ArcSpacingWrong { angle: AngleIndex },
    SquareSizeWrong { angle: AngleIndex },
    DotMisplaced { angle: AngleIndex },
    ArmTooShort { angle: AngleIndex },
    ArcTooSmall { angle: AngleIndex },
    AngleNameOffBisector { label: usize },
    TickSizeWrong { segment: SegmentIndex },
    LabelAwayFromElement { label: usize },
    KeyMissesMark { mark: usize },
}

enum Decision {
    Pass,
    Fail,
    Undecided,
}

fn column_value(column: &Column, index: usize) -> Option<Number> {
    match column {
        Column::F32(values) => values.get(index).map(|value| Number::F32(*value)),
        Column::F64(values) => values.get(index).map(|value| Number::F64(*value)),
    }
    .and_then(|value| value.to_exact().ok())
}

fn exact_point(figure: &Figure, point: PointIndex) -> Option<(Number, Number)> {
    let index = usize::try_from(point.0).ok()?;
    let [x, y] = &figure.points;
    Some((column_value(x, index)?, column_value(y, index)?))
}

fn difference(from: &(Number, Number), to: &(Number, Number)) -> Option<(Number, Number)> {
    Some((to.0.sub_exact(&from.0).ok()?, to.1.sub_exact(&from.1).ok()?))
}

fn cross(first: &(Number, Number), second: &(Number, Number)) -> Option<Number> {
    first
        .0
        .mul_exact(&second.1)
        .ok()?
        .sub_exact(&first.1.mul_exact(&second.0).ok()?)
        .ok()
}

fn dot(first: &(Number, Number), second: &(Number, Number)) -> Option<Number> {
    first
        .0
        .mul_exact(&second.0)
        .ok()?
        .add_exact(&first.1.mul_exact(&second.1).ok()?)
        .ok()
}

fn vertex_names(figure: &Figure) -> BTreeMap<PointIndex, &str> {
    let mut names = BTreeMap::new();
    for label in &figure.labels {
        if let (NameKind::Vertex, ElementIndex::Point(point)) = (label.names, label.element) {
            names.entry(point).or_insert(label.name.as_str());
        }
    }
    names
}

fn point_named(names: &BTreeMap<PointIndex, &str>, name: &str) -> Option<PointIndex> {
    names
        .iter()
        .find(|(_, vertex)| **vertex == name)
        .map(|(point, _)| *point)
}

fn check_vertex_labels(figure: &Figure, faults: &mut Vec<FigureFault>) {
    let vertex_labels: Vec<(usize, &Label)> = figure
        .labels
        .iter()
        .enumerate()
        .filter(|(_, label)| label.names == NameKind::Vertex)
        .collect();
    for (position, (index, label)) in vertex_labels.iter().enumerate() {
        if !matches!(label.element, ElementIndex::Point(_)) {
            faults.push(FigureFault::VertexLabelNotOnPoint { label: *index });
        }
        for (other_index, other) in vertex_labels.iter().skip(position + 1) {
            if label.name == other.name {
                faults.push(FigureFault::VertexLabelsShareName {
                    first: *index,
                    second: *other_index,
                });
            }
            if label.element == other.element {
                faults.push(FigureFault::VertexLabelsSharePoint {
                    first: *index,
                    second: *other_index,
                });
            }
        }
    }
}

fn check_side_labels(figure: &Figure, notation: &FigureNotation, faults: &mut Vec<FigureFault>) {
    let names = vertex_names(figure);
    for (index, label) in figure.labels.iter().enumerate() {
        let (NameKind::Side, ElementIndex::Segment(segment)) = (label.names, label.element) else {
            continue;
        };
        let Some(triple) = notation
            .triples
            .iter()
            .find(|triple| triple.side == label.name)
        else {
            continue;
        };
        let others: BTreeSet<&str> = notation
            .triples
            .iter()
            .filter(|other| other.vertex != triple.vertex)
            .map(|other| other.vertex.as_str())
            .collect();
        let ends: Option<BTreeSet<&str>> = figure.segment(segment).and_then(|segment| {
            Some(
                [*names.get(&segment.from)?, *names.get(&segment.to)?]
                    .into_iter()
                    .collect(),
            )
        });
        let is_right = ends.is_some_and(|ends| ends.len() == 2 && ends == others);
        if !is_right {
            faults.push(FigureFault::SideOnWrongSegment { label: index });
        }
    }
}

fn split_three_names<'name>(text: &'name str, vertices: &[&str]) -> Option<[&'name str; 3]> {
    let mut parts = Vec::with_capacity(3);
    let mut rest = text;
    while !rest.is_empty() {
        let vertex = vertices
            .iter()
            .filter(|vertex| rest.starts_with(**vertex))
            .max_by_key(|vertex| vertex.len())?;
        let (part, remainder) = rest.split_at(vertex.len());
        parts.push(part);
        rest = remainder;
    }
    <[&str; 3]>::try_from(parts).ok()
}

fn check_angle_labels(figure: &Figure, notation: &FigureNotation, faults: &mut Vec<FigureFault>) {
    let names = vertex_names(figure);
    let vertices: Vec<&str> = notation
        .triples
        .iter()
        .map(|triple| triple.vertex.as_str())
        .collect();
    for (index, label) in figure.labels.iter().enumerate() {
        let (NameKind::Angle, ElementIndex::Angle(angle)) = (label.names, label.element) else {
            continue;
        };
        let Some(angle) = figure.angle(angle) else {
            faults.push(FigureFault::AngleAtWrongVertex { label: index });
            continue;
        };
        let name_of = |point: PointIndex| names.get(&point).copied();
        let is_right = if let Some(triple) = notation
            .triples
            .iter()
            .find(|triple| triple.angle == label.name)
        {
            Some(name_of(angle.vertex) == Some(triple.vertex.as_str()))
        } else {
            notation
                .angle_by_points
                .iter()
                .find_map(|symbol| label.name.strip_prefix(symbol.as_str()))
                .and_then(|rest| split_three_names(rest, &vertices))
                .map(|[first, middle, last]| {
                    let arms: BTreeSet<Option<&str>> =
                        [name_of(angle.first_arm), name_of(angle.second_arm)]
                            .into_iter()
                            .collect();
                    let named: BTreeSet<Option<&str>> =
                        [Some(first), Some(last)].into_iter().collect();
                    name_of(angle.vertex) == Some(middle) && arms == named
                })
        };
        if is_right == Some(false) {
            faults.push(FigureFault::AngleAtWrongVertex { label: index });
        }
    }
}

fn check_vertex_order(figure: &Figure, notation: &FigureNotation, faults: &mut Vec<FigureFault>) {
    if notation.vertex_order != VertexOrder::Counterclockwise {
        return;
    }
    let names = vertex_names(figure);
    let corners: Option<Vec<(Number, Number)>> = notation
        .triples
        .iter()
        .take(3)
        .map(|triple| exact_point(figure, point_named(&names, &triple.vertex)?))
        .collect();
    let Some([first, second, third]) = corners.and_then(|corners| <[_; 3]>::try_from(corners).ok())
    else {
        return;
    };
    let area = difference(&first, &second)
        .zip(difference(&first, &third))
        .and_then(|(edge, other)| cross(&edge, &other));
    let is_counterclockwise = area
        .is_some_and(|area| compare_exact(&area, &Number::from(0_i64)) == Some(Ordering::Greater));
    if !is_counterclockwise {
        faults.push(FigureFault::VerticesNotCounterclockwise);
    }
}

fn check_names_used(
    figure: &Figure,
    notation: &FigureNotation,
    task: &FigureTask,
    faults: &mut Vec<FigureFault>,
) {
    for task_name in &task.names {
        let is_labelled = figure
            .labels
            .iter()
            .any(|label| label.name == task_name.name && label.names == task_name.kind);
        if !is_labelled {
            faults.push(FigureFault::TaskNameMissing {
                name: task_name.name.clone(),
            });
        }
    }
    let in_task = |name: &str| task.names.iter().any(|task_name| task_name.name == name);
    for (index, label) in figure.labels.iter().enumerate() {
        let is_valued = matches!(label.quantity, Quantity::Given(_) | Quantity::Answered(_));
        let is_corresponding_vertex = label.names == NameKind::Vertex
            && notation.triples.iter().any(|triple| {
                triple.vertex == label.name && (in_task(&triple.side) || in_task(&triple.angle))
            });
        if !(in_task(&label.name) || is_valued || is_corresponding_vertex) {
            faults.push(FigureFault::LabelNameUnused { label: index });
        }
    }
}

fn check_right_angles(
    figure: &Figure,
    notation: &FigureNotation,
    task: &FigureTask,
    faults: &mut Vec<FigureFault>,
) {
    let mut marked = BTreeSet::new();
    for (index, mark) in figure.marks.iter().enumerate() {
        let (MarkKind::RightAngle(style), ElementIndex::Angle(angle)) = (mark.kind, mark.element)
        else {
            continue;
        };
        marked.insert(angle);
        if !task.right_angles.contains(&angle) {
            faults.push(FigureFault::RightAngleMarkWithoutRightAngle { mark: index });
        }
        if style != notation.right_angle_mark {
            faults.push(FigureFault::RightAngleMarkStyle { mark: index });
        }
    }
    for angle in &task.right_angles {
        if !marked.contains(angle) {
            faults.push(FigureFault::RightAngleUnmarked { angle: *angle });
        }
    }
    let right_vertices: Vec<PointIndex> = marked
        .iter()
        .filter_map(|angle| figure.angle(*angle).map(|angle| angle.vertex))
        .collect();
    let hypotenuses: BTreeSet<SegmentIndex> = figure
        .labels
        .iter()
        .filter(|label| {
            label.designation == Some(Designation::Hypotenuse)
                || task.designations.iter().any(|(name, designation)| {
                    *designation == Designation::Hypotenuse && *name == label.name
                })
        })
        .filter_map(|label| match label.element {
            ElementIndex::Segment(segment) => Some(segment),
            ElementIndex::Point(_) | ElementIndex::Angle(_) => None,
        })
        .collect();
    for segment_index in hypotenuses {
        let touches = figure.segment(segment_index).is_some_and(|segment| {
            right_vertices
                .iter()
                .any(|vertex| segment.from == *vertex || segment.to == *vertex)
        });
        if touches {
            faults.push(FigureFault::HypotenuseTouchesRightAngle {
                segment: segment_index,
            });
        }
    }
}

fn fraction(numerator: i64, denominator: i64) -> Option<Number> {
    Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).ok()
}

fn pi_enclosure() -> Option<Interval> {
    Some(
        Interval::point(std::f64::consts::PI)?
            .hull(&Interval::point(std::f64::consts::PI.next_up())?),
    )
}

fn exact_bounds(value: &CoherentValue) -> Option<(Number, Number)> {
    match value {
        CoherentValue::Rational(number) => Some((number.clone(), number.clone())),
        CoherentValue::PiMultiple(multiple) => {
            let lower = multiple
                .mul_exact(&Number::F64(std::f64::consts::PI).to_exact().ok()?)
                .ok()?;
            let upper = multiple
                .mul_exact(
                    &Number::F64(std::f64::consts::PI.next_up())
                        .to_exact()
                        .ok()?,
                )
                .ok()?;
            match compare_exact(&lower, &upper)? {
                Ordering::Greater => Some((upper, lower)),
                Ordering::Less | Ordering::Equal => Some((lower, upper)),
            }
        }
        CoherentValue::Enclosure { lower, upper } => Some((lower.clone(), upper.clone())),
    }
}

fn length_decision(
    figure: &Figure,
    segment: SegmentIndex,
    value: &CoherentValue,
) -> Option<Decision> {
    let FigureScale::ToScale {
        view_units_per_metre,
    } = &figure.scale
    else {
        return None;
    };
    Some(
        compare_length(figure, view_units_per_metre, segment, value).unwrap_or(Decision::Undecided),
    )
}

fn compare_length(
    figure: &Figure,
    view_units_per_metre: &Number,
    segment: SegmentIndex,
    value: &CoherentValue,
) -> Option<Decision> {
    let segment = figure.segment(segment)?;
    let edge = difference(
        &exact_point(figure, segment.from)?,
        &exact_point(figure, segment.to)?,
    )?;
    let squared_length = dot(&edge, &edge)?;
    let passes = |given: &Number| -> Option<bool> {
        let scaled = view_units_per_metre.mul_exact(given).ok()?;
        let bound = |numerator: i64| -> Option<Number> {
            let factor = fraction(numerator, DRAWING_TOLERANCE_DENOMINATOR)?;
            let length = factor.mul_exact(&scaled).ok()?;
            length.mul_exact(&length).ok()
        };
        let low = bound(DRAWING_TOLERANCE_NUMERATOR_LOW)?;
        let high = bound(DRAWING_TOLERANCE_NUMERATOR_HIGH)?;
        Some(
            compare_exact(&squared_length, &low)? != Ordering::Less
                && compare_exact(&squared_length, &high)? != Ordering::Greater,
        )
    };
    let (lower, upper) = exact_bounds(value)?;
    Some(match (passes(&lower)?, passes(&upper)?) {
        (true, true) => Decision::Pass,
        (false, false) => Decision::Fail,
        _ => Decision::Undecided,
    })
}

fn angle_decision(figure: &Figure, angle: AngleIndex, value: &CoherentValue) -> Option<Decision> {
    if !matches!(figure.scale, FigureScale::ToScale { .. }) {
        return None;
    }
    Some(compare_angle(figure, angle, value).unwrap_or(Decision::Undecided))
}

fn compare_angle(figure: &Figure, angle: AngleIndex, value: &CoherentValue) -> Option<Decision> {
    let angle = figure.angle(angle)?;
    let vertex = exact_point(figure, angle.vertex)?;
    let first = difference(&vertex, &exact_point(figure, angle.first_arm)?)?;
    let second = difference(&vertex, &exact_point(figure, angle.second_arm)?)?;
    let sine = Interval::from_exact(&cross(&first, &second)?)?;
    let cosine = Interval::from_exact(&dot(&first, &second)?)?;
    if sine.contains_zero() && cosine.upper() >= 0.0 {
        return Some(Decision::Undecided);
    }
    let pi = pi_enclosure()?;
    let full_turn = pi.add(&pi)?;
    let corners = [
        (sine.lower(), cosine.lower()),
        (sine.lower(), cosine.upper()),
        (sine.upper(), cosine.lower()),
        (sine.upper(), cosine.upper()),
    ];
    let mut drawn: Option<Interval> = None;
    for (sine_corner, cosine_corner) in corners {
        let principal = checked_atan2_f64(sine_corner, cosine_corner).ok()?;
        let widened =
            Interval::point(principal.next_down())?.hull(&Interval::point(principal.next_up())?);
        let normalized = if sine_corner.is_sign_negative() {
            widened.add(&full_turn)?
        } else {
            widened
        };
        drawn = Some(match drawn {
            Some(enclosure) => enclosure.hull(&normalized),
            None => normalized,
        });
    }
    let drawn = drawn?;
    let given = match value {
        CoherentValue::Rational(number) => Interval::from_exact(number)?,
        CoherentValue::PiMultiple(multiple) => Interval::from_exact(multiple)?.mul(&pi)?,
        CoherentValue::Enclosure { lower, upper } => {
            Interval::from_exact(lower)?.hull(&Interval::from_exact(upper)?)
        }
    };
    let tolerance = pi.div(&Interval::point(DEGREES_PER_HALF_TURN)?)?;
    Some(match drawn.sub(&given)?.abs().less_or_equal(&tolerance) {
        Truth::True => Decision::Pass,
        Truth::False => Decision::Fail,
        Truth::Unknown => Decision::Undecided,
    })
}

fn is_length(dimension: &Dimension) -> bool {
    dimension
        .exponents
        .iter()
        .enumerate()
        .all(|(index, exponent)| *exponent == i8::from(index == LENGTH_EXPONENT))
}

fn check_values(figure: &Figure, faults: &mut Vec<FigureFault>) {
    for (index, label) in figure.labels.iter().enumerate() {
        let (Quantity::Given(value) | Quantity::Answered(value)) = &label.quantity else {
            continue;
        };
        if value.printed.is_empty() {
            faults.push(FigureFault::ValueMissing { label: index });
        }
        let decision = match label.element {
            ElementIndex::Segment(segment) => {
                if !is_length(&value.dimension) {
                    faults.push(FigureFault::DimensionMismatch { label: index });
                    continue;
                }
                length_decision(figure, segment, &value.value)
            }
            ElementIndex::Angle(angle) => {
                if value.dimension != Dimension::DIMENSIONLESS {
                    faults.push(FigureFault::DimensionMismatch { label: index });
                    continue;
                }
                angle_decision(figure, angle, &value.value)
            }
            ElementIndex::Point(_) => None,
        };
        match decision {
            None | Some(Decision::Pass) => {}
            Some(Decision::Fail) => faults.push(FigureFault::ScaleMismatch { label: index }),
            Some(Decision::Undecided) => faults.push(FigureFault::ScaleUndecided { label: index }),
        }
    }
}

fn check_unique_names(figure: &Figure, task: &FigureTask, faults: &mut Vec<FigureFault>) {
    let mut elements: BTreeMap<&str, BTreeSet<ElementIndex>> = BTreeMap::new();
    for label in &figure.labels {
        elements
            .entry(label.name.as_str())
            .or_default()
            .insert(label.element);
    }
    for (name, named) in &elements {
        if named.len() > 1 {
            faults.push(FigureFault::NameNamesTwoElements {
                name: (*name).to_string(),
            });
        }
    }
    for task_name in &task.names {
        let differs = figure
            .labels
            .iter()
            .any(|label| label.name == task_name.name && label.names != task_name.kind);
        if differs {
            faults.push(FigureFault::TaskNameKindDiffers {
                name: task_name.name.clone(),
            });
        }
    }
}

fn check_equality_marks(
    figure: &Figure,
    notation: &FigureNotation,
    task: &FigureTask,
    faults: &mut Vec<FigureFault>,
) {
    let mut groups: BTreeMap<(bool, u8), Vec<usize>> = BTreeMap::new();
    for (index, mark) in figure.marks.iter().enumerate() {
        let is_named = figure
            .labels
            .iter()
            .any(|label| label.element == mark.element && label.names == NameKind::Angle);
        let key = match mark.kind {
            MarkKind::AngleArc { count } if count >= FIRST_EQUALITY_ARC_COUNT => (true, count),
            MarkKind::AngleArc { count } if notation.allows_equality_marks && !is_named => {
                (true, count)
            }
            MarkKind::EqualTicks { count } => (false, count),
            MarkKind::AngleArc { .. } | MarkKind::RightAngle(_) | MarkKind::Direction => continue,
        };
        if !notation.allows_equality_marks {
            faults.push(FigureFault::EqualityMarksNotAllowed { mark: index });
            continue;
        }
        groups.entry(key).or_default().push(index);
    }
    for members in groups.values() {
        for (position, first) in members.iter().enumerate() {
            for second in members.iter().skip(position + 1) {
                let (Some(first_mark), Some(second_mark)) =
                    (figure.marks.get(*first), figure.marks.get(*second))
                else {
                    continue;
                };
                let pair = (first_mark.element, second_mark.element);
                let is_given = task
                    .equalities
                    .iter()
                    .any(|given| *given == pair || (given.1, given.0) == pair);
                if !is_given {
                    faults.push(FigureFault::EqualityNotGiven {
                        first: *first,
                        second: *second,
                    });
                }
            }
        }
    }
}

fn check_directions(figure: &Figure, faults: &mut Vec<FigureFault>) {
    for (index, mark) in figure.marks.iter().enumerate() {
        if mark.kind != MarkKind::Direction {
            continue;
        }
        let is_oriented = match mark.element {
            ElementIndex::Angle(angle) => {
                figure.angle(angle).is_some_and(|angle| angle.is_oriented)
            }
            ElementIndex::Point(_) | ElementIndex::Segment(_) => false,
        };
        if !is_oriented {
            faults.push(FigureFault::DirectionOnUnorientedAngle { mark: index });
        }
    }
}

fn check_sought_steps(figure: &Figure, faults: &mut Vec<FigureFault>) {
    let mut without_step = 0_usize;
    let mut steps = BTreeSet::new();
    for label in &figure.labels {
        match label.quantity {
            Quantity::Sought { step: None } => without_step += 1,
            Quantity::Sought { step: Some(step) } => {
                if !steps.insert(step) {
                    faults.push(FigureFault::SoughtStepRepeated { step });
                }
            }
            Quantity::Named | Quantity::Given(_) | Quantity::Answered(_) => {}
        }
    }
    if without_step > 1 {
        faults.push(FigureFault::SeveralSoughtWithoutStep);
    }
}

fn check_marks_used(figure: &Figure, task: &FigureTask, faults: &mut Vec<FigureFault>) {
    for (index, mark) in figure.marks.iter().enumerate() {
        let element = mark.element;
        let is_labelled_for_task = figure.labels.iter().any(|label| {
            label.element == element
                && (!matches!(label.quantity, Quantity::Named)
                    || task
                        .names
                        .iter()
                        .any(|task_name| task_name.name == label.name))
        });
        let is_right_angle = match element {
            ElementIndex::Angle(angle) => task.right_angles.contains(&angle),
            ElementIndex::Point(_) | ElementIndex::Segment(_) => false,
        };
        let is_in_equality = task
            .equalities
            .iter()
            .any(|(first, second)| *first == element || *second == element);
        if !(is_labelled_for_task || is_right_angle || is_in_equality) {
            faults.push(FigureFault::MarkNotUsed { mark: index });
        }
    }
}

pub fn check_figure(
    figure: &Figure,
    notation: &FigureNotation,
    task: &FigureTask,
) -> Vec<FigureFault> {
    let mut faults = Vec::new();
    check_vertex_labels(figure, &mut faults);
    check_side_labels(figure, notation, &mut faults);
    check_angle_labels(figure, notation, &mut faults);
    check_vertex_order(figure, notation, &mut faults);
    check_names_used(figure, notation, task, &mut faults);
    check_right_angles(figure, notation, task, &mut faults);
    check_values(figure, &mut faults);
    check_unique_names(figure, task, &mut faults);
    check_equality_marks(figure, notation, task, &mut faults);
    check_directions(figure, &mut faults);
    check_sought_steps(figure, &mut faults);
    check_marks_used(figure, task, &mut faults);
    faults
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::figure::{Angle, Label, LabelValue, Mark, Segment, Stroke};

    fn exact(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    pub(crate) fn length() -> Dimension {
        let mut exponents = [0; 8];
        exponents[LENGTH_EXPONENT] = 1;
        Dimension { exponents }
    }

    fn given(value: CoherentValue, dimension: Dimension, printed: &str) -> Quantity {
        Quantity::Given(LabelValue {
            value,
            dimension,
            printed: String::from(printed),
        })
    }

    fn label(element: ElementIndex, names: NameKind, name: &str, quantity: Quantity) -> Label {
        Label {
            element,
            names,
            name: String::from(name),
            quantity,
            designation: None,
        }
    }

    fn point(index: u32) -> ElementIndex {
        ElementIndex::Point(PointIndex(index))
    }

    fn side(index: u32) -> ElementIndex {
        ElementIndex::Segment(SegmentIndex(index))
    }

    fn corner(index: u32) -> ElementIndex {
        ElementIndex::Angle(AngleIndex(index))
    }

    pub(crate) fn right_triangle() -> Figure {
        let segment = |from: u32, to: u32| Segment {
            from: PointIndex(from),
            to: PointIndex(to),
            stroke: Stroke::Solid,
        };
        let angle = |vertex: u32, first_arm: u32, second_arm: u32| Angle {
            vertex: PointIndex(vertex),
            first_arm: PointIndex(first_arm),
            second_arm: PointIndex(second_arm),
            is_oriented: false,
        };
        let mut hypotenuse = label(
            side(2),
            NameKind::Side,
            "c",
            Quantity::Sought { step: None },
        );
        hypotenuse.designation = Some(Designation::Hypotenuse);
        Figure {
            points: [
                Column::F64(vec![4.0, 0.0, 0.0]),
                Column::F64(vec![0.0, 3.0, 0.0]),
            ],
            segments: vec![segment(1, 2), segment(0, 2), segment(0, 1)],
            angles: vec![angle(0, 1, 2), angle(2, 0, 1)],
            marks: vec![Mark {
                element: corner(1),
                kind: MarkKind::RightAngle(RightAngleMark::ArcWithDot),
            }],
            labels: vec![
                label(point(0), NameKind::Vertex, "A", Quantity::Named),
                label(point(1), NameKind::Vertex, "B", Quantity::Named),
                label(point(2), NameKind::Vertex, "C", Quantity::Named),
                label(
                    side(0),
                    NameKind::Side,
                    "a",
                    given(
                        CoherentValue::Rational(Number::from(3_i64)),
                        length(),
                        "3 m",
                    ),
                ),
                label(
                    side(1),
                    NameKind::Side,
                    "b",
                    given(
                        CoherentValue::Rational(Number::from(4_i64)),
                        length(),
                        "4 m",
                    ),
                ),
                hypotenuse,
            ],
            scale: FigureScale::ToScale {
                view_units_per_metre: Number::from(1_i64),
            },
        }
    }

    pub(crate) fn german_notation() -> FigureNotation {
        let triple = |vertex: &str, side: &str, angle: &str| NameTriple {
            vertex: String::from(vertex),
            side: String::from(side),
            angle: String::from(angle),
        };
        FigureNotation {
            triples: vec![
                triple("A", "a", "α"),
                triple("B", "b", "β"),
                triple("C", "c", "γ"),
            ],
            angle_by_points: vec![String::from("∠"), String::from("∡")],
            vertex_order: VertexOrder::Counterclockwise,
            right_angle_mark: RightAngleMark::ArcWithDot,
            allows_equality_marks: false,
        }
    }

    fn task_name(name: &str, kind: NameKind) -> TaskName {
        TaskName {
            name: String::from(name),
            kind,
        }
    }

    pub(crate) fn pythagoras_task() -> FigureTask {
        FigureTask {
            names: vec![
                task_name("a", NameKind::Side),
                task_name("b", NameKind::Side),
                task_name("c", NameKind::Side),
            ],
            right_angles: vec![AngleIndex(1)],
            designations: vec![(String::from("c"), Designation::Hypotenuse)],
            equalities: Vec::new(),
        }
    }

    fn faults_of(
        figure: &Figure,
        notation: &FigureNotation,
        task: &FigureTask,
    ) -> Vec<FigureFault> {
        check_figure(figure, notation, task)
    }

    fn base_faults(figure: &Figure) -> Vec<FigureFault> {
        faults_of(figure, &german_notation(), &pythagoras_task())
    }

    fn angle_label(name: &str, quantity: Quantity, angle: u32) -> Label {
        label(corner(angle), NameKind::Angle, name, quantity)
    }

    #[test]
    fn consistent_right_triangle_has_no_faults() {
        let faults = base_faults(&right_triangle());

        assert_eq!(faults, Vec::new());
    }

    #[test]
    fn c1_two_vertices_named_alike_is_a_fault() {
        let mut figure = right_triangle();
        figure.labels[1].name = String::from("A");

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::VertexLabelsShareName {
            first: 0,
            second: 1
        }));
    }

    #[test]
    fn c1_two_vertex_labels_on_one_point_is_a_fault() {
        let mut figure = right_triangle();
        figure.labels[1].element = point(0);

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::VertexLabelsSharePoint {
            first: 0,
            second: 1
        }));
    }

    #[test]
    fn c2_side_on_a_segment_touching_its_own_vertex_is_a_fault() {
        let mut figure = right_triangle();
        figure.labels[3].element = side(1);
        figure.labels[4].element = side(0);

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::SideOnWrongSegment { label: 3 }));
    }

    #[test]
    fn c2_side_name_outside_the_triples_is_not_checked() {
        let mut figure = right_triangle();
        figure.labels[3].name = String::from("h");

        let faults = base_faults(&figure);

        assert!(
            !faults
                .iter()
                .any(|fault| matches!(fault, FigureFault::SideOnWrongSegment { .. }))
        );
    }

    #[test]
    fn c3_angle_named_for_another_vertex_is_a_fault() {
        let mut figure = right_triangle();
        figure.labels.push(angle_label("α", Quantity::Named, 1));

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::AngleAtWrongVertex { label: 6 }));
    }

    #[test]
    fn c3_angle_by_three_points_at_its_middle_vertex_passes() {
        let mut figure = right_triangle();
        figure.labels.push(angle_label("∠BAC", Quantity::Named, 0));

        let faults = base_faults(&figure);

        assert!(
            !faults
                .iter()
                .any(|fault| matches!(fault, FigureFault::AngleAtWrongVertex { .. }))
        );
    }

    #[test]
    fn c3_angle_by_three_points_with_the_wrong_middle_is_a_fault() {
        let mut figure = right_triangle();
        figure.labels.push(angle_label("∠ABC", Quantity::Named, 0));

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::AngleAtWrongVertex { label: 6 }));
    }

    fn clockwise_triangle() -> Figure {
        let mut figure = right_triangle();
        figure.labels[0].element = point(1);
        figure.labels[1].element = point(0);
        figure
    }

    #[test]
    fn c4_clockwise_vertices_are_a_fault_where_the_order_is_counterclockwise() {
        let faults = base_faults(&clockwise_triangle());

        assert!(faults.contains(&FigureFault::VerticesNotCounterclockwise));
    }

    #[test]
    fn c4_clockwise_vertices_pass_where_the_order_is_free() {
        let mut notation = german_notation();
        notation.vertex_order = VertexOrder::Unordered;

        let faults = faults_of(&clockwise_triangle(), &notation, &pythagoras_task());

        assert!(!faults.contains(&FigureFault::VerticesNotCounterclockwise));
    }

    #[test]
    fn c5_task_name_missing_from_the_figure_is_a_fault() {
        let mut task = pythagoras_task();
        task.names.push(task_name("h", NameKind::Side));

        let faults = faults_of(&right_triangle(), &german_notation(), &task);

        assert!(faults.contains(&FigureFault::TaskNameMissing {
            name: String::from("h")
        }));
    }

    #[test]
    fn c5_vertex_name_no_task_name_depends_on_is_a_fault() {
        let mut task = pythagoras_task();
        task.names.retain(|name| name.name != "a");
        let mut figure = right_triangle();
        figure.labels[3].quantity = Quantity::Named;

        let faults = faults_of(&figure, &german_notation(), &task);

        assert!(faults.contains(&FigureFault::LabelNameUnused { label: 0 }));
    }

    #[test]
    fn c6_right_angle_without_its_mark_is_a_fault() {
        let mut figure = right_triangle();
        figure.marks.clear();

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::RightAngleUnmarked {
            angle: AngleIndex(1)
        }));
    }

    #[test]
    fn c6_right_angle_mark_on_an_angle_not_given_as_right_is_a_fault() {
        let mut figure = right_triangle();
        figure.marks[0].element = corner(0);

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::RightAngleMarkWithoutRightAngle { mark: 0 }));
    }

    #[test]
    fn c6_hypotenuse_at_the_right_angle_is_a_fault() {
        let mut figure = right_triangle();
        figure.labels[5].element = side(0);
        figure.labels[3].element = side(2);

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::HypotenuseTouchesRightAngle {
            segment: SegmentIndex(0)
        }));
    }

    #[test]
    fn point_4_right_angle_mark_of_the_other_curriculum_is_a_fault() {
        let mut figure = right_triangle();
        figure.marks[0].kind = MarkKind::RightAngle(RightAngleMark::Square);

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::RightAngleMarkStyle { mark: 0 }));
    }

    #[test]
    fn c7_given_value_without_printed_text_is_a_fault() {
        let mut figure = right_triangle();
        figure.labels[3].quantity =
            given(CoherentValue::Rational(Number::from(3_i64)), length(), "");

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::ValueMissing { label: 3 }));
    }

    #[test]
    fn c7_side_value_without_length_dimension_is_a_fault() {
        let mut figure = right_triangle();
        figure.labels[3].quantity = given(
            CoherentValue::Rational(Number::from(3_i64)),
            Dimension::DIMENSIONLESS,
            "3",
        );

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::DimensionMismatch { label: 3 }));
    }

    #[test]
    fn c7_length_off_the_drawing_is_a_fault_in_a_figure_to_scale() {
        let mut figure = right_triangle();
        figure.labels[3].quantity = given(
            CoherentValue::Rational(Number::from(4_i64)),
            length(),
            "4 m",
        );

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::ScaleMismatch { label: 3 }));
    }

    #[test]
    fn c7_length_within_two_percent_passes() {
        let mut figure = right_triangle();
        figure.labels[3].quantity =
            given(CoherentValue::Rational(exact(303, 100)), length(), "3.03 m");

        let faults = base_faults(&figure);

        assert!(!faults.contains(&FigureFault::ScaleMismatch { label: 3 }));
    }

    #[test]
    fn c7_length_off_the_drawing_passes_in_a_sketch() {
        let mut figure = right_triangle();
        figure.scale = FigureScale::Sketch;
        figure.labels[3].quantity = given(
            CoherentValue::Rational(Number::from(4_i64)),
            length(),
            "4 m",
        );

        let faults = base_faults(&figure);

        assert_eq!(faults, Vec::new());
    }

    #[test]
    fn c7_enclosure_straddling_the_tolerance_is_undecided() {
        let mut figure = right_triangle();
        figure.labels[3].quantity = given(
            CoherentValue::Enclosure {
                lower: Number::from(3_i64),
                upper: exact(31, 10),
            },
            length(),
            "3 m",
        );

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::ScaleUndecided { label: 3 }));
    }

    #[test]
    fn c7_right_angle_given_as_half_pi_passes() {
        let mut figure = right_triangle();
        figure.labels.push(angle_label(
            "γ",
            given(
                CoherentValue::PiMultiple(exact(1, 2)),
                Dimension::DIMENSIONLESS,
                "90°",
            ),
            1,
        ));

        let faults = base_faults(&figure);

        assert_eq!(faults, Vec::new());
    }

    #[test]
    fn c7_angle_given_as_a_third_of_pi_on_a_right_angle_is_a_fault() {
        let mut figure = right_triangle();
        figure.labels.push(angle_label(
            "γ",
            given(
                CoherentValue::PiMultiple(exact(1, 3)),
                Dimension::DIMENSIONLESS,
                "60°",
            ),
            1,
        ));

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::ScaleMismatch { label: 6 }));
    }

    #[test]
    fn c8_one_name_on_two_elements_is_a_fault() {
        let mut figure = right_triangle();
        figure.labels[4].name = String::from("a");

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::NameNamesTwoElements {
            name: String::from("a")
        }));
    }

    #[test]
    fn c8_task_using_a_name_for_another_kind_is_a_fault() {
        let mut task = pythagoras_task();
        task.names.push(task_name("A", NameKind::Angle));

        let faults = faults_of(&right_triangle(), &german_notation(), &task);

        assert!(faults.contains(&FigureFault::TaskNameKindDiffers {
            name: String::from("A")
        }));
    }

    fn ticked_triangle() -> Figure {
        let mut figure = right_triangle();
        for segment in [0, 1] {
            figure.marks.push(Mark {
                element: side(segment),
                kind: MarkKind::EqualTicks { count: 1 },
            });
        }
        figure
    }

    #[test]
    fn point_3_equality_ticks_where_the_curriculum_does_not_confirm_them_are_a_fault() {
        let faults = base_faults(&ticked_triangle());

        assert!(faults.contains(&FigureFault::EqualityMarksNotAllowed { mark: 1 }));
    }

    #[test]
    fn point_3_equal_ticks_on_sides_not_given_as_equal_are_a_fault() {
        let mut notation = german_notation();
        notation.allows_equality_marks = true;

        let faults = faults_of(&ticked_triangle(), &notation, &pythagoras_task());

        assert!(faults.contains(&FigureFault::EqualityNotGiven {
            first: 1,
            second: 2
        }));
    }

    #[test]
    fn point_3_equal_ticks_on_sides_given_as_equal_pass() {
        let mut notation = german_notation();
        notation.allows_equality_marks = true;
        let mut task = pythagoras_task();
        task.equalities.push((side(1), side(0)));

        let faults = faults_of(&ticked_triangle(), &notation, &task);

        assert_eq!(faults, Vec::new());
    }

    fn arrowed_triangle(is_oriented: bool) -> Figure {
        let mut figure = right_triangle();
        figure.angles[1].is_oriented = is_oriented;
        figure.marks.push(Mark {
            element: corner(1),
            kind: MarkKind::Direction,
        });
        figure
    }

    #[test]
    fn point_7_direction_on_an_unoriented_angle_is_a_fault() {
        let faults = base_faults(&arrowed_triangle(false));

        assert!(faults.contains(&FigureFault::DirectionOnUnorientedAngle { mark: 1 }));
    }

    #[test]
    fn point_7_direction_on_an_oriented_angle_passes() {
        let faults = base_faults(&arrowed_triangle(true));

        assert_eq!(faults, Vec::new());
    }

    fn two_sought(first: Option<u32>, second: Option<u32>) -> Figure {
        let mut figure = right_triangle();
        figure.labels[4].quantity = Quantity::Sought { step: first };
        figure.labels[5].quantity = Quantity::Sought { step: second };
        figure
    }

    #[test]
    fn point_18_two_sought_elements_without_steps_are_a_fault() {
        let faults = base_faults(&two_sought(None, None));

        assert!(faults.contains(&FigureFault::SeveralSoughtWithoutStep));
    }

    #[test]
    fn point_18_two_sought_elements_in_one_step_are_a_fault() {
        let faults = base_faults(&two_sought(Some(1), Some(1)));

        assert!(faults.contains(&FigureFault::SoughtStepRepeated { step: 1 }));
    }

    #[test]
    fn point_18_sought_elements_in_separate_steps_pass() {
        let faults = base_faults(&two_sought(Some(1), Some(2)));

        assert_eq!(faults, Vec::new());
    }

    #[test]
    fn point_20_arc_on_an_angle_the_task_does_not_use_is_a_fault() {
        let mut figure = right_triangle();
        figure.marks.push(Mark {
            element: corner(0),
            kind: MarkKind::AngleArc { count: 1 },
        });

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::MarkNotUsed { mark: 1 }));
    }

    #[test]
    fn point_20_arc_on_an_angle_the_task_names_passes() {
        let mut figure = right_triangle();
        figure.marks.push(Mark {
            element: corner(0),
            kind: MarkKind::AngleArc { count: 1 },
        });
        figure.labels.push(angle_label("α", Quantity::Named, 0));
        let mut task = pythagoras_task();
        task.names.push(task_name("α", NameKind::Angle));

        let faults = faults_of(&figure, &german_notation(), &task);

        assert_eq!(faults, Vec::new());
    }

    fn single_arcs_where_equality_marks_count(
        alpha_is_named: bool,
    ) -> (Figure, FigureNotation, FigureTask) {
        let mut figure = right_triangle();
        for angle in [0, 1] {
            figure.marks.push(Mark {
                element: corner(angle),
                kind: MarkKind::AngleArc { count: 1 },
            });
        }
        let mut task = pythagoras_task();
        if alpha_is_named {
            figure.labels.push(angle_label("α", Quantity::Named, 0));
            task.names.push(task_name("α", NameKind::Angle));
        }
        let mut notation = german_notation();
        notation.allows_equality_marks = true;
        (figure, notation, task)
    }

    #[test]
    fn point_3_single_arcs_on_unnamed_angles_claim_equality_where_marks_count() {
        let (figure, notation, task) = single_arcs_where_equality_marks_count(false);

        let faults = faults_of(&figure, &notation, &task);

        assert!(faults.contains(&FigureFault::EqualityNotGiven {
            first: 1,
            second: 2
        }));
    }

    #[test]
    fn point_3_single_arc_on_a_named_angle_claims_no_equality_where_marks_count() {
        let (figure, notation, task) = single_arcs_where_equality_marks_count(true);

        let faults = faults_of(&figure, &notation, &task);

        assert_eq!(faults, Vec::new());
    }

    fn straight_angle_figure(second_arm: (f64, f64)) -> Figure {
        Figure {
            points: [
                Column::F64(vec![0.0, 0.1, second_arm.0]),
                Column::F64(vec![0.0, 0.0, second_arm.1]),
            ],
            segments: Vec::new(),
            angles: vec![Angle {
                vertex: PointIndex(0),
                first_arm: PointIndex(1),
                second_arm: PointIndex(2),
                is_oriented: false,
            }],
            marks: Vec::new(),
            labels: vec![angle_label(
                "δ",
                given(
                    CoherentValue::PiMultiple(Number::from(1_i64)),
                    Dimension::DIMENSIONLESS,
                    "180°",
                ),
                0,
            )],
            scale: FigureScale::ToScale {
                view_units_per_metre: Number::from(1_i64),
            },
        }
    }

    fn scale_faults(second_arm: (f64, f64)) -> Vec<FigureFault> {
        faults_of(
            &straight_angle_figure(second_arm),
            &german_notation(),
            &FigureTask::default(),
        )
        .into_iter()
        .filter(|fault| {
            matches!(
                fault,
                FigureFault::ScaleMismatch { .. } | FigureFault::ScaleUndecided { .. }
            )
        })
        .collect()
    }

    #[test]
    fn c7_straight_angle_of_exactly_pi_is_decided() {
        assert_eq!(scale_faults((-1.0, 0.0)), Vec::new());
    }

    #[test]
    fn c7_angle_just_below_pi_is_decided() {
        assert_eq!(scale_faults((-0.1, 5e-324)), Vec::new());
    }

    #[test]
    fn c7_angle_just_above_pi_is_decided() {
        assert_eq!(scale_faults((-0.1, -5e-324)), Vec::new());
    }

    #[test]
    fn c7_angle_near_zero_is_undecided() {
        assert_eq!(
            scale_faults((0.1, 5e-324)),
            vec![FigureFault::ScaleUndecided { label: 0 }]
        );
    }

    #[test]
    fn c7_angle_near_a_full_turn_is_undecided() {
        assert_eq!(
            scale_faults((0.1, -5e-324)),
            vec![FigureFault::ScaleUndecided { label: 0 }]
        );
    }

    #[test]
    fn c7_angle_without_an_enclosure_is_undecided_not_passed() {
        let mut figure = straight_angle_figure((-1.0, 0.0));
        figure.angles[0].second_arm = PointIndex(7);

        let faults = faults_of(&figure, &german_notation(), &FigureTask::default());

        assert!(faults.contains(&FigureFault::ScaleUndecided { label: 0 }));
    }

    #[test]
    fn c7_length_without_its_points_is_undecided_not_passed() {
        let mut figure = right_triangle();
        figure.segments[0].to = PointIndex(7);

        let faults = base_faults(&figure);

        assert!(faults.contains(&FigureFault::ScaleUndecided { label: 3 }));
    }

    #[test]
    fn c7_corner_rounding_to_negative_zero_is_normalized_by_the_sign_of_its_sine() {
        let mut figure = straight_angle_figure((1.0e308, -1.0e-19));
        figure.labels[0].quantity = given(
            CoherentValue::PiMultiple(Number::from(2_i64)),
            Dimension::DIMENSIONLESS,
            "360°",
        );

        let faults = faults_of(&figure, &german_notation(), &FigureTask::default());

        assert_eq!(faults, Vec::new());
    }
}
