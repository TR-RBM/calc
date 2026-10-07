mod axis_unit;
mod colour;
mod columns;
mod default_view;
mod detached;
mod direct;
mod escape_time;
mod exact_order;
mod figure;
mod figure_check;
mod figure_layout;
mod legend;
mod orbit;
mod parameter;
mod precision;
mod primitive;
mod record;
mod refinement;
mod sampling;
mod scene;
mod style;
mod view;

pub use calc_core::Seed;
pub use calc_exec::{BackendKind, Domain};
pub use colour::{Colour, categorical_colour, diverging_colour, domain_colour, sequential_colour};
pub use direct::{
    BitLayoutRequest, DirectPictureError, FormulaRequest, GraphArrows, GraphRequest,
    bit_layout_scene, formula_scene, graph_scene,
};
pub use escape_time::{
    ESCAPED_CELL, EscapeTimeForm, INSIDE_CELL, IterationLimitRule, UNDECIDED_CELL,
    default_view as escape_time_default_view,
};
pub use figure::{
    Angle, AngleIndex, CoherentValue, Designation, ElementIndex, Figure, FigureScale, Label,
    LabelValue, Mark, MarkKind, NameKind, PointIndex, Quantity, RightAngleMark, Segment,
    SegmentIndex, Stroke,
};
pub use figure_check::{
    FigureFault, FigureNotation, FigureTask, NameTriple, TaskName, VertexOrder, check_figure,
};
pub use figure_layout::{
    ArcLayout, DotLayout, FigureLayout, LabelLayout, LayoutPoint, StrokeLayout, TickLayout,
    check_label_layout,
};
pub use legend::{ColourLegend, layer_legend, shape_legend};
pub use orbit::{Orbit, OrbitRequest, escape_time_limit, read_orbit};
pub use parameter::{FreeParameter, ParameterRole};
pub use primitive::{
    Arrows, Band, BitLayout, CellGrid, Column, ComplexGrid, Formula, Graph, Points, Polyline,
    Primitive, ScalarGrid, TriangleMesh, Voxels,
};
pub use record::{
    AdaptiveSpacing, AxisResolution, CorpusReference, IntegratorMethod, Interval, LayeringMethod,
    Resolution, ResultId, SamplingDiagnostics, SamplingMethod, SceneInput, SceneRecord,
};
pub use refinement::{
    RefinementPlan, RefinementRun, RefinementStep, level_divisions, plan_refinement,
    prepare_refinement,
};
pub use sampling::{
    AxisBounds, EscapeTimeRequest, SampleAxis, SampleError, SamplePlan, SampleRequest,
    SampledShape, SamplingRun, plan_sampling, prepare_sampling, sample,
};
pub use scene::{
    ColumnEnclosures, Frame, InputIndex, Layer, LayerPosition, Location, PrecisionLimit, Reading,
    SCENE_FORMAT_VERSION, Scene, SceneError, ViewIndex,
};
pub use style::{ColourMap, ColourMapping, Emphasis, KindColour, LinePattern, Marker, StyleRole};
pub use view::{AxisUnit, Camera, Dimension, Projection, Scale, View, View2, View3, ViewAxis};
