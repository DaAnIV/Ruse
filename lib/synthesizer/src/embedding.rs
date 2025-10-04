use std::collections::{HashMap, HashSet, VecDeque};

use crate::context::{Context, ContextArray, ValuesMap};
use itertools::{self, izip};
use ruse_object_graph::{
    graph_equality::{self, NodeMatcherMap},
    value::{ObjectValue, Value},
    GraphIndex, GraphsMap, NodeIndex, RootName,
};

#[cfg(feature = "trace_embeddings")]
use itertools::Itertools;

#[cfg(feature = "trace_embeddings")]
pub(crate) mod embeddings_tracing {
    macro_rules! __embeddings_trace {
        (prog: $prog:expr, $($arg:tt)+) => { $crate::trace_prog!(target: "ruse::embedding", $prog, $($arg)+); };
        ($($arg:tt)+) => { tracing::trace!(target: "ruse::embedding", $($arg)+); };
    }

    macro_rules! __embeddings_span {
        ($($arg:tt)+) => { tracing::span!(target: "ruse::embedding", tracing::Level::TRACE, $($arg)+).entered() };
    }

    pub(crate) use __embeddings_span as span;
    pub(crate) use __embeddings_trace as trace;
}
#[cfg(not(feature = "trace_embeddings"))]
pub(crate) mod embeddings_tracing {
    macro_rules! __embeddings_trace {
        ($($arg:tt)+) => {};
    }

    macro_rules! __embeddings_span {
        ($($arg:tt)+) => {
            0
        };
    }

    pub(crate) use __embeddings_span as span;
    pub(crate) use __embeddings_trace as trace;
}

pub fn merge_context_arrays(
    p_1_array: &ContextArray,
    q_1_array: &ContextArray,
    p_2_array: &ContextArray,
    q_2_array: &ContextArray,
) -> Result<(ContextArray, ContextArray), ()> {
    let mut merged_pre_ctx_vec = Vec::with_capacity(p_1_array.len());
    let mut merged_post_ctx_vec = Vec::with_capacity(p_1_array.len());

    for (p_1, q_1, p_2, q_2) in izip!(
        p_1_array.iter(),
        q_1_array.iter(),
        p_2_array.iter(),
        q_2_array.iter(),
    ) {
        let (merged_pre_ctx, merged_post_ctx) = merge_context(p_1, q_1, p_2, q_2)?;
        merged_pre_ctx_vec.push(merged_pre_ctx);
        merged_post_ctx_vec.push(merged_post_ctx);
    }

    let mut merged_pre_ctx = ContextArray::from(merged_pre_ctx_vec);
    let mut merged_post_ctx = ContextArray::from(merged_post_ctx_vec);

    // This is a conservative estimate of the depth
    // If the contexts are equal this is exact, otherwise it may be an underestimate
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

pub(crate) fn merge_context(
    p_1: &Context,
    q_1: &Context,
    p_2: &Context,
    q_2: &Context,
) -> Result<(Box<Context>, Box<Context>), ()> {
    let _span = embeddings_tracing::span!("Merging contexts",
        p_1.json = %p_1.json_display(),
        q_1.json = %q_1.json_display(),
        p_2.json = %p_2.json_display(),
        q_2.json = %q_2.json_display(),
    );

    embeddings_tracing::trace!("Starting merge");

    if !verify_matching_primitive_values(q_1, p_2) {
        return Err(());
    }

    let (p_1_hat, q_2_hat) = create_hat_values(p_1, q_1, p_2, q_2);
    let (p_1_map_hat, q_2_map_hat) = create_hat_graphs_map(p_1, q_1, p_2, q_2)?;
    let outputs = q_1.outputs().chain(q_2.outputs()).cloned().collect();

    let pre_ctx_hat = Context::with_values(p_1_hat, p_1_map_hat.into(), p_1.graph_id_gen.clone());
    let post_ctx_hat = Context::with_values_and_outputs(
        q_2_hat,
        outputs,
        q_2_map_hat.into(),
        q_2.graph_id_gen.clone(),
    );

    embeddings_tracing::trace!({pre_ctx_hat.json = %pre_ctx_hat.json_display(), post_ctx_hat.json = %post_ctx_hat.json_display()}, "Contexts merged");
    Ok((pre_ctx_hat, post_ctx_hat))
}

fn create_hat_graphs_map<'a>(
    p_1: &'a Context,
    q_1: &'a Context,
    p_2: &'a Context,
    q_2: &'a Context,
) -> Result<(GraphsMap, GraphsMap), ()> {
    let (mut p_1_map_hat, mut q_2_map_hat) = init_hat_graphs_map(p_1, q_1, p_2, q_2);

    let mut p1_hat_nodes_matches = HashMap::new();
    let mut q2_hat_nodes_matches = HashMap::new();

    let (intersection, only_p_2, only_q_1) = divide_roots(p_1, q_1, p_2, q_2);

    let mut equal_nodes = HashMap::new();

    if !graph_equality::equal_graphs_by_nodes_with_map(
        &q_1.graphs_map,
        &p_2.graphs_map,
        intersection
            .iter()
            .map(|(_, o_1, _)| (o_1.graph_id, o_1.node)),
        intersection
            .iter()
            .map(|(_, _, o_2)| (o_2.graph_id, o_2.node)),
        &mut equal_nodes,
    ) {
        return Err(());
    }

    embeddings_tracing::trace!("Intersecting roots graphs are equal");

    embed_original_graph(p_1, &mut p_1_map_hat, &mut p1_hat_nodes_matches, false)?;
    embed_original_graph(q_2, &mut q_2_map_hat, &mut q2_hat_nodes_matches, true)?;

    for (nodes_1, nodes_2) in equal_nodes {
        p1_hat_nodes_matches.insert(nodes_2, nodes_1);
        q2_hat_nodes_matches.insert(nodes_1, nodes_2);
    }

    embed_other_graph(
        p_2,
        &only_p_2,
        &mut p_1_map_hat,
        &mut p1_hat_nodes_matches,
        (p_1, q_1),
        false,
    )?;
    embed_other_graph(
        q_1,
        &only_q_1,
        &mut q_2_map_hat,
        &mut q2_hat_nodes_matches,
        (p_2, q_2),
        true,
    )?;

    Ok((p_1_map_hat, q_2_map_hat))
}

