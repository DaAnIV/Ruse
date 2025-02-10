use std::collections::{HashMap, HashSet, VecDeque};

use crate::context::{Context, ContextArray, VariableName};
use itertools::{self, izip};
use ruse_object_graph::{
    graph_equality, graph_walk,
    value::{ObjectValue, Value},
    GraphIndex, GraphsMap, NodeIndex, RootName,
};

#[cfg(feature = "trace_embeddings")]
use itertools::Itertools;

#[cfg(feature = "trace_embeddings")]
macro_rules! embeddings_trace {
    ($($arg:tt)+) => { tracing::trace!($($arg)+); }
}
#[cfg(not(feature = "trace_embeddings"))]
macro_rules! embeddings_trace {
    ($($arg:tt)+) => {};
}

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

    let mut merged_pre_ctx = ContextArray::from(merged_pre_ctx_vec);
    let mut merged_post_ctx = ContextArray::from(merged_post_ctx_vec);

    merged_pre_ctx.depth = p_1_array.depth.max(p_2_array.depth);
    merged_post_ctx.depth = q_1_array.depth.max(q_2_array.depth);

    Ok((merged_pre_ctx, merged_post_ctx))
}

fn verify_matching_primitive_values(q_1: &Context, p_2: &Context) -> bool {
    let mut variables = HashSet::new();
    variables.extend(q_1.variable_names());
    variables.extend(p_2.variable_names());
    for var in variables {
        match (q_1.get_var_value(var), p_2.get_var_value(var)) {
            (Some(post_val_1), Some(pre_val_2)) => match (post_val_1, pre_val_2) {
                (Value::Primitive(prim_1), Value::Primitive(prim_2)) => {
                    if prim_1 != prim_2 {
                        return false;
                    }
                }
                (Value::Object(_), Value::Object(_)) => {
                    continue;
                }
                (_, _) => return false,
            },
            _ => (),
        }
    }

    return true;
}

fn context_reachable_graph_roots(ctx: &Context) -> HashMap<RootName, ObjectValue> {
    let mut roots = HashMap::new();

    let value_nodes = ctx.values.iter().filter_map(|(_, value)| {
        if let Some(obj_val) = value.obj() {
            Some((obj_val.graph_id, obj_val.node))
        } else {
            None
        }
    });

    for (g, node_id, node) in
        graph_walk::ObjectGraphWalker::from_nodes(&ctx.graphs_map, value_nodes)
    {
        if let Some(root_name) = ctx.graphs_map.node_root_names(&node_id) {
            for r in root_name {
                roots.insert(
                    r.clone(),
                    ObjectValue {
                        obj_type: node.obj_type().clone(),
                        graph_id: g.id,
                        node: node_id,
                    },
                );
            }
        }
    }

    roots
}

