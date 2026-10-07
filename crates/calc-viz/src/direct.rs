use std::cmp::Ordering;

use calc_core::graph::{
    Arrow, DirectedGraph, LayeringError, NodeId, ReductionError, longest_path_layering,
    transitive_reduction,
};
use calc_exec::{BackendKind, Domain};
use calc_numbers::{Integer, Number};

use crate::primitive::{BitLayout, Column, Formula, Graph, Primitive};
use crate::record::{
    CorpusReference, Interval, LayeringMethod, Resolution, ResultId, SamplingDiagnostics,
    SamplingMethod, SceneInput, SceneRecord,
};
use crate::scene::{Frame, InputIndex, Layer, Scene, SceneError, ViewIndex};
use crate::style::{Emphasis, KindColour, LinePattern, Marker, StyleRole};
use crate::view::{AxisUnit, Dimension, Scale, View, View2, ViewAxis};

const SIGN_BITS: u32 = 1;
const UNSAMPLED_DIVISIONS: u32 = 1;
const BINARY32_EXPONENT_BITS: u32 = 8;
const BINARY64_EXPONENT_BITS: u32 = 11;
const BINARY32_BITS: u32 = 32;
const BINARY64_BITS: u32 = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormulaRequest {
    pub result: ResultId,
    pub text: String,
    pub anchor: [Number; 2],
    pub view_x: Interval,
    pub view_y: Interval,
    pub kind: KindColour,
    pub references: Vec<CorpusReference>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitLayoutRequest {
    pub result: ResultId,
    pub expression_text: String,
    pub value: Number,
    pub kind: KindColour,
    pub references: Vec<CorpusReference>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GraphArrows {
    All,
    TransitiveReduction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphRequest {
    pub result: ResultId,
    pub expression_text: String,
    pub graph: DirectedGraph,
    pub arrows: GraphArrows,
    pub kind: KindColour,
    pub references: Vec<CorpusReference>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DirectPictureError {
    NotMachineNumber,
    Cycle(NodeId),
    Scene(SceneError),
}

fn direct_record(
    result: ResultId,
    expression_text: &str,
    method: SamplingMethod,
    domain: Domain,
    references: &[CorpusReference],
) -> SceneRecord {
    SceneRecord {
        result,
        inputs: vec![SceneInput {
            expression: expression_text.to_owned(),
            sampling_box: Vec::new(),
            variables: Vec::new(),
        }],
        method,
        resolution: Resolution {
            domain,
            axes: Vec::new(),
            adaptive: None,
        },
        seed: None,
        backend: BackendKind::Cpu,
        references: references.to_vec(),
        diagnostics: SamplingDiagnostics::default(),
    }
}

fn plane(x: Interval, y: Interval) -> View {
    let axis = |range| ViewAxis {
        range,
        scale: Scale::Linear,
        dimension: Dimension::DIMENSIONLESS,
        unit: AxisUnit::dimensionless(),
        divisions: UNSAMPLED_DIVISIONS,
    };
    View::View2(Box::new(View2 {
        x: axis(x),
        y: axis(y),
    }))
}

fn single_layer_scene(
    record: SceneRecord,
    view: View,
    primitive: Primitive,
    style: StyleRole,
) -> Result<Scene, DirectPictureError> {
    let frame = Frame {
        parameter_values: Vec::new(),
        layers: vec![Layer {
            view: ViewIndex(0),
            input: InputIndex(0),
            primitive,
            style,
            value_bounds: Vec::new(),
            precision: None,
            columns: None,
            readings: Vec::new(),
        }],
    };
    Scene::new(record, Vec::new(), vec![view], vec![frame]).map_err(DirectPictureError::Scene)
}

fn plain_style(kind: KindColour, line: LinePattern, marker: Marker) -> StyleRole {
    StyleRole {
        kind,
        colour_map: None,
        line,
        marker,
        emphasis: Emphasis::Normal,
    }
}

fn interval(lower: i64, upper: i64) -> Interval {
    Interval {
        lower: Number::from(lower),
        upper: Number::from(upper),
    }
}

fn count_number(count: usize) -> Number {
    Number::Integer(Integer::from(u64::try_from(count).unwrap_or(u64::MAX)))
}

pub fn formula_scene(request: &FormulaRequest) -> Result<Scene, DirectPictureError> {
    let record = direct_record(
        request.result,
        &request.text,
        SamplingMethod::Direct,
        Domain::F64,
        &request.references,
    );
    let primitive = Primitive::Formula(Formula {
        expression: request.text.clone(),
        anchor: request.anchor.to_vec(),
    });
    single_layer_scene(
        record,
        plane(request.view_x.clone(), request.view_y.clone()),
        primitive,
        plain_style(request.kind, LinePattern::None, Marker::None),
    )
}

fn bits_most_significant_first(bits: u64, count: u32) -> Vec<bool> {
    (0..count)
        .rev()
        .map(|position| {
            bits.checked_shr(position)
                .is_some_and(|shifted| shifted & 1 == 1)
        })
        .collect()
}

pub fn bit_layout_scene(request: &BitLayoutRequest) -> Result<Scene, DirectPictureError> {
    let (bits, bit_count, exponent_bits, domain) = match request.value {
        Number::F32(value) => (
            u64::from(value.to_bits()),
            BINARY32_BITS,
            BINARY32_EXPONENT_BITS,
            Domain::F32,
        ),
        Number::F64(value) => (
            value.to_bits(),
            BINARY64_BITS,
            BINARY64_EXPONENT_BITS,
            Domain::F64,
        ),
        Number::Integer(_) | Number::Rational(_) => {
            return Err(DirectPictureError::NotMachineNumber);
        }
    };
    let record = direct_record(
        request.result,
        &request.expression_text,
        SamplingMethod::Direct,
        domain,
        &request.references,
    );
    let primitive = Primitive::BitLayout(BitLayout {
        bits: bits_most_significant_first(bits, bit_count),
        exponent_start: SIGN_BITS,
        fraction_start: SIGN_BITS + exponent_bits,
    });
    let view = plane(interval(0, i64::from(bit_count)), interval(0, 1));
    single_layer_scene(
        record,
        view,
        primitive,
        plain_style(request.kind, LinePattern::None, Marker::None),
    )
}

fn mean_slot_order(left: (usize, usize), right: (usize, usize)) -> Ordering {
    let widen = |value: usize| u128::try_from(value).unwrap_or(u128::MAX);
    let (left_sum, left_count) = left;
    let (right_sum, right_count) = right;
    let left_scaled = widen(left_sum).saturating_mul(widen(right_count));
    let right_scaled = widen(right_sum).saturating_mul(widen(left_count));
    left_scaled.cmp(&right_scaled)
}

fn ordered_layers(graph: &DirectedGraph, layers: &[usize]) -> Vec<Vec<NodeId>> {
    let layer_count = layers.iter().max().map_or(0, |largest| largest + 1);
    let mut predecessors = vec![Vec::new(); graph.node_count()];
    for arrow in graph.arrows() {
        if let Some(sources) = predecessors.get_mut(arrow.target.0) {
            sources.push(arrow.source);
        }
    }
    let mut slots = vec![0_usize; graph.node_count()];
    let mut ordered = Vec::with_capacity(layer_count);
    for layer in 0..layer_count {
        let mut members = layers
            .iter()
            .enumerate()
            .filter(|(_, node_layer)| **node_layer == layer)
            .map(|(node, _)| {
                let sources = predecessors.get(node).map_or(&[][..], Vec::as_slice);
                let slot_sum = sources
                    .iter()
                    .filter_map(|source| slots.get(source.0))
                    .sum::<usize>();
                (NodeId(node), (slot_sum, sources.len().max(1)))
            })
            .collect::<Vec<(NodeId, (usize, usize))>>();
        if layer > 0 {
            members.sort_by(|(left_node, left), (right_node, right)| {
                mean_slot_order(*left, *right).then(left_node.cmp(right_node))
            });
        }
        for (slot, (node, _)) in members.iter().enumerate() {
            if let Some(stored) = slots.get_mut(node.0) {
                *stored = slot;
            }
        }
        ordered.push(members.into_iter().map(|(node, _)| node).collect());
    }
    ordered
}

pub fn graph_scene(request: &GraphRequest) -> Result<Scene, DirectPictureError> {
    let graph = &request.graph;
    let layering = longest_path_layering(graph).map_err(|error| match error {
        LayeringError::Cycle(node) => DirectPictureError::Cycle(node),
    })?;
    let drawn = match request.arrows {
        GraphArrows::All => graph.clone(),
        GraphArrows::TransitiveReduction => {
            transitive_reduction(graph).map_err(|error| match error {
                ReductionError::Cycle(node) => DirectPictureError::Cycle(node),
            })?
        }
    };
    let layers = ordered_layers(graph, layering.layers());
    let mut x_positions = vec![0.0_f64; graph.node_count()];
    let mut y_positions = vec![0.0_f64; graph.node_count()];
    let mut layer_position = 0.0;
    for members in &layers {
        let mut slot_position = 0.0;
        for node in members {
            if let Some(x) = x_positions.get_mut(node.0) {
                *x = layer_position;
            }
            if let Some(y) = y_positions.get_mut(node.0) {
                *y = 0.0 - slot_position;
            }
            slot_position += 1.0;
        }
        layer_position += 1.0;
    }
    let widest_layer = layers.iter().map(Vec::len).max().unwrap_or(0);
    let edge = |arrow: &Arrow| {
        let index = |node: NodeId| u32::try_from(node.0).unwrap_or(u32::MAX);
        [index(arrow.source), index(arrow.target)]
    };
    let record = direct_record(
        request.result,
        &request.expression_text,
        SamplingMethod::GraphLayout {
            layering: LayeringMethod::LongestPath,
        },
        Domain::F64,
        &request.references,
    );
    let view = plane(
        Interval {
            lower: Number::from(-1_i64),
            upper: count_number(layers.len()),
        },
        Interval {
            lower: count_number(widest_layer)
                .negate_exact()
                .unwrap_or_else(|_| Number::from(-1_i64)),
            upper: Number::from(1_i64),
        },
    );
    let primitive = Primitive::Graph(Graph {
        positions: vec![Column::F64(x_positions), Column::F64(y_positions)],
        edges: drawn.arrows().iter().map(edge).collect(),
        is_directed: true,
    });
    single_layer_scene(
        record,
        view,
        primitive,
        plain_style(request.kind, LinePattern::Solid, Marker::Circle),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Location;

    fn arrow(source: usize, target: usize) -> Arrow {
        Arrow {
            source: NodeId(source),
            target: NodeId(target),
        }
    }

    fn graph_request(node_count: usize, arrows: &[Arrow], drawn: GraphArrows) -> GraphRequest {
        GraphRequest {
            result: ResultId(3),
            expression_text: String::from("session"),
            graph: DirectedGraph::new(node_count, arrows).unwrap(),
            arrows: drawn,
            kind: KindColour::Exact,
            references: Vec::new(),
        }
    }

    fn bit_request(value: Number) -> BitLayoutRequest {
        BitLayoutRequest {
            result: ResultId(1),
            expression_text: String::from("-1"),
            value,
            kind: KindColour::Numeric,
            references: Vec::new(),
        }
    }

    fn formula_request(anchor_y: Number) -> FormulaRequest {
        FormulaRequest {
            result: ResultId(2),
            text: String::from("x^2 + y^2"),
            anchor: [Number::from(0_i64), anchor_y],
            view_x: interval(-1, 1),
            view_y: interval(-1, 1),
            kind: KindColour::Symbolic,
            references: Vec::new(),
        }
    }

    fn only_primitive(scene: &Scene) -> &Primitive {
        &scene.frames[0].layers[0].primitive
    }

    fn f64_values(column: &Column) -> Vec<f64> {
        match column {
            Column::F64(values) => values.clone(),
            Column::F32(_) => panic!("expected an f64 column"),
        }
    }

    #[test]
    fn formula_places_the_given_text_at_its_anchor() {
        let scene = formula_scene(&formula_request(Number::from(1_i64))).unwrap();

        assert_eq!(
            only_primitive(&scene),
            &Primitive::Formula(Formula {
                expression: String::from("x^2 + y^2"),
                anchor: vec![Number::from(0_i64), Number::from(1_i64)]
            })
        );
    }

    #[test]
    fn formula_with_machine_anchor_is_a_scene_error() {
        let result = formula_scene(&formula_request(Number::F64(0.5)));

        assert!(matches!(
            result,
            Err(DirectPictureError::Scene(SceneError::NotExact(
                Location::Layer(_)
            )))
        ));
    }

    #[test]
    fn bit_layout_of_negative_one_in_binary32_has_sign_and_biased_exponent() {
        let scene = bit_layout_scene(&bit_request(Number::F32(-1.0))).unwrap();

        let Primitive::BitLayout(layout) = only_primitive(&scene) else {
            panic!("expected a bit layout");
        };
        let mut expected = vec![true, false, true, true, true, true, true, true, true];
        expected.extend([false; 23]);
        assert_eq!(
            (&layout.bits, layout.exponent_start, layout.fraction_start),
            (&expected, 1, 9)
        );
    }

    #[test]
    fn bit_layout_of_binary64_has_eleven_exponent_bits() {
        let scene = bit_layout_scene(&bit_request(Number::F64(f64::MIN_POSITIVE))).unwrap();

        let Primitive::BitLayout(layout) = only_primitive(&scene) else {
            panic!("expected a bit layout");
        };
        assert_eq!(
            (layout.bits.len(), layout.fraction_start, layout.bits[11]),
            (64, 12, true)
        );
    }

    #[test]
    fn bit_layout_of_exact_number_is_rejected() {
        let result = bit_layout_scene(&bit_request(Number::from(1_i64)));

        assert_eq!(result, Err(DirectPictureError::NotMachineNumber));
    }

    #[test]
    fn graph_nodes_are_placed_by_layer_and_mean_predecessor_slot() {
        let arrows = [arrow(0, 3), arrow(1, 2), arrow(0, 4), arrow(1, 4)];
        let request = graph_request(5, &arrows, GraphArrows::All);

        let scene = graph_scene(&request).unwrap();

        let Primitive::Graph(graph) = only_primitive(&scene) else {
            panic!("expected a graph");
        };
        assert_eq!(
            [
                f64_values(&graph.positions[0]),
                f64_values(&graph.positions[1])
            ],
            [
                vec![0.0, 0.0, 1.0, 1.0, 1.0],
                vec![0.0, -1.0, -2.0, 0.0, -1.0]
            ]
        );
    }

    #[test]
    fn graph_draws_only_the_transitive_reduction_when_asked() {
        let arrows = [arrow(0, 1), arrow(1, 2), arrow(0, 2)];
        let request = graph_request(3, &arrows, GraphArrows::TransitiveReduction);

        let scene = graph_scene(&request).unwrap();

        let Primitive::Graph(graph) = only_primitive(&scene) else {
            panic!("expected a graph");
        };
        assert_eq!(graph.edges, vec![[0, 1], [1, 2]]);
    }

    #[test]
    fn graph_record_names_longest_path_layering() {
        let request = graph_request(2, &[arrow(0, 1)], GraphArrows::All);

        let scene = graph_scene(&request).unwrap();

        assert_eq!(
            scene.record.method,
            SamplingMethod::GraphLayout {
                layering: LayeringMethod::LongestPath
            }
        );
    }

    #[test]
    fn graph_with_a_cycle_is_rejected() {
        let request = graph_request(2, &[arrow(0, 1), arrow(1, 0)], GraphArrows::All);

        let result = graph_scene(&request);

        assert_eq!(result, Err(DirectPictureError::Cycle(NodeId(0))));
    }

    #[test]
    fn direct_picture_has_no_bounds_precision_or_readings() {
        let scene = formula_scene(&formula_request(Number::from(1_i64))).unwrap();

        let layer = &scene.frames[0].layers[0];
        assert!(
            layer.value_bounds.is_empty() && layer.precision.is_none() && layer.readings.is_empty()
        );
    }
}