fn create_hat_values<'a>(
    p_1: &'a Context,
    q_1: &'a Context,
    p_2: &'a Context,
    q_2: &'a Context,
) -> (ValuesMap, ValuesMap) {
    let mut p_1_hat = p_1.values.as_ref().clone();
    let mut q_2_hat = q_2.values.as_ref().clone();

    for (n, v) in p_2.variables() {
        if !p_1_hat.contains_key(n) {
            p_1_hat.insert(n.clone(), v.clone());
        }
    }
    for (n, v) in q_1.variables() {
        if !q_2_hat.contains_key(n) {
            q_2_hat.insert(n.clone(), v.clone());
        }
    }

    (p_1_hat, q_2_hat)
}

fn init_hat_graphs_map<'a>(
    p_1: &'a Context,
    q_1: &'a Context,
    p_2: &'a Context,
    q_2: &'a Context,
) -> (GraphsMap, GraphsMap) {
    let mut p_1_map_hat = GraphsMap::default();
    let mut q_2_map_hat = GraphsMap::default();

    p_1_map_hat.add_static_graphs(&p_2.graphs_map);
    p_1_map_hat.add_static_graphs(&p_1.graphs_map);
    q_2_map_hat.add_static_graphs(&q_2.graphs_map);
    q_2_map_hat.add_static_graphs(&q_1.graphs_map);

    (p_1_map_hat, q_2_map_hat)
}

