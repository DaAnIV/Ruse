use std::collections::{HashMap, HashSet, VecDeque};

use crate::context::{Context, ContextArray, ValuesMap, VariableName};
use itertools::{self, izip, Itertools};
use ruse_object_graph::{
    graph_equality, value::{ObjectValue, Value}, vobj, GraphIndex, GraphsMap, NodeIndex, ObjectGraph
};
use tracing::trace;

pub fn merge_context_arrays(
    p_1_array: &ContextArray,
    q_1_array: &ContextArray,
    p_2_array: &ContextArray,
    q_2_array: &ContextArray,
) -> Result<(ContextArray, ContextArray), ()> {
    if q_1_array == p_2_array {
        let mut q_2_hat = q_2_array.clone();
        q_2_hat.extend_graphs_map(q_1_array);
        return Ok((p_1_array.clone(), q_2_hat));
    }

    let mut merged_pre_ctx_vec = Vec::with_capacity(p_1_array.len());
    let mut merged_post_ctx_vec = Vec::with_capacity(p_1_array.len());

    for (p_1, q_1, p_2, q_2) in izip!(
        p_1_array.iter(),
        q_1_array.iter(),
        p_2_array.iter(),
        q_2_array.iter()
    ) {
        let (merged_pre_ctx, merged_post_ctx) = merge_context(p_1, q_1, p_2, q_2)?;
        merged_pre_ctx_vec.push(merged_pre_ctx);
        merged_post_ctx_vec.push(merged_post_ctx);
    }

    Ok((
        ContextArray::from(merged_pre_ctx_vec),
        ContextArray::from(merged_post_ctx_vec),
    ))
}

pub(crate) fn merge_context(
    p_1: &Context,
    q_1: &Context,
    p_2: &Context,
    q_2: &Context,
) -> Result<(Context, Context), ()> {
    // trace!(target: "ruse::embedding", "Merging contexts");
    // trace!(target: "ruse::embedding", "p_1: {}", p_1);
    // trace!(target: "ruse::embedding", "q_1: {}", q_1);
    // trace!(target: "ruse::embedding", "p_2: {}", p_2);
    // trace!(target: "ruse::embedding", "q_2: {}", q_2);

    let mut p_1_hat = p_1.values.as_ref().clone();
    let mut q_2_hat = q_2.values.as_ref().clone();
    let mut p_1_map_hat = GraphsMap::default();
    let mut q_2_map_hat = GraphsMap::default();

    let mut p1_hat_nodes_matches = HashMap::new();
    let mut q2_hat_nodes_matches = HashMap::new();

    let mut pre_values = HashMap::<VariableName, Value>::new();
    let mut post_values = HashMap::<VariableName, Value>::new();

    let mut intersection = Vec::new();
    let mut only_p_2 = Vec::new();
    let mut only_q_1 = Vec::new();

    let mut variables = HashSet::new();
    variables.extend(q_1.variables());
    variables.extend(p_2.variables());

    for var in variables {
        match (q_1.get_var_value(var), p_2.get_var_value(var)) {
            (None, Some(pre_val_2)) => match pre_val_2 {
                Value::Primitive(_) => {
                    pre_values.insert(var.clone(), pre_val_2.clone());
                }
                Value::Object(o) => only_p_2.push((var, o.clone())),
            },
            (Some(post_val_1), None) => match post_val_1 {
                Value::Primitive(_) => {
                    post_values.insert(var.clone(), post_val_1.clone());
                }
                Value::Object(o) => only_q_1.push((var, o.clone())),
            },
            (Some(post_val_1), Some(pre_val_2)) => match (post_val_1, pre_val_2) {
                (Value::Primitive(prim_1), Value::Primitive(prim_2)) => {
                    if prim_1 != prim_2 {
                        return Err(());
                    }
                    if let Some(pre_val_1) = p_1.get_var_value(var) {
                        pre_values.insert(var.clone(), pre_val_1.clone());
                    }
                }
                (Value::Object(o_1), Value::Object(o_2)) => {
                    intersection.push((var, o_1.clone(), o_2.clone()))
                }
                (_, _) => return Err(()),
            },
            (None, None) => continue,
        }
    }

    // trace!(
    //     target: "ruse::embedding", "intersection: [{}]",
    //     intersection.iter().map(|x| x.0).join(",")
    // );
    // trace!(target: "ruse::embedding", "only_p_2: [{}]", only_p_2.iter().map(|x| x.0).join(","));
    // trace!(target: "ruse::embedding", "only_q_1: [{}]", only_q_1.iter().map(|x| x.0).join(","));

    let new_nodes_1 = triplet_new_nodes(p_1, q_1);
    let new_nodes_2 = HashSet::default();

    if !graph_equality::equal_graphs_by_nodes(
        &q_1.graphs_map,
        &p_2.graphs_map,
        intersection
            .iter()
            .map(|(_, o_1, _)| (o_1.graph_id, o_1.node)),
        intersection
            .iter()
            .map(|(_, _, o_2)| (o_2.graph_id, o_2.node)),
    ) {
        return Err(());
    }

    for (var, _, _) in &intersection {
        if let Some(q_2_o) = q_2.get_var_value(var) {
            embed_object_value(
                var,
                q_2_o.obj().unwrap(),
                &mut q_2_hat,
                &mut q_2_map_hat,
                q_2,
                &new_nodes_2,
                &mut q2_hat_nodes_matches,
            );
        }

        if let Some(p_1_o) = p_1.get_var_value(var) {
            embed_object_value(
                var,
                p_1_o.obj().unwrap(),
                &mut p_1_hat,
                &mut p_1_map_hat,
                p_1,
                &new_nodes_1,
                &mut p1_hat_nodes_matches,
            );
        }
    }

    for (var, o_2) in &only_p_2 {
        embed_object_value(
            var,
            o_2,
            &mut p_1_hat,
            &mut p_1_map_hat,
            p_2,
            &new_nodes_1,
            &mut p1_hat_nodes_matches,
        );
        if let Some(q_o_2) = q_2.get_var_value(var) {
            embed_object_value(
                var,
                q_o_2.obj().unwrap(),
                &mut q_2_hat,
                &mut q_2_map_hat,
                q_2,
                &new_nodes_2,
                &mut q2_hat_nodes_matches,
            );
        }
    }
    for (var, o_1) in &only_q_1 {
        embed_object_value(
            var,
            o_1,
            &mut q_2_hat,
            &mut q_2_map_hat,
            q_1,
            &new_nodes_2,
            &mut q2_hat_nodes_matches,
        );
    }

    p_1_map_hat.extend(&p_1.graphs_map);
    q_2_map_hat.extend(&q_2.graphs_map);

    q_2_map_hat.extend(&q_1.graphs_map);

    Ok((
        Context::with_values(p_1_hat, p_1_map_hat.into(), p_1.graph_id_gen.clone()),
        Context::with_values(q_2_hat, q_2_map_hat.into(), q_2.graph_id_gen.clone()),
    ))
}

