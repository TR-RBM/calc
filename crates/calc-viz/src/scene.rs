use std::cmp::Ordering;

use calc_exec::Domain;
use calc_numbers::Number;

use crate::escape_time::UNDECIDED_CELL;
use crate::exact_order::{compare_exact, is_positive_exact};
use crate::figure::{
    AngleIndex, CoherentValue, ElementIndex, Figure, FigureScale, MarkKind, PointIndex, Quantity,
    SegmentIndex,
};
use crate::parameter::FreeParameter;
use crate::primitive::{CellGrid, Column, Primitive};
use crate::record::{Interval, SamplingMethod, SceneRecord};
use crate::style::{Emphasis, StyleRole};
use crate::view::{AxisUnit, Camera, Dimension, Projection, Scale, View, ViewAxis};

pub const SCENE_FORMAT_VERSION: u16 = 5;

const TWO_AXES: usize = 2;
const THREE_AXES: usize = 3;
const SIGN_BIT_COUNT: u32 = 1;
const FULL_TURN_DEGREES: i64 = 360;
const QUARTER_TURN_DEGREES: i64 = 90;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewIndex(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InputIndex(pub u32);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PrecisionLimit {
    pub grid_exhausted: Vec<u32>,
    pub unresolved_samples: u64,
    pub unknown_bounds: u64,
    pub marked_columns: u64,
    pub varies_below_bounds: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColumnEnclosures {
    pub lower: Vec<f64>,
    pub upper: Vec<f64>,
    pub hit_lower: Vec<f64>,
    pub hit_upper: Vec<f64>,
    pub marked: Vec<bool>,
    pub varies_below_bounds: bool,
}

impl ColumnEnclosures {
    pub fn is_unresolved_at_width(&self, column: usize, step: f64) -> bool {
        let (Some(lower), Some(upper)) = (self.lower.get(column), self.upper.get(column)) else {
            return false;
        };
        if !lower.is_finite() || !upper.is_finite() || self.marked.get(column) == Some(&true) {
            return false;
        }
        let proven = match (self.hit_lower.get(column), self.hit_upper.get(column)) {
            (Some(low), Some(high)) if !low.is_nan() && !high.is_nan() => high - low,
            _ => 0.0,
        };
        (upper - lower) - proven > step
    }

    pub fn unresolved_at_width(&self, step: f64) -> usize {
        (0..self.lower.len())
            .filter(|column| self.is_unresolved_at_width(*column, step))
            .count()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reading {
    ValueAt { variables: Vec<String> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub view: ViewIndex,
    pub input: InputIndex,
    pub primitive: Primitive,
    pub style: StyleRole,
    pub value_bounds: Vec<Column>,
    pub precision: Option<PrecisionLimit>,
    pub columns: Option<ColumnEnclosures>,
    pub readings: Vec<Reading>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub parameter_values: Vec<Number>,
    pub layers: Vec<Layer>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub format_version: u16,
    pub record: SceneRecord,
    pub parameters: Vec<FreeParameter>,
    pub views: Vec<View>,
    pub frames: Vec<Frame>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerPosition {
    pub frame: usize,
    pub layer: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Location {
    View(usize),
    Input(usize),
    Parameter(usize),
    Frame(usize),
    Layer(LayerPosition),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneError {
    UnknownFormatVersion(u16),
    NoFrame,
    NotExact(Location),
    EmptyInterval(Location),
    VariableCount {
        input: usize,
        axes: usize,
        variables: usize,
    },
    SeedMismatch {
        method_draws_random_numbers: bool,
    },
    ParameterOutOfRange(usize),
    ParameterStepNotPositive(usize),
    ParameterValueCount {
        frame: usize,
        expected: usize,
        found: usize,
    },
    ViewOutOfRange {
        layer: LayerPosition,
        view: ViewIndex,
    },
    InputOutOfRange {
        layer: LayerPosition,
        input: InputIndex,
    },
    ViewKindMismatch {
        layer: LayerPosition,
        axes: usize,
    },
    CoordinateCount {
        layer: LayerPosition,
        expected: usize,
        found: usize,
    },
    ColumnLength {
        layer: LayerPosition,
        expected: usize,
        found: usize,
    },
    IndexOutOfRange {
        layer: LayerPosition,
        index: u32,
        count: usize,
    },
    BitFieldBoundaries(LayerPosition),
    ViewDivisionsZero {
        view: usize,
    },
    CameraAngleOutOfRange(usize),
    ValueBoundsCount {
        layer: LayerPosition,
        expected: usize,
        found: usize,
    },
    ValueBoundsNotF64(LayerPosition),
    PrecisionAxisOutOfRange {
        layer: LayerPosition,
        axis: u32,
    },
    ReadingNotOffered(LayerPosition),
    DegenerateElement {
        layer: LayerPosition,
        element: ElementIndex,
    },
    LabelKindMismatch {
        layer: LayerPosition,
        label: usize,
    },
    MarkElementMismatch {
        layer: LayerPosition,
        mark: usize,
    },
    ScaleNotPositive(LayerPosition),
    AxisFactorNotPositive {
        view: usize,
        axis: usize,
    },
    TemperatureScaleOnOtherDimension {
        view: usize,
        axis: usize,
    },
    TemperatureScaleOnLogarithmicAxis {
        view: usize,
        axis: usize,
    },
    AxisSymbolMissing {
        view: usize,
        axis: usize,
    },
    AxisSymbolOnDimensionless {
        view: usize,
        axis: usize,
    },
    ClassColumnMissing(LayerPosition),
    ClassCodeInvalid(LayerPosition),
    ClassCountsDisagree {
        cells: u64,
        classified: u64,
    },
    LabelValueInvalid {
        layer: LayerPosition,
        label: usize,
    },
}

impl Scene {
    pub fn new(
        record: SceneRecord,
        parameters: Vec<FreeParameter>,
        views: Vec<View>,
        frames: Vec<Frame>,
    ) -> Result<Scene, SceneError> {
        let scene = Scene {
            format_version: SCENE_FORMAT_VERSION,
            record,
            parameters,
            views,
            frames,
        };
        scene.check()?;
        Ok(scene)
    }

    pub fn check(&self) -> Result<(), SceneError> {
        if self.format_version != SCENE_FORMAT_VERSION {
            return Err(SceneError::UnknownFormatVersion(self.format_version));
        }
        self.check_record()?;
        self.check_parameters()?;
        self.check_views()?;
        if self.frames.is_empty() {
            return Err(SceneError::NoFrame);
        }
        for (frame_index, frame) in self.frames.iter().enumerate() {
            self.check_frame(frame_index, frame)?;
        }
        self.check_escape_time_classes()
    }

    fn check_escape_time_classes(&self) -> Result<(), SceneError> {
        if !matches!(self.record.method, SamplingMethod::EscapeTime { .. }) {
            return Ok(());
        }
        let mut cells = 0_u64;
        for (frame_index, frame) in self.frames.iter().enumerate() {
            for (layer_index, layer) in frame.layers.iter().enumerate() {
                let position = LayerPosition {
                    frame: frame_index,
                    layer: layer_index,
                };
                let Primitive::ScalarGrid(grid) = &layer.primitive else {
                    return Err(SceneError::ClassColumnMissing(position));
                };
                let Some(classes) = &grid.classes else {
                    return Err(SceneError::ClassColumnMissing(position));
                };
                let expected = grid.scalar.len();
                if classes.len() != expected {
                    return Err(SceneError::ColumnLength {
                        layer: position,
                        expected,
                        found: classes.len(),
                    });
                }
                if classes.iter().any(|class| *class > UNDECIDED_CELL) {
                    return Err(SceneError::ClassCodeInvalid(position));
                }
                cells = cells.saturating_add(u64::try_from(expected).unwrap_or(u64::MAX));
            }
        }
        let diagnostics = &self.record.diagnostics;
        let classified = diagnostics
            .escaped_cells
            .saturating_add(diagnostics.inside_cells)
            .saturating_add(diagnostics.undecided_cells);
        if classified == cells {
            Ok(())
        } else {
            Err(SceneError::ClassCountsDisagree { cells, classified })
        }
    }

    fn check_record(&self) -> Result<(), SceneError> {
        let record = &self.record;
        for (input_index, input) in record.inputs.iter().enumerate() {
            for interval in &input.sampling_box {
                check_interval(interval, Location::Input(input_index))?;
            }
            if input.variables.len() != input.sampling_box.len() {
                return Err(SceneError::VariableCount {
                    input: input_index,
                    axes: input.sampling_box.len(),
                    variables: input.variables.len(),
                });
            }
        }
        let method_draws_random_numbers = record.method.draws_random_numbers();
        if method_draws_random_numbers != record.seed.is_some() {
            return Err(SceneError::SeedMismatch {
                method_draws_random_numbers,
            });
        }
        Ok(())
    }

    fn check_parameters(&self) -> Result<(), SceneError> {
        for (index, parameter) in self.parameters.iter().enumerate() {
            let location = Location::Parameter(index);
            check_exact(&parameter.lower, location)?;
            check_exact(&parameter.upper, location)?;
            check_exact(&parameter.current, location)?;
            let is_in_range = is_not_greater(&parameter.lower, &parameter.current)
                && is_not_greater(&parameter.current, &parameter.upper);
            if !is_in_range {
                return Err(SceneError::ParameterOutOfRange(index));
            }
            if let Some(step) = &parameter.step {
                check_exact(step, location)?;
                if !is_positive_exact(step) {
                    return Err(SceneError::ParameterStepNotPositive(index));
                }
            }
        }
        Ok(())
    }

    fn check_views(&self) -> Result<(), SceneError> {
        for (index, view) in self.views.iter().enumerate() {
            let location = Location::View(index);
            for (axis_index, axis) in view.axes().into_iter().enumerate() {
                check_interval(&axis.range, location)?;
                if axis.divisions == 0 {
                    return Err(SceneError::ViewDivisionsZero { view: index });
                }
                check_axis_unit(axis, index, axis_index)?;
            }
            if let View::View3(view) = view {
                check_camera(&view.camera, location, index)?;
            }
        }
        Ok(())
    }

    fn check_frame(&self, frame_index: usize, frame: &Frame) -> Result<(), SceneError> {
        if frame.parameter_values.len() != self.parameters.len() {
            return Err(SceneError::ParameterValueCount {
                frame: frame_index,
                expected: self.parameters.len(),
                found: frame.parameter_values.len(),
            });
        }
        for value in &frame.parameter_values {
            check_exact(value, Location::Frame(frame_index))?;
        }
        for (layer_index, layer) in frame.layers.iter().enumerate() {
            let position = LayerPosition {
                frame: frame_index,
                layer: layer_index,
            };
            self.check_layer(position, layer)?;
        }
        Ok(())
    }

    fn check_layer(&self, position: LayerPosition, layer: &Layer) -> Result<(), SceneError> {
        let view = usize::try_from(layer.view.0)
            .ok()
            .and_then(|index| self.views.get(index))
            .ok_or(SceneError::ViewOutOfRange {
                layer: position,
                view: layer.view,
            })?;
        let has_input = usize::try_from(layer.input.0)
            .ok()
            .is_some_and(|index| index < self.record.inputs.len());
        if !has_input {
            return Err(SceneError::InputOutOfRange {
                layer: position,
                input: layer.input,
            });
        }
        if let Some(mapping) = &layer.style.colour_map {
            check_interval(&mapping.range, Location::Layer(position))?;
        }
        check_primitive(position, &layer.primitive, view.axis_count())?;
        check_value_bounds(position, layer)?;
        if let Some(precision) = &layer.precision
            && let Some(axis) = precision
                .grid_exhausted
                .iter()
                .find(|axis| usize::try_from(**axis).map_or(true, |axis| axis >= view.axis_count()))
        {
            return Err(SceneError::PrecisionAxisOutOfRange {
                layer: position,
                axis: *axis,
            });
        }
        if !layer.readings.is_empty() && value_columns(&layer.primitive).is_empty() {
            return Err(SceneError::ReadingNotOffered(position));
        }
        Ok(())
    }
}

fn check_axis_unit(axis: &ViewAxis, view: usize, axis_index: usize) -> Result<(), SceneError> {
    let location = Location::View(view);
    match &axis.unit {
        AxisUnit::Coherent { .. } => {}
        AxisUnit::Unit { factor, .. } => {
            check_exact(factor, location)?;
            if !is_positive_exact(factor) {
                return Err(SceneError::AxisFactorNotPositive {
                    view,
                    axis: axis_index,
                });
            }
        }
        AxisUnit::TemperatureScale { factor, offset, .. } => {
            check_exact(factor, location)?;
            check_exact(offset, location)?;
            if !is_positive_exact(factor) {
                return Err(SceneError::AxisFactorNotPositive {
                    view,
                    axis: axis_index,
                });
            }
            if axis.dimension != Dimension::TEMPERATURE {
                return Err(SceneError::TemperatureScaleOnOtherDimension {
                    view,
                    axis: axis_index,
                });
            }
            if axis.scale == Scale::Logarithmic {
                return Err(SceneError::TemperatureScaleOnLogarithmicAxis {
                    view,
                    axis: axis_index,
                });
            }
        }
    }
    let is_dimensionless = axis.dimension == Dimension::DIMENSIONLESS;
    let has_symbol = !axis.unit.symbol().is_empty();
    if !is_dimensionless && !has_symbol {
        return Err(SceneError::AxisSymbolMissing {
            view,
            axis: axis_index,
        });
    }
    if is_dimensionless && has_symbol && axis.unit.is_coherent() {
        return Err(SceneError::AxisSymbolOnDimensionless {
            view,
            axis: axis_index,
        });
    }
    Ok(())
}

fn check_exact(number: &Number, location: Location) -> Result<(), SceneError> {
    if number.is_exact() {
        Ok(())
    } else {
        Err(SceneError::NotExact(location))
    }
}

fn is_not_greater(left: &Number, right: &Number) -> bool {
    compare_exact(left, right).is_some_and(|order| order != Ordering::Greater)
}

fn check_interval(interval: &Interval, location: Location) -> Result<(), SceneError> {
    check_exact(&interval.lower, location)?;
    check_exact(&interval.upper, location)?;
    if compare_exact(&interval.lower, &interval.upper) == Some(Ordering::Less) {
        Ok(())
    } else {
        Err(SceneError::EmptyInterval(location))
    }
}

fn check_camera(camera: &Camera, location: Location, view: usize) -> Result<(), SceneError> {
    check_exact(&camera.azimuth_degrees, location)?;
    check_exact(&camera.elevation_degrees, location)?;
    if let Projection::Perspective {
        field_of_view_degrees,
    } = &camera.projection
    {
        check_exact(field_of_view_degrees, location)?;
    }
    let is_between = |number: &Number, lower: i64, upper: i64, includes_upper: bool| {
        let above_lower = compare_exact(number, &Number::from(lower))
            .is_some_and(|order| order != Ordering::Less);
        let below_upper = compare_exact(number, &Number::from(upper)).is_some_and(|order| {
            order == Ordering::Less || (includes_upper && order == Ordering::Equal)
        });
        above_lower && below_upper
    };
    let angles_are_in_range = is_between(&camera.azimuth_degrees, 0, FULL_TURN_DEGREES, false)
        && is_between(
            &camera.elevation_degrees,
            -QUARTER_TURN_DEGREES,
            QUARTER_TURN_DEGREES,
            true,
        );
    if angles_are_in_range {
        Ok(())
    } else {
        Err(SceneError::CameraAngleOutOfRange(view))
    }
}

fn last_column(columns: &[Column]) -> Vec<&Column> {
    columns.last().into_iter().collect()
}

pub(crate) fn value_columns(primitive: &Primitive) -> Vec<&Column> {
    match primitive {
        Primitive::Polyline(polyline) => last_column(&polyline.coordinates),
        Primitive::Band(band) => vec![&band.lower, &band.upper],
        Primitive::Points(points) => last_column(&points.coordinates),
        Primitive::Arrows(arrows) => arrows.components.iter().collect(),
        Primitive::ScalarGrid(grid) => vec![&grid.scalar],
        Primitive::ComplexGrid(grid) => vec![&grid.real, &grid.imaginary],
        Primitive::TriangleMesh(mesh) => last_column(&mesh.vertices),
        Primitive::Voxels(voxels) => voxels.scalar.iter().collect(),
        Primitive::Graph(_)
        | Primitive::Formula(_)
        | Primitive::BitLayout(_)
        | Primitive::Figure(_) => Vec::new(),
    }
}

fn check_value_bounds(position: LayerPosition, layer: &Layer) -> Result<(), SceneError> {
    let columns = value_columns(&layer.primitive);
    let awaits_bounds =
        layer.value_bounds.is_empty() && layer.style.emphasis == Emphasis::Provisional;
    if awaits_bounds {
        return Ok(());
    }
    if layer.value_bounds.len() != columns.len() {
        return Err(SceneError::ValueBoundsCount {
            layer: position,
            expected: columns.len(),
            found: layer.value_bounds.len(),
        });
    }
    for (bounds, column) in layer.value_bounds.iter().zip(columns) {
        if bounds.domain() != Domain::F64 {
            return Err(SceneError::ValueBoundsNotF64(position));
        }
        check_length(position, bounds, column.len())?;
    }
    Ok(())
}

fn check_primitive(
    position: LayerPosition,
    primitive: &Primitive,
    axes: usize,
) -> Result<(), SceneError> {
    match primitive {
        Primitive::Polyline(polyline) => {
            check_coordinates(position, &polyline.coordinates, axes)?;
            Ok(())
        }
        Primitive::Band(band) => {
            require_axes(position, axes, TWO_AXES)?;
            let length = band.abscissa.len();
            check_length(position, &band.lower, length)?;
            check_length(position, &band.upper, length)
        }
        Primitive::Points(points) => {
            let length = check_coordinates(position, &points.coordinates, axes)?;
            check_optional_length(position, points.scalar.as_ref(), length)
        }
        Primitive::Arrows(arrows) => {
            let length = check_coordinates(position, &arrows.bases, axes)?;
            let component_length = check_coordinates(position, &arrows.components, axes)?;
            if component_length == length {
                Ok(())
            } else {
                Err(SceneError::ColumnLength {
                    layer: position,
                    expected: length,
                    found: component_length,
                })
            }
        }
        Primitive::ScalarGrid(grid) => {
            let cell_count = check_cells(position, &grid.cells, axes)?;
            check_length(position, &grid.scalar, cell_count)
        }
        Primitive::ComplexGrid(grid) => {
            require_axes(position, axes, TWO_AXES)?;
            let cell_count = check_cells(position, &grid.cells, axes)?;
            check_length(position, &grid.real, cell_count)?;
            check_length(position, &grid.imaginary, cell_count)
        }
        Primitive::TriangleMesh(mesh) => {
            let length = check_coordinates(position, &mesh.vertices, axes)?;
            let indices = mesh.triangles.iter().flatten();
            check_indices(position, indices, length)?;
            check_optional_length(position, mesh.scalar.as_ref(), length)
        }
        Primitive::Voxels(voxels) => {
            require_axes(position, axes, THREE_AXES)?;
            check_optional_length(position, voxels.scalar.as_ref(), voxels.occupied.len())
        }
        Primitive::Graph(graph) => {
            let length = check_coordinates(position, &graph.positions, axes)?;
            check_indices(position, graph.edges.iter().flatten(), length)
        }
        Primitive::Formula(formula) => {
            if formula.anchor.len() != axes {
                return Err(SceneError::CoordinateCount {
                    layer: position,
                    expected: axes,
                    found: formula.anchor.len(),
                });
            }
            for number in &formula.anchor {
                check_exact(number, Location::Layer(position))?;
            }
            Ok(())
        }
        Primitive::BitLayout(layout) => {
            require_axes(position, axes, TWO_AXES)?;
            let bit_count = usize::try_from(layout.fraction_start)
                .ok()
                .filter(|start| *start < layout.bits.len());
            let is_ordered = layout.exponent_start == SIGN_BIT_COUNT
                && layout.exponent_start < layout.fraction_start
                && bit_count.is_some();
            if is_ordered {
                Ok(())
            } else {
                Err(SceneError::BitFieldBoundaries(position))
            }
        }
        Primitive::Figure(figure) => {
            require_axes(position, axes, TWO_AXES)?;
            check_figure_structure(position, figure)
        }
    }
}

fn check_point_index(
    position: LayerPosition,
    point: PointIndex,
    count: usize,
) -> Result<(), SceneError> {
    check_indices(position, [point.0].iter(), count)
}

fn check_element(
    position: LayerPosition,
    figure: &Figure,
    element: ElementIndex,
) -> Result<(), SceneError> {
    let (index, count) = match element {
        ElementIndex::Point(PointIndex(index)) => (index, figure.point_count()),
        ElementIndex::Segment(SegmentIndex(index)) => (index, figure.segments.len()),
        ElementIndex::Angle(AngleIndex(index)) => (index, figure.angles.len()),
    };
    check_indices(position, [index].iter(), count)
}

fn check_label_value(
    position: LayerPosition,
    label: usize,
    quantity: &Quantity,
) -> Result<(), SceneError> {
    let (Quantity::Given(value) | Quantity::Answered(value)) = quantity else {
        return Ok(());
    };
    let is_valid = match &value.value {
        CoherentValue::Rational(number) | CoherentValue::PiMultiple(number) => number.is_exact(),
        CoherentValue::Enclosure { lower, upper } => {
            compare_exact(lower, upper).is_some_and(|order| order != Ordering::Greater)
        }
    };
    if is_valid {
        Ok(())
    } else {
        Err(SceneError::LabelValueInvalid {
            layer: position,
            label,
        })
    }
}

fn check_figure_structure(position: LayerPosition, figure: &Figure) -> Result<(), SceneError> {
    let [x, y] = &figure.points;
    let count = check_coordinates(position, &[x.clone(), y.clone()], TWO_AXES)?;
    for (index, segment) in figure.segments.iter().enumerate() {
        check_point_index(position, segment.from, count)?;
        check_point_index(position, segment.to, count)?;
        if segment.from == segment.to {
            return Err(SceneError::DegenerateElement {
                layer: position,
                element: ElementIndex::Segment(SegmentIndex(
                    u32::try_from(index).unwrap_or(u32::MAX),
                )),
            });
        }
    }
    for (index, angle) in figure.angles.iter().enumerate() {
        for point in [angle.vertex, angle.first_arm, angle.second_arm] {
            check_point_index(position, point, count)?;
        }
        let coincide = angle.vertex == angle.first_arm
            || angle.vertex == angle.second_arm
            || angle.first_arm == angle.second_arm;
        if coincide {
            return Err(SceneError::DegenerateElement {
                layer: position,
                element: ElementIndex::Angle(AngleIndex(u32::try_from(index).unwrap_or(u32::MAX))),
            });
        }
    }
    for (index, label) in figure.labels.iter().enumerate() {
        check_element(position, figure, label.element)?;
        if label.element.kind() != label.names {
            return Err(SceneError::LabelKindMismatch {
                layer: position,
                label: index,
            });
        }
        check_label_value(position, index, &label.quantity)?;
    }
    for (index, mark) in figure.marks.iter().enumerate() {
        check_element(position, figure, mark.element)?;
        let fits = match mark.kind {
            MarkKind::AngleArc { .. } | MarkKind::RightAngle(_) | MarkKind::Direction => {
                matches!(mark.element, ElementIndex::Angle(_))
            }
            MarkKind::EqualTicks { .. } => matches!(mark.element, ElementIndex::Segment(_)),
        };
        if !fits {
            return Err(SceneError::MarkElementMismatch {
                layer: position,
                mark: index,
            });
        }
    }
    if let FigureScale::ToScale {
        view_units_per_metre,
    } = &figure.scale
        && !is_positive_exact(view_units_per_metre)
    {
        return Err(SceneError::ScaleNotPositive(position));
    }
    Ok(())
}

fn require_axes(position: LayerPosition, axes: usize, required: usize) -> Result<(), SceneError> {
    if axes == required {
        Ok(())
    } else {
        Err(SceneError::ViewKindMismatch {
            layer: position,
            axes,
        })
    }
}

fn check_length(
    position: LayerPosition,
    column: &Column,
    expected: usize,
) -> Result<(), SceneError> {
    if column.len() == expected {
        Ok(())
    } else {
        Err(SceneError::ColumnLength {
            layer: position,
            expected,
            found: column.len(),
        })
    }
}

fn check_optional_length(
    position: LayerPosition,
    column: Option<&Column>,
    expected: usize,
) -> Result<(), SceneError> {
    match column {
        Some(column) => check_length(position, column, expected),
        None => Ok(()),
    }
}

fn check_coordinates(
    position: LayerPosition,
    columns: &[Column],
    axes: usize,
) -> Result<usize, SceneError> {
    if columns.len() != axes {
        return Err(SceneError::CoordinateCount {
            layer: position,
            expected: axes,
            found: columns.len(),
        });
    }
    let length = columns.first().map_or(0, Column::len);
    for column in columns {
        check_length(position, column, length)?;
    }
    Ok(length)
}

fn check_indices<'index>(
    position: LayerPosition,
    indices: impl Iterator<Item = &'index u32>,
    count: usize,
) -> Result<(), SceneError> {
    for index in indices {
        let is_valid = usize::try_from(*index).is_ok_and(|index| index < count);
        if !is_valid {
            return Err(SceneError::IndexOutOfRange {
                layer: position,
                index: *index,
                count,
            });
        }
    }
    Ok(())
}

fn check_cells(
    position: LayerPosition,
    cells: &CellGrid,
    axes: usize,
) -> Result<usize, SceneError> {
    if cells.region.len() != axes || cells.counts.len() != axes {
        return Err(SceneError::CoordinateCount {
            layer: position,
            expected: axes,
            found: cells.region.len().min(cells.counts.len()),
        });
    }
    for interval in &cells.region {
        check_interval(interval, Location::Layer(position))?;
    }
    let cell_count = cells
        .counts
        .iter()
        .try_fold(1_usize, |product, count| {
            usize::try_from(*count)
                .ok()
                .and_then(|count| product.checked_mul(count))
        })
        .unwrap_or(usize::MAX);
    Ok(cell_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::{
        Arrows, Band, BitLayout, ComplexGrid, Formula, Graph, Points, Polyline, ScalarGrid,
        TriangleMesh, Voxels,
    };
    use crate::record::{Resolution, ResultId, SamplingDiagnostics, SamplingMethod, SceneInput};
    use crate::style::{ColourMap, ColourMapping, KindColour, LinePattern, Marker};
    use crate::view::{Dimension, Scale, View2, View3, ViewAxis};
    use calc_core::Seed;
    use calc_exec::{BackendKind, Domain};
    use calc_numbers::Integer;

    const FIRST_LAYER: LayerPosition = LayerPosition { frame: 0, layer: 0 };

    fn integer(value: i64) -> Number {
        Number::from(value)
    }

    fn unit_interval() -> Interval {
        Interval {
            lower: integer(0),
            upper: integer(1),
        }
    }

    fn axis() -> ViewAxis {
        ViewAxis {
            range: unit_interval(),
            scale: Scale::Linear,
            dimension: Dimension::DIMENSIONLESS,
            unit: AxisUnit::dimensionless(),
            divisions: 100,
        }
    }

    fn view2() -> View {
        View::View2(Box::new(View2 {
            x: axis(),
            y: axis(),
        }))
    }

    fn view3() -> View {
        View::View3(Box::new(View3 {
            x: axis(),
            y: axis(),
            z: axis(),
            camera: Camera {
                azimuth_degrees: integer(315),
                elevation_degrees: integer(30),
                projection: Projection::Orthographic,
            },
        }))
    }

    fn record() -> SceneRecord {
        SceneRecord {
            result: ResultId(1),
            inputs: vec![SceneInput {
                expression: String::from("x"),
                sampling_box: vec![unit_interval()],
                variables: vec![String::from("x")],
            }],
            method: SamplingMethod::UniformGrid,
            resolution: Resolution {
                domain: Domain::F64,
                axes: Vec::new(),
                adaptive: None,
            },
            seed: None,
            backend: BackendKind::Cpu,
            references: Vec::new(),
            diagnostics: SamplingDiagnostics::default(),
        }
    }

    fn style() -> StyleRole {
        StyleRole {
            kind: KindColour::Numeric,
            colour_map: None,
            line: LinePattern::Solid,
            marker: Marker::None,
            emphasis: Emphasis::Provisional,
        }
    }

    fn layer(primitive: Primitive) -> Layer {
        Layer {
            view: ViewIndex(0),
            input: InputIndex(0),
            primitive,
            style: style(),
            value_bounds: Vec::new(),
            precision: None,
            columns: None,
            readings: Vec::new(),
        }
    }

    fn column(length: usize) -> Column {
        Column::F64(vec![0.0; length])
    }

    fn frame(layers: Vec<Layer>) -> Frame {
        Frame {
            parameter_values: Vec::new(),
            layers,
        }
    }

    fn scene_with(view: View, primitive: Primitive) -> Result<Scene, SceneError> {
        Scene::new(
            record(),
            Vec::new(),
            vec![view],
            vec![frame(vec![layer(primitive)])],
        )
    }

    fn polyline(lengths: &[usize]) -> Primitive {
        Primitive::Polyline(Polyline {
            coordinates: lengths.iter().map(|length| column(*length)).collect(),
        })
    }

    fn cells(counts: Vec<u32>) -> CellGrid {
        CellGrid {
            region: counts.iter().map(|_| unit_interval()).collect(),
            counts,
        }
    }

    fn slider(
        lower: Number,
        current: Number,
        upper: Number,
        step: Option<Number>,
    ) -> FreeParameter {
        FreeParameter {
            name: String::from("a"),
            lower,
            upper,
            step,
            current,
            role: crate::parameter::ParameterRole::Slider,
        }
    }

    #[test]
    fn valid_scene_gets_the_current_format_version() {
        let scene = scene_with(view2(), polyline(&[3, 3])).unwrap();

        assert_eq!(scene.format_version, SCENE_FORMAT_VERSION);
    }

    #[test]
    fn unknown_format_version_is_rejected() {
        let mut scene = scene_with(view2(), polyline(&[3, 3])).unwrap();
        scene.format_version = SCENE_FORMAT_VERSION + 1;

        let result = scene.check();

        assert_eq!(
            result,
            Err(SceneError::UnknownFormatVersion(SCENE_FORMAT_VERSION + 1))
        );
    }

    #[test]
    fn scene_without_frames_is_rejected() {
        let result = Scene::new(record(), Vec::new(), vec![view2()], Vec::new());

        assert_eq!(result, Err(SceneError::NoFrame));
    }

    #[test]
    fn machine_number_in_view_bounds_is_not_exact() {
        let mut view = View2 {
            x: axis(),
            y: axis(),
        };
        view.y.range.upper = Number::F64(1.0);

        let result = scene_with(View::View2(Box::new(view)), polyline(&[1, 1]));

        assert_eq!(result, Err(SceneError::NotExact(Location::View(0))));
    }

    #[test]
    fn view_interval_with_equal_bounds_is_empty() {
        let mut view = View2 {
            x: axis(),
            y: axis(),
        };
        view.x.range.upper = integer(0);

        let result = scene_with(View::View2(Box::new(view)), polyline(&[1, 1]));

        assert_eq!(result, Err(SceneError::EmptyInterval(Location::View(0))));
    }

    #[test]
    fn machine_number_in_camera_is_not_exact() {
        let mut view = view3();
        if let View::View3(view) = &mut view {
            view.camera.projection = Projection::Perspective {
                field_of_view_degrees: Number::F32(45.0),
            };
        }

        let result = scene_with(view, polyline(&[1, 1, 1]));

        assert_eq!(result, Err(SceneError::NotExact(Location::View(0))));
    }

    #[test]
    fn input_with_more_axes_than_variables_is_rejected() {
        let mut record = record();
        record.inputs[0].variables.clear();

        let result = Scene::new(
            record,
            Vec::new(),
            vec![view2()],
            vec![frame(vec![layer(polyline(&[1, 1]))])],
        );

        assert_eq!(
            result,
            Err(SceneError::VariableCount {
                input: 0,
                axes: 1,
                variables: 0
            })
        );
    }

    #[test]
    fn reversed_sampling_box_is_empty() {
        let mut record = record();
        record.inputs[0].sampling_box[0] = Interval {
            lower: integer(1),
            upper: integer(0),
        };

        let result = Scene::new(
            record,
            Vec::new(),
            vec![view2()],
            vec![frame(vec![layer(polyline(&[1, 1]))])],
        );

        assert_eq!(result, Err(SceneError::EmptyInterval(Location::Input(0))));
    }

    #[test]
    fn monte_carlo_without_seed_is_rejected() {
        let mut record = record();
        record.method = SamplingMethod::MonteCarlo { sample_count: 10 };

        let result = Scene::new(record, Vec::new(), vec![view2()], vec![frame(Vec::new())]);

        assert_eq!(
            result,
            Err(SceneError::SeedMismatch {
                method_draws_random_numbers: true
            })
        );
    }

    #[test]
    fn uniform_grid_with_seed_is_rejected() {
        let mut record = record();
        record.seed = Some(Seed {
            value: 42,
            generator: String::from("test"),
        });

        let result = Scene::new(record, Vec::new(), vec![view2()], vec![frame(Vec::new())]);

        assert_eq!(
            result,
            Err(SceneError::SeedMismatch {
                method_draws_random_numbers: false
            })
        );
    }

    #[test]
    fn parameter_current_above_upper_is_out_of_range() {
        let parameter = slider(integer(0), integer(2), integer(1), None);

        let result = Scene::new(record(), vec![parameter], vec![view2()], Vec::new());

        assert_eq!(result, Err(SceneError::ParameterOutOfRange(0)));
    }

    #[test]
    fn parameter_with_machine_bound_is_not_exact() {
        let parameter = slider(Number::F64(0.0), integer(0), integer(1), None);

        let result = Scene::new(record(), vec![parameter], vec![view2()], Vec::new());

        assert_eq!(result, Err(SceneError::NotExact(Location::Parameter(0))));
    }

    #[test]
    fn parameter_step_of_zero_is_not_positive() {
        let parameter = slider(integer(0), integer(0), integer(1), Some(integer(0)));

        let result = Scene::new(record(), vec![parameter], vec![view2()], Vec::new());

        assert_eq!(result, Err(SceneError::ParameterStepNotPositive(0)));
    }

    #[test]
    fn frame_with_missing_parameter_value_is_rejected() {
        let parameter = slider(integer(0), integer(0), integer(1), None);

        let result = Scene::new(
            record(),
            vec![parameter],
            vec![view2()],
            vec![frame(Vec::new())],
        );

        assert_eq!(
            result,
            Err(SceneError::ParameterValueCount {
                frame: 0,
                expected: 1,
                found: 0
            })
        );
    }

    #[test]
    fn frame_with_machine_parameter_value_is_not_exact() {
        let parameter = slider(integer(0), integer(0), integer(1), None);
        let frame = Frame {
            parameter_values: vec![Number::F64(0.5)],
            layers: Vec::new(),
        };

        let result = Scene::new(record(), vec![parameter], vec![view2()], vec![frame]);

        assert_eq!(result, Err(SceneError::NotExact(Location::Frame(0))));
    }

    #[test]
    fn layer_naming_a_missing_view_is_rejected() {
        let mut layer = layer(polyline(&[1, 1]));
        layer.view = ViewIndex(1);

        let result = Scene::new(
            record(),
            Vec::new(),
            vec![view2()],
            vec![frame(vec![layer])],
        );

        assert_eq!(
            result,
            Err(SceneError::ViewOutOfRange {
                layer: FIRST_LAYER,
                view: ViewIndex(1)
            })
        );
    }

    #[test]
    fn layer_naming_a_missing_input_is_rejected() {
        let mut layer = layer(polyline(&[1, 1]));
        layer.input = InputIndex(1);

        let result = Scene::new(
            record(),
            Vec::new(),
            vec![view2()],
            vec![frame(vec![layer])],
        );

        assert_eq!(
            result,
            Err(SceneError::InputOutOfRange {
                layer: FIRST_LAYER,
                input: InputIndex(1)
            })
        );
    }

    #[test]
    fn colour_map_with_reversed_range_is_empty() {
        let mut layer = layer(polyline(&[1, 1]));
        layer.style.colour_map = Some(ColourMapping {
            map: ColourMap::Sequential,
            range: Interval {
                lower: integer(1),
                upper: integer(-1),
            },
        });

        let result = Scene::new(
            record(),
            Vec::new(),
            vec![view2()],
            vec![frame(vec![layer])],
        );

        assert_eq!(
            result,
            Err(SceneError::EmptyInterval(Location::Layer(FIRST_LAYER)))
        );
    }

    #[test]
    fn polyline_with_three_columns_in_plane_view_is_rejected() {
        let result = scene_with(view2(), polyline(&[2, 2, 2]));

        assert_eq!(
            result,
            Err(SceneError::CoordinateCount {
                layer: FIRST_LAYER,
                expected: 2,
                found: 3
            })
        );
    }

    #[test]
    fn polyline_with_unequal_columns_is_rejected() {
        let result = scene_with(view2(), polyline(&[2, 3]));

        assert_eq!(
            result,
            Err(SceneError::ColumnLength {
                layer: FIRST_LAYER,
                expected: 2,
                found: 3
            })
        );
    }

    #[test]
    fn band_in_space_view_is_a_view_kind_mismatch() {
        let band = Primitive::Band(Band {
            abscissa: column(2),
            lower: column(2),
            upper: column(2),
        });

        let result = scene_with(view3(), band);

        assert_eq!(
            result,
            Err(SceneError::ViewKindMismatch {
                layer: FIRST_LAYER,
                axes: 3
            })
        );
    }

    #[test]
    fn points_with_short_scalar_column_is_rejected() {
        let points = Primitive::Points(Points {
            coordinates: vec![column(4), column(4)],
            scalar: Some(column(3)),
        });

        let result = scene_with(view2(), points);

        assert_eq!(
            result,
            Err(SceneError::ColumnLength {
                layer: FIRST_LAYER,
                expected: 4,
                found: 3
            })
        );
    }

    #[test]
    fn arrows_with_fewer_components_than_bases_are_rejected() {
        let arrows = Primitive::Arrows(Arrows {
            bases: vec![column(3), column(3)],
            components: vec![column(2), column(2)],
        });

        let result = scene_with(view2(), arrows);

        assert_eq!(
            result,
            Err(SceneError::ColumnLength {
                layer: FIRST_LAYER,
                expected: 3,
                found: 2
            })
        );
    }

    #[test]
    fn scalar_grid_needs_one_value_per_cell() {
        let grid = Primitive::ScalarGrid(ScalarGrid {
            cells: cells(vec![2, 3]),
            scalar: column(5),
            classes: None,
        });

        let result = scene_with(view2(), grid);

        assert_eq!(
            result,
            Err(SceneError::ColumnLength {
                layer: FIRST_LAYER,
                expected: 6,
                found: 5
            })
        );
    }

    #[test]
    fn complex_grid_with_short_imaginary_column_is_rejected() {
        let grid = Primitive::ComplexGrid(ComplexGrid {
            cells: cells(vec![2, 2]),
            real: column(4),
            imaginary: column(3),
        });

        let result = scene_with(view2(), grid);

        assert_eq!(
            result,
            Err(SceneError::ColumnLength {
                layer: FIRST_LAYER,
                expected: 4,
                found: 3
            })
        );
    }

    #[test]
    fn triangle_index_past_the_vertices_is_rejected() {
        let mesh = Primitive::TriangleMesh(TriangleMesh {
            vertices: vec![column(3), column(3), column(3)],
            triangles: vec![[0, 1, 3]],
            scalar: None,
        });

        let result = scene_with(view3(), mesh);

        assert_eq!(
            result,
            Err(SceneError::IndexOutOfRange {
                layer: FIRST_LAYER,
                index: 3,
                count: 3
            })
        );
    }

    #[test]
    fn voxels_in_plane_view_are_a_view_kind_mismatch() {
        let voxels = Primitive::Voxels(Voxels {
            occupied: vec![[0, 0, 0]],
            scalar: None,
        });

        let result = scene_with(view2(), voxels);

        assert_eq!(
            result,
            Err(SceneError::ViewKindMismatch {
                layer: FIRST_LAYER,
                axes: 2
            })
        );
    }

    #[test]
    fn graph_edge_to_missing_node_is_rejected() {
        let graph = Primitive::Graph(Graph {
            positions: vec![column(2), column(2)],
            edges: vec![[0, 1], [1, 2]],
            is_directed: true,
        });

        let result = scene_with(view2(), graph);

        assert_eq!(
            result,
            Err(SceneError::IndexOutOfRange {
                layer: FIRST_LAYER,
                index: 2,
                count: 2
            })
        );
    }

    #[test]
    fn formula_anchor_with_machine_number_is_not_exact() {
        let formula = Primitive::Formula(Formula {
            expression: String::from("x^2"),
            anchor: vec![integer(0), Number::F64(0.5)],
        });

        let result = scene_with(view2(), formula);

        assert_eq!(
            result,
            Err(SceneError::NotExact(Location::Layer(FIRST_LAYER)))
        );
    }

    #[test]
    fn bit_layout_with_fraction_before_exponent_is_rejected() {
        let layout = Primitive::BitLayout(BitLayout {
            bits: vec![false; 32],
            exponent_start: 9,
            fraction_start: 1,
        });

        let result = scene_with(view2(), layout);

        assert_eq!(result, Err(SceneError::BitFieldBoundaries(FIRST_LAYER)));
    }

    #[test]
    fn bit_layout_of_binary32_is_accepted() {
        let layout = Primitive::BitLayout(BitLayout {
            bits: vec![false; 32],
            exponent_start: 1,
            fraction_start: 9,
        });

        let result = scene_with(view2(), layout);

        assert!(result.is_ok());
    }

    #[test]
    fn rational_bounds_are_exact() {
        let half = Number::fraction(&Integer::from(1_i64), &Integer::from(2_i64)).unwrap();
        let mut view = View2 {
            x: axis(),
            y: axis(),
        };
        view.x.range.lower = half;

        let result = scene_with(View::View2(Box::new(view)), polyline(&[1, 1]));

        assert!(result.is_ok());
    }

    fn plane_view_with_divisions(divisions: u32) -> View {
        let mut view = View2 {
            x: axis(),
            y: axis(),
        };
        view.y.divisions = divisions;
        View::View2(Box::new(view))
    }

    fn settled(mut layer: Layer) -> Layer {
        layer.style.emphasis = Emphasis::Normal;
        layer
    }

    fn scene_with_layer(view: View, layer: Layer) -> Result<Scene, SceneError> {
        Scene::new(record(), Vec::new(), vec![view], vec![frame(vec![layer])])
    }

    fn camera_view(azimuth: i64, elevation: i64) -> View {
        let mut view = view3();
        if let View::View3(view) = &mut view {
            view.camera.azimuth_degrees = integer(azimuth);
            view.camera.elevation_degrees = integer(elevation);
        }
        view
    }

    #[test]
    fn view_axis_with_zero_divisions_is_rejected() {
        let result = scene_with(plane_view_with_divisions(0), polyline(&[1, 1]));

        assert_eq!(result, Err(SceneError::ViewDivisionsZero { view: 0 }));
    }

    #[test]
    fn camera_azimuth_of_a_full_turn_is_out_of_range() {
        let result = scene_with(camera_view(360, 0), polyline(&[1, 1, 1]));

        assert_eq!(result, Err(SceneError::CameraAngleOutOfRange(0)));
    }

    #[test]
    fn camera_elevation_above_a_quarter_turn_is_out_of_range() {
        let result = scene_with(camera_view(0, 91), polyline(&[1, 1, 1]));

        assert_eq!(result, Err(SceneError::CameraAngleOutOfRange(0)));
    }

    #[test]
    fn camera_looking_straight_down_is_accepted() {
        let result = scene_with(camera_view(0, -90), polyline(&[1, 1, 1]));

        assert!(result.is_ok());
    }

    #[test]
    fn settled_layer_without_value_bounds_is_rejected() {
        let result = scene_with_layer(view2(), settled(layer(polyline(&[2, 2]))));

        assert_eq!(
            result,
            Err(SceneError::ValueBoundsCount {
                layer: FIRST_LAYER,
                expected: 1,
                found: 0
            })
        );
    }

    #[test]
    fn provisional_layer_may_wait_for_its_value_bounds() {
        let result = scene_with_layer(view2(), layer(polyline(&[2, 2])));

        assert!(result.is_ok());
    }

    #[test]
    fn band_takes_one_bounds_column_per_bound() {
        let band = Primitive::Band(Band {
            abscissa: column(2),
            lower: column(2),
            upper: column(2),
        });
        let mut layer = settled(layer(band));
        layer.value_bounds = vec![column(2), column(2)];

        let result = scene_with_layer(view2(), layer);

        assert!(result.is_ok());
    }

    #[test]
    fn value_bounds_in_f32_are_rejected() {
        let mut layer = settled(layer(polyline(&[2, 2])));
        layer.value_bounds = vec![Column::F32(vec![0.0; 2])];

        let result = scene_with_layer(view2(), layer);

        assert_eq!(result, Err(SceneError::ValueBoundsNotF64(FIRST_LAYER)));
    }

    #[test]
    fn value_bounds_shorter_than_their_column_are_rejected() {
        let mut layer = settled(layer(polyline(&[3, 3])));
        layer.value_bounds = vec![column(2)];

        let result = scene_with_layer(view2(), layer);

        assert_eq!(
            result,
            Err(SceneError::ColumnLength {
                layer: FIRST_LAYER,
                expected: 3,
                found: 2
            })
        );
    }

    #[test]
    fn graph_takes_no_value_bounds() {
        let graph = Primitive::Graph(Graph {
            positions: vec![column(1), column(1)],
            edges: Vec::new(),
            is_directed: true,
        });
        let mut layer = settled(layer(graph));
        layer.value_bounds = vec![column(1)];

        let result = scene_with_layer(view2(), layer);

        assert_eq!(
            result,
            Err(SceneError::ValueBoundsCount {
                layer: FIRST_LAYER,
                expected: 0,
                found: 1
            })
        );
    }

    #[test]
    fn precision_limit_naming_a_missing_axis_is_rejected() {
        let mut layer = layer(polyline(&[2, 2]));
        layer.precision = Some(PrecisionLimit {
            grid_exhausted: vec![2],
            unresolved_samples: 0,
            unknown_bounds: 0,
            marked_columns: 0,
            varies_below_bounds: false,
        });

        let result = scene_with_layer(view2(), layer);

        assert_eq!(
            result,
            Err(SceneError::PrecisionAxisOutOfRange {
                layer: FIRST_LAYER,
                axis: 2
            })
        );
    }

    #[test]
    fn reading_on_a_bit_layout_is_not_offered() {
        let layout = Primitive::BitLayout(BitLayout {
            bits: vec![false; 32],
            exponent_start: 1,
            fraction_start: 9,
        });
        let mut layer = layer(layout);
        layer.readings = vec![Reading::ValueAt {
            variables: vec![String::from("x")],
        }];

        let result = scene_with_layer(view2(), layer);

        assert_eq!(result, Err(SceneError::ReadingNotOffered(FIRST_LAYER)));
    }

    fn figure_scene(figure: crate::figure::Figure) -> Result<Scene, SceneError> {
        scene_with(view2(), Primitive::Figure(Box::new(figure)))
    }

    fn triangle() -> crate::figure::Figure {
        crate::figure_check::tests::right_triangle()
    }

    #[test]
    fn consistent_figure_is_a_valid_scene() {
        let result = figure_scene(triangle());

        assert!(result.is_ok());
    }

    #[test]
    fn figure_in_a_space_view_is_a_view_kind_mismatch() {
        let result = scene_with(view3(), Primitive::Figure(Box::new(triangle())));

        assert_eq!(
            result,
            Err(SceneError::ViewKindMismatch {
                layer: FIRST_LAYER,
                axes: 3
            })
        );
    }

    #[test]
    fn segment_from_a_point_to_itself_is_degenerate() {
        let mut figure = triangle();
        figure.segments[0].to = figure.segments[0].from;

        let result = figure_scene(figure);

        assert_eq!(
            result,
            Err(SceneError::DegenerateElement {
                layer: FIRST_LAYER,
                element: ElementIndex::Segment(SegmentIndex(0))
            })
        );
    }

    #[test]
    fn angle_with_its_vertex_on_an_arm_is_degenerate() {
        let mut figure = triangle();
        figure.angles[0].first_arm = figure.angles[0].vertex;

        let result = figure_scene(figure);

        assert_eq!(
            result,
            Err(SceneError::DegenerateElement {
                layer: FIRST_LAYER,
                element: ElementIndex::Angle(AngleIndex(0))
            })
        );
    }

    #[test]
    fn segment_to_a_missing_point_is_out_of_range() {
        let mut figure = triangle();
        figure.segments[0].to = PointIndex(5);

        let result = figure_scene(figure);

        assert_eq!(
            result,
            Err(SceneError::IndexOutOfRange {
                layer: FIRST_LAYER,
                index: 5,
                count: 3
            })
        );
    }

    #[test]
    fn side_name_on_a_point_is_a_label_kind_mismatch() {
        let mut figure = triangle();
        figure.labels[3].element = ElementIndex::Point(PointIndex(0));

        let result = figure_scene(figure);

        assert_eq!(
            result,
            Err(SceneError::LabelKindMismatch {
                layer: FIRST_LAYER,
                label: 3
            })
        );
    }

    #[test]
    fn right_angle_mark_on_a_segment_is_a_mark_element_mismatch() {
        let mut figure = triangle();
        figure.marks[0].element = ElementIndex::Segment(SegmentIndex(0));

        let result = figure_scene(figure);

        assert_eq!(
            result,
            Err(SceneError::MarkElementMismatch {
                layer: FIRST_LAYER,
                mark: 0
            })
        );
    }

    #[test]
    fn zero_scale_is_not_positive() {
        let mut figure = triangle();
        figure.scale = FigureScale::ToScale {
            view_units_per_metre: integer(0),
        };

        let result = figure_scene(figure);

        assert_eq!(result, Err(SceneError::ScaleNotPositive(FIRST_LAYER)));
    }

    #[test]
    fn reversed_enclosure_is_an_invalid_label_value() {
        let mut figure = triangle();
        figure.labels[3].quantity = Quantity::Given(crate::figure::LabelValue {
            value: CoherentValue::Enclosure {
                lower: integer(4),
                upper: integer(3),
            },
            dimension: crate::figure_check::tests::length(),
            printed: String::from("3 m"),
        });

        let result = figure_scene(figure);

        assert_eq!(
            result,
            Err(SceneError::LabelValueInvalid {
                layer: FIRST_LAYER,
                label: 3
            })
        );
    }

    fn escape_scene(classes: Option<Vec<u8>>, escaped: u64) -> Result<Scene, SceneError> {
        let mut record = record();
        record.method = SamplingMethod::EscapeTime {
            form: crate::escape_time::EscapeTimeForm::QuadraticParameter,
            limit_rule: crate::escape_time::IterationLimitRule::DEFAULT,
            iterations_used: 256,
        };
        record.diagnostics.escaped_cells = escaped;
        let grid = Primitive::ScalarGrid(ScalarGrid {
            cells: cells(vec![2, 1]),
            scalar: column(2),
            classes,
        });
        Scene::new(
            record,
            Vec::new(),
            vec![view2()],
            vec![frame(vec![layer(grid)])],
        )
    }

    #[test]
    fn classified_escape_time_grid_is_a_valid_scene() {
        let result = escape_scene(Some(vec![0, 0]), 2);

        assert!(result.is_ok());
    }

    #[test]
    fn escape_time_grid_without_classes_is_rejected() {
        let result = escape_scene(None, 2);

        assert_eq!(result, Err(SceneError::ClassColumnMissing(FIRST_LAYER)));
    }

    #[test]
    fn escape_time_class_code_above_undecided_is_rejected() {
        let result = escape_scene(Some(vec![0, 3]), 2);

        assert_eq!(result, Err(SceneError::ClassCodeInvalid(FIRST_LAYER)));
    }

    #[test]
    fn escape_time_classes_shorter_than_the_grid_are_rejected() {
        let result = escape_scene(Some(vec![0]), 2);

        assert_eq!(
            result,
            Err(SceneError::ColumnLength {
                layer: FIRST_LAYER,
                expected: 2,
                found: 1
            })
        );
    }

    #[test]
    fn escape_time_counts_that_miss_cells_are_rejected() {
        let result = escape_scene(Some(vec![0, 0]), 1);

        assert_eq!(
            result,
            Err(SceneError::ClassCountsDisagree {
                cells: 2,
                classified: 1
            })
        );
    }

    fn unit_axis(unit: AxisUnit, dimension: Dimension, scale: Scale) -> View {
        let mut x = axis();
        x.unit = unit;
        x.dimension = dimension;
        x.scale = scale;
        View::View2(Box::new(View2 { x, y: axis() }))
    }

    fn length() -> Dimension {
        let mut exponents = [0; 8];
        exponents[0] = 1;
        Dimension { exponents }
    }

    fn celsius() -> AxisUnit {
        AxisUnit::TemperatureScale {
            factor: integer(1),
            offset: Number::fraction(&Integer::from(5463_i64), &Integer::from(20_i64)).unwrap(),
            symbol: String::from("°C"),
        }
    }

    #[test]
    fn celsius_axis_on_a_temperature_is_valid() {
        let result = scene_with(
            unit_axis(celsius(), Dimension::TEMPERATURE, Scale::Linear),
            polyline(&[1, 1]),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn unit_axis_with_zero_factor_is_rejected() {
        let unit = AxisUnit::Unit {
            factor: integer(0),
            pi_exponent: 0,
            symbol: String::from("cm"),
        };

        let result = scene_with(unit_axis(unit, length(), Scale::Linear), polyline(&[1, 1]));

        assert_eq!(
            result,
            Err(SceneError::AxisFactorNotPositive { view: 0, axis: 0 })
        );
    }

    #[test]
    fn temperature_scale_offset_in_machine_numbers_is_not_exact() {
        let unit = AxisUnit::TemperatureScale {
            factor: integer(1),
            offset: Number::F64(273.15),
            symbol: String::from("°C"),
        };

        let result = scene_with(
            unit_axis(unit, Dimension::TEMPERATURE, Scale::Linear),
            polyline(&[1, 1]),
        );

        assert_eq!(result, Err(SceneError::NotExact(Location::View(0))));
    }

    #[test]
    fn temperature_scale_on_a_length_is_rejected() {
        let result = scene_with(
            unit_axis(celsius(), length(), Scale::Linear),
            polyline(&[1, 1]),
        );

        assert_eq!(
            result,
            Err(SceneError::TemperatureScaleOnOtherDimension { view: 0, axis: 0 })
        );
    }

    #[test]
    fn temperature_scale_on_a_logarithmic_axis_is_rejected() {
        let result = scene_with(
            unit_axis(celsius(), Dimension::TEMPERATURE, Scale::Logarithmic),
            polyline(&[1, 1]),
        );

        assert_eq!(
            result,
            Err(SceneError::TemperatureScaleOnLogarithmicAxis { view: 0, axis: 0 })
        );
    }

    #[test]
    fn dimensioned_axis_without_a_symbol_is_rejected() {
        let result = scene_with(
            unit_axis(AxisUnit::dimensionless(), length(), Scale::Linear),
            polyline(&[1, 1]),
        );

        assert_eq!(
            result,
            Err(SceneError::AxisSymbolMissing { view: 0, axis: 0 })
        );
    }

    #[test]
    fn dimensionless_coherent_axis_with_a_symbol_is_rejected() {
        let unit = AxisUnit::Coherent {
            symbol: String::from("m"),
        };

        let result = scene_with(
            unit_axis(unit, Dimension::DIMENSIONLESS, Scale::Linear),
            polyline(&[1, 1]),
        );

        assert_eq!(
            result,
            Err(SceneError::AxisSymbolOnDimensionless { view: 0, axis: 0 })
        );
    }
}
