# graph

## What it does

Directed graphs, for laying out the session dependency graph on the Board view and for graph scenes.

`DirectedGraph::new` takes a node count and a list of arrows between node indices. A graph has at least one node, and every arrow must name existing nodes. Arrows form a set: a repeated arrow is stored once, and arrows are kept in sorted order. `successors` lists the targets of a node's arrows. `is_acyclic` tells whether no arrow walk of positive length returns to its start.

`longest_path_layering` gives each node the length of the longest arrow walk that ends at it. A node with no incoming arrow is in layer 0, and every other node is one layer above its highest predecessor. This is the longest-path layering, so every arrow points to a higher layer. `is_layering` checks that a list of layers has one entry per node and increases along every arrow.

`transitive_reduction` keeps exactly the arrows that no longer arrow walk bypasses. It computes, in reverse topological order, the set of nodes reachable from each node, and removes an arrow when its target is reachable from another successor of its source. The result has the same nodes and the same reachability, and it is the smallest such subset of the arrows.

Both operations need an acyclic graph and return `Cycle` with a node on a cycle otherwise. The node is found by walking backwards from the smallest node left over by the topological sort, so the same graph always reports the same node. The topological sort takes the smallest ready node first, so every result depends only on the graph.

## How to test

`cargo test -p calc-core graph`

Each behaviour has one test on a small graph with a known answer. The reduction has a test with nodes past the first 64, where its node sets use a second word.
