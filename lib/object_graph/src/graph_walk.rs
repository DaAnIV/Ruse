use std::collections::{HashSet, VecDeque};

use crate::{
    graph_node::ObjectGraphNode, EdgeEndPoint, GraphIndex, GraphsMap, NodeIndex, ObjectGraph,
};

pub struct ObjectGraphWalker<'a> {
    graphs_map: &'a GraphsMap,
    nodes: VecDeque<(GraphIndex, NodeIndex)>,
    seen: HashSet<NodeIndex>,
}

impl<'a> ObjectGraphWalker<'a> {
    pub fn from_node(graphs_map: &'a GraphsMap, start_graph: GraphIndex, start_node: NodeIndex) -> Self {
        let mut instance = Self {
            graphs_map,
            nodes: VecDeque::new(),
            seen: Default::default(),
        };

        instance.nodes.push_back((start_graph, start_node));

        instance
    }

    pub fn from_graph(graphs_map: &'a GraphsMap, graph: GraphIndex) -> Self {
        Self::from_graphs(graphs_map, [graph])
    }

    pub fn from_graphs<I>(graphs_map: &'a GraphsMap, graphs: I) -> Self 
    where
        I: IntoIterator<Item = GraphIndex> {
        let mut instance = Self {
            graphs_map,
            nodes: VecDeque::new(),
            seen: Default::default(),
        };

        for graph_id in graphs {
            let graph = &instance.graphs_map[graph_id];
            for (_, id) in &graph.roots {
                instance.push_node(graph_id, *id);
            }
        }

        instance
    }

    fn push_node(&mut self, graph_id: GraphIndex, node_id: NodeIndex) {
        if self.seen.contains(&node_id) {
            return;
        }

        self.seen.insert(node_id);
        self.nodes.push_back((graph_id, node_id));
    }
}

impl<'a> std::iter::Iterator for ObjectGraphWalker<'a> {
    type Item = (&'a ObjectGraph, &'a ObjectGraphNode);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((graph_id, node_id)) = self.nodes.pop_front() {
            
            let graph = &self.graphs_map[graph_id];
            let node = graph.get_node(&node_id).unwrap();

            for (_, neig) in &node.pointers {
                match neig {
                    EdgeEndPoint::Internal(neig_node) => {
                        self.push_node(graph_id, *neig_node)
                    }
                    EdgeEndPoint::Chain(neig_graph, neig_node) => {
                        self.push_node(*neig_graph, *neig_node)
                    }
                }
            }
            Some((graph.as_ref(), node.as_ref()))
        } else {
            None
        }
    }
}
