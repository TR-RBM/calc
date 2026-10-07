use calc_numbers::{Integer, Number};
use calc_render::PictureText;
use calc_viz::{
    Angle, AngleIndex, AxisUnit, BackendKind, Camera, CellGrid, CoherentValue, ColourMap,
    ColourMapping, Column, Designation, Dimension, Domain, ElementIndex, Emphasis, EscapeTimeForm,
    Figure, FigureScale, Frame, InputIndex, Interval, IterationLimitRule, KindColour, Label,
    LabelValue, Layer, LinePattern, Mark, MarkKind, Marker, NameKind, PointIndex, Polyline,
    PrecisionLimit, Primitive, Projection, Quantity, Resolution, ResultId, RightAngleMark,
    SamplingDiagnostics, SamplingMethod, ScalarGrid, Scale, Scene, SceneInput, SceneRecord,
    Segment, SegmentIndex, Stroke, StyleRole, TriangleMesh, View, View2, View3, ViewAxis,
    ViewIndex,
};

const LENGTH: Dimension = Dimension {
    exponents: [1, 0, 0, 0, 0, 0, 0, 0],
};

fn fraction(numerator: i64, denominator: i64) -> Number {
    Number::fraction(&Integer::from(numerator), &Integer::from(denominator))
        .expect("non-zero denominator")
}

fn interval(lower: Number, upper: Number) -> Interval {
    Interval { lower, upper }
}

fn axis(range: Interval, dimension: Dimension, symbol: &str) -> ViewAxis {
    ViewAxis {
        range,
        scale: Scale::Linear,
        dimension,
        unit: AxisUnit::Coherent {
            symbol: String::from(symbol),
        },
        divisions: 300,
    }
}

