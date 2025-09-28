use std::sync::atomic::{self, AtomicUsize};

use crate::{GraphIndex, NodeIndex};

#[derive(Debug)]
pub struct GraphIdGenerator {
    node_id: AtomicUsize,
    graph_id: AtomicUsize,
}

impl GraphIdGenerator {
    pub fn with_initial_values(node_id: NodeIndex, graph_id: GraphIndex) -> Self {
        Self {
            node_id: node_id.0.into(),
            graph_id: graph_id.0.into(),
        }
    }

    pub fn get_id_for_node(&self) -> NodeIndex {
        NodeIndex(self.node_id.fetch_add(1, atomic::Ordering::Relaxed))
    }

    pub fn get_id_for_graph(&self) -> GraphIndex {
        GraphIndex(self.graph_id.fetch_add(1, atomic::Ordering::Relaxed))
    }

    pub fn max_node_id(&self) -> NodeIndex {
        NodeIndex(self.node_id.load(atomic::Ordering::Relaxed))
    }

    pub fn max_graph_id(&self) -> GraphIndex {
        GraphIndex(self.graph_id.load(atomic::Ordering::Relaxed))
    }
}

impl Default for GraphIdGenerator {
    fn default() -> Self {
        Self::with_initial_values(0.into(), 0.into())
    }
}
