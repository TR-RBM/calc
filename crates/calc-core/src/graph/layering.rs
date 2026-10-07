use super::directed_graph::{DirectedGraph, NodeId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layering {
    layers: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayeringError {
    Cycle(NodeId),
}

impl Layering {
    pub fn layer(&self, node: NodeId) -> Option<usize> {
        self.layers.get(node.0).copied()
    }

    pub fn layers(&self) -> &[usize] {
        &self.layers
    }

    pub fn layer_count(&self) -> usize {
        self.layers.iter().max().map_or(0, |largest| largest + 1)
    }
}

pub fn longest_path_layering(graph: &DirectedGraph) -> Result<Layering, LayeringError> {
    let order = graph.topological_order().map_err(LayeringError::Cycle)?;
    let mut layers = vec![0_usize; graph.node_count()];
    for node in order {
        let next_layer = layers.get(node.0).map_or(0, |layer| layer + 1);
        for successor in graph.successors(node) {
            if let Some(layer) = layers.get_mut(successor.0) {
                *layer = (*layer).max(next_layer);
            }
        }
    }
    Ok(Layering { layers })
}

pub fn is_layering(graph: &DirectedGraph, layers: &[usize]) -> bool {
    layers.len() == graph.node_count()
        && graph.arrows().iter().all(|arrow| {
            match (layers.get(arrow.source.0), layers.get(arrow.target.0)) {
                (Some(source), Some(target)) => source < target,
                _ => false,
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::directed_graph::Arrow;

    fn graph(node_count: usize, arrows: &[(usize, usize)]) -> DirectedGraph {
        let arrows = arrows
            .iter()
            .map(|(source, target)| Arrow {
                source: NodeId(*source),
                target: NodeId(*target),
            })
            .collect::<Vec<Arrow>>();
        DirectedGraph::new(node_count, &arrows).unwrap()
    }

    #[test]
    fn nodes_without_incoming_arrows_are_in_layer_zero() {
        let graph = graph(3, &[(0, 2)]);

        let layering = longest_path_layering(&graph).unwrap();

        assert_eq!(
            (layering.layer(NodeId(0)), layering.layer(NodeId(1))),
            (Some(0), Some(0))
        );
    }

    #[test]
    fn node_is_one_above_its_highest_predecessor() {
        let graph = graph(3, &[(0, 1), (1, 2), (0, 2)]);

        let layering = longest_path_layering(&graph).unwrap();

        assert_eq!(layering.layers(), &[0, 1, 2]);
    }

    #[test]
    fn layer_is_the_longest_arrow_walk_ending_at_the_node() {
        let graph = graph(5, &[(4, 3), (3, 2), (2, 0), (1, 0)]);

        let layering = longest_path_layering(&graph).unwrap();

        assert_eq!(layering.layers(), &[3, 0, 2, 1, 0]);
    }

    #[test]
    fn layer_count_is_one_more_than_the_top_layer() {
        let graph = graph(4, &[(0, 1), (1, 2)]);

        let layering = longest_path_layering(&graph).unwrap();

        assert_eq!(layering.layer_count(), 3);
    }

    #[test]
    fn longest_path_layering_is_a_layering() {
        let graph = graph(6, &[(0, 3), (1, 3), (3, 4), (2, 4), (0, 5), (4, 5)]);

        let layering = longest_path_layering(&graph).unwrap();

        assert!(is_layering(&graph, layering.layers()));
    }

    #[test]
    fn graph_with_a_cycle_has_no_layering() {
        let graph = graph(3, &[(0, 1), (1, 2), (2, 1)]);

        let result = longest_path_layering(&graph);

        assert_eq!(result, Err(LayeringError::Cycle(NodeId(1))));
    }

    #[test]
    fn equal_layers_along_an_arrow_are_not_a_layering() {
        let graph = graph(2, &[(0, 1)]);

        assert!(!is_layering(&graph, &[1, 1]));
    }

    #[test]
    fn layers_for_fewer_nodes_than_the_graph_are_not_a_layering() {
        let graph = graph(3, &[(0, 1)]);

        assert!(!is_layering(&graph, &[0, 1]));
    }
}
