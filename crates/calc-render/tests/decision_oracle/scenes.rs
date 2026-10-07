use calc_numbers::{Integer, Number};
use calc_viz::{
    AxisUnit, BackendKind, CellGrid, ColourMap, ColourMapping, Column, Dimension, Domain, Emphasis,
    Frame, InputIndex, Interval, KindColour, Layer, LinePattern, Marker, Polyline, Primitive,
    Resolution, ResultId, SamplingDiagnostics, SamplingMethod, ScalarGrid, Scale, Scene,
    SceneInput, SceneRecord, StyleRole, View, View2, ViewAxis, ViewIndex,
};

pub const DIVISIONS: u32 = 300;
const INPUT_TEXT: &str = "x";
const INPUT_VARIABLE: &str = "x";

pub fn integer(value: i64) -> Number {
    Number::from(value)
}

pub fn fraction(numerator: i64, denominator: i64) -> Number {
    Number::fraction(&Integer::from(numerator), &Integer::from(denominator))
        .expect("denominators in scenes are not zero")
}

pub fn interval(lower: Number, upper: Number) -> Interval {
    Interval { lower, upper }
}

pub fn unit_interval() -> Interval {
    interval(integer(0), integer(1))
}

pub fn axis(range: Interval) -> ViewAxis {
    ViewAxis {
        range,
        scale: Scale::Linear,
        dimension: Dimension::DIMENSIONLESS,
        unit: AxisUnit::Coherent {
            symbol: String::new(),
        },
        divisions: DIVISIONS,
    }
}

pub fn plane_view(x: Interval, y: Interval) -> View {
    View::View2(Box::new(View2 {
        x: axis(x),
        y: axis(y),
    }))
}

pub fn line_style() -> StyleRole {
    StyleRole {
        kind: KindColour::Sampled,
        colour_map: None,
        line: LinePattern::Solid,
        marker: Marker::None,
        emphasis: Emphasis::Normal,
    }
}

pub fn colour_map_style(map: ColourMap, range: Interval) -> StyleRole {
    StyleRole {
        colour_map: Some(ColourMapping { map, range }),
        line: LinePattern::None,
        ..line_style()
    }
}

pub fn zero_bounds(length: usize) -> Column {
    Column::F64(vec![0.0; length])
}

pub fn layer(primitive: Primitive, style: StyleRole, value_bounds: Vec<Column>) -> Layer {
    Layer {
        view: ViewIndex(0),
        input: InputIndex(0),
        primitive,
        style,
        value_bounds,
        precision: None,
        columns: None,
        readings: Vec::new(),
    }
}

fn direct_record() -> SceneRecord {
    SceneRecord {
        result: ResultId(0),
        inputs: vec![SceneInput {
            expression: INPUT_TEXT.to_string(),
            sampling_box: vec![unit_interval()],
            variables: vec![INPUT_VARIABLE.to_string()],
        }],
        method: SamplingMethod::Direct,
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

pub fn scene_with(views: Vec<View>, layers: Vec<Layer>) -> Scene {
    Scene::new(
        direct_record(),
        Vec::new(),
        views,
        vec![Frame {
            parameter_values: Vec::new(),
            layers,
        }],
    )
    .expect("test scenes are valid scenes")
}

pub fn unit_scene(layers: Vec<Layer>) -> Scene {
    scene_with(vec![plane_view(unit_interval(), unit_interval())], layers)
}

pub fn without_layers(scene: &Scene) -> Scene {
    let mut empty = scene.clone();
    for frame in &mut empty.frames {
        frame.layers.clear();
    }
    empty
}

pub fn polyline_layer(x: Vec<f64>, y: Vec<f64>) -> Layer {
    let length = y.len();
    layer(
        Primitive::Polyline(Polyline {
            coordinates: vec![Column::F64(x), Column::F64(y)],
        }),
        line_style(),
        vec![zero_bounds(length)],
    )
}

pub fn scalar_grid_layer(counts: [u32; 2], values: Vec<f64>, map: ColourMap) -> Layer {
    let length = values.len();
    layer(
        Primitive::ScalarGrid(ScalarGrid {
            cells: CellGrid {
                region: vec![unit_interval(), unit_interval()],
                counts: counts.to_vec(),
            },
            scalar: Column::F64(values),
            classes: None,
        }),
        colour_map_style(map, unit_interval()),
        vec![zero_bounds(length)],
    )
}
