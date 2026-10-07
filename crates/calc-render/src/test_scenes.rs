use calc_numbers::Number;
use calc_viz::{
    AxisUnit, BackendKind, Camera, ColourMap, ColourMapping, Column, Dimension, Domain, Emphasis,
    Frame, InputIndex, Interval, KindColour, Layer, LinePattern, Marker, Primitive, Projection,
    Resolution, ResultId, SamplingDiagnostics, SamplingMethod, Scale, Scene, SceneInput,
    SceneRecord, StyleRole, View, View2, View3, ViewAxis, ViewIndex,
};

use crate::geometry::ScaleFactor;
use crate::picture_text::PictureText;
use crate::render::{RenderError, RenderRequest, Rendered, render_scene};
use crate::text::font::FontSet;
use crate::theme::Theme;

pub(crate) const WIDTH: u16 = 400;
pub(crate) const HEIGHT: u16 = 300;

pub(crate) fn interval(lower: i64, upper: i64) -> Interval {
    Interval {
        lower: Number::from(lower),
        upper: Number::from(upper),
    }
}

pub(crate) fn axis(lower: i64, upper: i64) -> ViewAxis {
    ViewAxis {
        range: interval(lower, upper),
        scale: Scale::Linear,
        dimension: Dimension::DIMENSIONLESS,
        unit: AxisUnit::dimensionless(),
        divisions: 200,
    }
}

pub(crate) fn plane(lower: i64, upper: i64) -> View {
    View::View2(Box::new(View2 {
        x: axis(lower, upper),
        y: axis(lower, upper),
    }))
}

pub(crate) fn metre_plane(lower: i64, upper: i64) -> View {
    let metres = |lower, upper| ViewAxis {
        dimension: Dimension {
            exponents: [1, 0, 0, 0, 0, 0, 0, 0],
        },
        unit: AxisUnit::Coherent {
            symbol: String::from("m"),
        },
        ..axis(lower, upper)
    };
    View::View2(Box::new(View2 {
        x: metres(lower, upper),
        y: metres(lower, upper),
    }))
}

pub(crate) fn metre_space(lower: i64, upper: i64, azimuth: i64, elevation: i64) -> View {
    let metres = |lower, upper| ViewAxis {
        dimension: Dimension {
            exponents: [1, 0, 0, 0, 0, 0, 0, 0],
        },
        unit: AxisUnit::Coherent {
            symbol: String::from("m"),
        },
        ..axis(lower, upper)
    };
    View::View3(Box::new(View3 {
        x: metres(lower, upper),
        y: metres(lower, upper),
        z: metres(lower, upper),
        camera: Camera {
            azimuth_degrees: Number::from(azimuth),
            elevation_degrees: Number::from(elevation),
            projection: Projection::Orthographic,
        },
    }))
}

pub(crate) fn space(lower: i64, upper: i64, azimuth: i64, elevation: i64) -> View {
    View::View3(Box::new(View3 {
        x: axis(lower, upper),
        y: axis(lower, upper),
        z: axis(lower, upper),
        camera: Camera {
            azimuth_degrees: Number::from(azimuth),
            elevation_degrees: Number::from(elevation),
            projection: Projection::Orthographic,
        },
    }))
}

pub(crate) fn record(method: SamplingMethod, diagnostics: SamplingDiagnostics) -> SceneRecord {
    SceneRecord {
        result: ResultId(1),
        inputs: vec![SceneInput {
            expression: String::from("f"),
            sampling_box: Vec::new(),
            variables: Vec::new(),
        }],
        method,
        resolution: Resolution {
            domain: Domain::F64,
            axes: Vec::new(),
            adaptive: None,
        },
        seed: None,
        backend: BackendKind::Cpu,
        references: Vec::new(),
        diagnostics,
    }
}

pub(crate) fn style() -> StyleRole {
    StyleRole {
        kind: KindColour::Numeric,
        colour_map: None,
        line: LinePattern::Solid,
        marker: Marker::None,
        emphasis: Emphasis::Normal,
    }
}

pub(crate) fn sequential(lower: i64, upper: i64) -> StyleRole {
    StyleRole {
        colour_map: Some(ColourMapping {
            map: ColourMap::Sequential,
            range: interval(lower, upper),
        }),
        ..style()
    }
}

pub(crate) fn f64s(values: &[f64]) -> Column {
    Column::F64(values.to_vec())
}

pub(crate) fn layer(primitive: Primitive, style: StyleRole, value_bounds: Vec<Column>) -> Layer {
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

pub(crate) fn scene_with(record: SceneRecord, view: View, layers: Vec<Layer>) -> Scene {
    Scene::new(
        record,
        Vec::new(),
        vec![view],
        vec![Frame {
            parameter_values: Vec::new(),
            layers,
        }],
    )
    .expect("scene is well formed")
}

pub(crate) fn scene(view: View, layers: Vec<Layer>) -> Scene {
    scene_with(
        record(SamplingMethod::UniformGrid, SamplingDiagnostics::default()),
        view,
        layers,
    )
}

pub(crate) fn render_sized(
    scene: &Scene,
    width: u16,
    height: u16,
    text: &PictureText,
) -> Result<Rendered, RenderError> {
    let fonts = FontSet::bundled().expect("bundled fonts load");
    render_scene(
        &RenderRequest {
            settled: None,
            scene,
            frame: 0,
            width,
            height,
            text_size: 14,
            scale: ScaleFactor::from_percent(100).expect("non-zero scale"),
            theme: Theme::light(),
            decimal_separator: '.',
            text,
        },
        &fonts,
    )
}

pub(crate) fn render(scene: &Scene) -> Rendered {
    render_sized(scene, WIDTH, HEIGHT, &PictureText::default()).expect("scene renders")
}

pub(crate) fn view_pixel(
    rendered: &Rendered,
    x: f64,
    y: f64,
    lower: f64,
    upper: f64,
) -> (u16, u16) {
    let area = rendered.layout.views[0].plot_area;
    let fraction = |value: f64| (value - lower) / (upper - lower);
    let across = f64::from(area.x) + fraction(x) * f64::from(area.width);
    let down = f64::from(area.y) + f64::from(area.height) - fraction(y) * f64::from(area.height);
    (
        crate::mapping::pixel_floor(across),
        crate::mapping::pixel_floor(down),
    )
}

pub(crate) fn rgba(colour: crate::colour::Colour) -> [u8; 4] {
    [colour.red, colour.green, colour.blue, colour.alpha]
}

pub(crate) fn window() -> [u8; 4] {
    rgba(Theme::light().window)
}
