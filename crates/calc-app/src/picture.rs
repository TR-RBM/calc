use std::cmp::Ordering;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use calc_concepts::QuantityKind;
use calc_core::{Diagnostic, evaluate_exact};
use calc_exec::{Backend, Domain};
use calc_expr::{ExprId, ExprPool, SymbolId};
use calc_numbers::{Integer, Number};
use calc_syntax::parse_expression;
use calc_viz::{
    AxisBounds, AxisUnit, Camera, ColourLegend, Dimension, Emphasis, Interval, IterationLimitRule,
    KindColour, Projection, Reading, RefinementPlan, RefinementStep, ResultId, SampleAxis,
    SampleError, SampleRequest, SampledShape, SamplingRun, Scene, View,
};

use calc_units::{BaseDimension, Dimension as UnitDimension};

use calc_i18n::{Locale, Message, render};

use crate::json::{self, Json};
use crate::line_picture::{
    AxisDisplayUnit, CameraProjection, Picture, PictureCamera, PictureParameter,
};
use crate::platform::{Job, JobState};
use crate::result_record::LineId;
use crate::session::{Precision, Settings};
use crate::session_file::number_json;

const PICTURE_SAMPLE_LIMIT: u64 = 16_000_000;
const SAMPLES_PER_STEP: usize = 4096;
const BOUNDS_PER_STEP: usize = 512;
const VALUE_AXIS_ABSENT_DIVISIONS: u32 = 1;
const GESTURE_PRECISION_BITS: u32 = 32;
const DEFAULT_AXIS_BOUND: i64 = 10;
const COMPLEX_AXIS_BOUND: i64 = 2;
const TEMPERATURE_LOWER_BOUND: i64 = 0;
const TEMPERATURE_UPPER_BOUND: i64 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PictureShape {
    Curve,
    ComplexGrid,
    ScalarGrid,
    Surface,
    EscapeTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AxisState {
    pub range: Option<Interval>,
    pub unit: AxisDisplayUnit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewState {
    pub axes: Vec<AxisState>,
    pub camera: Option<PictureCamera>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AxisTitle {
    Name(String),
    Quantity(QuantityKind),
    RealPart(String),
    ImaginaryPart(String),
}

pub fn axis_title_text(title: &AxisTitle, locale: &Locale) -> String {
    match title {
        AxisTitle::Name(name) => name.clone(),
        AxisTitle::Quantity(kind) => {
            render(&crate::unit_override::quantity_kind_message(*kind), locale).to_string()
        }
        AxisTitle::RealPart(name) => render(
            &Message::CommonPictureRealPart { name: name.clone() },
            locale,
        )
        .to_string(),
        AxisTitle::ImaginaryPart(name) => render(
            &Message::CommonPictureImaginaryPart { name: name.clone() },
            locale,
        )
        .to_string(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureDefaults {
    pub views: Vec<ViewState>,
    pub axis_titles: Vec<Vec<AxisTitle>>,
    pub value_name: String,
    pub shape: PictureShape,
    pub legend: Option<ColourLegend>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PictureSlot(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlotRequest {
    pub line: LineId,
    pub slot: PictureSlot,
    pub views: Option<Vec<ViewState>>,
    pub divisions: Vec<Vec<u32>>,
    pub parameters: Option<Vec<PictureParameter>>,
    pub iteration_limit: Option<IterationLimitRule>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlotError {
    UnknownLine(LineId),
    NotPlottable(LineId),
    LineNumberTooLarge(LineId),
    Expansion(Diagnostic),
    ViewCount {
        expected: usize,
        found: usize,
    },
    ViewAxisCount {
        view: usize,
        expected: usize,
        found: usize,
    },
    UnknownUnit {
        view: usize,
        axis: usize,
    },
    UnitOfOtherDimension {
        view: usize,
        axis: usize,
    },
    EmptyInterval {
        view: usize,
        axis: usize,
    },
    UnknownParameter(String),
    NoAxisLeft(LineId),
    DivisionsMissing {
        view: usize,
        axis: usize,
    },
    ViewKindMismatch(LineId),
    RangeMissing {
        view: usize,
        axis: usize,
    },
    ValueKindNotPlottable(LineId),
    TooManyAxisVariables(LineId),
    IterationLimitNotApplicable(LineId),
    ParametersNotApplicable(LineId),
    Sample(SampleError),
}

#[derive(Clone, Debug, PartialEq)]
pub enum PictureEvent {
    Samples {
        line: LineId,
        slot: PictureSlot,
        generation: u64,
        scene: Box<Scene>,
    },
    Bounds {
        line: LineId,
        slot: PictureSlot,
        generation: u64,
        scene: Box<Scene>,
    },
    Failed {
        line: LineId,
        slot: PictureSlot,
        generation: u64,
        error: SampleError,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlotStage {
    Samples,
    Bounds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PictureAddress {
    pub line: LineId,
    pub slot: PictureSlot,
    pub generation: u64,
}

pub(crate) enum PlotWork {
    Expression(Box<SamplingRun>),
    Refinement(Box<RefinementPlan>),
}

enum PlotRun {
    Expression {
        run: Box<SamplingRun>,
        stage: PlotStage,
    },
    Levels {
        plan: Option<Box<RefinementPlan>>,
        run: Option<Box<calc_viz::RefinementRun>>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct SettleRequest {
    pub line: LineId,
    pub views: Vec<ViewState>,
    pub parameters: Vec<PictureParameter>,
    pub iteration_limit: Option<IterationLimitRule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadingAxis {
    pub range: Interval,
    pub divisions: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompletedScene {
    pub generation: u64,
    pub views: Vec<Vec<ReadingAxis>>,
    pub layers: Vec<Vec<Reading>>,
}

pub(crate) type SceneCompletion = Arc<Mutex<Option<CompletedScene>>>;

fn completed_scene(generation: u64, scene: &Scene) -> CompletedScene {
    CompletedScene {
        generation,
        views: scene
            .views
            .iter()
            .map(|view| {
                view.axes()
                    .into_iter()
                    .map(|axis| ReadingAxis {
                        range: axis.range.clone(),
                        divisions: axis.divisions,
                    })
                    .collect()
            })
            .collect(),
        layers: scene
            .frames
            .first()
            .map(|frame| {
                frame
                    .layers
                    .iter()
                    .map(|layer| layer.readings.clone())
                    .collect()
            })
            .unwrap_or_default(),
    }
}

pub struct PlotJob {
    run: PlotRun,
    cameras: Vec<Option<PictureCamera>>,
    backends: Arc<Vec<Arc<dyn Backend>>>,
    cancellation: Arc<AtomicBool>,
    events: Sender<PictureEvent>,
    address: PictureAddress,
    completion: SceneCompletion,
    sent: u64,
}

impl PlotJob {
    pub(crate) fn new(
        work: PlotWork,
        cameras: Vec<Option<PictureCamera>>,
        backends: Arc<Vec<Arc<dyn Backend>>>,
        cancellation: Arc<AtomicBool>,
        events: Sender<PictureEvent>,
        address: PictureAddress,
        completion: SceneCompletion,
    ) -> Self {
        let run = match work {
            PlotWork::Expression(run) => PlotRun::Expression {
                run,
                stage: PlotStage::Samples,
            },
            PlotWork::Refinement(plan) => PlotRun::Levels {
                plan: Some(plan),
                run: None,
            },
        };
        Self {
            run,
            cameras,
            backends,
            cancellation,
            events,
            address,
            completion,
            sent: 0,
        }
    }

    pub fn sent_events(&self) -> u64 {
        self.sent
    }

    pub fn slot(&self) -> PictureSlot {
        self.address.slot
    }

    pub fn generation(&self) -> u64 {
        self.address.generation
    }

    fn record_completion(&self, scene: &Scene) {
        if self.is_cancelled() {
            return;
        }
        if let Ok(mut completion) = self.completion.lock() {
            *completion = Some(completed_scene(self.address.generation, scene));
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancellation.load(AtomicOrdering::SeqCst)
    }

    fn with_cameras(&self, mut scene: Scene) -> Scene {
        for (view, camera) in scene.views.iter_mut().zip(&self.cameras) {
            if let (View::View3(space), Some(camera)) = (view, camera) {
                space.camera = viz_camera(camera);
            }
        }
        scene
    }
}

fn as_provisional(mut scene: Scene) -> Scene {
    for frame in &mut scene.frames {
        for layer in &mut frame.layers {
            layer.value_bounds.clear();
            layer.style.emphasis = Emphasis::Provisional;
        }
    }
    scene
}

impl PlotJob {
    fn send_unless_cancelled(&mut self, event: PictureEvent) {
        if !self.is_cancelled() {
            self.events.send(event).ok();
            self.sent = self.sent.saturating_add(1);
        }
    }

    fn samples(&self, scene: Scene) -> PictureEvent {
        PictureEvent::Samples {
            line: self.address.line,
            slot: self.address.slot,
            generation: self.address.generation,
            scene: Box::new(self.with_cameras(scene)),
        }
    }

    fn bounds(&self, scene: Scene) -> PictureEvent {
        PictureEvent::Bounds {
            line: self.address.line,
            slot: self.address.slot,
            generation: self.address.generation,
            scene: Box::new(self.with_cameras(scene)),
        }
    }

    fn failed(&mut self, error: SampleError) -> JobState {
        self.send_unless_cancelled(PictureEvent::Failed {
            line: self.address.line,
            slot: self.address.slot,
            generation: self.address.generation,
            error,
        });
        JobState::Finished
    }

    fn step_levels(&mut self) -> JobState {
        let backends: Vec<&dyn Backend> = self.backends.iter().map(AsRef::as_ref).collect();
        let PlotRun::Levels { plan, run } = &mut self.run else {
            return JobState::Finished;
        };
        if let Some(prepared) = plan.take() {
            *run = Some(Box::new(prepared.build()));
            return JobState::Pending;
        }
        let Some(levels) = run.as_mut() else {
            return JobState::Finished;
        };
        let cancelled = Arc::clone(&self.cancellation);
        let is_cancelled = move || cancelled.load(AtomicOrdering::SeqCst);
        match levels.step(&backends, SAMPLES_PER_STEP, &is_cancelled) {
            Ok(RefinementStep::Pending) => JobState::Pending,
            Ok(RefinementStep::Cancelled) => JobState::Finished,
            Ok(RefinementStep::Level {
                level,
                levels,
                scene,
            }) => {
                if level + 1 < levels {
                    self.send_unless_cancelled(self.samples(as_provisional(*scene)));
                    JobState::Pending
                } else {
                    self.record_completion(&scene);
                    self.send_unless_cancelled(self.bounds(*scene));
                    JobState::Finished
                }
            }
            Err(error) => self.failed(error),
        }
    }
}

impl Job for PlotJob {
    fn step(&mut self) -> JobState {
        if self.is_cancelled() {
            return JobState::Finished;
        }
        let backends: Vec<&dyn Backend> = self.backends.iter().map(AsRef::as_ref).collect();
        let (run, stage) = match &mut self.run {
            PlotRun::Levels { .. } => return self.step_levels(),
            PlotRun::Expression { run, stage } => (run, stage),
        };
        match stage {
            PlotStage::Samples => match run.step_samples(&backends, SAMPLES_PER_STEP) {
                Ok(None) => JobState::Pending,
                Ok(Some(scene)) => {
                    let event = self.samples(scene);
                    self.send_unless_cancelled(event);
                    if let PlotRun::Expression { stage, .. } = &mut self.run {
                        *stage = PlotStage::Bounds;
                    }
                    JobState::Pending
                }
                Err(error) => self.failed(error),
            },
            PlotStage::Bounds => match run.step_bounds(BOUNDS_PER_STEP) {
                Ok(None) => JobState::Pending,
                Ok(Some(scene)) => {
                    self.record_completion(&scene);
                    let event = self.bounds(scene);
                    self.send_unless_cancelled(event);
                    JobState::Finished
                }
                Err(error) => self.failed(error),
            },
        }
    }
}

fn viz_camera(camera: &PictureCamera) -> Camera {
    Camera {
        azimuth_degrees: camera.azimuth_degrees.clone(),
        elevation_degrees: camera.elevation_degrees.clone(),
        projection: match &camera.projection {
            CameraProjection::Orthographic => Projection::Orthographic,
            CameraProjection::Perspective {
                field_of_view_degrees,
            } => Projection::Perspective {
                field_of_view_degrees: field_of_view_degrees.clone(),
            },
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PictureOutline {
    pub shape: PictureShape,
    pub sampled_axes: usize,
    pub view_axes: usize,
}

pub(crate) fn outline_for(
    line: LineId,
    axis_variables: usize,
    is_complex: bool,
    is_space: bool,
) -> Result<PictureOutline, PlotError> {
    let outline = |shape, sampled_axes, view_axes| PictureOutline {
        shape,
        sampled_axes,
        view_axes,
    };
    match (axis_variables, is_complex, is_space) {
        (0, _, _) => Err(PlotError::NotPlottable(line)),
        (1, false, false) => Ok(outline(PictureShape::Curve, 1, 2)),
        (1, true, false) => Ok(outline(PictureShape::ComplexGrid, 2, 2)),
        (1, _, true) => Err(PlotError::ViewKindMismatch(line)),
        (2, false, false) => Ok(outline(PictureShape::ScalarGrid, 2, 2)),
        (2, false, true) => Ok(outline(PictureShape::Surface, 2, 3)),
        (2, true, _) => Err(PlotError::ValueKindNotPlottable(line)),
        _ => Err(PlotError::TooManyAxisVariables(line)),
    }
}

pub(crate) fn outline_legend(outline: PictureOutline) -> Option<ColourLegend> {
    match outline.shape {
        PictureShape::Curve => None,
        PictureShape::ScalarGrid | PictureShape::Surface => Some(ColourLegend::Sequential),
        PictureShape::ComplexGrid => Some(ColourLegend::DomainColouring),
        PictureShape::EscapeTime => Some(ColourLegend::EscapeTime),
    }
}

pub(crate) fn default_sampled_interval(
    outline: PictureOutline,
    dimension: UnitDimension,
    unit: &AxisDisplayUnit,
) -> Interval {
    let interval = |lower: i64, upper: i64| Interval {
        lower: Number::from(lower),
        upper: Number::from(upper),
    };
    let is_temperature = dimension
        == UnitDimension::of_base(BaseDimension::ThermodynamicTemperature)
        && matches!(
            unit,
            AxisDisplayUnit::Coherent | AxisDisplayUnit::TemperatureScale(_)
        );
    if outline.shape == PictureShape::ComplexGrid {
        interval(-COMPLEX_AXIS_BOUND, COMPLEX_AXIS_BOUND)
    } else if is_temperature {
        interval(TEMPERATURE_LOWER_BOUND, TEMPERATURE_UPPER_BOUND)
    } else {
        interval(-DEFAULT_AXIS_BOUND, DEFAULT_AXIS_BOUND)
    }
}

pub(crate) fn default_views(outline: PictureOutline) -> Vec<ViewState> {
    let axes = (0..outline.view_axes)
        .map(|axis| AxisState {
            range: (axis < outline.sampled_axes).then(|| {
                default_sampled_interval(
                    outline,
                    UnitDimension::DIMENSIONLESS,
                    &AxisDisplayUnit::Coherent,
                )
            }),
            unit: AxisDisplayUnit::Coherent,
        })
        .collect();
    vec![ViewState { axes, camera: None }]
}

pub(crate) fn stored_views(picture: &Picture) -> Vec<ViewState> {
    picture
        .views
        .iter()
        .map(|view| ViewState {
            axes: view
                .axes
                .iter()
                .map(|axis| AxisState {
                    range: Some(Interval {
                        lower: axis.lower.clone(),
                        upper: axis.upper.clone(),
                    }),
                    unit: axis.unit.clone(),
                })
                .collect(),
            camera: view.camera.clone(),
        })
        .collect()
}

pub(crate) struct PlotAxis {
    pub range: Option<Interval>,
    pub divisions: u32,
    pub dimension: Dimension,
    pub unit: AxisUnit,
}

pub(crate) struct PlotPlan<'a> {
    pub line: LineId,
    pub outline: PictureOutline,
    pub expression: ExprId,
    pub variables: &'a [(SymbolId, String)],
    pub input: &'a str,
    pub axes: Vec<PlotAxis>,
    pub value_dimension: Dimension,
    pub settings: Settings,
}

pub(crate) fn sample_request(plan: &PlotPlan<'_>) -> Result<SampleRequest, PlotError> {
    let result = u32::try_from(plan.line.number())
        .map(ResultId)
        .map_err(|_| PlotError::LineNumberTooLarge(plan.line))?;
    let sampled = plan.outline.sampled_axes;
    let axes = plan
        .axes
        .iter()
        .take(sampled)
        .enumerate()
        .filter_map(|(index, axis)| {
            plan.variables
                .get(index.min(plan.variables.len().saturating_sub(1)))
                .map(|(symbol, name)| SampleAxis {
                    symbol: *symbol,
                    name: name.clone(),
                    bounds: AxisBounds::Exact(axis.range.clone().unwrap_or_else(|| {
                        default_sampled_interval(
                            plan.outline,
                            UnitDimension::DIMENSIONLESS,
                            &AxisDisplayUnit::Coherent,
                        )
                    })),
                    divisions: axis.divisions,
                    dimension: axis.dimension,
                    unit: axis.unit.clone(),
                })
        })
        .collect();
    let value_axis = plan.axes.get(sampled);
    let shape = match plan.outline.shape {
        PictureShape::Curve => SampledShape::Curve(plan.expression),
        PictureShape::ComplexGrid => SampledShape::ComplexGrid(plan.expression),
        PictureShape::ScalarGrid => SampledShape::ScalarGrid(plan.expression),
        PictureShape::Surface => SampledShape::Surface(plan.expression),
        PictureShape::EscapeTime => SampledShape::EscapeTime,
    };
    let domain = match plan.settings.precision {
        Precision::F32 => Domain::F32,
        Precision::F64 | Precision::Exact => Domain::F64,
    };
    Ok(SampleRequest {
        result,
        expression_text: plan.input.to_string(),
        shape,
        axes,
        value_range: value_axis.and_then(|axis| axis.range.clone()),
        value_dimension: plan.value_dimension,
        value_unit: value_axis.map_or_else(AxisUnit::dimensionless, |axis| axis.unit.clone()),
        value_divisions: value_axis.map_or(VALUE_AXIS_ABSENT_DIVISIONS, |axis| axis.divisions),
        domain,
        preference: plan.settings.backend,
        sample_limit: PICTURE_SAMPLE_LIMIT,
        kind: KindColour::Numeric,
        references: Vec::new(),
        escape_time: None,
    })
}

fn exact_sign(number: &Number) -> Option<Ordering> {
    let numerator = match number {
        Number::Integer(integer) => integer,
        Number::Rational(rational) => rational.numerator(),
        Number::F32(_) | Number::F64(_) => return None,
    };
    Some(if numerator.is_zero() {
        Ordering::Equal
    } else if numerator.is_negative() {
        Ordering::Less
    } else {
        Ordering::Greater
    })
}

fn exact_order(left: &Number, right: &Number) -> Option<Ordering> {
    left.sub_exact(right).ok().as_ref().and_then(exact_sign)
}

fn finite_exact(value: f64) -> Option<Number> {
    Number::F64(value).to_exact().ok()
}

fn gesture_step(width: &Number) -> Option<Number> {
    let two = Number::from(2_i64);
    let half = Number::fraction(&Integer::one(), &Integer::from(2_i64)).ok()?;
    let target = width
        .div_exact(&Number::Integer(
            Integer::from(2_i64).pow(GESTURE_PRECISION_BITS),
        ))
        .ok()?;
    let mut step = Number::from(1_i64);
    while exact_order(&step, &target)? == Ordering::Greater {
        step = step.mul_exact(&half).ok()?;
    }
    loop {
        let doubled = step.mul_exact(&two).ok()?;
        if exact_order(&doubled, &target)? == Ordering::Greater {
            return Some(step);
        }
        step = doubled;
    }
}

fn round_to_multiple(value: &Number, step: &Number) -> Option<Number> {
    let quotient = value.div_exact(step).ok()?;
    let (numerator, denominator) = match &quotient {
        Number::Integer(_) => return Some(value.clone()),
        Number::Rational(rational) => {
            (rational.numerator().clone(), rational.denominator().clone())
        }
        Number::F32(_) | Number::F64(_) => return None,
    };
    let (floor, remainder) = numerator.div_rem_euclid(&denominator).ok()?;
    let twice_remainder = &remainder * &Integer::from(2_i64);
    let rounded = match twice_remainder.cmp(&denominator) {
        Ordering::Less => floor,
        Ordering::Greater => &floor + &Integer::one(),
        Ordering::Equal => {
            let (_, parity) = floor.div_rem_euclid(&Integer::from(2_i64)).ok()?;
            if parity.is_zero() {
                floor
            } else {
                &floor + &Integer::one()
            }
        }
    };
    Number::Integer(rounded).mul_exact(step).ok()
}

pub fn settle_gesture_range(lower: &Number, upper: &Number) -> Option<Interval> {
    let width = upper.sub_exact(lower).ok()?;
    if exact_sign(&width)? != Ordering::Greater {
        return None;
    }
    let step = gesture_step(&width)?;
    Some(Interval {
        lower: round_to_multiple(lower, &step)?,
        upper: round_to_multiple(upper, &step)?,
    })
}

pub fn pan_range(range: &Interval, offset: f64) -> Option<Interval> {
    let offset = finite_exact(offset)?;
    settle_gesture_range(
        &range.lower.add_exact(&offset).ok()?,
        &range.upper.add_exact(&offset).ok()?,
    )
}

pub fn zoom_range(range: &Interval, focus: f64, factor: f64) -> Option<Interval> {
    let focus = finite_exact(focus)?;
    let factor = finite_exact(factor)?;
    if exact_sign(&factor)? != Ordering::Greater {
        return None;
    }
    let scaled = |bound: &Number| {
        bound
            .sub_exact(&focus)
            .and_then(|distance| distance.div_exact(&factor))
            .and_then(|distance| focus.add_exact(&distance))
            .ok()
    };
    settle_gesture_range(&scaled(&range.lower)?, &scaled(&range.upper)?)
}

pub fn exact_number(text: &str) -> Option<Number> {
    let mut pool = ExprPool::new();
    let expression = parse_expression(&mut pool, text).ok()?;
    evaluate_exact(&mut pool, expression)
        .ok()?
        .rational_value()
        .cloned()
}

fn axis_unit_json(unit: &AxisDisplayUnit) -> Json {
    match unit {
        AxisDisplayUnit::Coherent => Json::Null,
        AxisDisplayUnit::Unit(text) => Json::object(vec![("unit", Json::string(text))]),
        AxisDisplayUnit::TemperatureScale(scale) => {
            Json::object(vec![("scale", Json::string(scale.name()))])
        }
    }
}

pub fn plot_json(
    path: &str,
    width: u16,
    height: u16,
    views: &[ViewState],
    scene: &Scene,
) -> Vec<u8> {
    let views_json = views
        .iter()
        .map(|view| {
            Json::object(vec![(
                "axes",
                Json::Array(
                    view.axes
                        .iter()
                        .map(|axis| {
                            let (lower, upper) = match &axis.range {
                                Some(range) => {
                                    (number_json(&range.lower), number_json(&range.upper))
                                }
                                None => (Json::Null, Json::Null),
                            };
                            Json::object(vec![
                                ("lower", lower),
                                ("upper", upper),
                                ("unit", axis_unit_json(&axis.unit)),
                            ])
                        })
                        .collect(),
                ),
            )])
        })
        .collect();
    let layers = scene
        .frames
        .first()
        .map(|frame| {
            frame
                .layers
                .iter()
                .map(|layer| match &layer.precision {
                    None => Json::Null,
                    Some(limit) => Json::object(vec![
                        (
                            "grid_exhausted",
                            Json::Array(
                                limit
                                    .grid_exhausted
                                    .iter()
                                    .map(|axis| Json::Count(u64::from(*axis)))
                                    .collect(),
                            ),
                        ),
                        ("unresolved_samples", Json::Count(limit.unresolved_samples)),
                        ("unknown_bounds", Json::Count(limit.unknown_bounds)),
                        ("marked_columns", Json::Count(limit.marked_columns)),
                        (
                            "varies_below_bounds",
                            Json::Boolean(limit.varies_below_bounds),
                        ),
                    ]),
                })
                .collect()
        })
        .unwrap_or_default();
    json::write_canonical(&Json::object(vec![
        ("path", Json::string(path)),
        ("width", Json::Count(u64::from(width))),
        ("height", Json::Count(u64::from(height))),
        ("views", Json::Array(views_json)),
        ("precision_limits", Json::Array(layers)),
    ]))
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reading::{ReadCoordinate, ReadError, ReadEvent, ReadRequest};
    use crate::session::Session;
    use crate::session::tests::{fixed_clock, session};
    use calc_core::ResultValue;
    use calc_viz::{Column, Emphasis, Primitive};
    use std::sync::mpsc::{Receiver, channel};

    fn range(lower: i64, upper: i64) -> Interval {
        Interval {
            lower: Number::from(lower),
            upper: Number::from(upper),
        }
    }

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    fn axis(lower: i64, upper: i64) -> AxisState {
        AxisState {
            range: Some(range(lower, upper)),
            unit: AxisDisplayUnit::Coherent,
        }
    }

    fn view(axes: Vec<AxisState>) -> ViewState {
        ViewState { axes, camera: None }
    }

    fn curve_request(line: LineId, lower: i64, upper: i64, divisions: u32) -> PlotRequest {
        slot_request(line, PictureSlot(0), lower, upper, divisions)
    }

    fn slot_request(
        line: LineId,
        slot: PictureSlot,
        lower: i64,
        upper: i64,
        divisions: u32,
    ) -> PlotRequest {
        PlotRequest {
            line,
            slot,
            iteration_limit: None,
            views: Some(vec![view(vec![axis(lower, upper), axis(0, 1)])]),
            divisions: vec![vec![divisions, 100]],
            parameters: None,
        }
    }

    fn run_to_end(job: &mut PlotJob) -> usize {
        let mut steps = 1;
        while job.step() == JobState::Pending {
            steps += 1;
        }
        steps
    }

    fn finished_scene(events: &Receiver<PictureEvent>) -> Scene {
        match events.try_recv() {
            Ok(PictureEvent::Samples { .. }) => finished_scene(events),
            Ok(PictureEvent::Bounds { scene, .. }) => *scene,
            other => panic!("expected a finished picture, got {other:?}"),
        }
    }

    fn provisional_scene(events: &Receiver<PictureEvent>) -> Scene {
        match events.try_recv() {
            Ok(PictureEvent::Samples { scene, .. }) => *scene,
            other => panic!("expected a provisional picture, got {other:?}"),
        }
    }

    fn curve_bits(scene: &Scene) -> Vec<u64> {
        let Primitive::Polyline(polyline) = &scene.frames[0].layers[0].primitive else {
            panic!("expected a polyline");
        };
        match &polyline.coordinates[1] {
            Column::F64(values) => values.iter().map(|value| value.to_bits()).collect(),
            Column::F32(_) => panic!("expected f64 samples"),
        }
    }

    fn square_session() -> (Session, LineId) {
        let mut session = session();
        let line = session.enter("x^2").unwrap();
        (session, line)
    }

    fn function_session() -> (Session, LineId) {
        let mut session = session();
        let line = session.enter("f(x) = x^2").unwrap();
        (session, line)
    }

    fn plotted(session: &mut Session, line: LineId) -> u64 {
        let (sender, _events) = channel();
        let mut job = session.plot(&curve_request(line, 0, 1, 2), sender).unwrap();
        let generation = job.generation();
        run_to_end(&mut job);
        generation
    }

    fn read_request(line: LineId, generation: u64, at: Vec<ReadCoordinate>) -> ReadRequest {
        ReadRequest {
            line,
            slot: PictureSlot(0),
            generation,
            layer: 0,
            at,
            commit: false,
        }
    }

    fn reading_value(events: &Receiver<ReadEvent>) -> ResultValue {
        match events.try_iter().last() {
            Some(ReadEvent::ReadingFinished { value, .. }) => value.computed().value().clone(),
            other => panic!("expected a finished reading, got {other:?}"),
        }
    }

    fn unit_axis(lower: i64, upper: i64, unit: &str) -> AxisState {
        AxisState {
            range: Some(range(lower, upper)),
            unit: AxisDisplayUnit::Unit(unit.to_owned()),
        }
    }

    fn speed_scene(input: AxisState, value: AxisState) -> Result<Scene, PlotError> {
        let mut session = session();
        let line = session.enter("x * 1 m/s").unwrap();
        let (sender, events) = channel();
        let mut job = session.plot(
            &PlotRequest {
                line,
                slot: PictureSlot(0),
                iteration_limit: None,
                views: Some(vec![view(vec![input, value])]),
                divisions: vec![vec![4, 4]],
                parameters: None,
            },
            sender,
        )?;
        run_to_end(&mut job);
        Ok(finished_scene(&events))
    }

    #[test]
    fn an_input_axis_takes_the_dimension_of_the_unit_it_is_shown_in() {
        let scene = speed_scene(unit_axis(0, 2, "s"), axis(0, 2)).expect("a picture");

        let axes = scene.views[0].axes();

        assert_eq!(axes[0].unit.symbol(), "s");
    }

    #[test]
    fn a_value_axis_takes_the_dimension_the_input_unit_gives_it() {
        let scene = speed_scene(unit_axis(0, 2, "s"), axis(0, 2)).expect("a picture");

        let axes = scene.views[0].axes();

        assert_eq!(axes[1].unit.symbol(), "m");
    }

    #[test]
    fn a_value_axis_can_be_shown_in_a_unit_of_the_dimension_its_input_gives_it() {
        let scene = speed_scene(unit_axis(0, 2, "s"), unit_axis(0, 200, "cm")).expect("a picture");

        let axes = scene.views[0].axes();

        assert_eq!(axes[1].unit.symbol(), "cm");
    }

    #[test]
    fn an_input_unit_of_another_dimension_than_the_value_asks_for_is_taken_as_given() {
        let scene = speed_scene(unit_axis(0, 2, "h"), axis(0, 2)).expect("a picture");

        let axes = scene.views[0].axes();

        assert_eq!((axes[0].unit.symbol(), axes[1].unit.symbol()), ("h", "m"));
    }

    #[test]
    fn a_value_unit_of_another_dimension_is_refused() {
        let refused = speed_scene(unit_axis(0, 2, "s"), unit_axis(0, 2, "kg"));

        assert_eq!(
            refused.err(),
            Some(PlotError::UnitOfOtherDimension { view: 0, axis: 1 })
        );
    }

    fn parameter(name: &str, value: i64) -> PictureParameter {
        PictureParameter {
            name: name.to_owned(),
            value: Number::from(value),
        }
    }

    fn plotted_with(input: &str, parameters: Vec<PictureParameter>) -> Result<Scene, PlotError> {
        let mut session = session();
        let line = session.enter(input).unwrap();
        let (sender, events) = channel();
        let mut job = session.plot(
            &PlotRequest {
                line,
                slot: PictureSlot(0),
                iteration_limit: None,
                views: Some(vec![view(vec![axis(0, 1), axis(0, 1)])]),
                divisions: vec![vec![4, 4]],
                parameters: Some(parameters),
            },
            sender,
        )?;
        run_to_end(&mut job);
        Ok(finished_scene(&events))
    }

    fn titles_of(input: &str, axes: Vec<AxisState>) -> Vec<AxisTitle> {
        let mut session = session();
        let line = session.enter(input).unwrap();
        session
            .axis_titles(line, None, &[view(axes)])
            .expect("titles")
            .remove(0)
    }

    #[test]
    fn an_axis_whose_dimension_names_a_quantity_carries_that_name() {
        let titles = titles_of("x * 1 m/s", vec![unit_axis(0, 2, "s"), axis(0, 2)]);

        assert_eq!(
            titles,
            vec![
                AxisTitle::Quantity(calc_concepts::QuantityKind::Time),
                AxisTitle::Quantity(calc_concepts::QuantityKind::Length)
            ]
        );
    }

    #[test]
    fn an_axis_without_a_dimension_keeps_the_name_it_had() {
        let titles = titles_of("x^2", vec![axis(0, 2), axis(0, 2)]);

        assert_eq!(
            titles,
            vec![
                AxisTitle::Name(String::from("x")),
                AxisTitle::Name(String::from("r1"))
            ]
        );
    }

    #[test]
    fn a_dimension_that_names_no_single_quantity_keeps_the_name_it_had() {
        let titles = titles_of("x * 1 J", vec![axis(0, 2), axis(0, 2)]);

        assert_eq!(titles[1], AxisTitle::Name(String::from("r1")));
    }

    #[test]
    fn a_line_the_person_named_keeps_its_name_on_the_value_axis() {
        let titles = titles_of("v = x * 1 m/s", vec![unit_axis(0, 2, "s"), axis(0, 2)]);

        assert_eq!(titles[1], AxisTitle::Name(String::from("v")));
    }

    #[test]
    fn an_axis_unit_that_makes_the_expression_inhomogeneous_is_refused() {
        let mut session = session();
        let line = session.enter("min(x, 36)").unwrap();

        let refused =
            session.axis_titles(line, None, &[view(vec![unit_axis(0, 2, "s"), axis(0, 2)])]);

        assert!(matches!(refused, Err(PlotError::Expansion(_))));
    }

    #[test]
    fn a_parameter_names_a_free_variable_of_a_line_with_a_function_head() {
        let scene = plotted_with("psi(x) = n*x", vec![parameter("n", 3)]);

        assert!(scene.is_ok());
    }

    #[test]
    fn a_line_with_a_function_head_keeps_its_parameter_as_the_axis() {
        let scene = plotted_with("psi(x) = n*x", vec![parameter("n", 3)]).expect("a picture");

        assert_eq!(scene.record.inputs[0].variables, vec![String::from("x")]);
    }

    #[test]
    fn a_value_for_the_variable_the_picture_is_drawn_over_says_no_axis_is_left() {
        let refused = plotted_with("psi(x) = n*x", vec![parameter("x", 3)]);

        assert!(matches!(refused.err(), Some(PlotError::NoAxisLeft(_))));
    }

    #[test]
    fn a_parameter_that_names_nothing_in_the_line_is_refused() {
        let refused = plotted_with("psi(x) = n*x", vec![parameter("k", 3)]);

        assert_eq!(
            refused.err(),
            Some(PlotError::UnknownParameter(String::from("k")))
        );
    }

    #[test]
    fn a_transient_reading_evaluates_the_plotted_line_at_the_typed_coordinate() {
        let (mut session, line) = square_session();
        let generation = plotted(&mut session, line);
        let (sender, events) = channel();
        let request = read_request(
            line,
            generation,
            vec![ReadCoordinate::Exact(fraction(1, 2))],
        );

        let mut job = session.read(&request, sender).unwrap();
        while job.step() == JobState::Pending {}

        assert_eq!(reading_value(&events), ResultValue::Number(fraction(1, 4)));
    }

    #[test]
    fn a_pointer_coordinate_is_snapped_before_it_is_read() {
        let (mut session, line) = square_session();
        let generation = plotted(&mut session, line);
        let (sender, events) = channel();
        let request = read_request(line, generation, vec![ReadCoordinate::Pointer(0.2617)]);

        let mut job = session.read(&request, sender).unwrap();
        while job.step() == JobState::Pending {}

        assert_eq!(
            reading_value(&events),
            ResultValue::Number(fraction(9, 100))
        );
    }

    #[test]
    fn a_reading_of_an_older_generation_is_refused() {
        let (mut session, line) = square_session();
        let generation = plotted(&mut session, line);
        let (sender, _events) = channel();
        let request = read_request(
            line,
            generation - 1,
            vec![ReadCoordinate::Exact(fraction(1, 2))],
        );

        let outcome = session.read(&request, sender).err();

        assert_eq!(outcome, Some(ReadError::SceneNotComplete));
    }

    #[test]
    fn a_reading_before_the_bounds_arrive_is_refused() {
        let (mut session, line) = square_session();
        let (plot_sender, _plot_events) = channel();
        let job = session
            .plot(&curve_request(line, 0, 1, 2), plot_sender)
            .unwrap();
        let generation = job.generation();
        let (sender, _events) = channel();
        let request = read_request(
            line,
            generation,
            vec![ReadCoordinate::Exact(fraction(1, 2))],
        );

        let outcome = session.read(&request, sender).err();

        assert_eq!(outcome, Some(ReadError::SceneNotComplete));
    }

    #[test]
    fn a_running_plot_leaves_the_last_finished_scene_readable() {
        let (mut session, line) = square_session();
        let generation = plotted(&mut session, line);
        let (plot_sender, _plot_events) = channel();
        let _running = session
            .plot(&curve_request(line, 0, 1, 10_000), plot_sender)
            .unwrap();
        let (sender, events) = channel();
        let request = read_request(
            line,
            generation,
            vec![ReadCoordinate::Exact(fraction(1, 2))],
        );

        let mut job = session.read(&request, sender).unwrap();
        while job.step() == JobState::Pending {}

        assert_eq!(reading_value(&events), ResultValue::Number(fraction(1, 4)));
    }

    #[test]
    fn a_cancelled_slot_keeps_no_scene_to_read() {
        let (mut session, line) = square_session();
        let generation = plotted(&mut session, line);
        session.cancel_plot(line, PictureSlot(0));
        let (sender, _events) = channel();
        let request = read_request(
            line,
            generation,
            vec![ReadCoordinate::Exact(fraction(1, 2))],
        );

        let outcome = session.read(&request, sender).err();

        assert_eq!(outcome, Some(ReadError::SceneNotComplete));
    }

    #[test]
    fn a_reading_with_too_many_coordinates_names_the_count_it_wanted() {
        let (mut session, line) = square_session();
        let generation = plotted(&mut session, line);
        let (sender, _events) = channel();
        let request = read_request(
            line,
            generation,
            vec![
                ReadCoordinate::Exact(fraction(1, 2)),
                ReadCoordinate::Exact(fraction(1, 2)),
            ],
        );

        let outcome = session.read(&request, sender).err();

        assert_eq!(
            outcome,
            Some(ReadError::CoordinateCount {
                expected: 1,
                found: 2
            })
        );
    }

    #[test]
    fn a_reading_of_a_layer_that_offers_none_is_refused() {
        let (mut session, line) = square_session();
        let generation = plotted(&mut session, line);
        let (sender, _events) = channel();
        let mut request = read_request(
            line,
            generation,
            vec![ReadCoordinate::Exact(fraction(1, 2))],
        );
        request.layer = 7;

        let outcome = session.read(&request, sender).err();

        assert_eq!(outcome, Some(ReadError::LayerHasNoReadings { layer: 7 }));
    }

    #[test]
    fn a_committed_reading_enters_the_call_of_the_function() {
        let (mut session, line) = function_session();
        let generation = plotted(&mut session, line);
        let request = read_request(
            line,
            generation,
            vec![ReadCoordinate::Exact(fraction(1, 2))],
        );

        let committed = session.commit_reading(&request).unwrap();

        assert_eq!(session.line(committed).unwrap().input(), "f(0.5)");
    }

    #[test]
    fn a_committed_reading_keeps_the_picture_it_was_read_from() {
        let (mut session, line) = function_session();
        let generation = plotted(&mut session, line);
        let request = read_request(line, generation, vec![ReadCoordinate::Pointer(0.2617)]);

        let committed = session.commit_reading(&request).unwrap();

        let reading = session.line(committed).unwrap().reading().unwrap().clone();
        assert_eq!(
            (reading.source, reading.at.clone(), reading.snapped),
            (line, vec![fraction(3, 10)], true)
        );
    }

    #[test]
    fn a_committed_reading_of_a_line_without_a_function_form_is_refused() {
        let (mut session, line) = square_session();
        let generation = plotted(&mut session, line);
        let request = read_request(
            line,
            generation,
            vec![ReadCoordinate::Exact(fraction(1, 2))],
        );

        let outcome = session.commit_reading(&request).err();

        assert_eq!(outcome, Some(ReadError::ReadingNeedsFunctionForm(line)));
    }

    #[test]
    fn settle_picture_writes_the_settled_view_and_parameters() {
        let (mut session, line) = square_session();
        let request = SettleRequest {
            iteration_limit: None,
            line,
            views: vec![view(vec![axis(-2, 2), axis(0, 4)])],
            parameters: Vec::new(),
        };

        session.settle_picture(&request).unwrap();

        let picture = session.line(line).unwrap().picture().unwrap().clone();
        assert_eq!(
            (
                picture.views[0].axes[0].lower.clone(),
                picture.views[0].axes[0].upper.clone()
            ),
            (Number::from(-2_i64), Number::from(2_i64))
        );
    }

    #[test]
    fn settle_picture_refuses_an_axis_without_a_range() {
        let (mut session, line) = square_session();
        let request = SettleRequest {
            iteration_limit: None,
            line,
            views: vec![view(vec![
                AxisState {
                    range: None,
                    unit: AxisDisplayUnit::Coherent,
                },
                axis(0, 4),
            ])],
            parameters: Vec::new(),
        };

        let outcome = session.settle_picture(&request).err();

        assert_eq!(outcome, Some(PlotError::RangeMissing { view: 0, axis: 0 }));
    }

    #[test]
    fn settle_picture_refuses_a_parameter_the_line_does_not_have() {
        let (mut session, line) = square_session();
        let request = SettleRequest {
            iteration_limit: None,
            line,
            views: vec![view(vec![axis(-2, 2), axis(0, 4)])],
            parameters: vec![PictureParameter {
                name: "a".to_owned(),
                value: Number::from(1_i64),
            }],
        };

        let outcome = session.settle_picture(&request).err();

        assert_eq!(outcome, Some(PlotError::UnknownParameter("a".to_owned())));
    }

    fn plotted_escape_time(session: &mut Session, line: LineId) -> u64 {
        let (sender, _events) = channel();
        let mut job = session.plot(&escape_time_request(line, 8), sender).unwrap();
        let generation = job.generation();
        run_to_end(&mut job);
        generation
    }

    fn orbit_of(events: &Receiver<ReadEvent>) -> calc_viz::Orbit {
        match events.try_iter().last() {
            Some(ReadEvent::OrbitRead { orbit, .. }) => orbit,
            other => panic!("expected an orbit reading, got {other:?}"),
        }
    }

    #[test]
    fn settle_picture_of_an_escape_time_line_writes_its_iteration_limit() {
        let (mut session, line) = escape_time_session();
        let request = SettleRequest {
            iteration_limit: Some(IterationLimitRule::Fixed { iterations: 300 }),
            line,
            views: vec![view(vec![axis(-2, 1), axis(-1, 1)])],
            parameters: Vec::new(),
        };

        session.settle_picture(&request).unwrap();

        assert_eq!(
            session
                .line(line)
                .unwrap()
                .picture()
                .unwrap()
                .iteration_limit,
            Some(IterationLimitRule::Fixed { iterations: 300 })
        );
    }

    #[test]
    fn settle_picture_of_a_line_that_is_not_an_escape_time_line_refuses_an_iteration_limit() {
        let (mut session, line) = square_session();
        let request = SettleRequest {
            iteration_limit: Some(IterationLimitRule::Fixed { iterations: 300 }),
            line,
            views: vec![view(vec![axis(-2, 2), axis(0, 4)])],
            parameters: Vec::new(),
        };

        let refused = session.settle_picture(&request).err();

        assert_eq!(refused, Some(PlotError::IterationLimitNotApplicable(line)));
    }

    #[test]
    fn settle_picture_of_an_escape_time_line_refuses_a_parameter() {
        let (mut session, line) = escape_time_session();
        let request = SettleRequest {
            iteration_limit: None,
            line,
            views: vec![view(vec![axis(-2, 1), axis(-1, 1)])],
            parameters: vec![PictureParameter {
                name: "a".to_owned(),
                value: Number::from(1_i64),
            }],
        };

        let refused = session.settle_picture(&request).err();

        assert_eq!(refused, Some(PlotError::ParametersNotApplicable(line)));
    }

    #[test]
    fn a_transient_reading_of_an_escape_time_line_gives_the_class_count_and_limit() {
        let (mut session, line) = escape_time_session();
        let generation = plotted_escape_time(&mut session, line);
        let (sender, events) = channel();
        let request = read_request(
            line,
            generation,
            vec![
                ReadCoordinate::Exact(Number::from(2_i64)),
                ReadCoordinate::Exact(Number::from(2_i64)),
            ],
        );

        let mut job = session.read(&request, sender).unwrap();
        while job.step() == JobState::Pending {}

        let orbit = orbit_of(&events);
        assert_eq!(
            (orbit.class, orbit.count, orbit.limit),
            (calc_viz::ESCAPED_CELL, Some(2), 16)
        );
    }

    #[test]
    fn a_transient_reading_of_an_escape_time_line_uses_the_limit_of_its_scene() {
        let (mut session, line) = escape_time_session();
        let generation = plotted_escape_time(&mut session, line);
        let (sender, events) = channel();
        let request = read_request(
            line,
            generation,
            vec![
                ReadCoordinate::Exact(fraction(-3, 4)),
                ReadCoordinate::Exact(fraction(1, 4)),
            ],
        );

        let mut job = session.read(&request, sender).unwrap();
        while job.step() == JobState::Pending {}

        assert_eq!(orbit_of(&events).limit, 16);
    }

    #[test]
    fn a_committed_reading_of_an_escape_time_line_enters_its_own_line() {
        let (mut session, line) = escape_time_session();
        let generation = plotted_escape_time(&mut session, line);
        let request = read_request(
            line,
            generation,
            vec![
                ReadCoordinate::Exact(Number::from(2_i64)),
                ReadCoordinate::Exact(Number::from(2_i64)),
            ],
        );

        let entered = session.commit_reading(&request).unwrap();

        assert_eq!(
            session.line(entered).unwrap().input(),
            r#"escape_time_reading {"form": "quadratic_parameter", "at": {"real": {"type": "integer", "digits": "2"}, "imaginary": {"type": "integer", "digits": "2"}}, "limit": 16}"#
        );
    }

    #[test]
    fn a_committed_reading_line_carries_the_orbit_as_its_outcome() {
        let (mut session, line) = escape_time_session();
        let generation = plotted_escape_time(&mut session, line);
        let request = read_request(
            line,
            generation,
            vec![
                ReadCoordinate::Exact(Number::from(2_i64)),
                ReadCoordinate::Exact(Number::from(2_i64)),
            ],
        );

        let entered = session.commit_reading(&request).unwrap();

        assert_eq!(
            session.line(entered).unwrap().outcome(),
            &crate::session::Outcome::EscapeTimeReading(calc_viz::Orbit {
                class: calc_viz::ESCAPED_CELL,
                count: Some(2),
                limit: 16,
            })
        );
    }

    #[test]
    fn a_committed_reading_line_names_the_picture_it_was_read_from() {
        let (mut session, line) = escape_time_session();
        let generation = plotted_escape_time(&mut session, line);
        let request = read_request(
            line,
            generation,
            vec![
                ReadCoordinate::Exact(Number::from(2_i64)),
                ReadCoordinate::Exact(Number::from(2_i64)),
            ],
        );

        let entered = session.commit_reading(&request).unwrap();

        assert_eq!(
            session
                .line(entered)
                .unwrap()
                .reading()
                .map(|reading| reading.source),
            Some(line)
        );
    }

    #[test]
    fn plot_job_delivers_the_line_sampled_over_the_view() {
        let (mut session, line) = square_session();
        let (sender, events) = channel();
        let mut job = session.plot(&curve_request(line, 0, 1, 2), sender).unwrap();

        run_to_end(&mut job);

        let expected: Vec<u64> = [0.0_f64, 0.25, 1.0]
            .iter()
            .map(|value| value.to_bits())
            .collect();
        assert_eq!(curve_bits(&finished_scene(&events)), expected);
    }

    #[test]
    fn provisional_picture_arrives_before_the_finished_picture() {
        let (mut session, line) = square_session();
        let (sender, events) = channel();
        let mut job = session.plot(&curve_request(line, 0, 1, 2), sender).unwrap();

        run_to_end(&mut job);

        let kinds: Vec<&str> = events
            .try_iter()
            .map(|event| match event {
                PictureEvent::Samples { .. } => "provisional",
                PictureEvent::Bounds { .. } => "finished",
                PictureEvent::Failed { .. } => "failed",
            })
            .collect();
        assert_eq!(kinds, ["provisional", "finished"]);
    }

    #[test]
    fn provisional_picture_is_sent_before_any_bound_is_computed() {
        let (mut session, line) = square_session();
        let (sender, events) = channel();
        let mut job = session
            .plot(&curve_request(line, 0, 1, 10_000), sender)
            .unwrap();

        let states: Vec<JobState> = (0..3).map(|_| job.step()).collect();

        assert_eq!(states, [JobState::Pending; 3]);
        assert!(matches!(
            events.try_recv(),
            Ok(PictureEvent::Samples { .. })
        ));
    }

    #[test]
    fn provisional_picture_has_no_bounds_and_the_provisional_style() {
        let (mut session, line) = square_session();
        let (sender, events) = channel();
        let mut job = session.plot(&curve_request(line, 0, 1, 2), sender).unwrap();

        run_to_end(&mut job);

        let layer = provisional_scene(&events).frames[0].layers[0].clone();
        assert_eq!(
            (layer.value_bounds.len(), layer.style.emphasis),
            (0, Emphasis::Provisional)
        );
    }

    #[test]
    fn finished_picture_has_the_samples_of_the_provisional_picture_and_its_bounds() {
        let (mut session, line) = square_session();
        let (sender, events) = channel();
        let mut job = session.plot(&curve_request(line, 0, 1, 2), sender).unwrap();

        run_to_end(&mut job);

        let provisional = provisional_scene(&events);
        let finished = finished_scene(&events);
        let layer = &finished.frames[0].layers[0];
        assert_eq!(
            (
                curve_bits(&provisional),
                layer.value_bounds.len(),
                layer.style.emphasis
            ),
            (curve_bits(&finished), 1, Emphasis::Normal)
        );
    }

    #[test]
    fn newer_plot_of_the_same_slot_stops_the_older_job_in_its_bound_stage() {
        let (mut session, line) = square_session();
        let (sender, events) = channel();
        let mut older = session
            .plot(&curve_request(line, 0, 1, 10_000), sender.clone())
            .unwrap();
        (0..4).for_each(|_| {
            older.step();
        });
        provisional_scene(&events);
        session.plot(&curve_request(line, 0, 2, 2), sender).unwrap();

        let state = older.step();

        assert_eq!(state, JobState::Finished);
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn plot_of_another_slot_leaves_the_job_of_the_first_slot_running() {
        let (mut session, line) = square_session();
        let (sender, events) = channel();
        let mut inline = session
            .plot(
                &slot_request(line, PictureSlot(0), 0, 1, 10_000),
                sender.clone(),
            )
            .unwrap();
        (0..4).for_each(|_| {
            inline.step();
        });
        provisional_scene(&events);
        session
            .plot(&slot_request(line, PictureSlot(1), 0, 2, 2), sender)
            .unwrap();

        let state = inline.step();

        assert_eq!(state, JobState::Pending);
    }

    #[test]
    fn latest_generation_is_kept_for_each_slot_of_a_line() {
        let (mut session, line) = square_session();
        let (sender, _events) = channel();
        let inline = session
            .plot(&slot_request(line, PictureSlot(0), 0, 1, 2), sender.clone())
            .unwrap();
        let window = session
            .plot(&slot_request(line, PictureSlot(1), 0, 2, 2), sender)
            .unwrap();

        let generations = [
            session.latest_picture_generation(line, PictureSlot(0)),
            session.latest_picture_generation(line, PictureSlot(1)),
        ];

        assert_eq!(
            generations,
            [Some(inline.generation()), Some(window.generation())]
        );
    }

    #[test]
    fn picture_events_name_the_slot_they_were_requested_for() {
        let (mut session, line) = square_session();
        let (sender, events) = channel();
        let mut job = session
            .plot(&slot_request(line, PictureSlot(3), 0, 1, 2), sender)
            .unwrap();

        run_to_end(&mut job);

        let slots: Vec<PictureSlot> = events
            .try_iter()
            .map(|event| match event {
                PictureEvent::Samples { slot, .. }
                | PictureEvent::Bounds { slot, .. }
                | PictureEvent::Failed { slot, .. } => slot,
            })
            .collect();
        assert_eq!(slots, [PictureSlot(3), PictureSlot(3)]);
    }

    #[test]
    fn slots_are_not_saved_with_the_session() {
        let (mut session, line) = square_session();
        let before = session.save_to_bytes().unwrap();
        let (sender, _events) = channel();

        session
            .plot(&slot_request(line, PictureSlot(2), 0, 1, 2), sender)
            .unwrap();

        assert_eq!(session.save_to_bytes().unwrap(), before);
    }

    #[test]
    fn a_slot_that_goes_away_is_cancelled_with_its_running_job() {
        let (mut session, line) = square_session();
        let (sender, events) = channel();
        let mut job = session
            .plot(&slot_request(line, PictureSlot(1), 0, 1, 10_000), sender)
            .unwrap();
        (0..4).for_each(|_| {
            job.step();
        });
        provisional_scene(&events);
        session.cancel_plot(line, PictureSlot(1));

        let state = job.step();

        assert_eq!(state, JobState::Finished);
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn cancelled_slot_has_no_latest_generation() {
        let (mut session, line) = square_session();
        let (sender, _events) = channel();
        session
            .plot(&slot_request(line, PictureSlot(1), 0, 1, 2), sender)
            .unwrap();

        session.cancel_plot(line, PictureSlot(1));

        assert_eq!(
            session.latest_picture_generation(line, PictureSlot(1)),
            None
        );
    }

    #[test]
    fn cancelling_one_slot_leaves_the_job_of_another_slot_running() {
        let (mut session, line) = square_session();
        let (sender, events) = channel();
        let mut inline = session
            .plot(
                &slot_request(line, PictureSlot(0), 0, 1, 10_000),
                sender.clone(),
            )
            .unwrap();
        (0..4).for_each(|_| {
            inline.step();
        });
        provisional_scene(&events);
        session
            .plot(&slot_request(line, PictureSlot(1), 0, 2, 2), sender)
            .unwrap();
        session.cancel_plot(line, PictureSlot(1));

        let state = inline.step();

        assert_eq!(state, JobState::Pending);
    }

    fn escape_time_session() -> (Session, LineId) {
        let mut session = session();
        let line = session
            .enter(r#"escape_time {"form": "quadratic_parameter"}"#)
            .unwrap();
        (session, line)
    }

    fn escape_time_request(line: LineId, divisions: u32) -> PlotRequest {
        PlotRequest {
            line,
            slot: PictureSlot(0),
            views: None,
            divisions: vec![vec![divisions, divisions]],
            parameters: None,
            iteration_limit: Some(IterationLimitRule::Fixed { iterations: 16 }),
        }
    }

    fn event_kinds(events: &Receiver<PictureEvent>) -> Vec<&'static str> {
        events
            .try_iter()
            .map(|event| match event {
                PictureEvent::Samples { .. } => "level",
                PictureEvent::Bounds { .. } => "finest",
                PictureEvent::Failed { .. } => "failed",
            })
            .collect()
    }

    #[test]
    fn escape_time_levels_arrive_as_samples_and_the_finest_as_bounds() {
        let (mut session, line) = escape_time_session();
        let (sender, events) = channel();
        let mut job = session
            .plot(&escape_time_request(line, 16), sender)
            .unwrap();

        run_to_end(&mut job);

        assert_eq!(
            event_kinds(&events),
            ["level", "level", "level", "level", "finest"]
        );
    }

    #[test]
    fn coarse_escape_time_level_is_provisional_and_without_bounds() {
        let (mut session, line) = escape_time_session();
        let (sender, events) = channel();
        let mut job = session
            .plot(&escape_time_request(line, 16), sender)
            .unwrap();

        run_to_end(&mut job);

        let layer = provisional_scene(&events).frames[0].layers[0].clone();
        assert_eq!(
            (layer.value_bounds.len(), layer.style.emphasis),
            (0, Emphasis::Provisional)
        );
    }

    #[test]
    fn finest_escape_time_level_has_the_divisions_of_the_request() {
        let (mut session, line) = escape_time_session();
        let (sender, events) = channel();
        let mut job = session
            .plot(&escape_time_request(line, 16), sender)
            .unwrap();

        run_to_end(&mut job);

        let scene = finished_scene(&events);
        let Primitive::ScalarGrid(grid) = &scene.frames[0].layers[0].primitive else {
            panic!("expected a scalar grid");
        };
        assert_eq!(grid.cells.counts, vec![16, 16]);
    }

    #[test]
    fn escape_time_line_takes_the_default_view_of_its_form() {
        let (mut session, line) = escape_time_session();

        let views = session.picture_defaults(line, None).unwrap().views;

        assert_eq!(
            views[0]
                .axes
                .iter()
                .map(|axis| axis.range.clone().unwrap())
                .collect::<Vec<_>>(),
            vec![
                Interval {
                    lower: fraction(-5, 2),
                    upper: Number::from(1_i64)
                },
                Interval {
                    lower: fraction(-5, 4),
                    upper: fraction(5, 4)
                }
            ]
        );
    }

    #[test]
    fn escape_time_line_offers_the_escape_time_legend() {
        let (mut session, line) = escape_time_session();

        let defaults = session.picture_defaults(line, None).unwrap();

        assert_eq!(defaults.legend, Some(ColourLegend::EscapeTime));
    }

    #[test]
    fn iteration_limit_on_a_line_that_is_not_an_escape_time_line_is_refused() {
        let (mut session, line) = square_session();
        let (sender, _events) = channel();
        let mut request = curve_request(line, 0, 1, 2);
        request.iteration_limit = Some(IterationLimitRule::Fixed { iterations: 16 });

        let refused = session.plot(&request, sender);

        assert_eq!(
            refused.err(),
            Some(PlotError::IterationLimitNotApplicable(line))
        );
    }

    #[test]
    fn parameter_value_on_an_escape_time_line_is_refused() {
        let (mut session, line) = escape_time_session();
        let (sender, _events) = channel();
        let mut request = escape_time_request(line, 8);
        request.parameters = Some(vec![PictureParameter {
            name: String::from("k"),
            value: Number::from(1_i64),
        }]);

        let refused = session.plot(&request, sender);

        assert_eq!(
            refused.err(),
            Some(PlotError::ParametersNotApplicable(line))
        );
    }

    #[test]
    fn stored_iteration_limit_is_used_when_the_request_names_none() {
        let (mut session, line) = escape_time_session();
        session.set_line_picture_for_tests_with_limit(
            line,
            Some(IterationLimitRule::Fixed { iterations: 7 }),
        );
        let (sender, events) = channel();
        let mut request = escape_time_request(line, 8);
        request.iteration_limit = None;
        request.views = Some(vec![view(vec![axis(-2, 1), axis(-1, 1)])]);
        let mut job = session.plot(&request, sender).unwrap();

        run_to_end(&mut job);

        let calc_viz::SamplingMethod::EscapeTime { limit_rule, .. } =
            finished_scene(&events).record.method
        else {
            panic!("expected an escape-time method");
        };
        assert_eq!(limit_rule, IterationLimitRule::Fixed { iterations: 7 });
    }

    #[test]
    fn escape_time_plot_prepares_no_level_on_the_session_thread() {
        let (mut session, line) = escape_time_session();
        let (sender, events) = channel();

        let _job = session
            .plot(&escape_time_request(line, 16), sender)
            .unwrap();

        assert!(events.try_recv().is_err());
    }

    fn screen_request(line: LineId, slot: PictureSlot, width: u32, height: u32) -> PlotRequest {
        PlotRequest {
            line,
            slot,
            views: None,
            divisions: vec![vec![width, height]],
            parameters: None,
            iteration_limit: None,
        }
    }

    const SESSION_THREAD_BOUND_MILLISECONDS: u64 = 50;

    fn within_the_bound(elapsed: std::time::Duration) -> bool {
        cfg!(debug_assertions)
            || elapsed <= std::time::Duration::from_millis(SESSION_THREAD_BOUND_MILLISECONDS)
    }

    #[test]
    fn command_sent_during_a_preparation_is_answered_within_the_bound() {
        let (mut session, line) = escape_time_session();
        let (sender, events) = channel();

        let started = std::time::Instant::now();
        let job = session
            .plot(&screen_request(line, PictureSlot(0), 1920, 1080), sender)
            .unwrap();
        let summary = session.line_summary(session.line(line).unwrap());
        let elapsed = started.elapsed();

        assert!(within_the_bound(elapsed), "{elapsed:?}");
        assert_eq!(
            (summary.label.as_str(), events.try_recv().is_err()),
            ("r1", true)
        );
        drop(job);
    }

    #[test]
    fn stream_of_plots_for_one_slot_leaves_only_the_newest_preparation() {
        let (mut session, line) = escape_time_session();
        let (sender, events) = channel();
        let mut jobs: Vec<PlotJob> = (0..5)
            .map(|_| {
                session
                    .plot(
                        &screen_request(line, PictureSlot(0), 1920, 1080),
                        sender.clone(),
                    )
                    .unwrap()
            })
            .collect();

        let states: Vec<JobState> = jobs.iter_mut().map(Job::step).collect();

        assert_eq!(
            (
                states[..4].to_vec(),
                states[4] == JobState::Pending,
                events.try_recv().is_err()
            ),
            (vec![JobState::Finished; 4], true, true)
        );
    }

    #[test]
    fn resize_that_starts_three_preparations_leaves_the_next_command_within_the_bound() {
        let (mut session, line) = escape_time_session();
        let (sender, _events) = channel();

        let started = std::time::Instant::now();
        let jobs: Vec<PlotJob> = (0..3)
            .map(|slot| {
                session
                    .plot(
                        &screen_request(line, PictureSlot(slot), 1920, 1080),
                        sender.clone(),
                    )
                    .unwrap()
            })
            .collect();
        let summary = session.line_summary(session.line(line).unwrap());
        let elapsed = started.elapsed();

        assert!(within_the_bound(elapsed), "{elapsed:?}");
        assert_eq!((jobs.len(), summary.label.as_str()), (3, "r1"));
    }

    #[test]
    fn preparation_of_a_larger_picture_costs_the_session_thread_no_more() {
        let (mut session, line) = escape_time_session();
        let (sender, _events) = channel();

        let started = std::time::Instant::now();
        let job = session
            .plot(&screen_request(line, PictureSlot(0), 7680, 4320), sender)
            .unwrap();
        let elapsed = started.elapsed();

        assert!(within_the_bound(elapsed), "{elapsed:?}");
        drop(job);
    }

    #[test]
    fn live_zoom_samples_equal_the_earlier_samples_bit_for_bit() {
        let mut session = session();
        let line = session.enter("sin(x) / (x + 1)").unwrap();
        let (sender, events) = channel();
        let mut wide = session
            .plot(&curve_request(line, 0, 4, 8), sender.clone())
            .unwrap();
        run_to_end(&mut wide);
        let precomputed = curve_bits(&finished_scene(&events));
        let mut zoomed = session.plot(&curve_request(line, 1, 2, 2), sender).unwrap();

        run_to_end(&mut zoomed);

        assert_eq!(
            curve_bits(&finished_scene(&events)),
            precomputed[2..5].to_vec()
        );
    }

    #[test]
    fn plot_of_a_named_line_expands_its_references_and_keeps_the_variable_free() {
        let mut session = session();
        session.enter("a = 3").unwrap();
        let line = session.enter("a * x").unwrap();
        let (sender, events) = channel();
        let mut job = session.plot(&curve_request(line, 0, 1, 1), sender).unwrap();

        run_to_end(&mut job);

        assert_eq!(
            curve_bits(&finished_scene(&events)),
            vec![0.0_f64.to_bits(), 3.0_f64.to_bits()]
        );
    }

    #[test]
    fn job_runs_a_large_view_in_several_steps() {
        let (mut session, line) = square_session();
        let (sender, _events) = channel();
        let mut job = session
            .plot(&curve_request(line, 0, 1, 10_000), sender)
            .unwrap();

        let steps = run_to_end(&mut job);

        assert_eq!(steps, 23);
    }

    #[test]
    fn newer_plot_cancels_the_older_job_before_it_starts() {
        let (mut session, line) = square_session();
        let (sender, events) = channel();
        let mut older = session
            .plot(&curve_request(line, 0, 1, 2), sender.clone())
            .unwrap();
        let mut newer = session.plot(&curve_request(line, 0, 2, 2), sender).unwrap();

        let older_state = older.step();
        run_to_end(&mut newer);

        assert_eq!(older_state, JobState::Finished);
        assert!(matches!(
            events.try_recv(),
            Ok(PictureEvent::Samples { generation, .. }) if generation == newer.generation()
        ));
        assert!(matches!(
            events.try_recv(),
            Ok(PictureEvent::Bounds { generation, .. }) if generation == newer.generation()
        ));
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn newer_plot_stops_a_running_job_between_steps() {
        let (mut session, line) = square_session();
        let (sender, events) = channel();
        let mut older = session
            .plot(&curve_request(line, 0, 1, 10_000), sender.clone())
            .unwrap();
        older.step();
        session.plot(&curve_request(line, 0, 2, 2), sender).unwrap();

        let state = older.step();

        assert_eq!(state, JobState::Finished);
        assert!(events.try_recv().is_err());
    }

    #[test]
    fn plot_of_another_line_does_not_cancel() {
        let mut session = session();
        let first = session.enter("x^2").unwrap();
        let second = session.enter("x^3").unwrap();
        let (sender, events) = channel();
        let mut job = session
            .plot(&curve_request(first, 0, 1, 2), sender.clone())
            .unwrap();
        session
            .plot(&curve_request(second, 0, 1, 2), sender)
            .unwrap();

        run_to_end(&mut job);

        assert!(
            matches!(events.try_recv(), Ok(PictureEvent::Samples { line, .. }) if line == first)
        );
    }

    #[test]
    fn sampling_failure_is_sent_as_a_failed_event() {
        let mut session = Session::new(fixed_clock(), Vec::new());
        let line = session.enter("x^2").unwrap();
        let (sender, events) = channel();
        let mut job = session.plot(&curve_request(line, 0, 1, 2), sender).unwrap();

        run_to_end(&mut job);

        assert!(matches!(
            events.try_recv(),
            Ok(PictureEvent::Failed {
                error: SampleError::Select(_),
                ..
            })
        ));
    }

    #[test]
    fn unknown_line_is_rejected() {
        let mut session = session();
        let line = LineId::from_number(9).unwrap();
        let (sender, _events) = channel();

        let result = session.plot(&curve_request(line, 0, 1, 2), sender);

        assert_eq!(result.err(), Some(PlotError::UnknownLine(line)));
    }

    fn sampled_bits(session: &mut Session, line: LineId) -> Vec<u64> {
        let (sender, events) = channel();
        let mut job = session.plot(&curve_request(line, 0, 1, 2), sender).unwrap();
        run_to_end(&mut job);
        curve_bits(&finished_scene(&events))
    }

    fn bits(values: &[f64]) -> Vec<u64> {
        values.iter().map(|value| value.to_bits()).collect()
    }

    #[test]
    fn function_naming_line_plots_its_body_over_its_parameter() {
        let mut session = session();
        let line = session.enter("f(x) = x^2").unwrap();

        let sampled = sampled_bits(&mut session, line);

        assert_eq!(sampled, bits(&[0.0, 0.25, 1.0]));
    }

    #[test]
    fn called_function_is_inlined_and_its_head_is_not_a_variable() {
        let mut session = session();
        session.enter("g(x) = x^2").unwrap();
        let line = session.enter("g(t) + 1").unwrap();

        let sampled = sampled_bits(&mut session, line);

        assert_eq!(sampled, bits(&[1.0, 1.25, 2.0]));
    }

    #[test]
    fn function_line_calling_another_function_line_plots() {
        let mut session = session();
        session.enter("g(x) = x^2").unwrap();
        let line = session.enter("h(s) = 2 * g(s)").unwrap();

        let sampled = sampled_bits(&mut session, line);

        assert_eq!(sampled, bits(&[0.0, 0.5, 2.0]));
    }

    #[test]
    fn call_inside_a_sum_over_its_index_is_inlined_and_reaches_lowering() {
        let mut session = session();
        session.enter("g(x) = x^2").unwrap();
        let line = session.enter("t + sum(g(k), k, 1, 3)").unwrap();
        let (sender, _events) = channel();

        let result = session.plot(&curve_request(line, 0, 1, 2), sender);

        assert!(matches!(
            result.err(),
            Some(PlotError::Sample(SampleError::Lower(_)))
        ));
    }

    #[test]
    fn call_of_a_function_no_line_defines_is_a_sampling_error() {
        let mut session = session();
        let line = session.enter("u(t) + 1").unwrap();
        let (sender, _events) = channel();

        let result = session.plot(&curve_request(line, 0, 1, 2), sender);

        assert!(matches!(result.err(), Some(PlotError::Sample(_))));
    }

    #[test]
    fn line_calling_a_function_keeps_it_as_a_dependency() {
        let mut session = session();
        let definition = session.enter("g(x) = x^2").unwrap();
        let line = session.enter("g(t) + 1").unwrap();

        let recomputed = session.edit(definition, "g(x) = x^3").unwrap();

        assert!(recomputed.contains(&line));
    }

    #[test]
    fn three_axis_variables_are_too_many() {
        let mut session = session();
        let line = session.enter("x * y * z").unwrap();
        let (sender, _events) = channel();

        let result = session.plot(&curve_request(line, 0, 1, 2), sender);

        assert_eq!(result.err(), Some(PlotError::TooManyAxisVariables(line)));
    }

    #[test]
    fn line_with_one_free_variable_is_plottable() {
        let (mut session, line) = square_session();

        assert!(session.is_plottable(line));
    }

    #[test]
    fn line_without_a_free_variable_is_not_plottable() {
        let mut session = session();
        let line = session.enter("2 + 3").unwrap();

        assert_eq!(
            session.picture_defaults(line, None).err(),
            Some(PlotError::NotPlottable(line))
        );
    }

    fn camera() -> PictureCamera {
        PictureCamera {
            azimuth_degrees: Number::from(30_i64),
            elevation_degrees: Number::from(20_i64),
            projection: CameraProjection::Orthographic,
        }
    }

    #[test]
    fn one_axis_variable_in_a_space_view_is_a_view_kind_mismatch() {
        let (mut session, line) = square_session();
        let (sender, _events) = channel();
        let mut request = curve_request(line, 0, 1, 2);
        if let Some(views) = request.views.as_mut() {
            views[0].camera = Some(camera());
        }

        assert_eq!(
            session.plot(&request, sender).err(),
            Some(PlotError::ViewKindMismatch(line))
        );
    }

    #[test]
    fn two_axis_variables_with_a_complex_value_are_not_plottable() {
        let mut session = session();
        let line = session.enter("x + i * y").unwrap();

        assert_eq!(
            session.picture_defaults(line, None).err(),
            Some(PlotError::ValueKindNotPlottable(line))
        );
    }

    #[test]
    fn one_axis_variable_with_a_complex_value_gets_a_complex_grid_from_minus_two_to_two() {
        let mut session = session();
        let line = session.enter("z^2 + i").unwrap();

        let defaults = session.picture_defaults(line, None).unwrap();

        assert_eq!(
            (defaults.views, defaults.legend),
            (
                vec![view(vec![axis(-2, 2), axis(-2, 2)])],
                Some(ColourLegend::DomainColouring)
            )
        );
    }

    #[test]
    fn complex_grid_axes_are_titled_by_real_and_imaginary_part() {
        let mut session = session();
        let line = session.enter("z * z + i").unwrap();

        assert_eq!(
            session.picture_defaults(line, None).unwrap().axis_titles,
            vec![vec![
                AxisTitle::RealPart("z".to_owned()),
                AxisTitle::ImaginaryPart("z".to_owned())
            ]]
        );
    }

    #[test]
    fn imaginary_axis_title_reads_as_the_imaginary_part() {
        assert_eq!(
            axis_title_text(&AxisTitle::ImaginaryPart("z".to_owned()), &Locale::source()),
            "imaginary part of z"
        );
    }

    #[test]
    fn requested_parameters_take_a_variable_off_the_axes_before_sampling() {
        let mut session = session();
        let line = session.enter("a * x + y").unwrap();
        let parameters = [PictureParameter {
            name: "a".to_owned(),
            value: Number::from(2_i64),
        }];

        let without = session.picture_defaults(line, None);
        let with = session.picture_defaults(line, Some(&parameters)).unwrap();

        assert_eq!(
            (without.err(), with.shape),
            (
                Some(PlotError::TooManyAxisVariables(line)),
                PictureShape::ScalarGrid
            )
        );
    }

    #[test]
    fn empty_requested_parameters_clear_the_stored_ones() {
        let mut session = session();
        let line = session.enter("a * x").unwrap();
        let views = vec![view(vec![axis(0, 1), axis(0, 1)])];
        session.set_line_picture_for_tests(
            line,
            &views,
            vec![PictureParameter {
                name: "a".to_owned(),
                value: Number::from(3_i64),
            }],
        );
        let (sender, events) = channel();
        let request = PlotRequest {
            line,
            slot: PictureSlot(0),
            iteration_limit: None,
            views: Some(views),
            divisions: vec![vec![2, 2]],
            parameters: Some(Vec::new()),
        };
        let mut job = session.plot(&request, sender).unwrap();

        run_to_end(&mut job);

        assert!(matches!(
            finished_scene(&events).frames[0].layers[0].primitive,
            Primitive::ScalarGrid(_)
        ));
    }

    #[test]
    fn two_axis_variables_in_a_space_view_give_a_surface() {
        let mut session = session();
        let line = session.enter("x + y").unwrap();
        let (sender, events) = channel();
        let request = PlotRequest {
            line,
            slot: PictureSlot(0),
            iteration_limit: None,
            views: Some(vec![ViewState {
                axes: vec![axis(0, 1), axis(0, 1), axis(-5, 5)],
                camera: Some(camera()),
            }]),
            divisions: vec![vec![1, 1, 10]],
            parameters: None,
        };
        let mut job = session.plot(&request, sender).unwrap();

        run_to_end(&mut job);

        let scene = finished_scene(&events);
        assert!(matches!(scene.views[0], calc_viz::View::View3(_)));
    }

    #[test]
    fn curve_view_needs_two_axes() {
        let (mut session, line) = square_session();
        let (sender, _events) = channel();
        let request = PlotRequest {
            line,
            slot: PictureSlot(0),
            iteration_limit: None,
            views: Some(vec![view(vec![axis(0, 1)])]),
            divisions: vec![vec![2]],
            parameters: None,
        };

        let result = session.plot(&request, sender);

        assert_eq!(
            result.err(),
            Some(PlotError::ViewAxisCount {
                view: 0,
                expected: 2,
                found: 1
            })
        );
    }

    #[test]
    fn two_views_for_a_one_view_scene_is_a_view_count_error() {
        let (mut session, line) = square_session();
        let (sender, _events) = channel();
        let one = view(vec![axis(0, 1), axis(0, 1)]);
        let request = PlotRequest {
            line,
            slot: PictureSlot(0),
            iteration_limit: None,
            views: Some(vec![one.clone(), one]),
            divisions: vec![vec![2, 2], vec![2, 2]],
            parameters: None,
        };

        assert_eq!(
            session.plot(&request, sender).err(),
            Some(PlotError::ViewCount {
                expected: 1,
                found: 2
            })
        );
    }

    #[test]
    fn empty_view_range_is_an_empty_interval_error() {
        let (mut session, line) = square_session();
        let (sender, _events) = channel();

        let result = session.plot(&curve_request(line, 1, 1, 2), sender);

        assert_eq!(
            result.err(),
            Some(PlotError::EmptyInterval { view: 0, axis: 0 })
        );
    }

    #[test]
    fn missing_divisions_are_an_error() {
        let (mut session, line) = square_session();
        let (sender, _events) = channel();
        let mut request = curve_request(line, 0, 1, 2);
        request.divisions = vec![vec![2]];

        assert_eq!(
            session.plot(&request, sender).err(),
            Some(PlotError::DivisionsMissing { view: 0, axis: 1 })
        );
    }

    #[test]
    fn plot_without_views_uses_the_default_view() {
        let (mut session, line) = square_session();
        let (sender, events) = channel();
        let request = PlotRequest {
            line,
            slot: PictureSlot(0),
            iteration_limit: None,
            views: None,
            divisions: vec![vec![4, 100]],
            parameters: None,
        };
        let mut job = session.plot(&request, sender).unwrap();

        run_to_end(&mut job);

        let scene = finished_scene(&events);
        let calc_viz::View::View2(plane) = &scene.views[0] else {
            panic!("expected a plane view");
        };
        assert_eq!(plane.x.range, range(-10, 10));
    }

    #[test]
    fn default_views_of_a_line_come_before_sampling() {
        let (mut session, line) = square_session();

        let views = session.picture_defaults(line, None).unwrap().views;

        assert_eq!(
            views,
            vec![view(vec![
                axis(-10, 10),
                AxisState {
                    range: None,
                    unit: AxisDisplayUnit::Coherent
                }
            ])]
        );
    }

    #[test]
    fn defaults_name_the_variable_and_value_axes() {
        let mut session = session();
        let line = session.enter("area = x^2").unwrap();

        assert_eq!(
            session.picture_defaults(line, None).unwrap().axis_titles,
            vec![vec![
                AxisTitle::Name("x".to_owned()),
                AxisTitle::Name("area".to_owned())
            ]]
        );
    }

    #[test]
    fn grid_defaults_have_a_sequential_legend() {
        let mut session = session();
        let line = session.enter("x * y").unwrap();

        assert_eq!(
            session.picture_defaults(line, None).unwrap().legend,
            Some(ColourLegend::Sequential)
        );
    }

    #[test]
    fn exact_number_reads_a_decimal_exactly() {
        assert_eq!(exact_number("-0.5"), Some(fraction(-1, 2)));
    }

    #[test]
    fn exact_number_refuses_a_name() {
        assert_eq!(exact_number("x"), None);
    }

    #[test]
    fn plot_json_names_path_size_views_and_precision() {
        let (mut session, line) = square_session();
        let (sender, events) = channel();
        let request = curve_request(line, 0, 1, 2);
        let mut job = session.plot(&request, sender).unwrap();
        run_to_end(&mut job);
        let scene = finished_scene(&events);

        let text = String::from_utf8(plot_json(
            "square.png",
            800,
            600,
            request.views.as_deref().unwrap_or_default(),
            &scene,
        ))
        .unwrap();

        assert!(
            text.starts_with(
                "{\n  \"path\": \"square.png\",\n  \"width\": 800,\n  \"height\": 600,\n"
            ) && text.contains("\"precision_limits\": [")
        );
    }

    #[test]
    fn two_free_variables_give_a_scalar_grid() {
        let mut session = session();
        let line = session.enter("x + y").unwrap();
        let (sender, events) = channel();
        let request = PlotRequest {
            line,
            slot: PictureSlot(0),
            iteration_limit: None,
            views: Some(vec![view(vec![axis(0, 1), axis(0, 1)])]),
            divisions: vec![vec![2, 2]],
            parameters: None,
        };
        let mut job = session.plot(&request, sender).unwrap();

        run_to_end(&mut job);

        let scene = finished_scene(&events);
        assert!(matches!(
            scene.frames[0].layers[0].primitive,
            Primitive::ScalarGrid(_)
        ));
    }

    #[test]
    fn parameter_value_is_substituted_before_sampling() {
        let mut session = session();
        let line = session.enter("a * x").unwrap();
        let (sender, events) = channel();
        let request = PlotRequest {
            line,
            slot: PictureSlot(0),
            iteration_limit: None,
            views: Some(vec![view(vec![axis(0, 1), axis(0, 1)])]),
            divisions: vec![vec![1, 100]],
            parameters: Some(vec![PictureParameter {
                name: "a".to_owned(),
                value: Number::from(3_i64),
            }]),
        };
        let mut job = session.plot(&request, sender).unwrap();

        run_to_end(&mut job);

        assert_eq!(curve_bits(&finished_scene(&events)), bits(&[0.0, 3.0]));
    }

    #[test]
    fn unknown_parameter_is_an_error() {
        let (mut session, line) = square_session();
        let (sender, _events) = channel();
        let mut request = curve_request(line, 0, 1, 2);
        request.parameters = Some(vec![PictureParameter {
            name: "b".to_owned(),
            value: Number::from(1_i64),
        }]);

        assert_eq!(
            session.plot(&request, sender).err(),
            Some(PlotError::UnknownParameter("b".to_owned()))
        );
    }

    #[test]
    fn display_unit_of_another_dimension_is_an_error() {
        let (mut session, line) = square_session();
        let (sender, _events) = channel();
        let mut request = curve_request(line, 0, 1, 2);
        if let Some(views) = request.views.as_mut() {
            views[0].axes[1].unit = AxisDisplayUnit::Unit("cm".to_owned());
        }

        assert_eq!(
            session.plot(&request, sender).err(),
            Some(PlotError::UnitOfOtherDimension { view: 0, axis: 1 })
        );
    }

    #[test]
    fn value_axis_in_centimetres_carries_its_symbol() {
        let mut session = session();
        let line = session.enter("x * 1 m").unwrap();
        let (sender, events) = channel();
        let mut request = curve_request(line, 0, 1, 2);
        if let Some(views) = request.views.as_mut() {
            views[0].axes[1].unit = AxisDisplayUnit::Unit("cm".to_owned());
        }
        let mut job = session.plot(&request, sender).unwrap();

        run_to_end(&mut job);

        let scene = finished_scene(&events);
        let calc_viz::View::View2(plane) = &scene.views[0] else {
            panic!("expected a plane view");
        };
        assert_eq!(plane.y.unit.symbol(), "cm");
    }

    #[test]
    fn stored_picture_view_is_used_without_views() {
        let (mut session, line) = square_session();
        let mut stored = session.picture_defaults(line, None).unwrap().views;
        stored[0].axes[0].range = Some(range(2, 3));
        stored[0].axes[1].range = Some(range(0, 9));
        session.set_line_picture_for_tests(line, &stored, Vec::new());
        let (sender, events) = channel();
        let request = PlotRequest {
            line,
            slot: PictureSlot(0),
            iteration_limit: None,
            views: None,
            divisions: vec![vec![1, 100]],
            parameters: None,
        };
        let mut job = session.plot(&request, sender).unwrap();

        run_to_end(&mut job);

        assert_eq!(curve_bits(&finished_scene(&events)), bits(&[4.0, 9.0]));
    }

    #[test]
    fn line_number_beyond_u32_is_rejected() {
        let mut pool = calc_expr::ExprPool::new();
        let symbol = pool
            .intern_symbol("x", calc_expr::SymbolKind::Variable)
            .unwrap();
        let expression = pool.number(Number::from(0_i64)).unwrap();
        let variables = [(symbol, String::from("x"))];
        let line = LineId::from_number(u64::from(u32::MAX) + 1).unwrap();
        let plan = PlotPlan {
            line,
            outline: outline_for(line, 1, false, false).unwrap(),
            expression,
            variables: &variables,
            input: "x",
            axes: Vec::new(),
            value_dimension: Dimension::DIMENSIONLESS,
            settings: Settings::default(),
        };

        assert_eq!(
            sample_request(&plan).err(),
            Some(PlotError::LineNumberTooLarge(line))
        );
    }

    #[test]
    fn gesture_range_rounds_bounds_to_the_largest_power_of_two_within_width_over_two_to_the_32() {
        let lower = Number::F64(0.1).to_exact().unwrap();
        let upper = Number::from(1_i64);

        let settled = settle_gesture_range(&lower, &upper).unwrap();

        let step = fraction(1, 1 << 33);
        let quotient = settled.lower.div_exact(&step).unwrap();
        assert!(matches!(quotient, Number::Integer(_)));
        assert_eq!(settled.upper, upper);
    }

    #[test]
    fn gesture_range_rounds_a_tie_to_an_even_multiple() {
        let step_half = fraction(3, 1 << 34);

        let settled = settle_gesture_range(&step_half, &Number::from(1_i64)).unwrap();

        assert_eq!(settled.lower, fraction(2, 1 << 33));
    }

    #[test]
    fn gesture_range_without_width_is_ignored() {
        let result = settle_gesture_range(&Number::from(1_i64), &Number::from(1_i64));

        assert_eq!(result, None);
    }

    #[test]
    fn pan_moves_both_bounds_by_the_offset() {
        let result = pan_range(&range(0, 1), 0.5);

        assert_eq!(
            result,
            Some(Interval {
                lower: fraction(1, 2),
                upper: fraction(3, 2)
            })
        );
    }

    #[test]
    fn zoom_by_two_about_the_focus_halves_the_width() {
        let result = zoom_range(&range(0, 4), 1.0, 2.0);

        assert_eq!(
            result,
            Some(Interval {
                lower: fraction(1, 2),
                upper: fraction(5, 2)
            })
        );
    }

    #[test]
    fn zoom_by_a_non_positive_factor_is_ignored() {
        let result = zoom_range(&range(0, 4), 1.0, 0.0);

        assert_eq!(result, None);
    }

    #[test]
    fn pan_by_a_non_finite_offset_is_ignored() {
        let result = pan_range(&range(0, 1), f64::NAN);

        assert_eq!(result, None);
    }

    #[test]
    fn line_referring_to_a_solve_line_fails_expansion() {
        let mut session = session();
        let request = crate::parse_request(
            br#"{"phase": "ways", "object": "circle", "wanted": "circumference"}"#,
        )
        .unwrap();
        let solve_line = session.enter_solve(&request).unwrap();
        let line = session
            .enter(&format!(
                "{} + x",
                crate::session_file::line_label(solve_line)
            ))
            .unwrap();
        let (sender, _events) = channel();

        let result = session.plot(&curve_request(line, 0, 1, 2), sender);

        assert!(matches!(result.err(), Some(PlotError::Expansion(_))));
    }

    #[test]
    fn latest_generation_names_the_newest_plot_of_a_line() {
        let (mut session, line) = square_session();
        let (sender, _events) = channel();
        session
            .plot(&curve_request(line, 0, 1, 2), sender.clone())
            .unwrap();
        let newer = session.plot(&curve_request(line, 0, 2, 2), sender).unwrap();

        let latest = session.latest_picture_generation(line, PictureSlot(0));

        assert_eq!(latest, Some(newer.generation()));
    }
}