fn record(method: SamplingMethod, diagnostics: SamplingDiagnostics) -> SceneRecord {
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

fn style(kind: KindColour, colour_map: Option<ColourMapping>) -> StyleRole {
    StyleRole {
        kind,
        colour_map,
        line: LinePattern::Solid,
        marker: Marker::None,
        emphasis: Emphasis::Normal,
    }
}

fn layer(primitive: Primitive, style: StyleRole, value_bounds: Vec<Column>) -> Layer {
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

fn scene(record: SceneRecord, view: View2, layers: Vec<Layer>) -> Scene {
    Scene::new(
        record,
        Vec::new(),
        vec![View::View2(Box::new(view))],
        vec![Frame {
            parameter_values: Vec::new(),
            layers,
        }],
    )
    .expect("golden scene is well formed")
}

pub fn curve() -> Scene {
    let count = 81_u32;
    let xs: Vec<f64> = (0..count)
        .map(|index| f64::from(index) / 10.0 - 4.0)
        .collect();
    let ys: Vec<f64> = xs
        .iter()
        .map(|x| {
            if (*x - 1.0).abs() < 0.05 {
                f64::NAN
            } else {
                x * x / 4.0 - 1.0
            }
        })
        .collect();
    let bounds: Vec<f64> = xs
        .iter()
        .map(|x| if *x > 2.45 && *x < 3.05 { 0.3 } else { 0.0 })
        .collect();
    let mut curve = layer(
        Primitive::Polyline(Polyline {
            coordinates: vec![Column::F64(xs), Column::F64(ys)],
        }),
        style(KindColour::Numeric, None),
        vec![Column::F64(bounds)],
    );
    curve.precision = Some(PrecisionLimit {
        unresolved_samples: 6,
        ..PrecisionLimit::default()
    });
    scene(
        record(SamplingMethod::UniformGrid, SamplingDiagnostics::default()),
        View2 {
            x: axis(
                interval(Number::from(-4_i64), Number::from(4_i64)),
                LENGTH,
                "m",
            ),
            y: axis(
                interval(fraction(-3, 2), Number::from(3_i64)),
                Dimension::DIMENSIONLESS,
                "",
            ),
        },
        vec![curve],
    )
}

pub fn escape_time() -> Scene {
    let columns = 24_usize;
    let rows = 16_usize;
    let mut scalar = Vec::with_capacity(columns * rows);
    let mut classes = Vec::with_capacity(columns * rows);
    let mut counts = [0_u64; 3];
    for row in 0..rows {
        for column in 0..columns {
            let (dx, dy) = (column.abs_diff(columns / 2), row.abs_diff(rows / 2));
            let distance = dx * dx + dy * dy;
            let class = if distance < 16 {
                1
            } else if distance < 30 {
                2
            } else {
                0
            };
            counts[usize::from(class)] += 1;
            classes.push(class);
            scalar.push(if class == 0 {
                f64::from(u32::try_from(distance % 32).expect("small distance"))
            } else {
                f64::NAN
            });
        }
    }
    let cell_count = scalar.len();
    let method = SamplingMethod::EscapeTime {
        form: EscapeTimeForm::QuadraticParameter,
        limit_rule: IterationLimitRule::Fixed { iterations: 32 },
        iterations_used: 32,
    };
    let diagnostics = SamplingDiagnostics {
        escaped_cells: counts[0],
        inside_cells: counts[1],
        undecided_cells: counts[2],
        ..SamplingDiagnostics::default()
    };
    let grid = layer(
        Primitive::ScalarGrid(ScalarGrid {
            cells: CellGrid {
                region: vec![
                    interval(Number::from(-2_i64), Number::from(1_i64)),
                    interval(Number::from(-1_i64), Number::from(1_i64)),
                ],
                counts: vec![24, 16],
            },
            scalar: Column::F64(scalar),
            classes: Some(classes),
        }),
        style(
            KindColour::Sampled,
            Some(ColourMapping {
                map: ColourMap::Sequential,
                range: interval(Number::from(0_i64), Number::from(32_i64)),
            }),
        ),
        vec![Column::F64(vec![f64::NAN; cell_count])],
    );
    scene(
        record(method, diagnostics),
        View2 {
            x: axis(
                interval(Number::from(-2_i64), Number::from(1_i64)),
                Dimension::DIMENSIONLESS,
                "",
            ),
            y: axis(
                interval(Number::from(-1_i64), Number::from(1_i64)),
                Dimension::DIMENSIONLESS,
                "",
            ),
        },
        vec![grid],
    )
}

pub fn figure() -> Scene {
    let segment = |from, to| Segment {
        from: PointIndex(from),
        to: PointIndex(to),
        stroke: Stroke::Solid,
    };
    let vertex = |point, name: &str| Label {
        element: ElementIndex::Point(PointIndex(point)),
        names: NameKind::Vertex,
        name: String::from(name),
        quantity: Quantity::Named,
        designation: None,
    };
    let figure = Figure {
        points: [
            Column::F64(vec![0.0, 4.0, 0.0]),
            Column::F64(vec![0.0, 0.0, 3.0]),
        ],
        segments: vec![segment(0, 1), segment(1, 2), segment(2, 0)],
        angles: vec![
            Angle {
                vertex: PointIndex(0),
                first_arm: PointIndex(1),
                second_arm: PointIndex(2),
                is_oriented: false,
            },
            Angle {
                vertex: PointIndex(1),
                first_arm: PointIndex(2),
                second_arm: PointIndex(0),
                is_oriented: false,
            },
        ],
        marks: vec![
            Mark {
                element: ElementIndex::Angle(AngleIndex(0)),
                kind: MarkKind::RightAngle(RightAngleMark::Square),
            },
            Mark {
                element: ElementIndex::Angle(AngleIndex(1)),
                kind: MarkKind::AngleArc { count: 1 },
            },
        ],
        labels: vec![
            vertex(0, "C"),
            vertex(1, "A"),
            vertex(2, "B"),
            Label {
                element: ElementIndex::Segment(SegmentIndex(1)),
                names: NameKind::Side,
                name: String::from("c"),
                quantity: Quantity::Sought { step: None },
                designation: Some(Designation::Hypotenuse),
            },
            Label {
                element: ElementIndex::Segment(SegmentIndex(0)),
                names: NameKind::Side,
                name: String::from("b"),
                quantity: Quantity::Given(LabelValue {
                    value: CoherentValue::Rational(fraction(1, 25)),
                    dimension: LENGTH,
                    printed: String::from("4 cm"),
                }),
                designation: None,
            },
            Label {
                element: ElementIndex::Angle(AngleIndex(1)),
                names: NameKind::Angle,
                name: String::from("\u{3b1}"),
                quantity: Quantity::Named,
                designation: None,
            },
        ],
        scale: FigureScale::Sketch,
    };
    scene(
        record(SamplingMethod::Direct, SamplingDiagnostics::default()),
        View2 {
            x: axis(
                interval(Number::from(-2_i64), Number::from(6_i64)),
                Dimension::DIMENSIONLESS,
                "",
            ),
            y: axis(
                interval(Number::from(-1_i64), Number::from(4_i64)),
                Dimension::DIMENSIONLESS,
                "",
            ),
        },
        vec![layer(
            Primitive::Figure(Box::new(figure)),
            style(KindColour::Exact, None),
            Vec::new(),
        )],
    )
}

pub fn surface() -> Scene {
    let steps = 16_u32;
    let coordinate = |index: u32| f64::from(index) / f64::from(steps) * 2.0 - 1.0;
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut zs = Vec::new();
    for row in 0..=steps {
        for column in 0..=steps {
            let (x, y) = (coordinate(column), coordinate(row));
            xs.push(x);
            ys.push(y);
            zs.push(
                if y < 0.0 {
                    0.9 + y * 1.8
                } else {
                    0.9 - y * 1.8
                } * (1.0 - x * x * 0.5),
            );
        }
    }
    let width = steps + 1;
    let mut triangles = Vec::new();
    for row in 0..steps {
        for column in 0..steps {
            let corner = row * width + column;
            triangles.push([corner, corner + 1, corner + width + 1]);
            triangles.push([corner, corner + width + 1, corner + width]);
        }
    }
    let vertex_count = xs.len();
    let mesh = layer(
        Primitive::TriangleMesh(TriangleMesh {
            vertices: vec![Column::F64(xs), Column::F64(ys), Column::F64(zs.clone())],
            triangles,
            scalar: Some(Column::F64(zs)),
        }),
        style(
            KindColour::Sampled,
            Some(ColourMapping {
                map: ColourMap::Sequential,
                range: interval(Number::from(-1_i64), Number::from(1_i64)),
            }),
        ),
        vec![Column::F64(vec![0.0; vertex_count])],
    );
    let unit_axis = || {
        axis(
            interval(Number::from(-1_i64), Number::from(1_i64)),
            Dimension::DIMENSIONLESS,
            "",
        )
    };
    Scene::new(
        record(SamplingMethod::UniformGrid, SamplingDiagnostics::default()),
        Vec::new(),
        vec![View::View3(Box::new(View3 {
            x: unit_axis(),
            y: unit_axis(),
            z: unit_axis(),
            camera: Camera {
                azimuth_degrees: Number::from(20_i64),
                elevation_degrees: Number::from(15_i64),
                projection: Projection::Orthographic,
            },
        }))],
        vec![Frame {
            parameter_values: Vec::new(),
            layers: vec![mesh],
        }],
    )
    .expect("golden scene is well formed")
}

fn colour_grid(
    map: ColourMap,
    range: Interval,
    primitive: Primitive,
    bounds: Vec<Column>,
) -> Scene {
    let view_axis = |lower: i64, upper: i64| {
        axis(
            interval(Number::from(lower), Number::from(upper)),
            Dimension::DIMENSIONLESS,
            "",
        )
    };
    scene(
        record(SamplingMethod::UniformGrid, SamplingDiagnostics::default()),
        View2 {
            x: view_axis(-2, 2),
            y: view_axis(-2, 2),
        },
        vec![layer(
            primitive,
            style(KindColour::Sampled, Some(ColourMapping { map, range })),
            bounds,
        )],
    )
}

pub fn diverging_grid() -> Scene {
    let size = 20_usize;
    let mut scalar = Vec::with_capacity(size * size);
    for row in 0..size {
        for column in 0..size {
            let x = f64::from(u32::try_from(column).unwrap_or(0)) / 5.0 - 1.9;
            let y = f64::from(u32::try_from(row).unwrap_or(0)) / 5.0 - 1.9;
            scalar.push(if row == 3 && column == 4 {
                f64::NAN
            } else {
                x * y
            });
        }
    }
    let cells = scalar.len();
    colour_grid(
        ColourMap::Diverging,
        interval(fraction(-10, 3), fraction(10, 3)),
        Primitive::ScalarGrid(ScalarGrid {
            cells: CellGrid {
                region: vec![
                    interval(Number::from(-2_i64), Number::from(2_i64)),
                    interval(Number::from(-2_i64), Number::from(2_i64)),
                ],
                counts: vec![20, 20],
            },
            scalar: Column::F64(scalar),
            classes: None,
        }),
        vec![Column::F64(vec![0.0; cells])],
    )
}

pub fn domain_grid() -> Scene {
    let size = 24_usize;
    let mut real = Vec::with_capacity(size * size);
    let mut imaginary = Vec::with_capacity(size * size);
    for row in 0..size {
        for column in 0..size {
            let x = f64::from(u32::try_from(column).unwrap_or(0)) / 6.0 - 1.9;
            let y = f64::from(u32::try_from(row).unwrap_or(0)) / 6.0 - 1.9;
            real.push(x * x - y * y);
            imaginary.push(2.0 * x * y);
        }
    }
    let cells = real.len();
    colour_grid(
        ColourMap::DomainColouring,
        interval(Number::from(0_i64), Number::from(1_i64)),
        Primitive::ComplexGrid(calc_viz::ComplexGrid {
            cells: CellGrid {
                region: vec![
                    interval(Number::from(-2_i64), Number::from(2_i64)),
                    interval(Number::from(-2_i64), Number::from(2_i64)),
                ],
                counts: vec![24, 24],
            },
            real: Column::F64(real),
            imaginary: Column::F64(imaginary),
        }),
        vec![Column::F64(vec![0.0; cells]), Column::F64(vec![0.0; cells])],
    )
}

pub fn mesh_with_a_missing_vertex() -> Scene {
    let xs = vec![-1.5, 0.0, 1.5, -1.5, 0.0, 1.5, -1.5, 0.0, 1.5];
    let ys = vec![-1.5, -1.5, -1.5, 0.0, 0.0, 0.0, 1.5, 1.5, f64::NAN];
    let triangles = vec![
        [0, 1, 4],
        [0, 4, 3],
        [1, 2, 5],
        [1, 5, 4],
        [3, 4, 7],
        [3, 7, 6],
        [4, 5, 8],
        [4, 8, 7],
    ];
    let view_axis = |lower: i64, upper: i64| {
        axis(
            interval(Number::from(lower), Number::from(upper)),
            Dimension::DIMENSIONLESS,
            "",
        )
    };
    scene(
        record(SamplingMethod::UniformGrid, SamplingDiagnostics::default()),
        View2 {
            x: view_axis(-2, 2),
            y: view_axis(-2, 2),
        },
        vec![layer(
            Primitive::TriangleMesh(TriangleMesh {
                vertices: vec![Column::F64(xs), Column::F64(ys)],
                triangles,
                scalar: None,
            }),
            style(KindColour::Numeric, None),
            vec![Column::F64(vec![0.0; 9])],
        )],
    )
}

pub fn picture_text() -> PictureText {
    let mut text = PictureText {
        axis_titles: vec![vec![String::from("position"), String::from("value")]],
        sketch: String::from("sketch"),
        ..PictureText::default()
    };
    text.mark_keys.right_angle_square = String::from("right angle");
    text.mark_keys.angle_arc = String::from("angle");
    text.designations.hypotenuse = String::from("hypotenuse");
    text.precision.value_limit = String::from("precision exhausted at this scale");
    text.legend_titles = vec![String::from("colour value")];
    text.escape_time.inside = String::from("inside");
    text.escape_time.undecided = String::from("undecided");
    text.escape_time.undecided_share = String::from("undecided 21 percent");
    text.roles.missing = String::from("missing");
    text.unit_joiner = String::from("in");
    text.domain_legend.argument = String::from("argument");
    text.domain_legend.modulus = String::from("modulus");
    text.roles.unresolved = String::from("unresolved");
    text.roles.back_face = String::from("back face");
    text.roles.provisional = String::from("still computing");
    text
}