pub(crate) fn merge_context(
    p_1: &Context,
    q_1: &Context,
    p_2: &Context,
    q_2: &Context,
) -> Result<(Box<Context>, Box<Context>), ()> {
    embeddings_trace!("Merging contexts");
    embeddings_trace!("p_1: {}", p_1);
    embeddings_trace!("q_1: {}", q_1);
    embeddings_trace!("p_2: {}", p_2);
    embeddings_trace!("q_2: {}", q_2);

    if !verify_matching_primitive_values(q_1, p_2) {
        return Err(());
    }

    let mut p_1_hat = p_1.values.as_ref().clone();
    let mut q_2_hat = q_2.values.as_ref().clone();
    let mut p_1_map_hat = GraphsMap::default();
    let mut q_2_map_hat = GraphsMap::default();

    for (n, v) in p_2.values.iter() {
        if !p_1_hat.contains_key(n) {
            p_1_hat.insert(n.clone(), v.clone());
        }
    }
    for (n, v) in q_1.values.iter() {
        if !q_2_hat.contains_key(n) {
            q_2_hat.insert(n.clone(), v.clone());
        }
    }

    p_1_map_hat.add_static_graphs(&p_2.graphs_map);
    p_1_map_hat.add_static_graphs(&p_1.graphs_map);
    q_2_map_hat.add_static_graphs(&q_2.graphs_map);
    q_2_map_hat.add_static_graphs(&q_1.graphs_map);

    let mut p1_hat_nodes_matches = HashMap::new();
    let mut q2_hat_nodes_matches = HashMap::new();

    let p_1_roots = context_reachable_graph_roots(p_1);
    let q_1_roots = context_reachable_graph_roots(q_1);
    let p_2_roots = context_reachable_graph_roots(p_2);
    let q_2_roots = context_reachable_graph_roots(q_2);

    embeddings_trace!("p_1_roots: {:#?}", p_1_roots);
    embeddings_trace!("q_1_roots: {:#?}", q_1_roots);
    embeddings_trace!("p_2_roots: {:#?}", p_2_roots);
    embeddings_trace!("q_2_roots: {:#?}", q_2_roots);

    let mut intersection = Vec::new();
    let mut only_p_2 = Vec::new();
    let mut only_q_1 = Vec::new();

    for (r, q_1_o) in &q_1_roots {
        if let Some(p_2_o) = p_2_roots.get(r) {
            intersection.push((r, q_1_o, p_2_o));
        } else {
            only_q_1.push((r, q_1_o));
        }
    }
    for (r, p_2_o) in &p_2_roots {
        if !q_1_roots.contains_key(r) {
            only_p_2.push((r, p_2_o));
        }
    }

    embeddings_trace!(
        "intersection: [{}]",
        intersection.iter().map(|x| x.0).join(", "),
    );
    embeddings_trace!("only_p_2: [{}]", only_p_2.iter().map(|x| x.0).join(", "));
    embeddings_trace!("only_q_1: [{}]", only_q_1.iter().map(|x| x.0).join(", "));

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

    for (var, _, _) in intersection {
        if let Some(q_2_o) = q_2_roots.get(var) {
            embed_object_value(
                var,
                q_2_o,
                &mut q_2_map_hat,
                q_2,
                &new_nodes_2,
                &mut q2_hat_nodes_matches,
            );
        }

        if let Some(p_1_o) = p_1_roots.get(var) {
            embed_object_value(
                var,
                p_1_o,
                &mut p_1_map_hat,
                p_1,
                &new_nodes_1,
                &mut p1_hat_nodes_matches,
            );
        }
    }

    for (var, o_2) in only_p_2 {
        embed_object_value(
            var,
            o_2,
            &mut p_1_map_hat,
            p_2,
            &new_nodes_1,
            &mut p1_hat_nodes_matches,
        );
        if let Some(q_o_2) = q_2_roots.get(var) {
            embed_object_value(
                var,
                q_o_2,
                &mut q_2_map_hat,
                q_2,
                &new_nodes_2,
                &mut q2_hat_nodes_matches,
            );
        }
    }
    for (var, o_1) in only_q_1 {
        embed_object_value(
            var,
            o_1,
            &mut q_2_map_hat,
            q_1,
            &new_nodes_2,
            &mut q2_hat_nodes_matches,
        );
        if let Some(p_o_1) = p_1_roots.get(var) {
            embed_object_value(
                var,
                p_o_1,
                &mut p_1_map_hat,
                p_1,
                &new_nodes_1,
                &mut p1_hat_nodes_matches,
            );
        }
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
    map_hat: &mut GraphsMap,
    old_ctx: &Context,
    new_nodes: &HashSet<NodeIndex>,
    matches: &mut HashMap<NodeIndex, (GraphIndex, NodeIndex)>,
) -> bool {
    // trace!(target: "ruse::embedding", "Embedding {}", var);

    let new_var_value = match matches.get(&obj_val.node) {
        Some((graph_id, node_id)) => &(*graph_id, *node_id),
        None => {
            map_hat.ensure_graph(obj_val.graph_id);
            if !embed(
                map_hat,
                obj_val.graph_id,
                &old_ctx.graphs_map,
                obj_val.node,
                new_nodes,
                matches,
            ) {
                return false;
            }
            matches.get(&obj_val.node).unwrap()
        }
    };
    map_hat.set_as_root(var.clone(), new_var_value.0, new_var_value.1);

    true
}

fn embed(
    map_hat: &mut GraphsMap,
    graph_id: GraphIndex,
    graphs_map: &GraphsMap,
    node_id: NodeIndex,
    new_nodes: &HashSet<NodeIndex>,
    matches: &mut HashMap<NodeIndex, (GraphIndex, NodeIndex)>,
) -> bool {
    if matches.get(&node_id).is_some() {
        return true;
    }

    let mut edges = Vec::new();
    let mut q = VecDeque::new();

    q.push_back((graph_id, node_id));
    while let Some((cur_graph_id, cur_node_id)) = q.pop_front() {
        let graph = &graphs_map[cur_graph_id];
        if graph.is_static() {
            matches.insert(cur_node_id, (cur_graph_id, cur_node_id));
            continue;
        }
        let node = graph.get_node(&cur_node_id).unwrap();
        map_hat.construct_node(
            graph_id,
            cur_node_id,
            node.obj_type().clone(),
            node.fields().clone(),
        );

        matches.insert(cur_node_id, (graph_id, cur_node_id));
        for (field_name, old_neig) in node.pointers_iter() {
            if let Some((new_neig_graph, new_neig_id)) = matches.get(&old_neig.node) {
                if new_nodes.contains(new_neig_id) {
                    return false;
                }
                edges.push((
                    field_name.clone(),
                    graph_id,
                    cur_node_id,
                    *new_neig_graph,
                    *new_neig_id,
                ));
            } else {
                edges.push((
                    field_name.clone(),
                    graph_id,
                    cur_node_id,
                    graph_id,
                    old_neig.node,
                ));
                q.push_back((old_neig.graph.unwrap_or(cur_graph_id), old_neig.node));
            }
        }
    }

    for (field, graph_a, node_a, graph_b, node_b) in edges {
        map_hat.set_edge(field, graph_a, node_a, graph_b, node_b);
    }

    true
}
