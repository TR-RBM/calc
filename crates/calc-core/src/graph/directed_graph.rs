use std::cmp::Reverse;
use std::collections::BinaryHeap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Arrow {
    pub source: NodeId,
    pub target: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectedGraph {
    node_count: usize,
    arrows: Vec<Arrow>,
    successors: Vec<Vec<NodeId>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphBuildError {
    NoNodes,
    NodeOutOfRange { arrow: Arrow, node_count: usize },
}

impl DirectedGraph {
    pub fn new(node_count: usize, arrows: &[Arrow]) -> Result<DirectedGraph, GraphBuildError> {
        if node_count == 0 {
            return Err(GraphBuildError::NoNodes);
        }
        if let Some(arrow) = arrows
            .iter()
            .find(|arrow| arrow.source.0 >= node_count || arrow.target.0 >= node_count)
        {
            return Err(GraphBuildError::NodeOutOfRange {
                arrow: *arrow,
                node_count,
            });
        }
        let mut arrows = arrows.to_vec();
        arrows.sort_unstable();
        arrows.dedup();
        Ok(DirectedGraph::from_checked_arrows(node_count, arrows))
    }

    pub(super) fn from_checked_arrows(node_count: usize, arrows: Vec<Arrow>) -> DirectedGraph {
        let mut successors = vec![Vec::new(); node_count];
        for arrow in &arrows {
            if let Some(targets) = successors.get_mut(arrow.source.0) {
                targets.push(arrow.target);
            }
        }
        DirectedGraph {
            node_count,
            arrows,
            successors,
        }
    }

    pub fn node_count(&self) -> usize {
        self.node_count
    }

    pub fn arrows(&self) -> &[Arrow] {
        &self.arrows
    }

    pub fn successors(&self, node: NodeId) -> &[NodeId] {
        self.successors.get(node.0).map_or(&[], Vec::as_slice)
    }

    pub fn is_acyclic(&self) -> bool {
        self.topological_order().is_ok()
    }

    pub(super) fn topological_order(&self) -> Result<Vec<NodeId>, NodeId> {
        let mut predecessors = vec![Vec::new(); self.node_count];
        for arrow in &self.arrows {
            if let Some(sources) = predecessors.get_mut(arrow.target.0) {
                sources.push(arrow.source);
            }
        }
        let mut remaining_predecessors = predecessors.iter().map(Vec::len).collect::<Vec<usize>>();
        let mut ready = remaining_predecessors
            .iter()
            .enumerate()
            .filter(|(_, count)| **count == 0)
            .map(|(node, _)| Reverse(NodeId(node)))
            .collect::<BinaryHeap<Reverse<NodeId>>>();
        let mut order = Vec::with_capacity(self.node_count);
        while let Some(Reverse(node)) = ready.pop() {
            order.push(node);
            for successor in self.successors(node) {
                if let Some(count) = remaining_predecessors.get_mut(successor.0) {
                    *count -= 1;
                    if *count == 0 {
                        ready.push(Reverse(*successor));
                    }
                }
            }
        }
        if order.len() == self.node_count {
            return Ok(order);
        }
        Err(node_on_cycle(&predecessors, &remaining_predecessors))
    }
}

fn node_on_cycle(predecessors: &[Vec<NodeId>], remaining_predecessors: &[usize]) -> NodeId {
    let is_remaining = |node: &NodeId| {
        remaining_predecessors
            .get(node.0)
            .is_some_and(|count| *count > 0)
    };
    let start = remaining_predecessors
        .iter()
        .position(|count| *count > 0)
        .map_or(NodeId(0), NodeId);
    let mut visited = vec![false; remaining_predecessors.len()];
    let mut node = start;
    loop {
        match visited.get_mut(node.0) {
            Some(seen) if !*seen => *seen = true,
            _ => return node,
        }
        let previous = predecessors
            .get(node.0)
            .and_then(|sources| sources.iter().filter(|source| is_remaining(source)).min());
        match previous {
            Some(previous) => node = *previous,
            None => return node,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arrow(source: usize, target: usize) -> Arrow {
        Arrow {
            source: NodeId(source),
            target: NodeId(target),
        }
    }

    #[test]
    fn graph_without_nodes_is_rejected() {
        let result = DirectedGraph::new(0, &[]);

        assert_eq!(result, Err(GraphBuildError::NoNodes));
    }

    #[test]
    fn arrow_to_a_missing_node_is_rejected() {
        let result = DirectedGraph::new(2, &[arrow(0, 2)]);

        assert_eq!(
            result,
            Err(GraphBuildError::NodeOutOfRange {
                arrow: arrow(0, 2),
                node_count: 2
            })
        );
    }

    #[test]
    fn repeated_arrow_is_stored_once_in_sorted_order() {
        let graph = DirectedGraph::new(3, &[arrow(1, 2), arrow(0, 1), arrow(1, 2)]).unwrap();

        assert_eq!(graph.arrows(), &[arrow(0, 1), arrow(1, 2)]);
    }

    #[test]
    fn successors_are_the_targets_of_a_node_arrows() {
        let graph = DirectedGraph::new(3, &[arrow(0, 2), arrow(0, 1)]).unwrap();

        assert_eq!(graph.successors(NodeId(0)), &[NodeId(1), NodeId(2)]);
    }

    #[test]
    fn single_node_without_arrows_is_acyclic() {
        let graph = DirectedGraph::new(1, &[]).unwrap();

        assert!(graph.is_acyclic());
    }

    #[test]
    fn chain_is_acyclic() {
        let graph = DirectedGraph::new(3, &[arrow(0, 1), arrow(1, 2)]).unwrap();

        assert!(graph.is_acyclic());
    }

    #[test]
    fn loop_on_one_node_is_a_cycle() {
        let graph = DirectedGraph::new(2, &[arrow(1, 1)]).unwrap();

        assert!(!graph.is_acyclic());
    }

    #[test]
    fn two_opposite_arrows_are_a_cycle() {
        let graph = DirectedGraph::new(2, &[arrow(0, 1), arrow(1, 0)]).unwrap();

        assert!(!graph.is_acyclic());
    }

    #[test]
    fn topological_order_takes_the_smallest_ready_node_first() {
        let graph = DirectedGraph::new(4, &[arrow(3, 1), arrow(2, 0)]).unwrap();

        let order = graph.topological_order();

        assert_eq!(order, Ok(vec![NodeId(2), NodeId(0), NodeId(3), NodeId(1)]));
    }

    #[test]
    fn cycle_report_names_a_node_on_the_cycle_not_one_below_it() {
        let graph = DirectedGraph::new(3, &[arrow(2, 0), arrow(1, 2), arrow(2, 1)]).unwrap();

        let order = graph.topological_order();

        assert_eq!(order, Err(NodeId(2)));
    }
}
