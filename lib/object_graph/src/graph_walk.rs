use std::collections::{HashSet, VecDeque};

use crate::{graph_node::ObjectGraphNode, GraphIndex, GraphsMap, NodeIndex, ObjectGraph};

pub struct ObjectGraphWalker<'a> {
    graphs_map: &'a GraphsMap,
    nodes: VecDeque<(GraphIndex, NodeIndex)>,
    seen: HashSet<NodeIndex>,
}

impl<'a> ObjectGraphWalker<'a> {
    pub fn from_node(
        graphs_map: &'a GraphsMap,
        start_graph: GraphIndex,
        start_node: NodeIndex,
    ) -> Self {
        Self::from_nodes(graphs_map, [(start_graph, start_node)])
    }

    pub fn from_graphs_map(graphs_map: &'a GraphsMap) -> Self {
        Self::from_nodes(
            &graphs_map,
            graphs_map.roots().map(|(_, r)| (r.graph, r.node)),
        )
    }

    pub fn from_nodes<I>(graphs_map: &'a GraphsMap, nodes: I) -> Self
    where
        I: IntoIterator<Item = (GraphIndex, NodeIndex)>,
    {
        Self {
            graphs_map,
            nodes: nodes.into_iter().collect(),
            seen: Default::default(),
        }
    }

    fn push_node(&mut self, graph_id: GraphIndex, node_id: NodeIndex) {
        if self.seen.contains(&node_id) {
            return;
        }

        self.nodes.push_back((graph_id, node_id));
    }
}

impl<'a> std::iter::Iterator for ObjectGraphWalker<'a> {
    type Item = (&'a ObjectGraph, NodeIndex, &'a ObjectGraphNode);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((graph_id, node_id)) = self.nodes.pop_front() {
            if self.seen.contains(&node_id) {
                continue;
            }
            self.seen.insert(node_id);

            let graph = &self.graphs_map[graph_id];
            let node = graph.get_node(&node_id).unwrap();

            for (_, neig) in node.pointers_iter() {
                self.push_node(neig.graph.unwrap_or(graph_id), neig.node);
            }
            return Some((graph.as_ref(), node_id, node.as_ref()))
        }
        
        None
    }
}
