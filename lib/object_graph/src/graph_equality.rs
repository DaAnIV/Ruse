use std::collections::{HashMap, VecDeque};

use crate::{GraphIndex, GraphsMap, NodeIndex, RootName};

fn get_root_nodes<'a, I>(
    graphs_map_a: &GraphsMap,
    graphs_map_b: &GraphsMap,
    roots: I,
) -> Option<(Vec<(GraphIndex, NodeIndex)>, Vec<(GraphIndex, NodeIndex)>)>
where
    I: IntoIterator<Item = &'a RootName>,
{
    let mut root_nodes_a = Vec::new();
    let mut root_nodes_b = Vec::new();

    for r in roots {
        let root_a = graphs_map_a.get_root(r)?;
        let root_b = graphs_map_b.get_root(r)?;
        root_nodes_a.push((root_a.graph, root_a.node));
        root_nodes_b.push((root_b.graph, root_b.node));
    }

    Some((root_nodes_a, root_nodes_b))
}

pub fn equal_graphs_by_root_names<'a, I>(
    graphs_map_a: &GraphsMap,
    graphs_map_b: &GraphsMap,
    roots: I,
) -> bool
where
    I: IntoIterator<Item = &'a RootName>,
{
    if let Some((graph_a_roots, graph_b_roots)) = get_root_nodes(graphs_map_a, graphs_map_b, roots)
    {
        equal_graphs_by_nodes(graphs_map_a, graphs_map_b, graph_a_roots, graph_b_roots)
    } else {
        false
    }
}

pub fn equal_graphs_by_node(
    graphs_map_a: &GraphsMap,
    graphs_map_b: &GraphsMap,
    graph_a: GraphIndex,
    graph_b: GraphIndex,
    id_a: NodeIndex,
    id_b: NodeIndex,
) -> bool {
    equal_graphs_by_nodes(
        graphs_map_a,
        graphs_map_b,
        [(graph_a, id_a)],
        [(graph_b, id_b)],
    )
}

pub fn equal_graphs_by_nodes<I1, I2>(
    graphs_map_a: &GraphsMap,
    graphs_map_b: &GraphsMap,
    nodes_a: I1,
    nodes_b: I2,
) -> bool
where
    I1: IntoIterator<Item = (GraphIndex, NodeIndex)>,
    I2: IntoIterator<Item = (GraphIndex, NodeIndex)>,
{
    let mut equal_nodes: HashMap<(GraphIndex, NodeIndex), (GraphIndex, NodeIndex)> = HashMap::new();
    let mut nodes_queue = VecDeque::new();

    nodes_queue.extend(nodes_a.into_iter().zip(nodes_b.into_iter()));

    while let Some(((graph_id_a, cur_node_id_a), (graph_id_b, cur_node_id_b))) =
        nodes_queue.pop_front()
    {
        if let Some((matching_graph_id, matching_node_id)) =
            equal_nodes.get(&(graph_id_a, cur_node_id_a))
        {
            if *matching_graph_id != graph_id_b || *matching_node_id != cur_node_id_b {
                return false;
            }
            continue;
        }

        let graph_a = &graphs_map_a[graph_id_a];
        let graph_b = &graphs_map_b[graph_id_b];

        let node_a = graph_a.get_node(&cur_node_id_a).unwrap();
        let node_b = graph_b.get_node(&cur_node_id_b).unwrap();

        if node_a != node_b {
            return false;
        }
        equal_nodes.insert((graph_id_a, cur_node_id_a), (graph_id_b, cur_node_id_b));

        for ((_, neig_a), (_, neig_b)) in node_a.pointers_iter().zip(node_b.pointers_iter()) {
            let neig_a_id = neig_a.node;
            let neig_a_graph_id = neig_a.graph.unwrap_or(graph_id_a);
            let neig_b_id = neig_b.node;
            let neig_b_graph_id = neig_b.graph.unwrap_or(graph_id_b);

            nodes_queue.push_back(((neig_a_graph_id, neig_a_id), (neig_b_graph_id, neig_b_id)));
        }
    }

    true
}
