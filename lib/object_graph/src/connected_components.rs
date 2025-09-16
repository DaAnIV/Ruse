use std::collections::HashMap;

use union_find::UnionFind;

use crate::{graph_walk::ObjectGraphWalker, GraphsMap, NodeIndex};

#[derive(Clone)]
pub struct GraphsMapWeakComponents {
    node_to_key: HashMap<NodeIndex, usize>,
    uf: union_find::QuickUnionUf<union_find::UnionByRank>,
}

impl GraphsMapWeakComponents {
    pub fn new() -> Self {
        Self {
            node_to_key: Default::default(),
            uf: union_find::QuickUnionUf::new(0),
        }
    }

    pub fn from_graphs_map(graphs_map: &GraphsMap) -> Self {
        let node_to_key: HashMap<NodeIndex, usize> = HashMap::from_iter(
            graphs_map
                .node_ids()
                .enumerate()
                .map(|(key, node_id)| (*node_id, key)),
        );
        let mut uf = union_find::QuickUnionUf::<union_find::UnionByRank>::new(node_to_key.len());
        for (_, id, n) in ObjectGraphWalker::from_graphs_map(graphs_map) {
            for (_, edge) in n.pointers_iter() {
                uf.union(node_to_key[&id], node_to_key[&edge.node]);
            }
        }

        Self { node_to_key, uf }
    }

    pub fn add_new_node(&mut self, id: NodeIndex) {
        self.node_to_key.insert(id, self.node_to_key.len());
        self.uf.extend([Default::default()]);
    }

    pub fn add_edge(&mut self, a: NodeIndex, b: NodeIndex) {
        self.uf.union(self.node_to_key[&a], self.node_to_key[&b]);
    }

    pub fn is_connected(&self, a: NodeIndex, b: NodeIndex) -> bool {
        self.uf.find(self.node_to_key[&a]) == self.uf.find(self.node_to_key[&b])
    }
}
