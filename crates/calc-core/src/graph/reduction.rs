use super::directed_graph::{Arrow, DirectedGraph, NodeId};

const WORD_BITS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReductionError {
    Cycle(NodeId),
}

struct NodeSet {
    words: Vec<u64>,
}

impl NodeSet {
    fn empty(node_count: usize) -> NodeSet {
        NodeSet {
            words: vec![0; node_count.div_ceil(WORD_BITS)],
        }
    }

    fn insert(&mut self, node: NodeId) {
        if let Some(word) = self.words.get_mut(node.0 / WORD_BITS) {
            *word |= 1_u64 << (node.0 % WORD_BITS);
        }
    }

    fn contains(&self, node: NodeId) -> bool {
        self.words
            .get(node.0 / WORD_BITS)
            .is_some_and(|word| word & (1_u64 << (node.0 % WORD_BITS)) != 0)
    }

    fn insert_all(&mut self, other: &NodeSet) {
        for (word, other_word) in self.words.iter_mut().zip(&other.words) {
            *word |= other_word;
        }
    }
}

pub fn transitive_reduction(graph: &DirectedGraph) -> Result<DirectedGraph, ReductionError> {
    let order = graph.topological_order().map_err(ReductionError::Cycle)?;
    let node_count = graph.node_count();
    let mut reachable = (0..node_count)
        .map(|_| NodeSet::empty(node_count))
        .collect::<Vec<NodeSet>>();
    for node in order.iter().rev() {
        let mut from_node = NodeSet::empty(node_count);
        for successor in graph.successors(*node) {
            from_node.insert(*successor);
            if let Some(from_successor) = reachable.get(successor.0) {
                from_node.insert_all(from_successor);
            }
        }
        if let Some(slot) = reachable.get_mut(node.0) {
            *slot = from_node;
        }
    }
    let is_bypassed = |arrow: &Arrow| {
        graph.successors(arrow.source).iter().any(|detour| {
            reachable
                .get(detour.0)
                .is_some_and(|from_detour| from_detour.contains(arrow.target))
        })
    };
    let kept = graph
        .arrows()
        .iter()
        .filter(|arrow| !is_bypassed(arrow))
        .copied()
        .collect::<Vec<Arrow>>();
    Ok(DirectedGraph::from_checked_arrows(node_count, kept))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arrows(pairs: &[(usize, usize)]) -> Vec<Arrow> {
        pairs
            .iter()
            .map(|(source, target)| Arrow {
                source: NodeId(*source),
                target: NodeId(*target),
            })
            .collect()
    }

    fn graph(node_count: usize, pairs: &[(usize, usize)]) -> DirectedGraph {
        DirectedGraph::new(node_count, &arrows(pairs)).unwrap()
    }

    #[test]
    fn arrow_with_a_detour_is_removed() {
        let graph = graph(3, &[(0, 1), (1, 2), (0, 2)]);

        let reduction = transitive_reduction(&graph).unwrap();

        assert_eq!(reduction.arrows(), arrows(&[(0, 1), (1, 2)]).as_slice());
    }

    #[test]
    fn arrow_bypassed_by_a_long_walk_is_removed() {
        let graph = graph(4, &[(0, 1), (1, 2), (2, 3), (0, 3)]);

        let reduction = transitive_reduction(&graph).unwrap();

        assert_eq!(
            reduction.arrows(),
            arrows(&[(0, 1), (1, 2), (2, 3)]).as_slice()
        );
    }

    #[test]
    fn arrows_without_detour_are_kept() {
        let graph = graph(4, &[(0, 1), (0, 2), (1, 3), (2, 3)]);

        let reduction = transitive_reduction(&graph).unwrap();

        assert_eq!(reduction.arrows(), graph.arrows());
    }

    #[test]
    fn detour_through_a_node_past_the_first_word_is_found() {
        let graph = graph(70, &[(0, 65), (65, 69), (0, 69)]);

        let reduction = transitive_reduction(&graph).unwrap();

        assert_eq!(reduction.arrows(), arrows(&[(0, 65), (65, 69)]).as_slice());
    }

    #[test]
    fn reduction_keeps_every_node() {
        let graph = graph(5, &[(0, 1)]);

        let reduction = transitive_reduction(&graph).unwrap();

        assert_eq!(reduction.node_count(), 5);
    }

    #[test]
    fn graph_with_a_cycle_has_no_reduction() {
        let graph = graph(2, &[(0, 1), (1, 0)]);

        let result = transitive_reduction(&graph);

        assert_eq!(result, Err(ReductionError::Cycle(NodeId(0))));
    }
}
