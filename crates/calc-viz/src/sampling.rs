use std::cmp::Ordering;
use std::collections::HashMap;

use calc_core::{
    ExactEvaluationError, QuantityError, ResultKind, SampleInput, SamplePosition, enclose_sample,
    evaluate_exact, to_coherent_units,
};
use calc_exec::{
    Backend, BackendKind, Batch, Domain, LowerError, LowerInput, LowerSpec, Plan, Preference,
    RunError, SelectError, Selection, ValueForm, escape_time_iteration_plan, lower, select,
};
use calc_expr::{ExprId, ExprPool, SymbolId, substitute_symbols};
use calc_numbers::{Integer, Number};

use crate::axis_unit::{coherent_positions_merge, coherent_to_shown, shown_to_coherent};
use crate::columns::ColumnWork;
use crate::default_view::{
    default_axis_interval, finite_values, reference_modulus, value_interval,
};
use crate::detached::{DetachedExpressions, NotDetachable, detach};
use crate::escape_time::{
    EscapeTimeForm, IterationLimitRule, classify, default_view, is_proven_inside, iterations_for,
    plan_form,
};
use crate::exact_order::compare_exact;
use crate::precision::{
    axis_step, colour_step, count_resolution, is_grid_exhausted, is_unresolved_against,
};
use crate::primitive::{
    Arrows, Band, CellGrid, Column, ComplexGrid, Points, Polyline, Primitive, ScalarGrid,
    TriangleMesh, Voxels,
};
use crate::record::{
    AxisResolution, CorpusReference, Interval, Resolution, ResultId, SamplingDiagnostics,
    SamplingMethod, SceneInput, SceneRecord,
};
use crate::scene::{
    ColumnEnclosures, Frame, InputIndex, Layer, PrecisionLimit, Reading, Scene, SceneError,
    ViewIndex,
};
use crate::style::{
    ColourMap, ColourMapping, Emphasis, KindColour, LinePattern, Marker, StyleRole,
};
use crate::view::{AxisUnit, Camera, Dimension, Projection, Scale, View, View2, View3, ViewAxis};