fn triplet_new_nodes(p_ctx: &Context, q_ctx: &Context) -> HashSet<NodeIndex> {
    q_ctx
        .reachable_nodes()
        .difference(&p_ctx.reachable_nodes())
        .map(|x| x.1)
        .collect()
}

fn embed_object_value(
    var: &VariableName,
    obj_val: &ObjectValue,
    values_hat: &mut ValuesMap,
    map_hat: &mut GraphsMap,
    old_ctx: &Context,
    new_nodes: &HashSet<NodeIndex>,
    matches: &mut HashMap<NodeIndex, (GraphIndex, NodeIndex)>,
) -> bool {
    // trace!(target: "ruse::embedding", "Embedding {}", var);

    let (mut graph, new_var_value) = match matches.get(&obj_val.node) {
        Some((graph_id, node_id)) => {
            let graph = map_hat.get(graph_id).unwrap().as_ref().clone();
            (graph, &(*graph_id, *node_id))
        }
        None => {
            let mut graph = match map_hat.get(&obj_val.graph_id) {
                Some(old_graph) => old_graph.as_ref().clone(),
                None => ObjectGraph::new(obj_val.graph_id),
            };
            if !embed(
                &mut graph,
                &old_ctx.graphs_map,
                obj_val.graph_id,
                obj_val.node,
                new_nodes,
                matches,
            ) {
                return false;
            }
            (graph, matches.get(&obj_val.node).unwrap())
        }
    };
    graph.set_as_root(var.clone(), new_var_value.1);
    map_hat.insert_graph(graph.into());
    values_hat.insert(var.clone(), vobj!(new_var_value.0, new_var_value.1));

    true
}

fn embed(
    new_graph: &mut ObjectGraph,
    graphs_map: &GraphsMap,
    graph_id: GraphIndex,
    node_id: NodeIndex,
    new_nodes: &HashSet<NodeIndex>,
    matches: &mut HashMap<NodeIndex, (GraphIndex, NodeIndex)>,
) -> bool {
    if matches.get(&node_id).is_some() {
        return true;
    }

    let mut q = VecDeque::new();
    let new_graph_id = new_graph.id;

    q.push_back((graph_id, node_id));
    while let Some((cur_graph_id, cur_node_id)) = q.pop_front() {
        let graph = &graphs_map[cur_graph_id];
        let node = graph.get_node(&cur_node_id).unwrap();
        let new_node = new_graph.add_node(cur_node_id, node.clone_without_pointers());
        matches.insert(cur_node_id, (new_graph_id, cur_node_id));
        for (field_name, old_neig) in node.pointers_iter() {
            if let Some((new_neig_graph, new_neig_id)) = matches.get(old_neig.index()) {
                if new_nodes.contains(new_neig_id) {
                    return false;
                }
                if *new_neig_graph == new_graph_id {
                    new_node.insert_internal_edge(field_name.clone(), *new_neig_id);
                } else {
                    new_node.insert_chain_edge(field_name.clone(), *new_neig_graph, *new_neig_id);
                }
            } else {
                new_node.insert_internal_edge(field_name.clone(), *old_neig.index());
                match old_neig {
                    ruse_object_graph::EdgeEndPoint::Internal(old_neig_node) => {
                        q.push_back((cur_graph_id, *old_neig_node))
                    }
                    ruse_object_graph::EdgeEndPoint::Chain(old_neig_graph, old_neig_node) => {
                        q.push_back((*old_neig_graph, *old_neig_node))
                    }
                }
            }
        }
    }

    true
}
