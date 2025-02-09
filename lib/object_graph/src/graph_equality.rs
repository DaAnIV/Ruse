use std::collections::HashMap;

use crate::{
    graph_walk::ObjectGraphWalker, GraphIndex, GraphsMap, NodeIndex, RootName,
};

pub fn equal_graphs_by_root_names<'a, I>(
    graphs_map_a: &GraphsMap,
    graphs_map_b: &GraphsMap,
    roots: I,
) -> bool
where
    I: IntoIterator<Item = &'a RootName>,
{
    let mut equal_nodes: HashMap<(GraphIndex, NodeIndex), (GraphIndex, NodeIndex)> =
        HashMap::new();

    for r in roots {
        if let (Some(root_a), Some(root_b)) = (graphs_map_a.get_root(&r), graphs_map_b.get_root(&r)) {
            if !sim_walk_equal(
                graphs_map_a,
                root_a.graph,
                root_a.node,
                graphs_map_b,
                root_b.graph,
                root_b.node,
                &mut equal_nodes,
            ) {
                return false;
            }
        } else {
            return false;
        }
    }

    true
}

pub fn equal_graphs_by_node(
    graphs_map_a: &GraphsMap,
    graphs_map_b: &GraphsMap,
    graph_a: GraphIndex,
    graph_b: GraphIndex,
    id_a: NodeIndex,
    id_b: NodeIndex,
) -> bool {
    let mut equal_nodes: HashMap<(GraphIndex, NodeIndex), (GraphIndex, NodeIndex)> =
        HashMap::with_capacity(graphs_map_a[graph_a].node_count());

    sim_walk_equal(
        graphs_map_a,
        graph_a,
        id_a,
        graphs_map_b,
        graph_b,
        id_b,
        &mut equal_nodes,
    )
}

pub fn equal_graphs_by_nodes<I1, I2>(
    graphs_map_a: &GraphsMap,
    graphs_map_b: &GraphsMap,
    nodes_a: I1,
    nodes_b: I2,
) -> bool
where
    I1: Iterator<Item = (GraphIndex, NodeIndex)>,
    I2: Iterator<Item = (GraphIndex, NodeIndex)>,
{
    let mut equal_nodes: HashMap<(GraphIndex, NodeIndex), (GraphIndex, NodeIndex)> = HashMap::new();

    for ((graph_a, node_a), (graph_b, node_b)) in nodes_a.zip(nodes_b) {
        if !sim_walk_equal(
            graphs_map_a,
            graph_a,
            node_a,
            graphs_map_b,
            graph_b,
            node_b,
            &mut equal_nodes,
        ) {
            return false;
        }
    }

    true
}

pub fn sim_walk_equal(
    graphs_map_a: &GraphsMap,
    graph_a: GraphIndex,
    id_a: NodeIndex,
    graphs_map_b: &GraphsMap,
    graph_b: GraphIndex,
    id_b: NodeIndex,
    nodes_a_to_b: &mut HashMap<(GraphIndex, NodeIndex), (GraphIndex, NodeIndex)>,
) -> bool {
    let walker_a = ObjectGraphWalker::from_node(graphs_map_a, graph_a, id_a);
    let walker_b = ObjectGraphWalker::from_node(graphs_map_b, graph_b, id_b);

    for ((cur_graph_a, cur_node_id_a, cur_node_a), (cur_graph_b, cur_node_id_b, cur_node_b)) in
        walker_a.zip(walker_b)
    {
        if cur_node_a != cur_node_b {
            return false;
        }
        nodes_a_to_b.insert(
            (cur_graph_a.id, cur_node_id_a),
            (cur_graph_b.id, cur_node_id_b),
        );
    }

    true
}