const LARGEST_MESH_VERTEX_COUNT: u64 = 1 << 32;
const LARGEST_ESCAPE_ITERATIONS: u32 = 65_536;
const OVERVIEW_AZIMUTH_DEGREES: i64 = 315;
const OVERVIEW_ELEVATION_DEGREES: i64 = 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AxisBounds {
    Expressions { lower: ExprId, upper: ExprId },
    Exact(Interval),
    Default,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleAxis {
    pub symbol: SymbolId,
    pub name: String,
    pub bounds: AxisBounds,
    pub divisions: u32,
    pub dimension: Dimension,
    pub unit: AxisUnit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampledShape {
    Curve(ExprId),
    Band {
        lower: ExprId,
        upper: ExprId,
    },
    Points(ExprId),
    ScalarGrid(ExprId),
    VectorField {
        x_component: ExprId,
        y_component: ExprId,
    },
    ComplexGrid(ExprId),
    Surface(ExprId),
    Occupancy(ExprId),
    EscapeTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EscapeTimeRequest {
    pub form: EscapeTimeForm,
    pub limit_rule: IterationLimitRule,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleRequest {
    pub result: ResultId,
    pub expression_text: String,
    pub shape: SampledShape,
    pub axes: Vec<SampleAxis>,
    pub value_range: Option<Interval>,
    pub value_dimension: Dimension,
    pub value_unit: AxisUnit,
    pub value_divisions: u32,
    pub domain: Domain,
    pub preference: Preference,
    pub sample_limit: u64,
    pub kind: KindColour,
    pub references: Vec<CorpusReference>,
    pub escape_time: Option<EscapeTimeRequest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SampleError {
    AxisCount {
        expected: usize,
        found: usize,
    },
    BoundEvaluation {
        axis: usize,
        error: ExactEvaluationError,
    },
    BoundNotRational {
        axis: usize,
        kind: ResultKind,
    },
    EmptyInterval {
        axis: usize,
    },
    NoDivisions {
        axis: usize,
    },
    BoundNotExact {
        axis: usize,
    },
    ComplexAxesDiffer,
    ExpressionNotDetachable(ExprId),
    AxisUnitNotSupported {
        axis: usize,
    },
    ConversionNotBuilt(ExprId),
    SymbolNotDetachable(SymbolId),
    MeshTooLarge {
        vertices: u64,
    },
    SampleLimitExceeded {
        requested: u64,
        limit: u64,
    },
    Lower(LowerError),
    ReducedExpression(ExprId),
    Select(SelectError),
    Run(RunError),
    Scene(SceneError),
    RunFinished,
    EscapeTimeSettingsMissing,
    IterationLimitTooLarge {
        iterations: u32,
    },
    StageOrder,
    SamplesPending,
    Quantity(QuantityError),
    ValueDimensionMismatch {
        requested: Dimension,
        found: Dimension,
    },
    RefinementNeedsEscapeTime,
}

struct Axis {
    interval: Interval,
    fineness: Number,
    divisions: u32,
    name: String,
    sample_count: u64,
    is_cell_centred: bool,
    dimension: Dimension,
    unit: AxisUnit,
}

struct SceneSettings {
    result: ResultId,
    expression_text: String,
    value_range: Option<Interval>,
    value_dimension: Dimension,
    value_unit: AxisUnit,
    value_divisions: u32,
    domain: Domain,
    kind: KindColour,
    references: Vec<CorpusReference>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShapeKind {
    Curve,
    Band,
    Points,
    ScalarGrid,
    VectorField,
    ComplexGrid,
    Surface,
    Occupancy,
    EscapeTime,
}

struct ShapeLayout {
    axes: usize,
    at_cell_centres: bool,
    form: ValueForm,
}

struct Geometry {
    view: View,
    primitive: Primitive,
    style: StyleRole,
    backend: BackendKind,
    diagnostics: SamplingDiagnostics,
}

struct PlanRun {
    selection: Selection,
    outputs: Vec<Column>,
    bounds: Vec<Vec<f64>>,
    next_element: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunStage {
    NotStarted,
    OneStep,
    TwoStage,
}

pub struct SamplingRun {
    settings: SceneSettings,
    shape: ShapeKind,
    axes: Vec<Axis>,
    coordinates: Vec<Column>,
    length: usize,
    plans: Vec<Plan>,
    finished_outputs: Vec<Vec<Column>>,
    finished_bounds: Vec<Vec<Vec<f64>>>,
    current: Option<PlanRun>,
    detached: DetachedExpressions,
    positions: Vec<Vec<Number>>,
    is_complex: bool,
    escape_time: Option<(EscapeTimeRequest, u32)>,
    backend: Option<BackendKind>,
    preference: Preference,
    is_finished: bool,
    stage: RunStage,
    samples_finished: bool,
    bound_plan: usize,
    bound_next_element: usize,
}

pub fn sample(
    pool: &mut ExprPool,
    backends: &[&dyn Backend],
    request: &SampleRequest,
) -> Result<Scene, SampleError> {
    let mut run = prepare_sampling(pool, request)?;
    loop {
        if let Some(scene) = run.step(backends, usize::MAX)? {
            return Ok(scene);
        }
    }
}

pub fn plan_sampling(
    pool: &mut ExprPool,
    request: &SampleRequest,
) -> Result<SamplePlan, SampleError> {
    let layout = shape_layout(request.shape);
    if request.axes.len() != layout.axes {
        return Err(SampleError::AxisCount {
            expected: layout.axes,
            found: request.axes.len(),
        });
    }
    let lower_inputs = lower_inputs(request, &layout)?;
    let axes = request
        .axes
        .iter()
        .enumerate()
        .map(|(index, axis)| {
            let default = default_bounds(request, layout.form, index, axis);
            evaluate_axis(pool, (index, &default), axis, layout.at_cell_centres)
        })
        .collect::<Result<Vec<Axis>, SampleError>>()?;
    if matches!(request.shape, SampledShape::Surface(_)) {
        check_mesh_size(&axes)?;
    }
    let length = checked_batch_length(&axes, request.sample_limit)?;
    let (_, written_expressions) = shape_expressions(request.shape);
    let value = coherent_value(pool, request, &written_expressions)?;
    let sampled_expressions = convert_units(pool, request, &value.expressions)?;
    let sampled_symbols: Vec<SymbolId> = lower_inputs.iter().map(|input| input.symbol).collect();
    let detached = detach(pool, &sampled_expressions, &sampled_symbols).map_err(detach_error)?;
    let is_complex = layout.form == ValueForm::Complex;
    let spec = LowerSpec {
        domain: request.domain,
        inputs: lower_inputs,
        output: layout.form,
    };
    let (shape, _) = shape_expressions(request.shape);
    let expressions = sampled_expressions.clone();
    let escape_time = match shape {
        ShapeKind::EscapeTime => {
            let settings = request
                .escape_time
                .clone()
                .ok_or(SampleError::EscapeTimeSettingsMissing)?;
            let real_axis = axes.first().ok_or(SampleError::AxisCount {
                expected: layout.axes,
                found: 0,
            })?;
            let iterations =
                iterations_for(settings.limit_rule, &settings.form, &real_axis.interval)
                    .unwrap_or(u32::MAX);
            if iterations > LARGEST_ESCAPE_ITERATIONS {
                return Err(SampleError::IterationLimitTooLarge { iterations });
            }
            Some((settings, iterations))
        }
        _ => None,
    };
    let escape_plans = escape_time
        .iter()
        .map(|(settings, iterations)| {
            escape_time_iteration_plan(
                request.domain,
                plan_form(&settings.form, request.domain),
                *iterations,
            )
            .map_err(|_| SampleError::IterationLimitTooLarge {
                iterations: *iterations,
            })
        })
        .collect::<Result<Vec<Plan>, SampleError>>()?;
    let plans = expressions
        .iter()
        .map(|expression| {
            let plan = lower(pool, *expression, &spec).map_err(SampleError::Lower)?;
            if plan.reduce().is_some() {
                return Err(SampleError::ReducedExpression(*expression));
            }
            Ok(plan)
        })
        .collect::<Result<Vec<Plan>, SampleError>>()?
        .into_iter()
        .chain(escape_plans)
        .collect();
    Ok(SamplePlan {
        settings: SceneSettings {
            result: request.result,
            expression_text: request.expression_text.clone(),
            value_range: request.value_range.clone(),
            value_dimension: value.dimension,
            value_unit: value.unit,
            value_divisions: request.value_divisions,
            domain: request.domain,
            kind: request.kind,
            references: request.references.clone(),
        },
        shape,
        axes,
        length,
        plans,
        detached,
        is_complex,
        escape_time,
        preference: request.preference,
    })
}

fn detach_error(error: NotDetachable) -> SampleError {
    match error {
        NotDetachable::Node(expression) => SampleError::ExpressionNotDetachable(expression),
        NotDetachable::Symbol(symbol) => SampleError::SymbolNotDetachable(symbol),
    }
}

struct CoherentValue {
    expressions: Vec<ExprId>,
    dimension: Dimension,
    unit: AxisUnit,
}

fn coherent_symbol(pool: &ExprPool, coherent: &calc_core::CoherentExpression) -> String {
    let Some(unit) = coherent.unit else {
        return String::new();
    };
    let table = pool.units();
    let Ok(factors) = table.factors(unit) else {
        return String::new();
    };
    factors
        .iter()
        .filter_map(|factor| {
            let symbol = table.display_symbol(factor.named_unit()).ok()?;
            Some(match factor.exponent() {
                1 => symbol.to_string(),
                exponent => format!("{symbol}^{exponent}"),
            })
        })
        .collect::<Vec<String>>()
        .join("*")
}

fn coherent_value(
    pool: &mut ExprPool,
    request: &SampleRequest,
    expressions: &[ExprId],
) -> Result<CoherentValue, SampleError> {
    let mut converted = Vec::with_capacity(expressions.len());
    let mut found: Option<(Dimension, String)> = None;
    for expression in expressions {
        let coherent = to_coherent_units(pool, *expression).map_err(|error| match error {
            QuantityError::Access(_) | QuantityError::Build(_) => {
                SampleError::ConversionNotBuilt(*expression)
            }
            other => SampleError::Quantity(other),
        })?;
        let dimension = Dimension {
            exponents: coherent.dimension.exponents(),
        };
        match &found {
            Some((first, _)) if *first != dimension => {
                let first = *first;
                return Err(SampleError::ValueDimensionMismatch {
                    requested: first,
                    found: dimension,
                });
            }
            Some(_) => {}
            None => found = Some((dimension, coherent_symbol(pool, &coherent))),
        }
        converted.push(coherent.expression);
    }
    let (dimension, symbol) = found.unwrap_or((Dimension::DIMENSIONLESS, String::new()));
    if dimension == Dimension::DIMENSIONLESS && request.value_dimension == Dimension::DIMENSIONLESS
    {
        return Ok(CoherentValue {
            expressions: converted,
            dimension: request.value_dimension,
            unit: request.value_unit.clone(),
        });
    }
    let is_unspecified = request.value_dimension == Dimension::DIMENSIONLESS
        && request.value_unit == AxisUnit::dimensionless();
    if is_unspecified {
        return Ok(CoherentValue {
            expressions: converted,
            dimension,
            unit: AxisUnit::Coherent { symbol },
        });
    }
    let inputs_carry_a_dimension = request
        .axes
        .iter()
        .any(|axis| axis.dimension != Dimension::DIMENSIONLESS);
    if request.value_dimension != dimension && !inputs_carry_a_dimension {
        return Err(SampleError::ValueDimensionMismatch {
            requested: request.value_dimension,
            found: dimension,
        });
    }
    Ok(CoherentValue {
        expressions: converted,
        dimension: request.value_dimension,
        unit: request.value_unit.clone(),
    })
}

pub fn prepare_sampling(
    pool: &mut ExprPool,
    request: &SampleRequest,
) -> Result<SamplingRun, SampleError> {
    plan_sampling(pool, request).map(SamplePlan::build)
}

pub struct SamplePlan {
    settings: SceneSettings,
    shape: ShapeKind,
    axes: Vec<Axis>,
    length: usize,
    plans: Vec<Plan>,
    detached: DetachedExpressions,
    is_complex: bool,
    escape_time: Option<(EscapeTimeRequest, u32)>,
    preference: Preference,
}

impl SamplePlan {
    pub fn sample_count(&self) -> usize {
        self.length
    }

    pub fn build(self) -> SamplingRun {
        let positions = self.axes.iter().map(axis_positions).collect();
        let coordinates = sampling_coordinates(&self.axes, self.settings.domain, self.length);
        SamplingRun {
            settings: self.settings,
            shape: self.shape,
            axes: self.axes,
            coordinates,
            length: self.length,
            plans: self.plans,
            finished_outputs: Vec::new(),
            finished_bounds: Vec::new(),
            current: None,
            detached: self.detached,
            positions,
            is_complex: self.is_complex,
            escape_time: self.escape_time,
            backend: None,
            preference: self.preference,
            is_finished: false,
            stage: RunStage::NotStarted,
            samples_finished: false,
            bound_plan: 0,
            bound_next_element: 0,
        }
    }
}

fn convert_units(
    pool: &mut ExprPool,
    request: &SampleRequest,
    expressions: &[ExprId],
) -> Result<Vec<ExprId>, SampleError> {
    let converts_inputs = matches!(
        request.shape,
        SampledShape::Curve(_)
            | SampledShape::Band { .. }
            | SampledShape::Points(_)
            | SampledShape::Surface(_)
            | SampledShape::ScalarGrid(_)
            | SampledShape::Occupancy(_)
    );
    let converts_value = matches!(
        request.shape,
        SampledShape::Curve(_)
            | SampledShape::Band { .. }
            | SampledShape::Points(_)
            | SampledShape::Surface(_)
    );
    if !converts_inputs
        && let Some(axis) = request
            .axes
            .iter()
            .position(|axis| !axis.unit.is_coherent())
    {
        return Err(SampleError::AxisUnitNotSupported { axis });
    }
    let mut replacements = HashMap::new();
    let first_expression = expressions.first().copied();
    for axis in &request.axes {
        if axis.unit.is_coherent() {
            continue;
        }
        let converted = pool
            .symbol(axis.symbol)
            .ok()
            .and_then(|shown| shown_to_coherent(pool, shown, &axis.unit).ok());
        match (converted, first_expression) {
            (Some(coherent), _) => {
                replacements.insert(axis.symbol, coherent);
            }
            (None, Some(expression)) => return Err(SampleError::ConversionNotBuilt(expression)),
            (None, None) => return Ok(Vec::new()),
        }
    }
    let value_is_coherent = !converts_value || request.value_unit.is_coherent();
    if replacements.is_empty() && value_is_coherent {
        return Ok(expressions.to_vec());
    }
    expressions
        .iter()
        .map(|expression| {
            let failed = |_| SampleError::ConversionNotBuilt(*expression);
            let substituted = if replacements.is_empty() {
                *expression
            } else {
                substitute_symbols(pool, *expression, &replacements).map_err(failed)?
            };
            if value_is_coherent {
                Ok(substituted)
            } else {
                coherent_to_shown(pool, substituted, &request.value_unit)
                    .map_err(|_| SampleError::ConversionNotBuilt(*expression))
            }
        })
        .collect()
}

fn shape_expressions(shape: SampledShape) -> (ShapeKind, Vec<ExprId>) {
    match shape {
        SampledShape::Curve(expression) => (ShapeKind::Curve, vec![expression]),
        SampledShape::Band { lower, upper } => (ShapeKind::Band, vec![lower, upper]),
        SampledShape::Points(expression) => (ShapeKind::Points, vec![expression]),
        SampledShape::ScalarGrid(expression) => (ShapeKind::ScalarGrid, vec![expression]),
        SampledShape::VectorField {
            x_component,
            y_component,
        } => (ShapeKind::VectorField, vec![x_component, y_component]),
        SampledShape::ComplexGrid(expression) => (ShapeKind::ComplexGrid, vec![expression]),
        SampledShape::Surface(expression) => (ShapeKind::Surface, vec![expression]),
        SampledShape::Occupancy(expression) => (ShapeKind::Occupancy, vec![expression]),
        SampledShape::EscapeTime => (ShapeKind::EscapeTime, Vec::new()),
    }
}

fn column_slice(column: &Column, start: usize, end: usize) -> Column {
    match column {
        Column::F32(values) => Column::F32(values.get(start..end).unwrap_or_default().to_vec()),
        Column::F64(values) => Column::F64(values.get(start..end).unwrap_or_default().to_vec()),
    }
}

fn append_column(target: &mut Column, source: Column) {
    match (target, source) {
        (Column::F32(target), Column::F32(source)) => target.extend(source),
        (Column::F64(target), Column::F64(source)) => target.extend(source),
        (Column::F32(_), Column::F64(_)) | (Column::F64(_), Column::F32(_)) => {}
    }
}

fn empty_column(domain: Domain) -> Column {
    match domain {
        Domain::F32 => Column::F32(Vec::new()),
        Domain::F64 => Column::F64(Vec::new()),
    }
}

fn chunk_batch(coordinates: &[Column], domain: Domain, start: usize, end: usize) -> Batch {
    let length = end - start;
    let batch = match domain {
        Domain::F32 => Batch::from_f32_columns(
            length,
            coordinates
                .iter()
                .map(|column| match column_slice(column, start, end) {
                    Column::F32(values) => values,
                    Column::F64(_) => Vec::new(),
                })
                .collect(),
        ),
        Domain::F64 => Batch::from_f64_columns(
            length,
            coordinates
                .iter()
                .map(|column| match column_slice(column, start, end) {
                    Column::F64(values) => values,
                    Column::F32(_) => Vec::new(),
                })
                .collect(),
        ),
    };
    batch.unwrap_or_else(|_| Batch::zeroed(domain, coordinates.len(), 0))
}

impl SamplingRun {
    pub fn sample_count(&self) -> usize {
        self.length
    }

    pub fn step(
        &mut self,
        backends: &[&dyn Backend],
        chunk_length: usize,
    ) -> Result<Option<Scene>, SampleError> {
        if self.stage == RunStage::TwoStage {
            return Err(SampleError::StageOrder);
        }
        self.stage = RunStage::OneStep;
        self.run_chunk(backends, chunk_length, true)
    }

    pub fn step_samples(
        &mut self,
        backends: &[&dyn Backend],
        chunk_length: usize,
    ) -> Result<Option<Scene>, SampleError> {
        if self.stage == RunStage::OneStep {
            return Err(SampleError::StageOrder);
        }
        if self.samples_finished {
            return Err(SampleError::RunFinished);
        }
        self.stage = RunStage::TwoStage;
        self.run_chunk(backends, chunk_length, false)
    }

    pub fn step_bounds(&mut self, chunk_length: usize) -> Result<Option<Scene>, SampleError> {
        if self.is_finished {
            return Err(SampleError::RunFinished);
        }
        if !self.samples_finished {
            return Err(SampleError::SamplesPending);
        }
        let plan_index = self.bound_plan;
        let Some(columns) = self.finished_outputs.get(plan_index) else {
            self.is_finished = true;
            return self.build_scene(false).map(Some);
        };
        let start = self.bound_next_element;
        let end = start.saturating_add(chunk_length.max(1)).min(self.length);
        let channels: Vec<Vec<f64>> = columns
            .iter()
            .map(|column| chunk_values(column, start, end))
            .collect();
        let mut bounds = self
            .finished_bounds
            .get_mut(plan_index)
            .map(std::mem::take)
            .ok_or(SampleError::SamplesPending)?;
        self.enclose_chunk(plan_index, start, &channels, &mut bounds);
        if let Some(slot) = self.finished_bounds.get_mut(plan_index) {
            *slot = bounds;
        }
        if end >= self.length {
            self.bound_plan += 1;
            self.bound_next_element = 0;
        } else {
            self.bound_next_element = end;
        }
        if self.bound_plan == self.finished_outputs.len() {
            self.is_finished = true;
            return self.build_scene(false).map(Some);
        }
        Ok(None)
    }

    fn run_chunk(
        &mut self,
        backends: &[&dyn Backend],
        chunk_length: usize,
        encloses: bool,
    ) -> Result<Option<Scene>, SampleError> {
        if self.is_finished {
            return Err(SampleError::RunFinished);
        }
        let plan_index = self.finished_outputs.len();
        let Some(plan) = self.plans.get(plan_index) else {
            return self.finish_samples(encloses).map(Some);
        };
        let mut current = match self.current.take() {
            Some(current) => current,
            None => {
                let selection = select(backends, plan, self.length, self.preference)
                    .map_err(SampleError::Select)?;
                self.preference = Preference::Only(selection.kind);
                if self.backend.is_none() {
                    self.backend = Some(selection.kind);
                }
                PlanRun {
                    outputs: (0..plan.output_channel_count())
                        .map(|_| empty_column(self.settings.domain))
                        .collect(),
                    bounds: (0..plan.output_channel_count())
                        .map(|_| Vec::new())
                        .collect(),
                    selection,
                    next_element: 0,
                }
            }
        };
        let start = current.next_element;
        let end = start.saturating_add(chunk_length.max(1)).min(self.length);
        let inputs = chunk_batch(&self.coordinates, self.settings.domain, start, end);
        let mut outputs = Batch::zeroed(
            self.settings.domain,
            plan.output_channel_count(),
            plan.output_length(end - start),
        );
        current
            .selection
            .prepared
            .run(&inputs, &mut outputs)
            .map_err(SampleError::Run)?;
        let chunk_columns: Vec<Column> = (0..current.outputs.len())
            .map(|channel| output_column(&outputs, channel))
            .collect();
        if encloses {
            let channels: Vec<Vec<f64>> = chunk_columns
                .iter()
                .map(|column| chunk_values(column, 0, end - start))
                .collect();
            self.enclose_chunk(plan_index, start, &channels, &mut current.bounds);
        }
        for (target, column) in current.outputs.iter_mut().zip(chunk_columns) {
            append_column(target, column);
        }
        current.next_element = end;
        if end >= self.length {
            self.finished_outputs.push(current.outputs);
            self.finished_bounds.push(current.bounds);
        } else {
            self.current = Some(current);
        }
        if self.finished_outputs.len() == self.plans.len() {
            return self.finish_samples(encloses).map(Some);
        }
        Ok(None)
    }

    fn finish_samples(&mut self, encloses: bool) -> Result<Scene, SampleError> {
        self.samples_finished = true;
        if encloses {
            self.is_finished = true;
        }
        self.build_scene(!encloses)
    }

    fn value_bounds(plan_bounds: Vec<Vec<Vec<f64>>>, occupied: Option<Vec<usize>>) -> Vec<Column> {
        let mut channels: Vec<Vec<f64>> = plan_bounds.into_iter().flatten().collect();
        if let Some(all_values) = occupied {
            channels = channels
                .into_iter()
                .map(|bounds| {
                    all_values
                        .iter()
                        .filter_map(|index| bounds.get(*index).copied())
                        .collect()
                })
                .collect();
        }
        channels.into_iter().map(Column::F64).collect()
    }

    fn proven_inside_cells(&self) -> Vec<bool> {
        let is_parameter = matches!(
            &self.escape_time,
            Some((request, _)) if request.form == EscapeTimeForm::QuadraticParameter
        );
        let (Some(real), Some(imaginary)) = (self.positions.first(), self.positions.get(1)) else {
            return Vec::new();
        };
        if !is_parameter {
            return vec![false; self.length];
        }
        imaginary
            .iter()
            .flat_map(|imaginary| {
                real.iter()
                    .map(move |real| is_proven_inside(real, imaginary))
            })
            .collect()
    }

    fn sample_inputs(&self, element: usize) -> Vec<SampleInput> {
        let mut stride = 1_usize;
        let mut coordinates = Vec::with_capacity(self.positions.len());
        for axis_positions in &self.positions {
            let period = axis_positions.len().max(1);
            if let Some(position) = axis_positions.get((element / stride) % period) {
                coordinates.push(position.clone());
            }
            stride = stride.saturating_mul(period);
        }
        match (
            self.is_complex,
            self.detached.symbols.as_slice(),
            coordinates.as_slice(),
        ) {
            (true, [symbol], [real, imaginary]) => vec![SampleInput {
                symbol: *symbol,
                position: SamplePosition::Complex {
                    real: real.clone(),
                    imaginary: imaginary.clone(),
                },
            }],
            _ => self
                .detached
                .symbols
                .iter()
                .zip(coordinates)
                .map(|(symbol, position)| SampleInput {
                    symbol: *symbol,
                    position: SamplePosition::Real(position),
                })
                .collect(),
        }
    }

    fn enclose_chunk(
        &self,
        plan_index: usize,
        start: usize,
        channels: &[Vec<f64>],
        bounds: &mut [Vec<f64>],
    ) {
        let Some(root) = self.detached.roots.get(plan_index) else {
            return;
        };
        let chunk_length = channels.first().map_or(0, Vec::len);
        for offset in 0..chunk_length {
            let inputs = self.sample_inputs(start + offset);
            let enclosure = enclose_sample(&self.detached.pool, *root, &inputs);
            let value = |channel: usize| {
                channels
                    .get(channel)
                    .and_then(|values| values.get(offset))
                    .copied()
                    .unwrap_or(f64::NAN)
            };
            match bounds {
                [real, imaginary] => {
                    let (real_bound, imaginary_bound) =
                        enclosure.complex_rounding_bounds(value(0), value(1));
                    real.push(real_bound);
                    imaginary.push(imaginary_bound);
                }
                [single] => single.push(enclosure.rounding_bound(value(0))),
                _ => {}
            }
        }
    }

    fn build_scene(&mut self, is_provisional: bool) -> Result<Scene, SampleError> {
        let (outputs, plan_bounds) = if is_provisional {
            (self.finished_outputs.clone(), Vec::new())
        } else {
            (
                std::mem::take(&mut self.finished_outputs),
                std::mem::take(&mut self.finished_bounds),
            )
        };
        let settings = &self.settings;
        let backend = self.backend.unwrap_or(BackendKind::Cpu);
        let axes = self.axes.as_slice();
        let coordinates = self.coordinates.as_slice();
        let wrong_shape = || SampleError::AxisCount {
            expected: shape_layout_of(self.shape).axes,
            found: axes.len(),
        };
        let mut results = outputs.into_iter();
        let mut next_plan = || results.next().unwrap_or_default().into_iter();
        let mut occupied = None;
        let geometry = match (self.shape, axes, coordinates) {
            (ShapeKind::ScalarGrid, [x_axis, y_axis], [_, _]) => {
                let scalar = next_plan().next().ok_or_else(wrong_shape)?;
                grid_geometry(settings, x_axis, y_axis, scalar, backend, None)
            }
            (ShapeKind::EscapeTime, [real_axis, imaginary_axis], [_, _]) => {
                let mut outputs = next_plan();
                let counts = outputs.next().ok_or_else(wrong_shape)?;
                let flags = outputs.next().ok_or_else(wrong_shape)?;
                let inside = self.proven_inside_cells();
                let classified = classify(&counts, &flags, &inside);
                let mut geometry = grid_geometry(
                    settings,
                    real_axis,
                    imaginary_axis,
                    classified.scalar,
                    backend,
                    Some(classified.classes),
                );
                geometry.diagnostics = SamplingDiagnostics {
                    escaped_cells: classified.escaped,
                    inside_cells: classified.inside,
                    undecided_cells: classified.undecided,
                    ..SamplingDiagnostics::default()
                };
                geometry
            }
            (ShapeKind::Curve, [axis], [abscissa]) => {
                let values = next_plan().next().ok_or_else(wrong_shape)?;
                let diagnostics = count_special_values(&[&values]);
                let value_axis = value_view_axis(settings, &[&values]);
                Geometry {
                    view: plane_view(axis, value_axis),
                    primitive: Primitive::Polyline(Polyline {
                        coordinates: vec![abscissa.clone(), values],
                    }),
                    style: line_style(settings.kind, LinePattern::Solid, Marker::None),
                    backend,
                    diagnostics,
                }
            }
            (ShapeKind::Points, [axis], [abscissa]) => {
                let values = next_plan().next().ok_or_else(wrong_shape)?;
                let diagnostics = count_special_values(&[&values]);
                let value_axis = value_view_axis(settings, &[&values]);
                Geometry {
                    view: plane_view(axis, value_axis),
                    primitive: Primitive::Points(Points {
                        coordinates: vec![abscissa.clone(), values],
                        scalar: None,
                    }),
                    style: line_style(settings.kind, LinePattern::None, Marker::Dot),
                    backend,
                    diagnostics,
                }
            }
            (ShapeKind::Band, [axis], [abscissa]) => {
                let lower = next_plan().next().ok_or_else(wrong_shape)?;
                let upper = next_plan().next().ok_or_else(wrong_shape)?;
                let diagnostics = count_special_values(&[&lower, &upper]);
                let value_axis = value_view_axis(settings, &[&lower, &upper]);
                Geometry {
                    view: plane_view(axis, value_axis),
                    primitive: Primitive::Band(Band {
                        abscissa: abscissa.clone(),
                        lower,
                        upper,
                    }),
                    style: line_style(settings.kind, LinePattern::None, Marker::None),
                    backend,
                    diagnostics,
                }
            }
            (ShapeKind::VectorField, [x_axis, y_axis], [x_bases, y_bases]) => {
                let x_values = next_plan().next().ok_or_else(wrong_shape)?;
                let y_values = next_plan().next().ok_or_else(wrong_shape)?;
                Geometry {
                    view: box_view(x_axis, y_axis),
                    diagnostics: count_special_values(&[&x_values, &y_values]),
                    primitive: Primitive::Arrows(Arrows {
                        bases: vec![x_bases.clone(), y_bases.clone()],
                        components: vec![x_values, y_values],
                    }),
                    style: line_style(settings.kind, LinePattern::Solid, Marker::None),
                    backend,
                }
            }
            (ShapeKind::ComplexGrid, [real_axis, imaginary_axis], [_, _]) => {
                let mut parts = next_plan();
                let real = parts.next().ok_or_else(wrong_shape)?;
                let imaginary = parts.next().ok_or_else(wrong_shape)?;
                complex_geometry(
                    settings,
                    real_axis,
                    imaginary_axis,
                    real,
                    imaginary,
                    backend,
                )
            }
            (ShapeKind::Surface, [x_axis, y_axis], [x_positions, y_positions]) => {
                let heights = next_plan().next().ok_or_else(wrong_shape)?;
                let positions = [x_positions.clone(), y_positions.clone()];
                surface_geometry(settings, x_axis, y_axis, positions, heights, backend)
            }
            (ShapeKind::Occupancy, [x_axis, y_axis, z_axis], [_, _, _]) => {
                let values = next_plan().next().ok_or_else(wrong_shape)?;
                occupied = Some(occupied_positions(&values));
                occupancy_geometry(settings, [x_axis, y_axis, z_axis], &values, backend)
            }
            _ => return Err(wrong_shape()),
        };
        let mut geometry = geometry;
        let value_bounds = if is_provisional {
            geometry.style.emphasis = Emphasis::Provisional;
            Vec::new()
        } else if self.shape == ShapeKind::EscapeTime {
            vec![Column::F64(vec![f64::NAN; self.length])]
        } else {
            Self::value_bounds(plan_bounds, occupied)
        };
        let mut value_bounds = value_bounds;
        let mut columns = None;
        if !is_provisional && self.shape == ShapeKind::Curve {
            let root = self.detached.roots.first().copied();
            if let (Some(root), [symbol], Some(positions)) = (
                root,
                self.detached.symbols.as_slice(),
                self.positions.first(),
            ) {
                columns = curve_columns(
                    &mut self.detached.pool,
                    root,
                    *symbol,
                    positions,
                    &mut geometry,
                    &mut value_bounds,
                    settings.value_range.is_none(),
                );
            }
        }
        if !is_provisional && settings.value_range.is_none() && columns.is_none() {
            refit_default_ranges(&mut geometry, &value_bounds);
        }
        let method = match &self.escape_time {
            Some((request, iterations)) => SamplingMethod::EscapeTime {
                form: request.form.clone(),
                limit_rule: request.limit_rule,
                iterations_used: *iterations,
            },
            None => SamplingMethod::UniformGrid,
        };
        let record = SceneRecord {
            result: settings.result,
            inputs: vec![SceneInput {
                expression: settings.expression_text.clone(),
                sampling_box: axes.iter().map(|axis| axis.interval.clone()).collect(),
                variables: axes.iter().map(|axis| axis.name.clone()).collect(),
            }],
            method,
            resolution: Resolution {
                domain: settings.domain,
                axes: axes
                    .iter()
                    .map(|axis| AxisResolution {
                        sample_count: axis.sample_count,
                        fineness: axis.fineness.clone(),
                    })
                    .collect(),
                adaptive: None,
            },
            seed: None,
            backend: geometry.backend,
            references: settings.references.clone(),
            diagnostics: geometry.diagnostics,
        };
        let frame = Frame {
            parameter_values: Vec::new(),
            layers: vec![Layer {
                view: ViewIndex(0),
                input: InputIndex(0),
                precision: if is_provisional {
                    None
                } else {
                    with_marked_columns(
                        layer_precision(&geometry, &value_bounds, axes, settings.domain),
                        columns.as_ref(),
                    )
                },
                columns,
                value_bounds,
                readings: vec![Reading::ValueAt {
                    variables: axes.iter().map(|axis| axis.name.clone()).collect(),
                }],
                primitive: geometry.primitive,
                style: geometry.style,
            }],
        };
        Scene::new(record, Vec::new(), vec![geometry.view], vec![frame]).map_err(SampleError::Scene)
    }
}

fn column_f64(column: Option<&Column>) -> Vec<f64> {
    match column {
        Some(Column::F32(values)) => values.iter().map(|value| f64::from(*value)).collect(),
        Some(Column::F64(values)) => values.clone(),
        None => Vec::new(),
    }
}

fn value_step(geometry: &Geometry) -> Option<Number> {
    match &geometry.view {
        View::View2(view) => axis_step(&view.y),
        View::View3(_) => None,
    }
}

fn curve_columns(
    pool: &mut ExprPool,
    root: ExprId,
    symbol: SymbolId,
    positions: &[Number],
    geometry: &mut Geometry,
    value_bounds: &mut [Column],
    fits_range: bool,
) -> Option<ColumnEnclosures> {
    let Primitive::Polyline(polyline) = &mut geometry.primitive else {
        return None;
    };
    let values = column_f64(polyline.coordinates.last());
    let bounds = column_f64(value_bounds.first());
    let mut work = ColumnWork::new(pool, root, symbol, positions, &values, &bounds);
    let bounds = match (work.exact_values(), polyline.coordinates.last_mut()) {
        (Some((shown, exact_bounds)), Some(last @ Column::F64(_))) => {
            *last = Column::F64(shown.to_vec());
            if let Some(first) = value_bounds.first_mut() {
                *first = Column::F64(exact_bounds.to_vec());
            }
            exact_bounds.to_vec()
        }
        _ => bounds,
    };
    if fits_range {
        let shown = [value_bounds
            .first()
            .cloned()
            .unwrap_or(Column::F64(Vec::new()))];
        refit_default_ranges(geometry, &shown);
    }
    let step = value_step(geometry)?;
    work.mark(&step);
    if fits_range {
        work.note_spread(&values, &bounds);
    }
    work.refine(pool, root, symbol, step.round_to_f64_ties_even());
    Some(work.into_enclosures())
}

fn with_marked_columns(
    precision: Option<PrecisionLimit>,
    columns: Option<&ColumnEnclosures>,
) -> Option<PrecisionLimit> {
    let marked = columns.map_or(0, |columns| {
        u64::try_from(columns.marked.iter().filter(|marked| **marked).count()).unwrap_or(u64::MAX)
    });
    let below = columns.is_some_and(|columns| columns.varies_below_bounds);
    if marked == 0 && !below {
        return precision;
    }
    let mut precision = precision.unwrap_or_default();
    precision.marked_columns = marked;
    precision.varies_below_bounds = below;
    Some(precision)
}

fn chunk_values(column: &Column, start: usize, end: usize) -> Vec<f64> {
    match column {
        Column::F32(values) => values
            .get(start..end)
            .unwrap_or_default()
            .iter()
            .map(|value| f64::from(*value))
            .collect(),
        Column::F64(values) => values.get(start..end).unwrap_or_default().to_vec(),
    }
}

fn output_column(outputs: &Batch, channel: usize) -> Column {
    match outputs.domain() {
        Domain::F32 => Column::F32(outputs.f32_channel(channel).unwrap_or_default().to_vec()),
        Domain::F64 => Column::F64(outputs.f64_channel(channel).unwrap_or_default().to_vec()),
    }
}

fn shape_layout(shape: SampledShape) -> ShapeLayout {
    shape_layout_of(shape_expressions(shape).0)
}

fn shape_layout_of(shape: ShapeKind) -> ShapeLayout {
    let (axes, at_cell_centres, form) = match shape {
        ShapeKind::Curve | ShapeKind::Band | ShapeKind::Points => (1, false, ValueForm::Real),
        ShapeKind::Surface => (2, false, ValueForm::Real),
        ShapeKind::ScalarGrid | ShapeKind::VectorField | ShapeKind::EscapeTime => {
            (2, true, ValueForm::Real)
        }
        ShapeKind::ComplexGrid => (2, true, ValueForm::Complex),
        ShapeKind::Occupancy => (3, true, ValueForm::Real),
    };
    ShapeLayout {
        axes,
        at_cell_centres,
        form,
    }
}

fn lower_inputs(
    request: &SampleRequest,
    layout: &ShapeLayout,
) -> Result<Vec<LowerInput>, SampleError> {
    match (layout.form, request.axes.as_slice()) {
        (ValueForm::Complex, [real_axis, imaginary_axis]) => {
            if real_axis.symbol != imaginary_axis.symbol {
                return Err(SampleError::ComplexAxesDiffer);
            }
            Ok(vec![LowerInput {
                symbol: real_axis.symbol,
                form: ValueForm::Complex,
            }])
        }
        _ => Ok(request
            .axes
            .iter()
            .map(|axis| LowerInput {
                symbol: axis.symbol,
                form: ValueForm::Real,
            })
            .collect()),
    }
}

fn check_mesh_size(axes: &[Axis]) -> Result<(), SampleError> {
    let vertices = axes
        .iter()
        .try_fold(1_u64, |product, axis| {
            product.checked_mul(axis.sample_count)
        })
        .unwrap_or(u64::MAX);
    if vertices > LARGEST_MESH_VERTEX_COUNT {
        Err(SampleError::MeshTooLarge { vertices })
    } else {
        Ok(())
    }
}

fn evaluate_bound(
    pool: &mut ExprPool,
    axis_index: usize,
    bound: ExprId,
) -> Result<Number, SampleError> {
    let evaluation = evaluate_exact(pool, bound).map_err(|error| SampleError::BoundEvaluation {
        axis: axis_index,
        error,
    })?;
    match evaluation.rational_value() {
        Some(number) => Ok(number.clone()),
        None => Err(SampleError::BoundNotRational {
            axis: axis_index,
            kind: evaluation.kind(),
        }),
    }
}

fn count_number(count: u64) -> Number {
    Number::Integer(Integer::from(count))
}

fn default_bounds(
    request: &SampleRequest,
    form: ValueForm,
    index: usize,
    axis: &SampleAxis,
) -> Interval {
    if request.shape == SampledShape::EscapeTime {
        let escape_form = request
            .escape_time
            .as_ref()
            .map_or(EscapeTimeForm::QuadraticParameter, |settings| {
                settings.form.clone()
            });
        let [real, imaginary] = default_view(&escape_form);
        return if index == 0 { real } else { imaginary };
    }
    default_axis_interval(axis.dimension, &axis.unit, form == ValueForm::Complex)
}

fn evaluate_axis(
    pool: &mut ExprPool,
    (axis_index, default): (usize, &Interval),
    axis: &SampleAxis,
    at_cell_centres: bool,
) -> Result<Axis, SampleError> {
    let (lower, upper) = match &axis.bounds {
        AxisBounds::Default => (default.lower.clone(), default.upper.clone()),
        AxisBounds::Expressions { lower, upper } => (
            evaluate_bound(pool, axis_index, *lower)?,
            evaluate_bound(pool, axis_index, *upper)?,
        ),
        AxisBounds::Exact(interval) => {
            if !interval.lower.is_exact() || !interval.upper.is_exact() {
                return Err(SampleError::BoundNotExact { axis: axis_index });
            }
            (interval.lower.clone(), interval.upper.clone())
        }
    };
    if compare_exact(&lower, &upper) != Some(Ordering::Less) {
        return Err(SampleError::EmptyInterval { axis: axis_index });
    }
    if axis.divisions == 0 {
        return Err(SampleError::NoDivisions { axis: axis_index });
    }
    let length = upper
        .sub_exact(&lower)
        .expect("both bounds are exact rationals, checked above");
    let fineness = length
        .div_exact(&count_number(u64::from(axis.divisions)))
        .expect("the division count is positive, checked above");
    let sample_count = if at_cell_centres {
        u64::from(axis.divisions)
    } else {
        u64::from(axis.divisions) + 1
    };
    Ok(Axis {
        interval: Interval { lower, upper },
        fineness,
        divisions: axis.divisions,
        name: axis.name.clone(),
        sample_count,
        is_cell_centred: at_cell_centres,
        dimension: axis.dimension,
        unit: axis.unit.clone(),
    })
}

fn checked_batch_length(axes: &[Axis], sample_limit: u64) -> Result<usize, SampleError> {
    let requested = axes
        .iter()
        .try_fold(1_u64, |product, axis| {
            product.checked_mul(axis.sample_count)
        })
        .unwrap_or(u64::MAX);
    let limit = sample_limit.min(u64::try_from(usize::MAX).unwrap_or(u64::MAX));
    usize::try_from(requested)
        .ok()
        .filter(|_| requested <= limit)
        .ok_or(SampleError::SampleLimitExceeded { requested, limit })
}

fn axis_positions(axis: &Axis) -> Vec<Number> {
    let offset = if axis.is_cell_centred {
        Number::fraction(&Integer::one(), &Integer::from(2_i64)).expect("two is not zero")
    } else {
        Number::from(0_i64)
    };
    (0..axis.sample_count)
        .map(|step| {
            let steps = count_number(step)
                .add_exact(&offset)
                .expect("step and offset are exact");
            let distance = steps
                .mul_exact(&axis.fineness)
                .expect("steps and fineness are exact");
            axis.interval
                .lower
                .add_exact(&distance)
                .expect("lower bound and distance are exact")
        })
        .collect()
}

fn sampling_coordinates(axes: &[Axis], domain: Domain, length: usize) -> Vec<Column> {
    let mut stride = 1_usize;
    let mut columns = Vec::new();
    for axis in axes {
        let positions = axis_positions(axis);
        let period = positions.len().max(1);
        let position = |index: usize| positions.get((index / stride) % period);
        columns.push(match domain {
            Domain::F32 => Column::F32(
                (0..length)
                    .filter_map(position)
                    .map(Number::round_to_f32_ties_even)
                    .collect(),
            ),
            Domain::F64 => Column::F64(
                (0..length)
                    .filter_map(position)
                    .map(Number::round_to_f64_ties_even)
                    .collect(),
            ),
        });
        stride = stride.saturating_mul(period);
    }
    columns
}

fn count_special_values(columns: &[&Column]) -> SamplingDiagnostics {
    let mut diagnostics = SamplingDiagnostics::default();
    let mut count = |is_nan: bool, is_infinite: bool| {
        if is_nan {
            diagnostics.nan_samples = diagnostics.nan_samples.saturating_add(1);
        }
        if is_infinite {
            diagnostics.infinite_samples = diagnostics.infinite_samples.saturating_add(1);
        }
    };
    for column in columns {
        match column {
            Column::F32(values) => values
                .iter()
                .for_each(|value| count(value.is_nan(), value.is_infinite())),
            Column::F64(values) => values
                .iter()
                .for_each(|value| count(value.is_nan(), value.is_infinite())),
        }
    }
    diagnostics
}

fn refit_default_ranges(geometry: &mut Geometry, value_bounds: &[Column]) {
    let first = value_bounds.first();
    let second = value_bounds.get(1);
    let unresolved = |bounds: Option<&Column>, step: Option<Number>| {
        let bounds = bounds.cloned();
        move |index: usize| {
            let (Some(bounds), Some(step)) = (bounds.as_ref(), step.as_ref()) else {
                return false;
            };
            let bound = match bounds {
                Column::F32(values) => values.get(index).map(|value| f64::from(*value)),
                Column::F64(values) => values.get(index).copied(),
            };
            bound.is_some_and(|bound| is_unresolved_against(bound, step))
        }
    };
    match (&geometry.primitive, &mut geometry.view) {
        (Primitive::Polyline(Polyline { coordinates }), View::View2(view))
        | (Primitive::Points(Points { coordinates, .. }), View::View2(view)) => {
            if let Some(values) = coordinates.last() {
                let left_out = unresolved(first, axis_step(&view.y));
                view.y.range = value_interval(finite_values(&[values], &left_out));
            }
        }
        (Primitive::Band(band), View::View2(view)) => {
            let step = axis_step(&view.y);
            let lower_out = unresolved(first, step.clone());
            let upper_out = unresolved(second, step);
            let mut values = finite_values(&[&band.lower], &lower_out);
            values.extend(finite_values(&[&band.upper], &upper_out));
            view.y.range = value_interval(values);
        }
        (Primitive::TriangleMesh(mesh), View::View3(view)) => {
            if let Some(heights) = mesh.vertices.last() {
                let left_out = unresolved(first, axis_step(&view.z));
                view.z.range = value_interval(finite_values(&[heights], &left_out));
                if let Some(mapping) = geometry.style.colour_map.as_mut() {
                    mapping.range = view.z.range.clone();
                }
            }
        }
        (Primitive::ScalarGrid(grid), _) if grid.classes.is_none() => {
            let step = colour_step(&geometry.style);
            let left_out = unresolved(first, step);
            let range = value_interval(finite_values(&[&grid.scalar], &left_out));
            if let Some(mapping) = geometry.style.colour_map.as_mut() {
                mapping.range = range;
            }
        }
        _ => {}
    }
}

fn occupied_positions(values: &Column) -> Vec<usize> {
    let is_occupied: Vec<bool> = match values {
        Column::F32(column) => column.iter().map(|value| *value > 0.0).collect(),
        Column::F64(column) => column.iter().map(|value| *value > 0.0).collect(),
    };
    is_occupied
        .iter()
        .enumerate()
        .filter(|(_, occupied)| **occupied)
        .map(|(index, _)| index)
        .collect()
}

fn layer_precision(
    geometry: &Geometry,
    value_bounds: &[Column],
    axes: &[Axis],
    domain: Domain,
) -> Option<PrecisionLimit> {
    let grid_exhausted: Vec<u32> = axes
        .iter()
        .enumerate()
        .filter(|(_, axis)| {
            is_grid_exhausted(&axis.interval, &axis.fineness, domain)
                || (!axis.unit.is_coherent()
                    && coherent_positions_merge(&axis_positions(axis), &axis.unit, domain))
        })
        .filter_map(|(index, _)| u32::try_from(index).ok())
        .collect();
    let counts = count_resolution(
        &geometry.primitive,
        value_bounds,
        &geometry.view,
        &geometry.style,
    );
    let is_clean = grid_exhausted.is_empty() && counts.unresolved == 0 && counts.unknown == 0;
    (!is_clean).then_some(PrecisionLimit {
        grid_exhausted,
        unresolved_samples: counts.unresolved,
        unknown_bounds: counts.unknown,
        ..PrecisionLimit::default()
    })
}

fn view_axis(range: Interval, dimension: Dimension, unit: AxisUnit, divisions: u32) -> ViewAxis {
    ViewAxis {
        range,
        scale: Scale::Linear,
        dimension,
        unit,
        divisions,
    }
}

fn value_view_axis(request: &SceneSettings, columns: &[&Column]) -> ViewAxis {
    let range = match &request.value_range {
        Some(range) => range.clone(),
        None => value_interval(finite_values(columns, &|_| false)),
    };
    view_axis(
        range,
        request.value_dimension,
        request.value_unit.clone(),
        request.value_divisions,
    )
}

fn plane_view(axis: &Axis, value_axis: ViewAxis) -> View {
    View::View2(Box::new(View2 {
        x: view_axis(
            axis.interval.clone(),
            axis.dimension,
            axis.unit.clone(),
            axis.divisions,
        ),
        y: value_axis,
    }))
}

fn line_style(kind: KindColour, line: LinePattern, marker: Marker) -> StyleRole {
    StyleRole {
        kind,
        colour_map: None,
        line,
        marker,
        emphasis: Emphasis::Normal,
    }
}

fn grid_geometry(
    request: &SceneSettings,
    x_axis: &Axis,
    y_axis: &Axis,
    scalar: Column,
    backend: BackendKind,
    classes: Option<Vec<u8>>,
) -> Geometry {
    let diagnostics = count_special_values(&[&scalar]);
    let range = match &request.value_range {
        Some(range) => range.clone(),
        None => value_interval(finite_values(&[&scalar], &|_| false)),
    };
    Geometry {
        view: View::View2(Box::new(View2 {
            x: view_axis(
                x_axis.interval.clone(),
                x_axis.dimension,
                x_axis.unit.clone(),
                x_axis.divisions,
            ),
            y: view_axis(
                y_axis.interval.clone(),
                y_axis.dimension,
                y_axis.unit.clone(),
                y_axis.divisions,
            ),
        })),
        primitive: Primitive::ScalarGrid(ScalarGrid {
            cells: CellGrid {
                region: vec![x_axis.interval.clone(), y_axis.interval.clone()],
                counts: vec![x_axis.divisions, y_axis.divisions],
            },
            scalar,
            classes,
        }),
        style: StyleRole {
            kind: request.kind,
            colour_map: Some(ColourMapping {
                map: ColourMap::Sequential,
                range,
            }),
            line: LinePattern::None,
            marker: Marker::None,
            emphasis: Emphasis::Normal,
        },
        backend,
        diagnostics,
    }
}

fn box_view(x_axis: &Axis, y_axis: &Axis) -> View {
    View::View2(Box::new(View2 {
        x: view_axis(
            x_axis.interval.clone(),
            x_axis.dimension,
            x_axis.unit.clone(),
            x_axis.divisions,
        ),
        y: view_axis(
            y_axis.interval.clone(),
            y_axis.dimension,
            y_axis.unit.clone(),
            y_axis.divisions,
        ),
    }))
}

fn cell_grid(axes: &[&Axis]) -> CellGrid {
    CellGrid {
        region: axes.iter().map(|axis| axis.interval.clone()).collect(),
        counts: axes.iter().map(|axis| axis.divisions).collect(),
    }
}

fn complex_geometry(
    request: &SceneSettings,
    real_axis: &Axis,
    imaginary_axis: &Axis,
    real: Column,
    imaginary: Column,
    backend: BackendKind,
) -> Geometry {
    let range = request
        .value_range
        .clone()
        .unwrap_or_else(|| reference_modulus(&real, &imaginary));
    Geometry {
        view: box_view(real_axis, imaginary_axis),
        diagnostics: count_special_values(&[&real, &imaginary]),
        primitive: Primitive::ComplexGrid(ComplexGrid {
            cells: cell_grid(&[real_axis, imaginary_axis]),
            real,
            imaginary,
        }),
        style: StyleRole {
            kind: request.kind,
            colour_map: Some(ColourMapping {
                map: ColourMap::DomainColouring,
                range,
            }),
            line: LinePattern::None,
            marker: Marker::None,
            emphasis: Emphasis::Normal,
        },
        backend,
    }
}

fn mesh_triangles(x_axis: &Axis, y_axis: &Axis) -> Vec<[u32; 3]> {
    let row_length = x_axis.sample_count;
    let vertex =
        |column: u64, row: u64| u32::try_from(row * row_length + column).unwrap_or(u32::MAX);
    let mut triangles = Vec::new();
    for row in 0..u64::from(y_axis.divisions) {
        for column in 0..u64::from(x_axis.divisions) {
            let corner = vertex(column, row);
            let right = vertex(column + 1, row);
            let above = vertex(column, row + 1);
            let diagonal = vertex(column + 1, row + 1);
            triangles.push([corner, right, diagonal]);
            triangles.push([corner, diagonal, above]);
        }
    }
    triangles
}

fn exact_difference(left: &Number, right: &Number) -> Number {
    left.sub_exact(right).unwrap_or_else(|_| left.clone())
}

fn overview_camera() -> Camera {
    Camera {
        azimuth_degrees: Number::from(OVERVIEW_AZIMUTH_DEGREES),
        elevation_degrees: Number::from(OVERVIEW_ELEVATION_DEGREES),
        projection: Projection::Orthographic,
    }
}

fn space_view(x: ViewAxis, y: ViewAxis, z: ViewAxis) -> View {
    let camera = overview_camera();
    View::View3(Box::new(View3 { x, y, z, camera }))
}

fn surface_geometry(
    request: &SceneSettings,
    x_axis: &Axis,
    y_axis: &Axis,
    positions: [Column; 2],
    heights: Column,
    backend: BackendKind,
) -> Geometry {
    let height_axis = value_view_axis(request, &[&heights]);
    let [x_positions, y_positions] = positions;
    Geometry {
        diagnostics: count_special_values(&[&heights]),
        style: StyleRole {
            kind: request.kind,
            colour_map: Some(ColourMapping {
                map: ColourMap::Sequential,
                range: height_axis.range.clone(),
            }),
            line: LinePattern::None,
            marker: Marker::None,
            emphasis: Emphasis::Normal,
        },
        view: space_view(
            view_axis(
                x_axis.interval.clone(),
                x_axis.dimension,
                x_axis.unit.clone(),
                x_axis.divisions,
            ),
            view_axis(
                y_axis.interval.clone(),
                y_axis.dimension,
                y_axis.unit.clone(),
                y_axis.divisions,
            ),
            height_axis,
        ),
        primitive: Primitive::TriangleMesh(TriangleMesh {
            vertices: vec![x_positions, y_positions, heights.clone()],
            triangles: mesh_triangles(x_axis, y_axis),
            scalar: Some(heights),
        }),
        backend,
    }
}

fn voxel_axis(axis: &Axis) -> ViewAxis {
    let half = Number::fraction(&Integer::one(), &Integer::from(2_i64))
        .unwrap_or_else(|_| Number::from(0_i64));
    let cells = count_number(u64::from(axis.divisions));
    view_axis(
        Interval {
            lower: exact_difference(&Number::from(0_i64), &half),
            upper: exact_difference(&cells, &half),
        },
        Dimension::DIMENSIONLESS,
        AxisUnit::dimensionless(),
        axis.divisions,
    )
}

fn occupied_cells(values: &Column, counts: [u32; 3]) -> (Vec<[i64; 3]>, Column) {
    let is_occupied = |index: usize| match values {
        Column::F32(column) => column.get(index).is_some_and(|value| *value > 0.0),
        Column::F64(column) => column.get(index).is_some_and(|value| *value > 0.0),
    };
    let mut occupied = Vec::new();
    let mut kept = Vec::new();
    let mut index = 0_usize;
    for z in 0..counts[2] {
        for y in 0..counts[1] {
            for x in 0..counts[0] {
                if is_occupied(index) {
                    occupied.push([i64::from(x), i64::from(y), i64::from(z)]);
                    kept.push(index);
                }
                index += 1;
            }
        }
    }
    let scalar = match values {
        Column::F32(column) => Column::F32(
            kept.iter()
                .filter_map(|index| column.get(*index).copied())
                .collect(),
        ),
        Column::F64(column) => Column::F64(
            kept.iter()
                .filter_map(|index| column.get(*index).copied())
                .collect(),
        ),
    };
    (occupied, scalar)
}

fn occupancy_geometry(
    request: &SceneSettings,
    axes: [&Axis; 3],
    values: &Column,
    backend: BackendKind,
) -> Geometry {
    let [x_axis, y_axis, z_axis] = axes;
    let (occupied, scalar) = occupied_cells(
        values,
        [x_axis.divisions, y_axis.divisions, z_axis.divisions],
    );
    let range = request
        .value_range
        .clone()
        .unwrap_or_else(|| value_interval(finite_values(&[&scalar], &|_| false)));
    Geometry {
        view: space_view(voxel_axis(x_axis), voxel_axis(y_axis), voxel_axis(z_axis)),
        diagnostics: count_special_values(&[values]),
        primitive: Primitive::Voxels(Voxels {
            occupied,
            scalar: Some(scalar),
        }),
        style: StyleRole {
            kind: request.kind,
            colour_map: Some(ColourMapping {
                map: ColourMap::Sequential,
                range,
            }),
            line: LinePattern::None,
            marker: Marker::None,
            emphasis: Emphasis::Normal,
        },
        backend,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::escape_time::{ESCAPED_CELL, INSIDE_CELL, UNDECIDED_CELL};
    use crate::legend::{ColourLegend, layer_legend, shape_legend};
    use crate::primitive::ScalarGrid;
    use calc_exec::{
        Capabilities, CostParameters, Plan, PlanOp, PrepareError, Prepared, RunReport,
    };
    use calc_exec_cpu::CpuBackend;
    use calc_expr::{BinderKind, BuiltinConstant, Head, Operator, ReductionShape, SymbolKind};

    const SAMPLE_LIMIT: u64 = 1_000;

    struct Fixture {
        pool: ExprPool,
        x: SymbolId,
        x_node: ExprId,
    }

    impl Fixture {
        fn new() -> Fixture {
            let mut pool = ExprPool::new();
            let x = pool.intern_symbol("x", SymbolKind::Variable).unwrap();
            let x_node = pool.symbol(x).unwrap();
            Fixture { pool, x, x_node }
        }

        fn integer(&mut self, value: i64) -> ExprId {
            self.pool.number(Number::from(value)).unwrap()
        }

        fn apply(&mut self, operator: Operator, arguments: &[ExprId]) -> ExprId {
            self.pool
                .apply(Head::Operator(operator), arguments)
                .unwrap()
        }

        fn axis(&mut self, lower: i64, upper: i64, divisions: u32) -> SampleAxis {
            let lower = self.integer(lower);
            let upper = self.integer(upper);
            SampleAxis {
                symbol: self.x,
                name: String::from("x"),
                bounds: AxisBounds::Expressions { lower, upper },
                divisions,
                dimension: Dimension::DIMENSIONLESS,
                unit: AxisUnit::dimensionless(),
            }
        }

        fn request(&self, shape: SampledShape, axes: Vec<SampleAxis>) -> SampleRequest {
            SampleRequest {
                result: ResultId(7),
                expression_text: String::from("x^2"),
                shape,
                axes,
                value_range: None,
                value_dimension: Dimension::DIMENSIONLESS,
                value_unit: AxisUnit::dimensionless(),
                value_divisions: 100,
                domain: Domain::F64,
                preference: Preference::Automatic,
                sample_limit: SAMPLE_LIMIT,
                kind: KindColour::Numeric,
                references: vec![CorpusReference(String::from("M-VEK-D-032"))],
                escape_time: None,
            }
        }

        fn square_curve(&mut self, divisions: u32) -> SampleRequest {
            let square = self.apply(Operator::Mul, &[self.x_node, self.x_node]);
            let axis = self.axis(0, 1, divisions);
            self.request(SampledShape::Curve(square), vec![axis])
        }

        fn sample(&mut self, request: &SampleRequest) -> Result<Scene, SampleError> {
            let backend = CpuBackend::new();
            sample(&mut self.pool, &[&backend], request)
        }
    }

    struct FailingBackend {
        capabilities: Capabilities,
    }

    struct FailingPrepared;

    impl Backend for FailingBackend {
        fn kind(&self) -> BackendKind {
            BackendKind::Gpu
        }

        fn capabilities(&self) -> &Capabilities {
            &self.capabilities
        }

        fn prepare(&self, _plan: &Plan) -> Result<Box<dyn Prepared>, PrepareError> {
            Ok(Box::new(FailingPrepared))
        }
    }

    impl Prepared for FailingPrepared {
        fn run(&mut self, _inputs: &Batch, _outputs: &mut Batch) -> Result<RunReport, RunError> {
            Err(RunError::DeviceLost)
        }
    }

    fn bits_f64(column: &Column) -> Vec<u64> {
        match column {
            Column::F64(values) => values.iter().map(|value| value.to_bits()).collect(),
            Column::F32(_) => panic!("expected an f64 column"),
        }
    }

    fn expected_bits(values: &[f64]) -> Vec<u64> {
        values.iter().map(|value| value.to_bits()).collect()
    }

    fn only_primitive(scene: &Scene) -> &Primitive {
        &scene.frames[0].layers[0].primitive
    }

    fn fraction(numerator: i64, denominator: i64) -> Number {
        Number::fraction(&Integer::from(numerator), &Integer::from(denominator)).unwrap()
    }

    const LENGTH: Dimension = Dimension {
        exponents: [1, 0, 0, 0, 0, 0, 0, 0],
    };

    fn times_unit(fixture: &mut Fixture, unit: &str) -> ExprId {
        let one = fixture.integer(1);
        let unit = fixture.pool.units_mut().lookup(unit).unwrap();
        let quantity = fixture.pool.quantity(one, unit).unwrap();
        fixture.apply(Operator::Mul, &[fixture.x_node, quantity])
    }

    fn curve_floats(scene: &Scene) -> Vec<f64> {
        let Primitive::Polyline(polyline) = only_primitive(scene) else {
            panic!("expected a polyline");
        };
        let Column::F64(values) = &polyline.coordinates[1] else {
            panic!("expected f64 values");
        };
        values.clone()
    }

    #[test]
    fn curve_with_a_unit_takes_its_value_axis_unit_from_the_result() {
        let mut fixture = Fixture::new();
        let metres = times_unit(&mut fixture, "m");
        let axis = fixture.axis(0, 1, 2);
        let request = fixture.request(SampledShape::Curve(metres), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        let view = first_view(&scene);
        assert_eq!(
            (view.y.dimension, view.y.unit.clone()),
            (
                LENGTH,
                AxisUnit::Coherent {
                    symbol: String::from("m")
                }
            )
        );
    }

    #[test]
    fn curve_with_a_unit_samples_its_numbers() {
        let mut fixture = Fixture::new();
        let metres = times_unit(&mut fixture, "m");
        let axis = fixture.axis(0, 1, 2);
        let request = fixture.request(SampledShape::Curve(metres), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(curve_floats(&scene), vec![0.0, 0.5, 1.0]);
    }

    #[test]
    fn curve_in_a_smaller_unit_samples_coherent_values() {
        let mut fixture = Fixture::new();
        let centimetres = times_unit(&mut fixture, "cm");
        let axis = fixture.axis(0, 2, 2);
        let request = fixture.request(SampledShape::Curve(centimetres), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(curve_floats(&scene), vec![0.0, 0.01, 0.02]);
    }

    #[test]
    fn band_whose_bounds_differ_in_dimension_is_rejected() {
        let mut fixture = Fixture::new();
        let metres = times_unit(&mut fixture, "m");
        let seconds = times_unit(&mut fixture, "s");
        let axis = fixture.axis(0, 1, 2);
        let request = fixture.request(
            SampledShape::Band {
                lower: metres,
                upper: seconds,
            },
            vec![axis],
        );

        let result = fixture.sample(&request);

        assert!(matches!(
            result,
            Err(SampleError::ValueDimensionMismatch {
                requested: LENGTH,
                ..
            })
        ));
    }

    #[test]
    fn requested_value_dimension_other_than_the_result_is_rejected() {
        let mut fixture = Fixture::new();
        let metres = times_unit(&mut fixture, "m");
        let axis = fixture.axis(0, 1, 2);
        let mut request = fixture.request(SampledShape::Curve(metres), vec![axis]);
        request.value_dimension = Dimension::TEMPERATURE;
        request.value_unit = AxisUnit::Coherent {
            symbol: String::from("K"),
        };

        let result = fixture.sample(&request);

        assert_eq!(
            result,
            Err(SampleError::ValueDimensionMismatch {
                requested: Dimension::TEMPERATURE,
                found: LENGTH
            })
        );
    }

    #[test]
    fn bare_number_on_a_length_value_axis_is_rejected() {
        let mut fixture = Fixture::new();
        let axis = fixture.axis(0, 1, 2);
        let mut request = fixture.request(SampledShape::Curve(fixture.x_node), vec![axis]);
        request.value_dimension = LENGTH;
        request.value_unit = AxisUnit::Coherent {
            symbol: String::from("m"),
        };

        let result = fixture.sample(&request);

        assert_eq!(
            result,
            Err(SampleError::ValueDimensionMismatch {
                requested: LENGTH,
                found: Dimension::DIMENSIONLESS
            })
        );
    }

    #[test]
    fn dimensioned_function_argument_is_a_quantity_error() {
        let mut fixture = Fixture::new();
        let metres = times_unit(&mut fixture, "m");
        let sine = fixture.apply(Operator::Sin, &[metres]);
        let axis = fixture.axis(0, 1, 2);
        let request = fixture.request(SampledShape::Curve(sine), vec![axis]);

        let result = fixture.sample(&request);

        assert!(matches!(
            result,
            Err(SampleError::Quantity(
                QuantityError::DimensionedArgument { .. }
            ))
        ));
    }

    fn first_view(scene: &Scene) -> &View2 {
        match &scene.views[0] {
            View::View2(view) => view,
            View::View3(_) => panic!("expected a plane view"),
        }
    }

    #[test]
    fn curve_samples_the_partition_points_of_the_interval() {
        let mut fixture = Fixture::new();
        let request = fixture.square_curve(4);

        let scene = fixture.sample(&request).unwrap();

        let Primitive::Polyline(polyline) = only_primitive(&scene) else {
            panic!("expected a polyline");
        };
        assert_eq!(
            polyline
                .coordinates
                .iter()
                .map(bits_f64)
                .collect::<Vec<_>>(),
            vec![
                expected_bits(&[0.0, 0.25, 0.5, 0.75, 1.0]),
                expected_bits(&[0.0, 0.0625, 0.25, 0.5625, 1.0])
            ]
        );
    }

    #[test]
    fn band_holds_the_lower_and_upper_ordinates() {
        let mut fixture = Fixture::new();
        let one = fixture.integer(1);
        let lower = fixture.apply(Operator::Sub, &[fixture.x_node, one]);
        let upper = fixture.apply(Operator::Add, &[fixture.x_node, one]);
        let axis = fixture.axis(0, 2, 2);
        let request = fixture.request(SampledShape::Band { lower, upper }, vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        let Primitive::Band(band) = only_primitive(&scene) else {
            panic!("expected a band");
        };
        assert_eq!(
            [&band.abscissa, &band.lower, &band.upper].map(bits_f64),
            [
                expected_bits(&[0.0, 1.0, 2.0]),
                expected_bits(&[-1.0, 0.0, 1.0]),
                expected_bits(&[1.0, 2.0, 3.0])
            ]
        );
    }

    #[test]
    fn points_place_one_marker_per_sample() {
        let mut fixture = Fixture::new();
        let axis = fixture.axis(-1, 1, 2);
        let request = fixture.request(SampledShape::Points(fixture.x_node), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        let Primitive::Points(points) = only_primitive(&scene) else {
            panic!("expected points");
        };
        assert_eq!(
            points.coordinates.iter().map(bits_f64).collect::<Vec<_>>(),
            vec![
                expected_bits(&[-1.0, 0.0, 1.0]),
                expected_bits(&[-1.0, 0.0, 1.0])
            ]
        );
        assert_eq!(scene.frames[0].layers[0].style.marker, Marker::Dot);
    }

    #[test]
    fn scalar_grid_samples_cell_centres_with_the_first_axis_fastest() {
        let mut fixture = Fixture::new();
        let y = fixture
            .pool
            .intern_symbol("y", SymbolKind::Variable)
            .unwrap();
        let y_node = fixture.pool.symbol(y).unwrap();
        let ten = fixture.integer(10);
        let scaled = fixture.apply(Operator::Mul, &[ten, y_node]);
        let sum = fixture.apply(Operator::Add, &[fixture.x_node, scaled]);
        let x_axis = fixture.axis(0, 2, 2);
        let mut y_axis = fixture.axis(0, 1, 2);
        y_axis.symbol = y;
        y_axis.name = String::from("y");
        let request = fixture.request(SampledShape::ScalarGrid(sum), vec![x_axis, y_axis]);

        let scene = fixture.sample(&request).unwrap();

        let Primitive::ScalarGrid(grid) = only_primitive(&scene) else {
            panic!("expected a scalar grid");
        };
        assert_eq!(grid.cells.counts, vec![2, 2]);
        assert_eq!(bits_f64(&grid.scalar), expected_bits(&[3.0, 4.0, 8.0, 9.0]));
    }

    #[test]
    fn scalar_grid_colour_map_takes_the_rounded_value_interval() {
        let mut fixture = Fixture::new();
        let y = fixture
            .pool
            .intern_symbol("y", SymbolKind::Variable)
            .unwrap();
        let x_axis = fixture.axis(0, 2, 2);
        let mut y_axis = fixture.axis(0, 1, 1);
        y_axis.symbol = y;
        let request = fixture.request(
            SampledShape::ScalarGrid(fixture.x_node),
            vec![x_axis, y_axis],
        );

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            scene.frames[0].layers[0].style.colour_map,
            Some(ColourMapping {
                map: ColourMap::Sequential,
                range: Interval {
                    lower: fraction(2, 5),
                    upper: fraction(8, 5)
                }
            })
        );
    }

    #[test]
    fn resolution_holds_sample_count_and_fineness_of_the_partition() {
        let mut fixture = Fixture::new();
        let request = fixture.square_curve(4);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            scene.record.resolution,
            Resolution {
                domain: Domain::F64,
                axes: vec![AxisResolution {
                    sample_count: 5,
                    fineness: fraction(1, 4)
                }],
                adaptive: None
            }
        );
    }

    #[test]
    fn record_holds_the_input_text_box_and_variables() {
        let mut fixture = Fixture::new();
        let request = fixture.square_curve(4);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            scene.record.inputs,
            vec![SceneInput {
                expression: String::from("x^2"),
                sampling_box: vec![Interval {
                    lower: Number::from(0_i64),
                    upper: Number::from(1_i64)
                }],
                variables: vec![String::from("x")]
            }]
        );
    }

    #[test]
    fn record_names_method_backend_result_and_references_without_seed() {
        let mut fixture = Fixture::new();
        let request = fixture.square_curve(4);

        let scene = fixture.sample(&request).unwrap();

        let record = &scene.record;
        assert_eq!(
            (
                &record.method,
                record.seed.is_none(),
                record.backend,
                record.result,
                &record.references
            ),
            (
                &SamplingMethod::UniformGrid,
                true,
                BackendKind::Cpu,
                ResultId(7),
                &vec![CorpusReference(String::from("M-VEK-D-032"))]
            )
        );
    }

    #[test]
    fn bounds_are_evaluated_exactly_by_the_core() {
        let mut fixture = Fixture::new();
        let one = fixture.integer(1);
        let three = fixture.integer(3);
        let third = fixture.apply(Operator::Div, &[one, three]);
        let mut axis = fixture.axis(0, 1, 1);
        axis.bounds = AxisBounds::Expressions {
            lower: third,
            upper: fixture.integer(1),
        };
        let request = fixture.request(SampledShape::Curve(fixture.x_node), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(scene.record.inputs[0].sampling_box[0].lower, fraction(1, 3));
    }

    #[test]
    fn value_view_spans_the_finite_samples() {
        let mut fixture = Fixture::new();
        let request = fixture.square_curve(4);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            first_view(&scene).y.range,
            Interval {
                lower: Number::from(0_i64),
                upper: Number::from(1_i64)
            }
        );
    }

    #[test]
    fn constant_samples_widen_the_value_view_by_half_their_magnitude() {
        let mut fixture = Fixture::new();
        let two = fixture.integer(2);
        let axis = fixture.axis(0, 1, 2);
        let request = fixture.request(SampledShape::Curve(two), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            first_view(&scene).y.range,
            Interval {
                lower: Number::from(1_i64),
                upper: Number::from(3_i64)
            }
        );
    }

    fn default_axis(fixture: &Fixture, divisions: u32) -> SampleAxis {
        SampleAxis {
            symbol: fixture.x,
            name: String::from("x"),
            bounds: AxisBounds::Default,
            divisions,
            dimension: Dimension::DIMENSIONLESS,
            unit: AxisUnit::dimensionless(),
        }
    }

    #[test]
    fn open_bounds_sample_the_default_interval() {
        let mut fixture = Fixture::new();
        let axis = default_axis(&fixture, 4);
        let request = fixture.request(SampledShape::Curve(fixture.x_node), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            first_view(&scene).x.range,
            Interval {
                lower: Number::from(-10_i64),
                upper: Number::from(10_i64)
            }
        );
    }

    #[test]
    fn sine_over_the_default_interval_gets_minus_one_to_one() {
        let mut fixture = Fixture::new();
        let sine = fixture.apply(Operator::Sin, &[fixture.x_node]);
        let axis = default_axis(&fixture, 1000);
        let mut request = fixture.request(SampledShape::Curve(sine), vec![axis]);
        request.sample_limit = 10_000;

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            first_view(&scene).y.range,
            Interval {
                lower: Number::from(-1_i64),
                upper: Number::from(1_i64)
            }
        );
    }

    #[test]
    fn square_over_the_default_interval_gets_zero_to_one_hundred() {
        let mut fixture = Fixture::new();
        let square = fixture.apply(Operator::Mul, &[fixture.x_node, fixture.x_node]);
        let axis = default_axis(&fixture, 1000);
        let mut request = fixture.request(SampledShape::Curve(square), vec![axis]);
        request.sample_limit = 10_000;

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            first_view(&scene).y.range,
            Interval {
                lower: Number::from(0_i64),
                upper: Number::from(100_i64)
            }
        );
    }

    #[test]
    fn open_bounds_of_an_escape_time_picture_take_its_default_view() {
        let mut fixture = Fixture::new();
        let mut request = escape_request(
            &mut fixture,
            EscapeTimeForm::QuadraticParameter,
            IterationLimitRule::Fixed { iterations: 8 },
        );
        request.axes[0].bounds = AxisBounds::Default;
        request.axes[1].bounds = AxisBounds::Default;

        let scene = fixture.sample(&request).unwrap();

        let view = first_view(&scene);
        assert_eq!(
            (view.x.range.clone(), view.y.range.clone()),
            (
                Interval {
                    lower: fraction(-5, 2),
                    upper: Number::from(1_i64)
                },
                Interval {
                    lower: fraction(-5, 4),
                    upper: fraction(5, 4)
                }
            )
        );
    }

    #[test]
    fn unresolved_samples_are_left_out_of_the_default_value_interval() {
        let values: Vec<f64> = (0..=100).map(|index| f64::from(index) / 100.0).collect();
        let bounds: Vec<f64> = (0..=100)
            .map(|index| if index > 70 { 1.0 } else { 0.0 })
            .collect();
        let unit_axis = || {
            view_axis(
                Interval {
                    lower: Number::from(0_i64),
                    upper: Number::from(1_i64),
                },
                Dimension::DIMENSIONLESS,
                AxisUnit::dimensionless(),
                100,
            )
        };
        let mut geometry = Geometry {
            view: View::View2(Box::new(View2 {
                x: unit_axis(),
                y: unit_axis(),
            })),
            primitive: Primitive::Polyline(Polyline {
                coordinates: vec![Column::F64(values.clone()), Column::F64(values)],
            }),
            style: line_style(KindColour::Numeric, LinePattern::Solid, Marker::None),
            backend: BackendKind::Cpu,
            diagnostics: SamplingDiagnostics::default(),
        };

        refit_default_ranges(&mut geometry, &[Column::F64(bounds)]);

        let View::View2(view) = &geometry.view else {
            panic!("expected a plane view");
        };
        assert_eq!(view.y.range.upper, fraction(4, 5));
    }

    #[test]
    fn requested_value_range_replaces_the_sampled_one() {
        let mut fixture = Fixture::new();
        let mut request = fixture.square_curve(4);
        let range = Interval {
            lower: Number::from(-5_i64),
            upper: Number::from(5_i64),
        };
        request.value_range = Some(range.clone());

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(first_view(&scene).y.range, range);
    }

    #[test]
    fn non_finite_samples_are_kept_and_counted() {
        let mut fixture = Fixture::new();
        let logarithm = fixture.apply(Operator::Ln, &[fixture.x_node]);
        let axis = fixture.axis(-1, 1, 2);
        let request = fixture.request(SampledShape::Curve(logarithm), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        let Primitive::Polyline(polyline) = only_primitive(&scene) else {
            panic!("expected a polyline");
        };
        let Column::F64(values) = &polyline.coordinates[1] else {
            panic!("expected an f64 column");
        };
        assert!(values[0].is_nan());
        assert_eq!(values[1].to_bits(), f64::NEG_INFINITY.to_bits());
        assert_eq!(
            (
                scene.record.diagnostics.nan_samples,
                scene.record.diagnostics.infinite_samples
            ),
            (1, 1)
        );
    }

    #[test]
    fn samples_without_finite_value_get_the_unit_view_around_zero() {
        let mut fixture = Fixture::new();
        let zero = fixture.integer(0);
        let reciprocal = fixture.apply(Operator::Div, &[zero, zero]);
        let axis = fixture.axis(0, 1, 1);
        let request = fixture.request(SampledShape::Curve(reciprocal), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            first_view(&scene).y.range,
            Interval {
                lower: Number::from(-1_i64),
                upper: Number::from(1_i64)
            }
        );
    }

    #[test]
    fn f32_domain_keeps_f32_columns() {
        let mut fixture = Fixture::new();
        let mut request = fixture.square_curve(2);
        request.domain = Domain::F32;

        let scene = fixture.sample(&request).unwrap();

        let Primitive::Polyline(polyline) = only_primitive(&scene) else {
            panic!("expected a polyline");
        };
        assert_eq!(polyline.coordinates[1], Column::F32(vec![0.0, 0.25, 1.0]));
    }

    #[test]
    fn curve_with_two_axes_is_an_axis_count_error() {
        let mut fixture = Fixture::new();
        let mut request = fixture.square_curve(2);
        let second = fixture.axis(0, 1, 2);
        request.axes.push(second);

        let result = fixture.sample(&request);

        assert_eq!(
            result,
            Err(SampleError::AxisCount {
                expected: 1,
                found: 2
            })
        );
    }

    #[test]
    fn machine_number_bound_samples_as_the_number_it_is() {
        let mut fixture = Fixture::new();
        let mut request = fixture.square_curve(2);
        let machine = fixture.pool.number(Number::F64(1.0)).unwrap();
        request.axes[0].bounds = AxisBounds::Expressions {
            lower: fixture.integer(0),
            upper: machine,
        };
        let exact = fixture.integer(1);
        let mut same = fixture.square_curve(2);
        same.axes[0].bounds = AxisBounds::Expressions {
            lower: fixture.integer(0),
            upper: exact,
        };

        let result = fixture.sample(&request);

        assert_eq!(result, fixture.sample(&same));
        assert!(result.is_ok());
    }

    #[test]
    fn pi_bound_is_not_rational() {
        let mut fixture = Fixture::new();
        let mut request = fixture.square_curve(2);
        let pi = fixture.pool.symbol(BuiltinConstant::Pi.symbol()).unwrap();
        request.axes[0].bounds = AxisBounds::Expressions {
            lower: fixture.integer(0),
            upper: pi,
        };

        let result = fixture.sample(&request);

        assert!(matches!(
            result,
            Err(SampleError::BoundNotRational { axis: 0, .. })
        ));
    }

    #[test]
    fn equal_bounds_are_an_empty_interval() {
        let mut fixture = Fixture::new();
        let mut request = fixture.square_curve(2);
        let zero = fixture.integer(0);
        request.axes[0].bounds = AxisBounds::Expressions {
            lower: zero,
            upper: zero,
        };

        let result = fixture.sample(&request);

        assert_eq!(result, Err(SampleError::EmptyInterval { axis: 0 }));
    }

    #[test]
    fn zero_divisions_are_rejected() {
        let mut fixture = Fixture::new();
        let request = fixture.square_curve(0);

        let result = fixture.sample(&request);

        assert_eq!(result, Err(SampleError::NoDivisions { axis: 0 }));
    }

    #[test]
    fn request_above_the_sample_limit_fails_before_sampling() {
        let mut fixture = Fixture::new();
        let mut request = fixture.square_curve(4);
        request.sample_limit = 4;

        let result = fixture.sample(&request);

        assert_eq!(
            result,
            Err(SampleError::SampleLimitExceeded {
                requested: 5,
                limit: 4
            })
        );
    }

    #[test]
    fn expression_with_an_unsampled_variable_does_not_lower() {
        let mut fixture = Fixture::new();
        let y = fixture
            .pool
            .intern_symbol("y", SymbolKind::Variable)
            .unwrap();
        let y_node = fixture.pool.symbol(y).unwrap();
        let axis = fixture.axis(0, 1, 2);
        let request = fixture.request(SampledShape::Curve(y_node), vec![axis]);

        let result = fixture.sample(&request);

        assert_eq!(
            result,
            Err(SampleError::Lower(LowerError::UnboundSymbol(y)))
        );
    }

    #[test]
    fn sum_over_an_index_is_a_reduced_expression() {
        let mut fixture = Fixture::new();
        let first = fixture.integer(1);
        let last = fixture.integer(3);
        let index = fixture.pool.bound(0).unwrap();
        let body = fixture.apply(Operator::Mul, &[index, fixture.x_node]);
        let sum = fixture
            .pool
            .bind(
                BinderKind::Sum(ReductionShape::Halving),
                &[first, last],
                body,
            )
            .unwrap();
        let axis = fixture.axis(0, 1, 2);
        let request = fixture.request(SampledShape::Curve(sum), vec![axis]);

        let result = fixture.sample(&request);

        assert_eq!(result, Err(SampleError::ReducedExpression(sum)));
    }

    #[test]
    fn preferring_an_unregistered_backend_is_a_select_error() {
        let mut fixture = Fixture::new();
        let mut request = fixture.square_curve(2);
        request.preference = Preference::Only(BackendKind::Gpu);

        let result = fixture.sample(&request);

        assert_eq!(
            result,
            Err(SampleError::Select(SelectError::NotRegistered(
                BackendKind::Gpu
            )))
        );
    }

    #[test]
    fn backend_failure_is_a_run_error() {
        let mut fixture = Fixture::new();
        let request = fixture.square_curve(2);
        let backend = FailingBackend {
            capabilities: Capabilities {
                domains: vec![Domain::F64],
                operations: PlanOp::ALL.to_vec(),
                largest_batch_length: usize::MAX,
                largest_iteration_count: 0,
                reference_approximate_operations: false,
                cost: CostParameters {
                    setup_picoseconds: 0,
                    picoseconds_per_gate_evaluation: 0,
                    transfer: None,
                },
            },
        };

        let result = sample(&mut fixture.pool, &[&backend], &request);

        assert_eq!(result, Err(SampleError::Run(RunError::DeviceLost)));
    }

    #[test]
    fn machine_number_in_requested_value_range_is_a_scene_error() {
        let mut fixture = Fixture::new();
        let mut request = fixture.square_curve(2);
        request.value_range = Some(Interval {
            lower: Number::from(0_i64),
            upper: Number::F64(1.0),
        });

        let result = fixture.sample(&request);

        assert_eq!(
            result,
            Err(SampleError::Scene(SceneError::NotExact(
                crate::scene::Location::View(0)
            )))
        );
    }

    fn named_axis(
        fixture: &mut Fixture,
        name: &str,
        bounds: (i64, i64),
        divisions: u32,
    ) -> (SampleAxis, ExprId) {
        let symbol = fixture
            .pool
            .intern_symbol(name, SymbolKind::Variable)
            .unwrap();
        let node = fixture.pool.symbol(symbol).unwrap();
        let mut axis = fixture.axis(bounds.0, bounds.1, divisions);
        axis.symbol = symbol;
        axis.name = String::from(name);
        (axis, node)
    }

    #[test]
    fn vector_field_places_arrows_at_cell_centres() {
        let mut fixture = Fixture::new();
        let x_axis = fixture.axis(0, 2, 2);
        let (y_axis, y_node) = named_axis(&mut fixture, "y", (0, 2), 2);
        let shape = SampledShape::VectorField {
            x_component: y_node,
            y_component: fixture.x_node,
        };
        let request = fixture.request(shape, vec![x_axis, y_axis]);

        let scene = fixture.sample(&request).unwrap();

        let Primitive::Arrows(arrows) = only_primitive(&scene) else {
            panic!("expected arrows");
        };
        let columns = arrows.bases.iter().chain(&arrows.components);
        assert_eq!(
            columns.map(bits_f64).collect::<Vec<_>>(),
            vec![
                expected_bits(&[0.5, 1.5, 0.5, 1.5]),
                expected_bits(&[0.5, 0.5, 1.5, 1.5]),
                expected_bits(&[0.5, 0.5, 1.5, 1.5]),
                expected_bits(&[0.5, 1.5, 0.5, 1.5])
            ]
        );
    }

    #[test]
    fn complex_grid_holds_real_and_imaginary_parts_at_cell_centres() {
        let mut fixture = Fixture::new();
        let square = fixture.apply(Operator::Mul, &[fixture.x_node, fixture.x_node]);
        let real_axis = fixture.axis(0, 2, 2);
        let imaginary_axis = fixture.axis(0, 2, 2);
        let request = fixture.request(
            SampledShape::ComplexGrid(square),
            vec![real_axis, imaginary_axis],
        );

        let scene = fixture.sample(&request).unwrap();

        let Primitive::ComplexGrid(grid) = only_primitive(&scene) else {
            panic!("expected a complex grid");
        };
        assert_eq!(
            [bits_f64(&grid.real), bits_f64(&grid.imaginary)],
            [
                expected_bits(&[0.0, 2.0, -2.0, 0.0]),
                expected_bits(&[0.5, 1.5, 1.5, 4.5])
            ]
        );
    }

    #[test]
    fn complex_grid_is_coloured_around_its_rounded_median_modulus() {
        let mut fixture = Fixture::new();
        let real_axis = fixture.axis(0, 1, 1);
        let imaginary_axis = fixture.axis(0, 1, 1);
        let request = fixture.request(
            SampledShape::ComplexGrid(fixture.x_node),
            vec![real_axis, imaginary_axis],
        );

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            scene.frames[0].layers[0].style.colour_map,
            Some(ColourMapping {
                map: ColourMap::DomainColouring,
                range: Interval {
                    lower: Number::from(0_i64),
                    upper: fraction(4, 5)
                }
            })
        );
    }

    #[test]
    fn complex_axes_over_different_variables_are_rejected() {
        let mut fixture = Fixture::new();
        let real_axis = fixture.axis(0, 1, 1);
        let (imaginary_axis, _) = named_axis(&mut fixture, "y", (0, 1), 1);
        let request = fixture.request(
            SampledShape::ComplexGrid(fixture.x_node),
            vec![real_axis, imaginary_axis],
        );

        let result = fixture.sample(&request);

        assert_eq!(result, Err(SampleError::ComplexAxesDiffer));
    }

    fn legends(
        fixture: &mut Fixture,
        request: &SampleRequest,
    ) -> (Option<ColourLegend>, Option<ColourLegend>) {
        let scene = fixture.sample(request).unwrap();
        (
            shape_legend(&request.shape),
            layer_legend(&scene.frames[0].layers[0]),
        )
    }

    #[test]
    fn scalar_grid_legend_before_sampling_is_the_legend_of_its_layer() {
        let mut fixture = Fixture::new();
        let x_axis = fixture.axis(0, 1, 2);
        let (y_axis, _) = named_axis(&mut fixture, "y", (0, 1), 2);
        let request = fixture.request(
            SampledShape::ScalarGrid(fixture.x_node),
            vec![x_axis, y_axis],
        );

        let (before, after) = legends(&mut fixture, &request);

        assert_eq!(
            (before, after),
            (
                Some(ColourLegend::Sequential),
                Some(ColourLegend::Sequential)
            )
        );
    }

    #[test]
    fn complex_grid_legend_before_sampling_is_the_legend_of_its_layer() {
        let mut fixture = Fixture::new();
        let real_axis = fixture.axis(0, 2, 2);
        let imaginary_axis = fixture.axis(0, 2, 2);
        let request = fixture.request(
            SampledShape::ComplexGrid(fixture.x_node),
            vec![real_axis, imaginary_axis],
        );

        let (before, after) = legends(&mut fixture, &request);

        assert_eq!(
            (before, after),
            (
                Some(ColourLegend::DomainColouring),
                Some(ColourLegend::DomainColouring)
            )
        );
    }

    #[test]
    fn surface_legend_before_sampling_is_the_legend_of_its_layer() {
        let mut fixture = Fixture::new();
        let x_axis = fixture.axis(0, 1, 1);
        let (y_axis, y_node) = named_axis(&mut fixture, "y", (0, 1), 1);
        let sum = fixture.apply(Operator::Add, &[fixture.x_node, y_node]);
        let request = fixture.request(SampledShape::Surface(sum), vec![x_axis, y_axis]);

        let (before, after) = legends(&mut fixture, &request);

        assert_eq!(
            (before, after),
            (
                Some(ColourLegend::Sequential),
                Some(ColourLegend::Sequential)
            )
        );
    }

    #[test]
    fn occupancy_legend_before_sampling_is_the_legend_of_its_layer() {
        let mut fixture = Fixture::new();
        let request = occupancy_request(&mut fixture);

        let (before, after) = legends(&mut fixture, &request);

        assert_eq!(
            (before, after),
            (
                Some(ColourLegend::Sequential),
                Some(ColourLegend::Sequential)
            )
        );
    }

    #[test]
    fn escape_time_legend_before_sampling_is_the_legend_of_its_layer() {
        let mut fixture = Fixture::new();
        let request = escape_request(
            &mut fixture,
            EscapeTimeForm::QuadraticParameter,
            IterationLimitRule::Fixed { iterations: 8 },
        );

        let (before, after) = legends(&mut fixture, &request);

        assert_eq!(
            (before, after),
            (
                Some(ColourLegend::EscapeTime),
                Some(ColourLegend::EscapeTime)
            )
        );
    }

    #[test]
    fn curve_has_no_legend_before_or_after_sampling() {
        let mut fixture = Fixture::new();
        let request = fixture.square_curve(4);

        let (before, after) = legends(&mut fixture, &request);

        assert_eq!((before, after), (None, None));
    }

    #[test]
    fn surface_mesh_has_two_triangles_per_cell_over_partition_points() {
        let mut fixture = Fixture::new();
        let x_axis = fixture.axis(0, 1, 1);
        let (y_axis, y_node) = named_axis(&mut fixture, "y", (0, 1), 1);
        let sum = fixture.apply(Operator::Add, &[fixture.x_node, y_node]);
        let request = fixture.request(SampledShape::Surface(sum), vec![x_axis, y_axis]);

        let scene = fixture.sample(&request).unwrap();

        let Primitive::TriangleMesh(mesh) = only_primitive(&scene) else {
            panic!("expected a triangle mesh");
        };
        assert_eq!(mesh.triangles, vec![[0, 1, 3], [0, 3, 2]]);
        assert_eq!(
            mesh.vertices.iter().map(bits_f64).collect::<Vec<_>>(),
            vec![
                expected_bits(&[0.0, 1.0, 0.0, 1.0]),
                expected_bits(&[0.0, 0.0, 1.0, 1.0]),
                expected_bits(&[0.0, 1.0, 1.0, 2.0])
            ]
        );
    }

    #[test]
    fn surface_view_has_the_overview_camera() {
        let mut fixture = Fixture::new();
        let x_axis = fixture.axis(0, 1, 1);
        let (y_axis, y_node) = named_axis(&mut fixture, "y", (0, 1), 1);
        let sum = fixture.apply(Operator::Add, &[fixture.x_node, y_node]);
        let request = fixture.request(SampledShape::Surface(sum), vec![x_axis, y_axis]);

        let scene = fixture.sample(&request).unwrap();

        let View::View3(view) = &scene.views[0] else {
            panic!("expected a space view");
        };
        assert_eq!(
            (&view.camera.azimuth_degrees, &view.camera.elevation_degrees),
            (&Number::from(315_i64), &Number::from(30_i64))
        );
    }

    #[test]
    fn surface_with_more_vertices_than_u32_indices_is_rejected() {
        let mut fixture = Fixture::new();
        let x_axis = fixture.axis(0, 1, 70_000);
        let (y_axis, _) = named_axis(&mut fixture, "y", (0, 1), 70_000);
        let request = fixture.request(SampledShape::Surface(fixture.x_node), vec![x_axis, y_axis]);

        let result = fixture.sample(&request);

        assert_eq!(
            result,
            Err(SampleError::MeshTooLarge {
                vertices: 4_900_140_001
            })
        );
    }

    fn occupancy_request(fixture: &mut Fixture) -> SampleRequest {
        let one = fixture.integer(1);
        let inside = fixture.apply(Operator::Sub, &[one, fixture.x_node]);
        let x_axis = fixture.axis(0, 2, 2);
        let (y_axis, _) = named_axis(fixture, "y", (0, 1), 1);
        let (z_axis, _) = named_axis(fixture, "z", (0, 1), 1);
        fixture.request(
            SampledShape::Occupancy(inside),
            vec![x_axis, y_axis, z_axis],
        )
    }

    #[test]
    fn occupancy_keeps_cells_with_positive_values() {
        let mut fixture = Fixture::new();
        let request = occupancy_request(&mut fixture);

        let scene = fixture.sample(&request).unwrap();

        let Primitive::Voxels(voxels) = only_primitive(&scene) else {
            panic!("expected voxels");
        };
        assert_eq!(
            (&voxels.occupied, &voxels.scalar),
            (&vec![[0, 0, 0]], &Some(Column::F64(vec![0.5])))
        );
    }

    #[test]
    fn voxel_view_is_in_grid_units_centred_on_cells() {
        let mut fixture = Fixture::new();
        let request = occupancy_request(&mut fixture);

        let scene = fixture.sample(&request).unwrap();

        let View::View3(view) = &scene.views[0] else {
            panic!("expected a space view");
        };
        assert_eq!(
            view.x.range,
            Interval {
                lower: fraction(-1, 2),
                upper: fraction(3, 2)
            }
        );
    }

    fn exact_axis(fixture: &Fixture, lower: i64, upper: i64, divisions: u32) -> SampleAxis {
        SampleAxis {
            symbol: fixture.x,
            name: String::from("x"),
            bounds: AxisBounds::Exact(Interval {
                lower: Number::from(lower),
                upper: Number::from(upper),
            }),
            divisions,
            dimension: Dimension::DIMENSIONLESS,
            unit: AxisUnit::dimensionless(),
        }
    }

    fn sine_curve(fixture: &mut Fixture, axis: SampleAxis) -> SampleRequest {
        let sine = fixture.apply(Operator::Sin, &[fixture.x_node]);
        fixture.request(SampledShape::Curve(sine), vec![axis])
    }

    fn curve_values(scene: &Scene) -> Vec<u64> {
        let Primitive::Polyline(polyline) = only_primitive(scene) else {
            panic!("expected a polyline");
        };
        bits_f64(&polyline.coordinates[1])
    }

    #[test]
    fn view_with_exact_bounds_samples_the_same_bits_as_evaluated_bounds() {
        let mut fixture = Fixture::new();
        let evaluated_axis = fixture.axis(0, 4, 8);
        let evaluated = sine_curve(&mut fixture, evaluated_axis);
        let viewed_axis = exact_axis(&fixture, 0, 4, 8);
        let viewed = sine_curve(&mut fixture, viewed_axis);

        let first = fixture.sample(&evaluated).unwrap();
        let live = fixture.sample(&viewed).unwrap();

        assert_eq!(first, live);
    }

    #[test]
    fn zoomed_view_samples_equal_the_precomputed_samples_at_shared_positions() {
        let mut fixture = Fixture::new();
        let wide_axis = exact_axis(&fixture, 0, 4, 8);
        let wide = sine_curve(&mut fixture, wide_axis);
        let zoomed_axis = exact_axis(&fixture, 1, 2, 2);
        let zoomed = sine_curve(&mut fixture, zoomed_axis);

        let precomputed = curve_values(&fixture.sample(&wide).unwrap());
        let live = curve_values(&fixture.sample(&zoomed).unwrap());

        assert_eq!(live, precomputed[2..5].to_vec());
    }

    #[test]
    fn run_in_small_chunks_gives_the_same_scene_as_one_step() {
        let mut fixture = Fixture::new();
        let request_axis = exact_axis(&fixture, 0, 4, 10);
        let request = sine_curve(&mut fixture, request_axis);
        let whole = fixture.sample(&request).unwrap();
        let backend = CpuBackend::new();
        let mut run = prepare_sampling(&mut fixture.pool, &request).unwrap();

        let mut steps = 0;
        let chunked = loop {
            steps += 1;
            if let Some(scene) = run.step(&[&backend], 3).unwrap() {
                break scene;
            }
        };

        assert_eq!((chunked, steps), (whole, 4));
    }

    fn provisional_then_settled(
        fixture: &mut Fixture,
        request: &SampleRequest,
        chunk_length: usize,
    ) -> (Scene, Scene) {
        let backend = CpuBackend::new();
        let mut run = prepare_sampling(&mut fixture.pool, request).unwrap();
        let provisional = loop {
            if let Some(scene) = run.step_samples(&[&backend], chunk_length).unwrap() {
                break scene;
            }
        };
        let settled = loop {
            if let Some(scene) = run.step_bounds(chunk_length).unwrap() {
                break scene;
            }
        };
        (provisional, settled)
    }

    #[test]
    fn sample_stage_delivers_a_provisional_scene_without_bounds() {
        let mut fixture = Fixture::new();
        let request_axis = exact_axis(&fixture, 0, 4, 10);
        let request = sine_curve(&mut fixture, request_axis);

        let (provisional, _) = provisional_then_settled(&mut fixture, &request, 3);

        let layer = &provisional.frames[0].layers[0];
        assert_eq!(
            (
                layer.value_bounds.len(),
                layer.precision.clone(),
                layer.style.emphasis
            ),
            (0, None, Emphasis::Provisional)
        );
    }

    #[test]
    fn provisional_scene_has_the_samples_of_one_full_run() {
        let mut fixture = Fixture::new();
        let request_axis = exact_axis(&fixture, 0, 4, 10);
        let request = sine_curve(&mut fixture, request_axis);
        let whole = fixture.sample(&request).unwrap();

        let (provisional, _) = provisional_then_settled(&mut fixture, &request, 3);

        assert_eq!(curve_values(&provisional), curve_values(&whole));
    }

    #[test]
    fn bound_stage_delivers_the_scene_of_one_full_run() {
        let mut fixture = Fixture::new();
        let request_axis = exact_axis(&fixture, 0, 4, 10);
        let request = sine_curve(&mut fixture, request_axis);
        let whole = fixture.sample(&request).unwrap();

        let (_, settled) = provisional_then_settled(&mut fixture, &request, 3);

        assert_eq!(settled, whole);
    }

    #[test]
    fn bound_stage_before_the_samples_finish_is_an_error() {
        let mut fixture = Fixture::new();
        let request = fixture.square_curve(8);
        let backend = CpuBackend::new();
        let mut run = prepare_sampling(&mut fixture.pool, &request).unwrap();
        run.step_samples(&[&backend], 2).unwrap();

        let result = run.step_bounds(2);

        assert_eq!(result.err(), Some(SampleError::SamplesPending));
    }

    #[test]
    fn combined_step_after_the_sample_stage_is_an_error() {
        let mut fixture = Fixture::new();
        let request = fixture.square_curve(8);
        let backend = CpuBackend::new();
        let mut run = prepare_sampling(&mut fixture.pool, &request).unwrap();
        run.step_samples(&[&backend], 2).unwrap();

        let result = run.step(&[&backend], 2);

        assert_eq!(result.err(), Some(SampleError::StageOrder));
    }

    #[test]
    fn sample_stage_after_the_provisional_scene_is_an_error() {
        let mut fixture = Fixture::new();
        let request = fixture.square_curve(2);
        let backend = CpuBackend::new();
        let mut run = prepare_sampling(&mut fixture.pool, &request).unwrap();
        run.step_samples(&[&backend], usize::MAX).unwrap();

        let result = run.step_samples(&[&backend], usize::MAX);

        assert_eq!(result.err(), Some(SampleError::RunFinished));
    }

    #[test]
    fn step_after_the_scene_was_delivered_is_an_error() {
        let mut fixture = Fixture::new();
        let request = fixture.square_curve(2);
        let backend = CpuBackend::new();
        let mut run = prepare_sampling(&mut fixture.pool, &request).unwrap();
        run.step(&[&backend], usize::MAX).unwrap();

        let result = run.step(&[&backend], usize::MAX);

        assert_eq!(result.err(), Some(SampleError::RunFinished));
    }

    #[test]
    fn machine_number_in_exact_view_bounds_is_rejected() {
        let mut fixture = Fixture::new();
        let mut axis = exact_axis(&fixture, 0, 1, 2);
        axis.bounds = AxisBounds::Exact(Interval {
            lower: Number::from(0_i64),
            upper: Number::F64(1.0),
        });
        let request = fixture.request(SampledShape::Curve(fixture.x_node), vec![axis]);

        let result = fixture.sample(&request);

        assert_eq!(result, Err(SampleError::BoundNotExact { axis: 0 }));
    }

    #[test]
    fn sampled_view_axes_carry_the_divisions_of_the_request() {
        let mut fixture = Fixture::new();
        let mut request = fixture.square_curve(4);
        request.value_divisions = 240;

        let scene = fixture.sample(&request).unwrap();

        let view = first_view(&scene);
        assert_eq!((view.x.divisions, view.y.divisions), (4, 240));
    }

    #[test]
    fn sampled_layer_offers_a_reading_over_its_variables() {
        let mut fixture = Fixture::new();
        let request = fixture.square_curve(2);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            scene.frames[0].layers[0].readings,
            vec![Reading::ValueAt {
                variables: vec![String::from("x")]
            }]
        );
    }

    #[test]
    fn sampled_layer_is_drawn_with_normal_emphasis() {
        let mut fixture = Fixture::new();
        let request = fixture.square_curve(2);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(scene.frames[0].layers[0].style.emphasis, Emphasis::Normal);
    }

    fn rational_axis(
        fixture: &Fixture,
        lower: Number,
        upper: Number,
        divisions: u32,
    ) -> SampleAxis {
        SampleAxis {
            symbol: fixture.x,
            name: String::from("x"),
            bounds: AxisBounds::Exact(Interval { lower, upper }),
            divisions,
            dimension: Dimension::DIMENSIONLESS,
            unit: AxisUnit::dimensionless(),
        }
    }

    fn cancellation_curve(fixture: &mut Fixture, lower: Number, upper: Number) -> SampleRequest {
        let one = fixture.integer(1);
        let cosine = fixture.apply(Operator::Cos, &[fixture.x_node]);
        let numerator = fixture.apply(Operator::Sub, &[one, cosine]);
        let square = fixture.apply(Operator::Mul, &[fixture.x_node, fixture.x_node]);
        let quotient = fixture.apply(Operator::Div, &[numerator, square]);
        let axis = rational_axis(fixture, lower, upper, 10);
        let mut request = fixture.request(SampledShape::Curve(quotient), vec![axis]);
        request.value_range = Some(Interval {
            lower: fraction(2, 5),
            upper: fraction(3, 5),
        });
        request.value_divisions = 1000;
        request
    }

    fn layer_precision_of(scene: &Scene) -> Option<PrecisionLimit> {
        scene.frames[0].layers[0].precision.clone()
    }

    fn columns_of(scene: &Scene) -> ColumnEnclosures {
        scene.frames[0].layers[0].columns.clone().unwrap()
    }

    fn shown_values(scene: &Scene) -> Vec<f64> {
        let Primitive::Polyline(polyline) = &scene.frames[0].layers[0].primitive else {
            panic!("expected a polyline");
        };
        column_f64(polyline.coordinates.last())
    }

    #[test]
    fn a_cancelling_polynomial_is_drawn_from_its_exact_values() {
        let mut fixture = Fixture::new();
        let one = fixture.integer(1);
        let sum = fixture.apply(Operator::Add, &[fixture.x_node, one]);
        let line = fixture.apply(Operator::Sub, &[sum, one]);
        let axis = rational_axis(
            &fixture,
            fraction(1, 100_000_000_000_000_000),
            fraction(2, 100_000_000_000_000_000),
            4,
        );
        let request = fixture.request(SampledShape::Curve(line), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            shown_values(&scene),
            vec![1.0e-17, 1.25e-17, 1.5e-17, 1.75e-17, 2.0e-17]
        );
        assert_eq!(layer_precision_of(&scene), None);
    }

    #[test]
    fn a_sine_faster_than_its_columns_is_a_band_from_minus_one_to_one() {
        let mut fixture = Fixture::new();
        let thousand = fixture.integer(1000);
        let argument = fixture.apply(Operator::Mul, &[thousand, fixture.x_node]);
        let sine = fixture.apply(Operator::Sin, &[argument]);
        let axis = fixture.axis(0, 100, 4);
        let request = fixture.request(SampledShape::Curve(sine), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        let columns = columns_of(&scene);
        assert!(columns.lower.iter().all(|lower| *lower == -1.0));
        assert!(columns.upper.iter().all(|upper| *upper == 1.0));
    }

    #[test]
    fn a_column_between_unresolved_samples_is_marked_and_counted() {
        let mut fixture = Fixture::new();
        let request = cancellation_curve(
            &mut fixture,
            fraction(1, 10_000_000),
            fraction(2, 10_000_000),
        );

        let scene = fixture.sample(&request).unwrap();

        assert!(columns_of(&scene).marked.iter().all(|marked| *marked));
        assert_eq!(
            layer_precision_of(&scene).map(|limit| limit.marked_columns),
            Some(10)
        );
    }

    #[test]
    fn a_pole_leaves_its_column_without_an_enclosure() {
        let mut fixture = Fixture::new();
        let one = fixture.integer(1);
        let reciprocal = fixture.apply(Operator::Div, &[one, fixture.x_node]);
        let axis = fixture.axis(-1, 1, 2);
        let request = fixture.request(SampledShape::Curve(reciprocal), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        let columns = columns_of(&scene);
        assert!(columns.lower.iter().all(|lower| lower.is_nan()));
        assert!(!columns.marked.iter().any(|marked| *marked));
    }

    #[test]
    fn a_constant_sampled_below_its_bounds_is_noted() {
        let mut fixture = Fixture::new();
        let sine = fixture.apply(Operator::Sin, &[fixture.x_node]);
        let cosine = fixture.apply(Operator::Cos, &[fixture.x_node]);
        let sine_square = fixture.apply(Operator::Mul, &[sine, sine]);
        let cosine_square = fixture.apply(Operator::Mul, &[cosine, cosine]);
        let one = fixture.apply(Operator::Add, &[sine_square, cosine_square]);
        let axis = fixture.axis(0, 10, 20);
        let request = fixture.request(SampledShape::Curve(one), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            layer_precision_of(&scene).map(|limit| limit.varies_below_bounds),
            Some(true)
        );
    }

    #[test]
    fn exact_samples_have_bounds_of_at_most_one_rounding_and_no_precision_limit() {
        let mut fixture = Fixture::new();
        let request = fixture.square_curve(2);

        let scene = fixture.sample(&request).unwrap();

        let layer = &scene.frames[0].layers[0];
        let Column::F64(bounds) = &layer.value_bounds[0] else {
            panic!("expected f64 bounds");
        };
        assert!(bounds.len() == 3 && bounds.iter().all(|bound| *bound <= 2.0 * f64::EPSILON));
        assert_eq!(layer.precision, None);
    }

    #[test]
    fn cancellation_near_zero_is_beyond_the_value_limit() {
        let mut fixture = Fixture::new();
        let request = cancellation_curve(
            &mut fixture,
            fraction(1, 10_000_000),
            fraction(2, 10_000_000),
        );

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            layer_precision_of(&scene).map(|limit| limit.unresolved_samples),
            Some(11)
        );
    }

    #[test]
    fn cancellation_far_enough_from_zero_is_resolved() {
        let mut fixture = Fixture::new();
        let request = cancellation_curve(&mut fixture, fraction(1, 100_000), fraction(2, 100_000));

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(layer_precision_of(&scene), None);
    }

    #[test]
    fn view_finer_than_the_machine_grid_exhausts_the_axis() {
        let mut fixture = Fixture::new();
        let tiny = Number::fraction(&Integer::one(), &Integer::from(2_i64).pow(50)).unwrap();
        let upper = Number::from(1_i64).add_exact(&tiny).unwrap();
        let axis = rational_axis(&fixture, Number::from(1_i64), upper, 1000);
        let mut request = fixture.request(SampledShape::Curve(fixture.x_node), vec![axis]);
        request.sample_limit = 10_000;

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            layer_precision_of(&scene).map(|limit| limit.grid_exhausted),
            Some(vec![0])
        );
    }

    #[test]
    fn view_coarser_than_the_machine_grid_does_not_exhaust_the_axis() {
        let mut fixture = Fixture::new();
        let axis = rational_axis(&fixture, Number::from(1_i64), Number::from(2_i64), 1000);
        let mut request = fixture.request(SampledShape::Curve(fixture.x_node), vec![axis]);
        request.sample_limit = 10_000;

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(layer_precision_of(&scene), None);
    }

    #[test]
    fn samples_without_an_enclosure_have_unknown_bounds() {
        let mut fixture = Fixture::new();
        let logarithm = fixture.apply(Operator::Ln, &[fixture.x_node]);
        let axis = fixture.axis(-2, -1, 2);
        let request = fixture.request(SampledShape::Curve(logarithm), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            layer_precision_of(&scene).map(|limit| limit.unknown_bounds),
            Some(3)
        );
    }

    #[test]
    fn complex_grid_has_one_bounds_column_per_part() {
        let mut fixture = Fixture::new();
        let square = fixture.apply(Operator::Mul, &[fixture.x_node, fixture.x_node]);
        let real_axis = fixture.axis(0, 2, 2);
        let imaginary_axis = fixture.axis(0, 2, 2);
        let request = fixture.request(
            SampledShape::ComplexGrid(square),
            vec![real_axis, imaginary_axis],
        );

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(scene.frames[0].layers[0].value_bounds.len(), 2);
    }

    #[test]
    fn node_that_cannot_be_detached_is_an_expression_error() {
        let mut fixture = Fixture::new();
        let one = fixture.integer(1);

        let error = detach_error(NotDetachable::Node(one));

        assert_eq!(error, SampleError::ExpressionNotDetachable(one));
    }

    #[test]
    fn axis_symbol_of_another_pool_is_not_detachable() {
        let mut fixture = Fixture::new();
        let mut other = ExprPool::new();
        for name in ["p", "q", "r", "s", "t"] {
            other.intern_symbol(name, SymbolKind::Variable).unwrap();
        }
        let foreign = other.intern_symbol("u", SymbolKind::Variable).unwrap();
        let mut axis = fixture.axis(0, 1, 2);
        axis.symbol = foreign;
        let request = fixture.request(SampledShape::Curve(fixture.x_node), vec![axis]);

        let result = fixture.sample(&request);

        assert_eq!(result, Err(SampleError::SymbolNotDetachable(foreign)));
    }

    fn escape_request(
        fixture: &mut Fixture,
        form: EscapeTimeForm,
        limit_rule: IterationLimitRule,
    ) -> SampleRequest {
        let real_axis = fixture.axis(-2, 2, 4);
        let mut imaginary_axis = fixture.axis(0, 1, 1);
        imaginary_axis.bounds = AxisBounds::Exact(Interval {
            lower: fraction(-1, 2),
            upper: fraction(1, 2),
        });
        let mut request =
            fixture.request(SampledShape::EscapeTime, vec![real_axis, imaginary_axis]);
        request.escape_time = Some(EscapeTimeRequest { form, limit_rule });
        request
    }

    fn escape_grid(scene: &Scene) -> &ScalarGrid {
        let Primitive::ScalarGrid(grid) = only_primitive(scene) else {
            panic!("expected a scalar grid");
        };
        grid
    }

    #[test]
    fn escape_time_classifies_undecided_inside_and_escaped_cells() {
        let mut fixture = Fixture::new();
        let request = escape_request(
            &mut fixture,
            EscapeTimeForm::QuadraticParameter,
            IterationLimitRule::Fixed { iterations: 32 },
        );

        let scene = fixture.sample(&request).unwrap();

        let grid = escape_grid(&scene);
        let Column::F64(counts) = &grid.scalar else {
            panic!("expected f64 counts");
        };
        assert_eq!(
            grid.classes,
            Some(vec![
                UNDECIDED_CELL,
                INSIDE_CELL,
                ESCAPED_CELL,
                ESCAPED_CELL
            ])
        );
        assert!(counts[0].is_nan() && counts[1].is_nan());
        assert_eq!((counts[2], counts[3]), (5.0, 2.0));
    }

    #[test]
    fn escaped_counts_equal_those_of_the_unrolled_circuit() {
        let mut fixture = Fixture::new();
        let request = escape_request(
            &mut fixture,
            EscapeTimeForm::QuadraticParameter,
            IterationLimitRule::Fixed { iterations: 32 },
        );
        let unrolled =
            calc_exec::escape_time_plan(Domain::F64, calc_exec::EscapeTimePlanForm::Parameter, 32)
                .unwrap();
        let backend = CpuBackend::new();
        let inputs =
            Batch::from_f64_columns(4, vec![vec![-1.5, -0.5, 0.5, 1.5], vec![0.0; 4]]).unwrap();
        let mut outputs = Batch::zeroed(Domain::F64, 2, 4);
        backend
            .prepare(&unrolled)
            .unwrap()
            .run(&inputs, &mut outputs)
            .unwrap();

        let scene = fixture.sample(&request).unwrap();

        let Column::F64(counts) = &escape_grid(&scene).scalar else {
            panic!("expected f64 counts");
        };
        let Column::F64(unrolled_counts) = output_column(&outputs, 0) else {
            panic!("expected f64 outputs");
        };
        assert_eq!(
            (counts[2].to_bits(), counts[3].to_bits()),
            (unrolled_counts[2].to_bits(), unrolled_counts[3].to_bits())
        );
    }

    #[test]
    fn escape_time_diagnostics_count_each_class() {
        let mut fixture = Fixture::new();
        let request = escape_request(
            &mut fixture,
            EscapeTimeForm::QuadraticParameter,
            IterationLimitRule::Fixed { iterations: 32 },
        );

        let scene = fixture.sample(&request).unwrap();

        let diagnostics = scene.record.diagnostics;
        assert_eq!(
            (
                diagnostics.escaped_cells,
                diagnostics.inside_cells,
                diagnostics.undecided_cells,
                diagnostics.nan_samples
            ),
            (2, 1, 1, 0)
        );
    }

    #[test]
    fn escape_time_record_states_form_rule_and_iterations_used() {
        let mut fixture = Fixture::new();
        let request = escape_request(
            &mut fixture,
            EscapeTimeForm::QuadraticParameter,
            IterationLimitRule::DEFAULT,
        );

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            scene.record.method,
            SamplingMethod::EscapeTime {
                form: EscapeTimeForm::QuadraticParameter,
                limit_rule: IterationLimitRule::DEFAULT,
                iterations_used: 256
            }
        );
    }

    #[test]
    fn filled_julia_set_has_no_proven_inside_cells() {
        let mut fixture = Fixture::new();
        let form = EscapeTimeForm::QuadraticInitial {
            c_real: Number::from(-1_i64),
            c_imaginary: Number::from(0_i64),
        };
        let request = escape_request(
            &mut fixture,
            form,
            IterationLimitRule::Fixed { iterations: 16 },
        );

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(scene.record.diagnostics.inside_cells, 0);
    }

    #[test]
    fn escape_counts_carry_unknown_bounds_and_are_never_unresolved() {
        let mut fixture = Fixture::new();
        let request = escape_request(
            &mut fixture,
            EscapeTimeForm::QuadraticParameter,
            IterationLimitRule::Fixed { iterations: 8 },
        );

        let scene = fixture.sample(&request).unwrap();

        let precision = scene.frames[0].layers[0].precision.clone().unwrap();
        assert_eq!(
            (precision.unknown_bounds, precision.unresolved_samples),
            (4, 0)
        );
    }

    #[test]
    fn escape_time_without_its_settings_is_rejected() {
        let mut fixture = Fixture::new();
        let mut request = escape_request(
            &mut fixture,
            EscapeTimeForm::QuadraticParameter,
            IterationLimitRule::DEFAULT,
        );
        request.escape_time = None;

        let result = fixture.sample(&request);

        assert_eq!(result, Err(SampleError::EscapeTimeSettingsMissing));
    }

    #[test]
    fn iteration_limit_above_the_largest_is_rejected() {
        let mut fixture = Fixture::new();
        let request = escape_request(
            &mut fixture,
            EscapeTimeForm::QuadraticParameter,
            IterationLimitRule::Fixed { iterations: 70_000 },
        );

        let result = fixture.sample(&request);

        assert_eq!(
            result,
            Err(SampleError::IterationLimitTooLarge { iterations: 70_000 })
        );
    }

    #[test]
    fn escape_time_run_in_two_stages_settles_with_unknown_bounds_for_every_cell() {
        let mut fixture = Fixture::new();
        let request = escape_request(
            &mut fixture,
            EscapeTimeForm::QuadraticParameter,
            IterationLimitRule::Fixed { iterations: 8 },
        );
        let whole = fixture.sample(&request).unwrap();

        let (provisional, settled) = provisional_then_settled(&mut fixture, &request, 3);

        let provisional_layer = &provisional.frames[0].layers[0];
        let settled_layer = &settled.frames[0].layers[0];
        assert_eq!(
            (
                provisional_layer.value_bounds.len(),
                provisional_layer.style.emphasis
            ),
            (0, Emphasis::Provisional)
        );
        assert_eq!(
            settled_layer.value_bounds,
            vec![Column::F64(vec![f64::NAN; 4])]
        );
        assert_eq!(settled, whole);
    }

    fn unit_axis(
        fixture: &Fixture,
        lower: i64,
        upper: i64,
        divisions: u32,
        unit: AxisUnit,
    ) -> SampleAxis {
        let mut axis = exact_axis(fixture, lower, upper, divisions);
        axis.unit = unit;
        axis
    }

    fn degree() -> AxisUnit {
        AxisUnit::Unit {
            factor: fraction(1, 180),
            pi_exponent: 1,
            symbol: String::from("°"),
        }
    }

    fn celsius() -> AxisUnit {
        AxisUnit::TemperatureScale {
            factor: Number::from(1_i64),
            offset: fraction(5463, 20),
            symbol: String::from("°C"),
        }
    }

    fn polyline_columns(scene: &Scene) -> (Vec<u64>, Vec<u64>) {
        let Primitive::Polyline(polyline) = only_primitive(scene) else {
            panic!("expected a polyline");
        };
        (
            bits_f64(&polyline.coordinates[0]),
            bits_f64(&polyline.coordinates[1]),
        )
    }

    #[test]
    fn degree_axis_is_sampled_in_degrees_and_converted_in_the_plan() {
        let mut fixture = Fixture::new();
        let sine = fixture.apply(Operator::Sin, &[fixture.x_node]);
        let axis = unit_axis(&fixture, 0, 90, 2, degree());
        let request = fixture.request(SampledShape::Curve(sine), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        let scale = (1.0_f64 / 180.0) * calc_numbers::pow_f64(std::f64::consts::PI, 1.0);
        let expected = [0.0_f64, 45.0, 90.0]
            .map(|shown| calc_numbers::sin_f64(shown * scale).to_bits())
            .to_vec();
        assert_eq!(
            polyline_columns(&scene),
            (expected_bits(&[0.0, 45.0, 90.0]), expected)
        );
    }

    #[test]
    fn sampled_view_axis_carries_its_unit() {
        let mut fixture = Fixture::new();
        let axis = unit_axis(&fixture, 0, 90, 2, degree());
        let request = fixture.request(SampledShape::Curve(fixture.x_node), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(first_view(&scene).x.unit, degree());
    }

    #[test]
    fn celsius_input_axis_converts_readings_to_kelvin() {
        let mut fixture = Fixture::new();
        let mut axis = unit_axis(&fixture, 0, 100, 1, celsius());
        axis.dimension = Dimension::TEMPERATURE;
        let request = fixture.request(SampledShape::Curve(fixture.x_node), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        let offset = 5463.0_f64 / 20.0;
        assert_eq!(
            polyline_columns(&scene).1,
            vec![
                (1.0 * 0.0 + offset).to_bits(),
                (1.0 * 100.0 + offset).to_bits()
            ]
        );
    }

    #[test]
    fn celsius_value_axis_shows_kelvin_values_as_readings() {
        let mut fixture = Fixture::new();
        let axis = rational_axis(&fixture, fraction(5463, 20), fraction(7463, 20), 1);
        let kelvin = times_unit(&mut fixture, "K");
        let mut request = fixture.request(SampledShape::Curve(kelvin), vec![axis]);
        request.value_dimension = Dimension::TEMPERATURE;
        request.value_unit = celsius();

        let scene = fixture.sample(&request).unwrap();

        let offset = 5463.0_f64 / 20.0;
        let upper = 7463.0_f64 / 20.0;
        assert_eq!(
            polyline_columns(&scene).1,
            vec![
                ((offset - offset) / 1.0).to_bits(),
                ((upper - offset) / 1.0).to_bits()
            ]
        );
    }

    #[test]
    fn positions_merged_by_the_conversion_exhaust_the_grid() {
        let mut fixture = Fixture::new();
        let tiny = Number::fraction(&Integer::one(), &Integer::from(2_i64).pow(40)).unwrap();
        let upper = Number::from(1_i64).add_exact(&tiny).unwrap();
        let mut axis = rational_axis(&fixture, Number::from(1_i64), upper, 10);
        axis.unit = AxisUnit::Unit {
            factor: Number::fraction(&Integer::one(), &Integer::from(2_i64).pow(1070)).unwrap(),
            pi_exponent: 0,
            symbol: String::from("q"),
        };
        let request = fixture.request(SampledShape::Curve(fixture.x_node), vec![axis]);

        let scene = fixture.sample(&request).unwrap();

        assert_eq!(
            layer_precision_of(&scene).map(|limit| limit.grid_exhausted),
            Some(vec![0])
        );
    }

    #[test]
    fn vector_field_over_a_unit_axis_is_not_supported() {
        let mut fixture = Fixture::new();
        let x_axis = unit_axis(&fixture, 0, 90, 2, degree());
        let (y_axis, y_node) = named_axis(&mut fixture, "y", (0, 1), 2);
        let shape = SampledShape::VectorField {
            x_component: y_node,
            y_component: fixture.x_node,
        };
        let request = fixture.request(shape, vec![x_axis, y_axis]);

        let result = fixture.sample(&request);

        assert_eq!(result, Err(SampleError::AxisUnitNotSupported { axis: 0 }));
    }

    #[test]
    fn conversion_of_an_expression_outside_the_pool_is_not_built() {
        let mut fixture = Fixture::new();
        let mut other = ExprPool::new();
        let mut foreign = other.number(Number::from(0_i64)).unwrap();
        for value in 1..40 {
            foreign = other.number(Number::from(value)).unwrap();
        }
        let axis = unit_axis(&fixture, 0, 90, 2, degree());
        let request = fixture.request(SampledShape::Curve(foreign), vec![axis]);

        let result = fixture.sample(&request);

        assert_eq!(result, Err(SampleError::ConversionNotBuilt(foreign)));
    }
}