fn divide_roots<'a>(
    p_1: &'a Context,
    q_1: &'a Context,
    p_2: &'a Context,
    q_2: &'a Context,
) -> (
    Vec<(&'a RootName, &'a ObjectValue, &'a ObjectValue)>,
    Vec<(&'a RootName, &'a ObjectValue)>,
    Vec<(&'a RootName, &'a ObjectValue)>,
) {
    let _p_1_roots = p_1.object_variables().collect::<HashMap<_, _>>();
    let q_1_roots = q_1.object_variables().collect::<HashMap<_, _>>();
    let p_2_roots = p_2.object_variables().collect::<HashMap<_, _>>();
    let _q_2_roots = q_2.object_variables().collect::<HashMap<_, _>>();

    embeddings_tracing::trace!({
        p_1_roots = ?_p_1_roots,
        q_1_roots = ?q_1_roots,
        p_2_roots = ?p_2_roots,
        q_2_roots = ?_q_2_roots,
    }, "Roots");

    let mut intersection = Vec::new();
    let mut only_p_2 = Vec::new();
    let mut only_q_1 = Vec::new();

    for (r, q_1_o) in &q_1_roots {
        if let Some(p_2_o) = p_2_roots.get(r) {
            intersection.push((*r, *q_1_o, *p_2_o));
        } else {
            only_q_1.push((*r, *q_1_o));
        }
    }
    for (r, p_2_o) in &p_2_roots {
        if !q_1_roots.contains_key(r) {
            only_p_2.push((*r, *p_2_o));
        }
    }

    embeddings_tracing::trace!({
            intersection = %intersection.iter().map(|x| x.0).join(", "),
            only_p_2 = %only_p_2.iter().map(|x| x.0).join(", "),
            only_q_1 = %only_q_1.iter().map(|x| x.0).join(", "),
        }, "Roots sets");

    (intersection, only_p_2, only_q_1)
}

fn embed_original_graph(
    ctx: &Context,
    map_hat: &mut GraphsMap,
    nodes_matches: &mut NodeMatcherMap,
    add_outputs: bool,
) -> Result<(), ()> {
    for (var, obj_val) in ctx.object_variables() {
        embed_root_object_value(var, obj_val, map_hat, ctx, None, nodes_matches)?;
    }

    if add_outputs {
        for output in ctx.outputs() {
            if let Some(obj) = output.obj() {
                embed_object_value(obj, map_hat, ctx, None, nodes_matches)?;
            }
        }
    }

    Ok(())
}

fn embed_other_graph(
    other_ctx: &Context,
    other_graph_unique_roots: &[(&RootName, &ObjectValue)],
    map_hat: &mut GraphsMap,
    nodes_matches: &mut NodeMatcherMap,
    old_ctx: (&Context, &Context),
    add_outputs: bool,
) -> Result<(), ()> {
    for (var, obj_val) in other_graph_unique_roots {
        embed_root_object_value(
            var,
            obj_val,
            map_hat,
            other_ctx,
            Some(old_ctx),
            nodes_matches,
        )?;
    }

    if add_outputs {
        for output in other_ctx.outputs() {
            if let Some(obj) = output.obj() {
                embed_object_value(obj, map_hat, other_ctx, Some(old_ctx), nodes_matches)?;
            }
        }
    }

    Ok(())
}

fn embed_root_object_value(
    var: &RootName,
    obj_val: &ObjectValue,
    map_hat: &mut GraphsMap,
    ctx: &Context,
    old_context: Option<(&Context, &Context)>,
    matches: &mut NodeMatcherMap,
) -> Result<(), ()> {
    embed_object_value(obj_val, map_hat, ctx, old_context, matches)?;
    map_hat.set_as_root(var.clone(), obj_val.graph_id, obj_val.node);
    Ok(())
}

fn embed_object_value(
    obj_val: &ObjectValue,
    map_hat: &mut GraphsMap,
    ctx: &Context,
    old_context: Option<(&Context, &Context)>,
    matches: &mut NodeMatcherMap,
) -> Result<(), ()> {
    embeddings_tracing::trace!({ value = ?obj_val }, "Embedding object");
    embed(
        map_hat,
        obj_val.graph_id,
        &ctx.graphs_map,
        obj_val.node,
        old_context,
        matches,
    )
}

fn embed(
    map_hat: &mut GraphsMap,
    graph_id: GraphIndex,
    graphs_map: &GraphsMap,
    node_id: NodeIndex,
    old_context: Option<(&Context, &Context)>,
    matches: &mut NodeMatcherMap,
) -> Result<(), ()> {
    let mut edges = Vec::new();
    let mut q = VecDeque::new();

    q.push_back((graph_id, node_id));
    matches.insert((graph_id, node_id), (graph_id, node_id));
    while let Some((cur_graph_id, cur_node_id)) = q.pop_front() {
        if map_hat.contains_node(&cur_graph_id, &cur_node_id) {
            continue;
        }
        let graph = &graphs_map[cur_graph_id];
        if graph.is_static() {
            matches.insert((cur_graph_id, cur_node_id), (cur_graph_id, cur_node_id));
            continue;
        }
        let node = graph.get_node(&cur_node_id).unwrap();
        map_hat.ensure_graph(cur_graph_id);
        map_hat.construct_node(
            cur_graph_id,
            cur_node_id,
            node.obj_type().clone(),
            node.fields().clone(),
        );

        for (field_name, neig) in node.pointers_iter() {
            let neig_graph = neig.graph.unwrap_or(cur_graph_id);

            edges.push((
                field_name.clone(),
                cur_graph_id,
                cur_node_id,
                neig_graph,
                neig.node,
            ));
            if let Some((hat_graph, hat_node)) = matches.get(&(neig_graph, neig.node)) {
                if let Some((p, q)) = old_context {
                    if p.contains_node(hat_graph, hat_node) != q.contains_node(hat_graph, hat_node)
                    {
                        return Err(());
                    }
                }
            } else {
                matches.insert((neig_graph, neig.node), (neig_graph, neig.node));
            };
            q.push_back((neig_graph, neig.node));
        }
    }

    for (field, graph_a, node_a, graph_b, node_b) in edges {
        map_hat.set_edge(field, graph_a, node_a, graph_b, node_b);
    }

    Ok(())
}
